use semver::{BuildMetadata, Prerelease, Version};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

const UPDATE_CHANNEL_FIELD: &str = "updateChannel";
const NIGHTLY_IDENTIFIER: &str = "nightly";
const UPDATER_PLUGIN_KEY: &str = "updater";
const CHANNEL_ENDPOINTS_KEY: &str = "channelEndpoints";
const ENDPOINTS_KEY: &str = "endpoints";
const STABLE_ENDPOINT_ENV: &str = "ASTRBOT_DESKTOP_UPDATER_STABLE_ENDPOINT";
const NIGHTLY_ENDPOINT_ENV: &str = "ASTRBOT_DESKTOP_UPDATER_NIGHTLY_ENDPOINT";
// Canonical nightly version format lives in `src-tauri/nightly-version-format.json`.

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NightlyVersionFormatSpec {
    canonical_format: String,
    date_digits: usize,
    sha_hex_digits: usize,
    #[serde(default)]
    valid_examples: Vec<String>,
    #[serde(default)]
    invalid_examples: Vec<String>,
}

fn nightly_version_format_spec() -> &'static NightlyVersionFormatSpec {
    static SPEC: OnceLock<NightlyVersionFormatSpec> = OnceLock::new();
    SPEC.get_or_init(|| {
        serde_json::from_str(include_str!("../nightly-version-format.json"))
            .expect("nightly version format spec should parse")
    })
}

fn nightly_prerelease_format() -> &'static str {
    static FORMAT: OnceLock<String> = OnceLock::new();
    FORMAT.get_or_init(|| {
        nightly_version_format_spec()
            .canonical_format
            .strip_prefix("<base>-")
            .unwrap_or(&nightly_version_format_spec().canonical_format)
            .to_string()
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum UpdateChannel {
    Stable,
    Nightly,
}

impl UpdateChannel {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "stable" => Some(Self::Stable),
            "nightly" => Some(Self::Nightly),
            _ => None,
        }
    }

    pub(crate) fn config_key(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Nightly => "nightly",
        }
    }

    pub(crate) fn env_override_key(self) -> &'static str {
        match self {
            Self::Stable => STABLE_ENDPOINT_ENV,
            Self::Nightly => NIGHTLY_ENDPOINT_ENV,
        }
    }
}

fn desktop_state_path(packaged_root_dir: Option<&Path>) -> Option<PathBuf> {
    if let Ok(root) = env::var("ASTRBOT_ROOT") {
        let path = PathBuf::from(root.trim());
        if !path.as_os_str().is_empty() {
            return Some(path.join("data").join("desktop_state.json"));
        }
    }

    packaged_root_dir.map(|root| root.join("data").join("desktop_state.json"))
}

fn version_is_nightly(version: &Version) -> bool {
    version
        .pre
        .as_str()
        .split('.')
        .any(|identifier| identifier.eq_ignore_ascii_case(NIGHTLY_IDENTIFIER))
}

fn base_version(version: &Version) -> Version {
    let mut base = version.clone();
    base.pre = Prerelease::EMPTY;
    base.build = BuildMetadata::EMPTY;
    base
}

pub(crate) fn resolve_manifest_endpoint_from_sources(
    channel: UpdateChannel,
    updater_config: &Map<String, Value>,
) -> Result<String, String> {
    let non_empty = |raw: &str| {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    };

    if let Some(endpoint) = updater_config
        .get(CHANNEL_ENDPOINTS_KEY)
        .and_then(Value::as_object)
        .and_then(|channels| channels.get(channel.config_key()))
        .and_then(Value::as_str)
        .and_then(non_empty)
    {
        return Ok(endpoint);
    }

    if channel == UpdateChannel::Stable {
        if let Some(endpoint) = updater_config
            .get(ENDPOINTS_KEY)
            .and_then(Value::as_array)
            .and_then(|endpoints| endpoints.first())
            .and_then(Value::as_str)
            .and_then(non_empty)
        {
            return Ok(endpoint);
        }
    }

    let message = match channel {
        UpdateChannel::Stable => format!(
            "Missing updater endpoint for 'stable' channel. Configure plugins.updater.channelEndpoints.stable, plugins.updater.endpoints[0], or set {}.",
            channel.env_override_key()
        ),
        UpdateChannel::Nightly => format!(
            "Missing updater endpoint for '{}' channel. Configure plugins.updater.channelEndpoints.{} or set {}.",
            channel.config_key(),
            channel.config_key(),
            channel.env_override_key()
        ),
    };

    Err(message)
}

pub(crate) fn resolve_manifest_endpoint(
    plugins_config: &HashMap<String, Value>,
    channel: UpdateChannel,
) -> Result<String, String> {
    if let Ok(value) = env::var(channel.env_override_key()) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let updater_config = plugins_config
        .get(UPDATER_PLUGIN_KEY)
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "Missing plugins.{} configuration for '{}' channel updater resolution.",
                UPDATER_PLUGIN_KEY,
                channel.config_key()
            )
        })?;

    resolve_manifest_endpoint_from_sources(channel, updater_config)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NightlyVersionInfo {
    base: Version,
    date: u32,
    hash: String,
}

fn log_nightly_parse_failure(version: &Version, reason: impl std::fmt::Display) {
    crate::append_desktop_log(&format!(
        "failed to parse nightly prerelease '{}' as '{}': {}",
        version,
        nightly_prerelease_format(),
        reason,
    ));
}

fn parse_nightly_version_info(version: &Version) -> Option<NightlyVersionInfo> {
    let spec = nightly_version_format_spec();
    let mut identifiers = version.pre.as_str().split('.');

    let first = identifiers.next()?;
    if !first.eq_ignore_ascii_case(NIGHTLY_IDENTIFIER) {
        return None;
    }

    let date_raw = match identifiers.next() {
        Some(value) => value,
        None => {
            log_nightly_parse_failure(version, "missing date segment");
            return None;
        }
    };

    let hash_raw = match identifiers.next() {
        Some(value) => value,
        None => {
            log_nightly_parse_failure(version, "missing hash segment");
            return None;
        }
    };

    if identifiers.next().is_some() {
        log_nightly_parse_failure(version, "too many prerelease identifiers");
        return None;
    }

    if date_raw.len() != spec.date_digits || !date_raw.chars().all(|ch| ch.is_ascii_digit()) {
        log_nightly_parse_failure(
            version,
            format!("date segment is not {} ASCII digits", spec.date_digits),
        );
        return None;
    }

    if hash_raw.len() != spec.sha_hex_digits || !hash_raw.chars().all(|ch| ch.is_ascii_hexdigit()) {
        log_nightly_parse_failure(
            version,
            format!(
                "hash segment is not {} ASCII hex digits",
                spec.sha_hex_digits
            ),
        );
        return None;
    }

    let date = match date_raw.parse::<u32>() {
        Ok(value) => value,
        Err(_) => {
            log_nightly_parse_failure(version, "failed to parse date as u32");
            return None;
        }
    };

    Some(NightlyVersionInfo {
        base: base_version(version),
        date,
        hash: hash_raw.to_ascii_lowercase(),
    })
}

// Nightly-to-nightly comparisons only accept nightly remotes, then compare
// base version and nightly date. If both are equal, any differing hash is
// treated as a newer same-date rebuild so upstream rebuilds still advance
// without silently switching channels.
fn should_offer_nightly_update(current_version: &Version, remote_version: &Version) -> bool {
    let Some(current) = parse_nightly_version_info(current_version) else {
        return false;
    };
    let Some(remote) = parse_nightly_version_info(remote_version) else {
        return false;
    };

    if remote.base > current.base {
        return true;
    }
    if remote.base < current.base {
        return false;
    }
    if remote.date > current.date {
        return true;
    }
    if remote.date < current.date {
        return false;
    }

    remote.hash != current.hash
}

pub(crate) fn infer_channel_from_version(version: &Version) -> UpdateChannel {
    if version_is_nightly(version) {
        UpdateChannel::Nightly
    } else {
        UpdateChannel::Stable
    }
}

pub(crate) fn read_cached_update_channel(
    packaged_root_dir: Option<&Path>,
) -> Option<UpdateChannel> {
    let state_path = desktop_state_path(packaged_root_dir)?;
    let raw = fs::read_to_string(state_path).ok()?;
    let parsed: Value = serde_json::from_str(&raw).ok()?;
    let channel = parsed.get(UPDATE_CHANNEL_FIELD)?.as_str()?;
    UpdateChannel::parse(channel)
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent_dir) = path.parent() {
        fs::create_dir_all(parent_dir).map_err(|error| {
            format!(
                "Failed to create update channel directory {}: {}",
                parent_dir.display(),
                error
            )
        })?;
    }

    Ok(())
}

fn load_state(path: &Path) -> Result<Map<String, Value>, String> {
    let raw_value = match fs::read_to_string(path) {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(value) => value,
            Err(error) => {
                crate::append_desktop_log(&format!(
                    "failed to parse update channel state {}: {}. resetting state file",
                    path.display(),
                    error
                ));
                Value::Object(Map::new())
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Value::Object(Map::new()),
        Err(error) => {
            return Err(format!(
                "Failed to read update channel state {}: {}",
                path.display(),
                error
            ));
        }
    };

    Ok(match raw_value {
        Value::Object(object) => object,
        _ => {
            crate::append_desktop_log(&format!(
                "update channel state {} has non-object root; resetting state file",
                path.display()
            ));
            Map::new()
        }
    })
}

fn save_state(path: &Path, state: &Map<String, Value>) -> Result<(), String> {
    ensure_parent_dir(path)?;

    let serialized = serde_json::to_string_pretty(state)
        .map_err(|error| format!("Failed to serialize update channel state: {error}"))?;
    fs::write(path, serialized).map_err(|error| {
        format!(
            "Failed to write update channel state {}: {}",
            path.display(),
            error
        )
    })
}

pub(crate) fn write_cached_update_channel(
    channel: Option<UpdateChannel>,
    packaged_root_dir: Option<&Path>,
) -> Result<(), String> {
    let Some(state_path) = desktop_state_path(packaged_root_dir) else {
        let message =
            "Update channel state path is unavailable; cannot persist update channel selection."
                .to_string();
        crate::append_desktop_log(&message);
        return Err(message);
    };

    let mut state = load_state(&state_path)?;
    match channel {
        Some(channel) => {
            state.insert(
                UPDATE_CHANNEL_FIELD.to_string(),
                Value::String(channel.config_key().to_string()),
            );
        }
        None => {
            state.remove(UPDATE_CHANNEL_FIELD);
        }
    }

    save_state(&state_path, &state)
}

pub(crate) fn resolve_preferred_channel(
    current_version: &Version,
    packaged_root_dir: Option<&Path>,
) -> UpdateChannel {
    read_cached_update_channel(packaged_root_dir)
        .unwrap_or_else(|| infer_channel_from_version(current_version))
}

/// Cross-channel update policy:
/// - stable -> stable: only strictly newer semver releases.
/// - stable -> nightly: allow same-base or newer-base nightly builds after an explicit channel switch, but only when the remote itself is nightly.
/// - nightly -> nightly: compare base version, then nightly date, then hash.
/// - nightly -> stable: only newer stable base versions; same-base stable is treated as a downgrade.
pub(crate) fn should_offer_update(
    current_version: &Version,
    preferred_channel: UpdateChannel,
    remote_version: &Version,
) -> bool {
    let current_channel = infer_channel_from_version(current_version);
    match (current_channel, preferred_channel) {
        (UpdateChannel::Stable, UpdateChannel::Stable) => remote_version > current_version,
        (UpdateChannel::Nightly, UpdateChannel::Nightly) => {
            should_offer_nightly_update(current_version, remote_version)
        }
        (UpdateChannel::Stable, UpdateChannel::Nightly) => {
            version_is_nightly(remote_version)
                && base_version(remote_version) >= base_version(current_version)
        }
        (UpdateChannel::Nightly, UpdateChannel::Stable) => {
            base_version(remote_version) > base_version(current_version)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    mod test_support {
        use std::{
            fs,
            path::PathBuf,
            time::{SystemTime, UNIX_EPOCH},
        };

        pub(super) fn create_temp_case_dir(name: &str) -> PathBuf {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos();
            let dir = std::env::temp_dir().join(format!(
                "astrbot-desktop-update-channel-test-{}-{}-{}",
                std::process::id(),
                ts,
                name
            ));
            fs::create_dir_all(&dir).expect("create temp case dir");
            dir
        }

        pub(super) struct EnvVarGuard {
            key: &'static str,
            previous: Option<String>,
        }

        impl EnvVarGuard {
            pub(super) fn clear(key: &'static str) -> Self {
                let previous = std::env::var(key).ok();
                std::env::remove_var(key);
                Self { key, previous }
            }
        }

        impl Drop for EnvVarGuard {
            fn drop(&mut self) {
                match self.previous.as_deref() {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    use test_support::{create_temp_case_dir, EnvVarGuard};

    fn version(raw: &str) -> Version {
        Version::parse(raw).expect("version should parse")
    }

    #[test]
    fn infer_channel_from_version_detects_nightly_versions() {
        assert_eq!(
            infer_channel_from_version(&version("4.29.0-nightly.20260307.abcd1234")),
            UpdateChannel::Nightly
        );
        assert_eq!(
            infer_channel_from_version(&version("4.29.0-nightly.20260307.abcd1234.extra")),
            UpdateChannel::Nightly
        );
        assert_eq!(
            infer_channel_from_version(&version("4.29.0")),
            UpdateChannel::Stable
        );
    }

    #[test]
    fn parse_nightly_version_info_matches_shared_examples() {
        let spec = nightly_version_format_spec();

        for raw in &spec.valid_examples {
            let parsed = Version::parse(raw).expect("valid nightly version should parse");
            assert!(parse_nightly_version_info(&parsed).is_some(), "{raw}");
        }

        for raw in &spec.invalid_examples {
            let parsed = Version::parse(raw)
                .expect("invalid nightly example should still be semver-parseable");
            assert!(parse_nightly_version_info(&parsed).is_none(), "{raw}");
        }
    }

    #[test]
    fn resolve_manifest_endpoint_prefers_environment_override() {
        let _guard = EnvVarGuard::clear(UpdateChannel::Nightly.env_override_key());
        std::env::set_var(
            UpdateChannel::Nightly.env_override_key(),
            "https://env.example/nightly.json",
        );

        let updater_config = json!({
            "channelEndpoints": {
                "stable": "https://config.example/stable.json",
                "nightly": "https://config.example/nightly.json"
            },
            "endpoints": ["https://config.example/stable.json"]
        });
        let mut plugins = HashMap::new();
        plugins.insert(UPDATER_PLUGIN_KEY.to_string(), updater_config);

        let endpoint = resolve_manifest_endpoint(&plugins, UpdateChannel::Nightly)
            .expect("nightly endpoint should resolve");

        assert_eq!(endpoint, "https://env.example/nightly.json");
    }

    #[test]
    fn resolve_manifest_endpoint_reads_channel_specific_config() {
        let updater_config = json!({
            "channelEndpoints": {
                "stable": "https://config.example/stable.json",
                "nightly": "https://config.example/nightly.json"
            },
            "endpoints": ["https://config.example/stable-fallback.json"]
        });

        let stable = resolve_manifest_endpoint_from_sources(
            UpdateChannel::Stable,
            updater_config.as_object().expect("object config"),
        )
        .expect("stable endpoint should resolve");
        let nightly = resolve_manifest_endpoint_from_sources(
            UpdateChannel::Nightly,
            updater_config.as_object().expect("object config"),
        )
        .expect("nightly endpoint should resolve");

        assert_eq!(stable, "https://config.example/stable.json");
        assert_eq!(nightly, "https://config.example/nightly.json");
    }

    #[test]
    fn resolve_manifest_endpoint_uses_stable_endpoint_fallback_array() {
        let updater_config = json!({
            "endpoints": ["https://config.example/stable-fallback.json"]
        });

        let stable = resolve_manifest_endpoint_from_sources(
            UpdateChannel::Stable,
            updater_config.as_object().expect("object config"),
        )
        .expect("stable endpoint should resolve");

        assert_eq!(stable, "https://config.example/stable-fallback.json");
    }

    #[test]
    fn resolve_manifest_endpoint_reports_stable_fallback_in_error() {
        let updater_config = json!({});

        let error = resolve_manifest_endpoint_from_sources(
            UpdateChannel::Stable,
            updater_config.as_object().expect("object config"),
        )
        .expect_err("stable endpoint should be missing");

        assert_eq!(
            error,
            format!(
                "Missing updater endpoint for 'stable' channel. Configure plugins.updater.channelEndpoints.stable, plugins.updater.endpoints[0], or set {}.",
                UpdateChannel::Stable.env_override_key()
            )
        );
    }

    #[test]
    fn resolve_manifest_endpoint_allows_env_override_without_updater_config() {
        let _guard = EnvVarGuard::clear(UpdateChannel::Nightly.env_override_key());
        std::env::set_var(
            UpdateChannel::Nightly.env_override_key(),
            "https://env.example/nightly.json",
        );

        let result = resolve_manifest_endpoint(&HashMap::new(), UpdateChannel::Nightly);

        assert_eq!(
            result.expect("env override should resolve without updater config"),
            "https://env.example/nightly.json"
        );
    }

    #[test]
    fn write_cached_channel_errors_when_state_path_unavailable() {
        let _root_guard = EnvVarGuard::clear("ASTRBOT_ROOT");

        let result = write_cached_update_channel(Some(UpdateChannel::Nightly), None);

        assert_eq!(
            result.expect_err("missing state path should fail persistence"),
            "Update channel state path is unavailable; cannot persist update channel selection."
        );
    }

    #[test]
    fn read_cached_channel_round_trips_written_value() {
        let _root_guard = EnvVarGuard::clear("ASTRBOT_ROOT");
        let dir = create_temp_case_dir("round-trip");
        write_cached_update_channel(Some(UpdateChannel::Nightly), Some(&dir))
            .expect("write cached channel");

        assert_eq!(
            read_cached_update_channel(Some(&dir)),
            Some(UpdateChannel::Nightly)
        );

        fs::remove_dir_all(&dir).expect("cleanup temp case dir");
    }

    #[test]
    fn write_cached_channel_preserves_unrelated_state_fields() {
        let _root_guard = EnvVarGuard::clear("ASTRBOT_ROOT");
        let dir = create_temp_case_dir("preserve-fields");
        let state_path = dir.join("data").join("desktop_state.json");
        fs::create_dir_all(state_path.parent().expect("state dir")).expect("create state dir");
        fs::write(
            &state_path,
            serde_json::to_string_pretty(&json!({
                "shellLocale": "en-US",
                "windowBounds": { "width": 1280, "height": 720 }
            }))
            .expect("serialize existing state"),
        )
        .expect("write existing state");

        write_cached_update_channel(Some(UpdateChannel::Nightly), Some(&dir))
            .expect("write cached channel");

        let raw = fs::read_to_string(&state_path).expect("read state");
        let parsed: Value = serde_json::from_str(&raw).expect("parse state");
        assert_eq!(
            parsed.get("shellLocale").and_then(Value::as_str),
            Some("en-US")
        );
        assert_eq!(
            parsed
                .get("windowBounds")
                .and_then(Value::as_object)
                .and_then(|bounds| bounds.get("width"))
                .and_then(Value::as_i64),
            Some(1280)
        );
        assert_eq!(
            parsed.get(UPDATE_CHANNEL_FIELD).and_then(Value::as_str),
            Some("nightly")
        );

        fs::remove_dir_all(&dir).expect("cleanup temp case dir");
    }

    #[test]
    fn resolve_preferred_channel_falls_back_to_installed_version_channel() {
        let _root_guard = EnvVarGuard::clear("ASTRBOT_ROOT");
        let dir = create_temp_case_dir("fallback");

        assert_eq!(
            resolve_preferred_channel(&version("4.29.0-nightly.20260307.abcd1234"), Some(&dir)),
            UpdateChannel::Nightly
        );

        fs::remove_dir_all(&dir).expect("cleanup temp case dir");
    }

    #[test]
    fn stable_to_nightly_allows_same_base_version() {
        assert!(should_offer_update(
            &version("4.29.0"),
            UpdateChannel::Nightly,
            &version("4.29.0-nightly.20260307.abcd1234")
        ));
    }

    #[test]
    fn stable_channel_preferring_nightly_rejects_stable_remote() {
        assert!(!should_offer_update(
            &version("4.29.0"),
            UpdateChannel::Nightly,
            &version("4.30.0")
        ));
    }

    #[test]
    fn stable_channel_preferring_nightly_rejects_same_base_stable_remote() {
        assert!(!should_offer_update(
            &version("4.29.0"),
            UpdateChannel::Nightly,
            &version("4.29.0")
        ));
    }

    #[test]
    fn nightly_to_stable_rejects_same_base_version() {
        assert!(!should_offer_update(
            &version("4.29.0-nightly.20260307.abcd1234"),
            UpdateChannel::Stable,
            &version("4.29.0")
        ));
    }

    #[test]
    fn nightly_to_stable_allows_higher_base_version() {
        assert!(should_offer_update(
            &version("4.29.0-nightly.20260307.abcd1234"),
            UpdateChannel::Stable,
            &version("4.30.0")
        ));
    }

    #[test]
    fn nightly_same_base_different_hash_can_update() {
        assert!(should_offer_update(
            &version("4.29.0-nightly.20260307.ffffffff"),
            UpdateChannel::Nightly,
            &version("4.29.0-nightly.20260307.11111111")
        ));
    }

    #[test]
    fn nightly_channel_rejects_stable_remote_same_base_version() {
        assert!(!should_offer_update(
            &version("4.29.0-nightly.20260307.abcd1234"),
            UpdateChannel::Nightly,
            &version("4.29.0")
        ));
    }

    #[test]
    fn nightly_channel_rejects_stable_remote_higher_base_version() {
        assert!(!should_offer_update(
            &version("4.29.0-nightly.20260307.abcd1234"),
            UpdateChannel::Nightly,
            &version("4.30.0")
        ));
    }

    #[test]
    fn same_channel_updates_still_require_strictly_newer_versions() {
        assert!(should_offer_update(
            &version("4.29.0"),
            UpdateChannel::Stable,
            &version("4.30.0")
        ));
        assert!(!should_offer_update(
            &version("4.29.0-nightly.20260307.abcd1234"),
            UpdateChannel::Nightly,
            &version("4.29.0-nightly.20260306.abcdef12")
        ));
    }
}

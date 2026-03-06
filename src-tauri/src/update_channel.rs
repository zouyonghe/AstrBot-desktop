use semver::{BuildMetadata, Prerelease, Version};
use serde::Serialize;
use serde_json::{Map, Value};
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

const UPDATE_CHANNEL_FIELD: &str = "updateChannel";
const NIGHTLY_IDENTIFIER: &str = "nightly";
const UPDATER_PLUGIN_KEY: &str = "updater";
const CHANNEL_ENDPOINTS_KEY: &str = "channelEndpoints";
const ENDPOINTS_KEY: &str = "endpoints";
const STABLE_ENDPOINT_ENV: &str = "ASTRBOT_DESKTOP_UPDATER_STABLE_ENDPOINT";
const NIGHTLY_ENDPOINT_ENV: &str = "ASTRBOT_DESKTOP_UPDATER_NIGHTLY_ENDPOINT";
// Canonical nightly version format lives in `src-tauri/nightly-version-format.json`.

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

fn empty_state_object() -> Value {
    Value::Object(Map::new())
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if let Value::Object(map) = value {
        return map;
    }

    *value = empty_state_object();
    value
        .as_object_mut()
        .expect("value was just normalized into a JSON object")
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

fn non_empty_string(raw: Option<&str>) -> Option<String> {
    let value = raw?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn configured_channel_endpoint(
    updater_config: &Map<String, Value>,
    channel: UpdateChannel,
) -> Option<String> {
    updater_config
        .get(CHANNEL_ENDPOINTS_KEY)
        .and_then(Value::as_object)
        .and_then(|channels| channels.get(channel.config_key()))
        .and_then(Value::as_str)
        .and_then(|value| non_empty_string(Some(value)))
}

pub(crate) fn resolve_manifest_endpoint_from_sources(
    channel: UpdateChannel,
    updater_config: &Map<String, Value>,
    env_override: Option<&str>,
) -> Result<String, String> {
    if let Some(endpoint) = non_empty_string(env_override) {
        return Ok(endpoint);
    }

    if let Some(endpoint) = configured_channel_endpoint(updater_config, channel) {
        return Ok(endpoint);
    }

    if channel == UpdateChannel::Stable {
        if let Some(endpoint) = updater_config
            .get(ENDPOINTS_KEY)
            .and_then(Value::as_array)
            .and_then(|endpoints| endpoints.first())
            .and_then(Value::as_str)
            .and_then(|value| non_empty_string(Some(value)))
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
    let env_override = env::var(channel.env_override_key()).ok();
    if let Some(endpoint) = non_empty_string(env_override.as_deref()) {
        return Ok(endpoint);
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

    resolve_manifest_endpoint_from_sources(channel, updater_config, None)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NightlyVersionInfo {
    base: Version,
    date: u32,
    hash: String,
}

fn parse_nightly_version_info(version: &Version) -> Option<NightlyVersionInfo> {
    let mut identifiers = version.pre.as_str().split('.');
    let first = identifiers.next()?;
    if !first.eq_ignore_ascii_case(NIGHTLY_IDENTIFIER) {
        return None;
    }

    let date_raw = identifiers.next()?;
    let hash_raw = identifiers.next()?;
    if identifiers.next().is_some() {
        return None;
    }

    if date_raw.len() != 8 || !date_raw.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    if hash_raw.len() != 8 || !hash_raw.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }

    let date = date_raw.parse::<u32>().ok()?;
    let hash = hash_raw.to_ascii_lowercase();

    Some(NightlyVersionInfo {
        base: base_version(version),
        date,
        hash,
    })
}

fn should_offer_nightly_update(current_version: &Version, remote_version: &Version) -> bool {
    let Some(current) = parse_nightly_version_info(current_version) else {
        return remote_version > current_version;
    };
    let Some(remote) = parse_nightly_version_info(remote_version) else {
        return remote_version > current_version;
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

pub(crate) fn write_cached_update_channel(
    channel: Option<UpdateChannel>,
    packaged_root_dir: Option<&Path>,
) -> Result<(), String> {
    let Some(state_path) = desktop_state_path(packaged_root_dir) else {
        crate::append_desktop_log(
            "update channel state path is unavailable; skipping channel persistence",
        );
        return Ok(());
    };

    if let Some(parent_dir) = state_path.parent() {
        fs::create_dir_all(parent_dir).map_err(|error| {
            format!(
                "Failed to create update channel directory {}: {}",
                parent_dir.display(),
                error
            )
        })?;
    }

    let mut parsed = match fs::read_to_string(&state_path) {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(value) => value,
            Err(error) => {
                crate::append_desktop_log(&format!(
                    "failed to parse update channel state {}: {}. resetting state file",
                    state_path.display(),
                    error
                ));
                empty_state_object()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => empty_state_object(),
        Err(error) => {
            return Err(format!(
                "Failed to read update channel state {}: {}",
                state_path.display(),
                error
            ));
        }
    };
    if !parsed.is_object() {
        crate::append_desktop_log(&format!(
            "update channel state {} has non-object root; resetting state file",
            state_path.display()
        ));
    }
    let object = ensure_object(&mut parsed);

    if let Some(channel) = channel {
        let value = match channel {
            UpdateChannel::Stable => "stable",
            UpdateChannel::Nightly => "nightly",
        };
        object.insert(
            UPDATE_CHANNEL_FIELD.to_string(),
            Value::String(value.to_string()),
        );
    } else {
        object.remove(UPDATE_CHANNEL_FIELD);
    }

    let serialized = serde_json::to_string_pretty(&parsed)
        .map_err(|error| format!("Failed to serialize update channel state: {error}"))?;
    fs::write(&state_path, serialized).map_err(|error| {
        format!(
            "Failed to write update channel state {}: {}",
            state_path.display(),
            error
        )
    })
}

pub(crate) fn resolve_preferred_channel(
    current_version: &Version,
    packaged_root_dir: Option<&Path>,
) -> UpdateChannel {
    read_cached_update_channel(packaged_root_dir)
        .unwrap_or_else(|| infer_channel_from_version(current_version))
}

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
            base_version(remote_version) >= base_version(current_version)
        }
        (UpdateChannel::Nightly, UpdateChannel::Stable) => {
            base_version(remote_version) > base_version(current_version)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NightlyVersionFormatSpec {
        canonical_format: String,
        valid_examples: Vec<String>,
        invalid_examples: Vec<String>,
    }

    fn nightly_version_format_spec() -> NightlyVersionFormatSpec {
        serde_json::from_str(include_str!("../nightly-version-format.json"))
            .expect("nightly version format spec should parse")
    }

    fn version(raw: &str) -> Version {
        Version::parse(raw).expect("version should parse")
    }

    fn create_temp_case_dir(name: &str) -> PathBuf {
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

    #[test]
    fn infer_channel_from_version_detects_nightly_versions() {
        assert_eq!(
            infer_channel_from_version(&version("4.29.0-nightly.20260307.abcd1234")),
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
        assert_eq!(spec.canonical_format, "<base>-nightly.<YYYYMMDD>.<sha8>");

        for raw in spec.valid_examples {
            let parsed = Version::parse(&raw).expect("valid nightly version should parse");
            assert!(parse_nightly_version_info(&parsed).is_some(), "{raw}");
        }

        for raw in spec.invalid_examples {
            let parsed = Version::parse(&raw)
                .expect("invalid nightly example should still be semver-parseable");
            assert!(parse_nightly_version_info(&parsed).is_none(), "{raw}");
        }
    }

    #[test]
    fn resolve_manifest_endpoint_prefers_environment_override() {
        let updater_config = json!({
            "channelEndpoints": {
                "stable": "https://config.example/stable.json",
                "nightly": "https://config.example/nightly.json"
            },
            "endpoints": ["https://config.example/stable.json"]
        });

        let endpoint = resolve_manifest_endpoint_from_sources(
            UpdateChannel::Nightly,
            updater_config.as_object().expect("object config"),
            Some("https://env.example/nightly.json"),
        )
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
            None,
        )
        .expect("stable endpoint should resolve");
        let nightly = resolve_manifest_endpoint_from_sources(
            UpdateChannel::Nightly,
            updater_config.as_object().expect("object config"),
            None,
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
            None,
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
            None,
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
        let env_key = UpdateChannel::Nightly.env_override_key();
        let previous = std::env::var(env_key).ok();
        std::env::set_var(env_key, "https://env.example/nightly.json");

        let result = resolve_manifest_endpoint(&HashMap::new(), UpdateChannel::Nightly);

        match previous {
            Some(value) => std::env::set_var(env_key, value),
            None => std::env::remove_var(env_key),
        }

        assert_eq!(
            result.expect("env override should resolve without updater config"),
            "https://env.example/nightly.json"
        );
    }

    #[test]
    fn read_cached_channel_round_trips_written_value() {
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
    fn resolve_preferred_channel_falls_back_to_installed_version_channel() {
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

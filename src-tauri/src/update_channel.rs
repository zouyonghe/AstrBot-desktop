use semver::{BuildMetadata, Prerelease, Version};
use serde::Serialize;
use serde_json::{Map, Value};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const UPDATE_CHANNEL_FIELD: &str = "updateChannel";
const NIGHTLY_IDENTIFIER: &str = "nightly";
const STABLE_MANIFEST_URL: &str =
    "https://github.com/AstrBotDevs/AstrBot-desktop/releases/latest/download/latest-stable.json";
const NIGHTLY_MANIFEST_URL: &str =
    "https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/nightly/latest-nightly.json";

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

    pub(crate) fn manifest_url(self) -> &'static str {
        match self {
            Self::Stable => STABLE_MANIFEST_URL,
            Self::Nightly => NIGHTLY_MANIFEST_URL,
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

    let date = identifiers.next()?.parse::<u32>().ok()?;
    let hash = identifiers.next()?.to_string();
    if hash.is_empty() {
        return None;
    }

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
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

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
            &version("4.29.0-nightly.20260307.zzzzzzzz"),
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

use std::{fs, io::Write, path::Path, sync::Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DesktopSettings {
    pub(crate) launch_at_login: bool,
    pub(crate) silent_launch: bool,
    pub(crate) close_to_tray: bool,
}

#[derive(Debug)]
pub(crate) struct DesktopSettingsCache {
    settings: Mutex<DesktopSettings>,
}

impl DesktopSettingsCache {
    pub(crate) fn new(settings: DesktopSettings) -> Self {
        Self {
            settings: Mutex::new(settings),
        }
    }

    pub(crate) fn get(&self) -> DesktopSettings {
        match self.settings.lock() {
            Ok(guard) => *guard,
            Err(error) => {
                crate::append_desktop_log(&format!(
                    "desktop settings cache lock poisoned, returning inner value: {error}"
                ));
                *error.into_inner()
            }
        }
    }

    pub(crate) fn set(&self, settings: DesktopSettings) {
        match self.settings.lock() {
            Ok(mut guard) => *guard = settings,
            Err(error) => {
                crate::append_desktop_log(&format!(
                    "desktop settings cache lock poisoned, updating inner value: {error}"
                ));
                *error.into_inner() = settings;
            }
        }
    }
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            launch_at_login: false,
            silent_launch: false,
            close_to_tray: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DesktopSettingKey {
    LaunchAtLogin,
    SilentLaunch,
    CloseToTray,
}

fn default_launch_at_login() -> bool {
    false
}

fn default_silent_launch() -> bool {
    false
}

fn default_close_to_tray() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize)]
struct DesktopSettingsState {
    #[serde(rename = "launchAtLogin", default = "default_launch_at_login")]
    launch_at_login: bool,
    #[serde(rename = "silentLaunch", default = "default_silent_launch")]
    silent_launch: bool,
    #[serde(rename = "closeToTray", default = "default_close_to_tray")]
    close_to_tray: bool,
    #[serde(flatten)]
    other: Map<String, Value>,
}

impl Default for DesktopSettingsState {
    fn default() -> Self {
        Self {
            launch_at_login: default_launch_at_login(),
            silent_launch: default_silent_launch(),
            close_to_tray: default_close_to_tray(),
            other: Map::new(),
        }
    }
}

impl From<DesktopSettingsState> for DesktopSettings {
    fn from(state: DesktopSettingsState) -> Self {
        DesktopSettings {
            launch_at_login: state.launch_at_login,
            silent_launch: state.silent_launch,
            close_to_tray: state.close_to_tray,
        }
    }
}

fn load_state(path: &Path) -> Result<DesktopSettingsState, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DesktopSettingsState::default());
        }
        Err(error) => {
            return Err(format!(
                "Failed to read desktop settings state {}: {}",
                path.display(),
                error
            ));
        }
    };

    match serde_json::from_str::<DesktopSettingsState>(&raw) {
        Ok(state) => Ok(state),
        Err(error) => {
            crate::append_desktop_log(&format!(
                "failed to parse desktop settings state {}: {}. resetting state file",
                path.display(),
                error
            ));
            let default_state = DesktopSettingsState::default();
            if let Err(save_error) = save_state(path, &default_state) {
                crate::append_desktop_log(&format!(
                    "failed to persist reset desktop settings state {}: {}",
                    path.display(),
                    save_error
                ));
            }
            Ok(default_state)
        }
    }
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent_dir) = path.parent() {
        fs::create_dir_all(parent_dir).map_err(|error| {
            format!(
                "Failed to create desktop settings directory {}: {}",
                parent_dir.display(),
                error
            )
        })?;
    }

    Ok(())
}

fn save_state(path: &Path, state: &DesktopSettingsState) -> Result<(), String> {
    ensure_parent_dir(path)?;

    let serialized = serde_json::to_string_pretty(state)
        .map_err(|error| format!("Failed to serialize desktop settings state: {error}"))?;
    let tmp_name = format!(
        "{}.tmp",
        path.file_name()
            .map(|value| value.to_string_lossy())
            .unwrap_or_default()
    );
    let tmp_path = path.with_file_name(tmp_name);

    let mut file = fs::File::create(&tmp_path).map_err(|error| {
        format!(
            "Failed to create temporary desktop settings state file {}: {}",
            tmp_path.display(),
            error
        )
    })?;
    file.write_all(serialized.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|error| {
            format!(
                "Failed to write temporary desktop settings state file {}: {}",
                tmp_path.display(),
                error
            )
        })?;
    fs::rename(&tmp_path, path).map_err(|error| {
        format!(
            "Failed to atomically replace desktop settings state file {}: {}",
            path.display(),
            error
        )
    })
}

pub(crate) fn read_desktop_settings(packaged_root_dir: Option<&Path>) -> DesktopSettings {
    let Some(state_path) = crate::desktop_state::resolve_desktop_state_path(packaged_root_dir)
    else {
        return DesktopSettings::default();
    };

    match load_state(&state_path) {
        Ok(state) => DesktopSettings::from(state),
        Err(error) => {
            crate::append_desktop_log(&error);
            DesktopSettings::default()
        }
    }
}

pub(crate) fn write_desktop_setting(
    packaged_root_dir: Option<&Path>,
    key: DesktopSettingKey,
    value: bool,
) -> Result<DesktopSettings, String> {
    let Some(state_path) = crate::desktop_state::resolve_desktop_state_path(packaged_root_dir)
    else {
        let message =
            "Desktop settings state path is unavailable; cannot persist setting.".to_string();
        crate::append_desktop_log(&message);
        return Err(message);
    };

    let mut state = load_state(&state_path)?;
    match key {
        DesktopSettingKey::LaunchAtLogin => state.launch_at_login = value,
        DesktopSettingKey::SilentLaunch => state.silent_launch = value,
        DesktopSettingKey::CloseToTray => state.close_to_tray = value,
    }
    save_state(&state_path, &state)?;
    Ok(DesktopSettings::from(state))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;

    fn create_temp_case_dir(name: &str) -> PathBuf {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "astrbot-desktop-settings-test-{}-{}-{}",
            std::process::id(),
            name,
            ts
        ))
    }

    fn state_path(root: &std::path::Path) -> PathBuf {
        root.join("data").join("desktop_state.json")
    }

    #[test]
    fn desktop_settings_default_preserves_existing_close_to_tray_behavior() {
        assert_eq!(
            DesktopSettings::default(),
            DesktopSettings {
                launch_at_login: false,
                silent_launch: false,
                close_to_tray: true,
            }
        );
    }

    #[test]
    fn read_desktop_settings_maps_camel_case_fields() {
        let root = create_temp_case_dir("read");
        let path = state_path(&root);
        fs::create_dir_all(path.parent().expect("state parent")).expect("create state parent");
        fs::write(
            &path,
            r#"{"launchAtLogin":true,"silentLaunch":true,"closeToTray":false}"#,
        )
        .expect("write state");

        assert_eq!(
            read_desktop_settings(Some(&root)),
            DesktopSettings {
                launch_at_login: true,
                silent_launch: true,
                close_to_tray: false,
            }
        );
    }

    #[test]
    fn read_desktop_settings_applies_field_defaults_for_missing_values() {
        let root = create_temp_case_dir("defaults");
        let path = state_path(&root);
        fs::create_dir_all(path.parent().expect("state parent")).expect("create state parent");
        fs::write(&path, r#"{"silentLaunch":true}"#).expect("write state");

        assert_eq!(
            read_desktop_settings(Some(&root)),
            DesktopSettings {
                launch_at_login: false,
                silent_launch: true,
                close_to_tray: true,
            }
        );
    }

    #[test]
    fn desktop_settings_cache_returns_updated_value_without_reloading_file() {
        let cache = DesktopSettingsCache::new(DesktopSettings::default());
        let updated = DesktopSettings {
            launch_at_login: true,
            silent_launch: true,
            close_to_tray: false,
        };

        cache.set(updated);

        assert_eq!(cache.get(), updated);
    }

    #[test]
    fn write_desktop_setting_preserves_unknown_fields() {
        let root = create_temp_case_dir("preserve");
        let path = state_path(&root);
        fs::create_dir_all(path.parent().expect("state parent")).expect("create state parent");
        fs::write(
            &path,
            r#"{"locale":"zh-CN","updateChannel":"nightly","silentLaunch":false}"#,
        )
        .expect("write state");

        let updated = write_desktop_setting(Some(&root), DesktopSettingKey::SilentLaunch, true)
            .expect("write setting");

        assert!(updated.silent_launch);
        let raw = fs::read_to_string(&path).expect("read state");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse state");
        assert_eq!(
            parsed.get("locale").and_then(|value| value.as_str()),
            Some("zh-CN")
        );
        assert_eq!(
            parsed.get("updateChannel").and_then(|value| value.as_str()),
            Some("nightly")
        );
        assert_eq!(
            parsed.get("silentLaunch").and_then(|value| value.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn invalid_state_falls_back_to_defaults_and_write_resets_object() {
        let root = create_temp_case_dir("invalid");
        let path = state_path(&root);
        fs::create_dir_all(path.parent().expect("state parent")).expect("create state parent");
        fs::write(&path, "not-json").expect("write invalid state");

        assert_eq!(
            read_desktop_settings(Some(&root)),
            DesktopSettings::default()
        );

        let updated = write_desktop_setting(Some(&root), DesktopSettingKey::CloseToTray, false)
            .expect("write setting");

        assert!(!updated.close_to_tray);
        let raw = fs::read_to_string(&path).expect("read state");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse reset state");
        assert_eq!(
            parsed.get("closeToTray").and_then(|value| value.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn invalid_state_is_rewritten_to_defaults_on_read() {
        let root = create_temp_case_dir("invalid-read-reset");
        let path = state_path(&root);
        fs::create_dir_all(path.parent().expect("state parent")).expect("create state parent");
        fs::write(&path, "not-json").expect("write invalid state");

        assert_eq!(
            read_desktop_settings(Some(&root)),
            DesktopSettings::default()
        );

        let raw = fs::read_to_string(&path).expect("read reset state");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse reset state");
        assert_eq!(
            parsed
                .get("launchAtLogin")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            parsed.get("silentLaunch").and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            parsed.get("closeToTray").and_then(|value| value.as_bool()),
            Some(true)
        );
    }
}

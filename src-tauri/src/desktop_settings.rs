use std::{fs, io::Write, path::Path, sync::Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
        self.settings
            .lock()
            .expect("desktop settings cache lock")
            .clone()
    }

    pub(crate) fn set(&self, settings: DesktopSettings) {
        *self.settings.lock().expect("desktop settings cache lock") = settings;
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct DesktopSettings {
    #[serde(rename = "launchAtLogin", default = "default_launch_at_login")]
    pub(crate) launch_at_login: bool,
    #[serde(rename = "silentLaunch", default = "default_silent_launch")]
    pub(crate) silent_launch: bool,
    #[serde(rename = "closeToTray", default = "default_close_to_tray")]
    pub(crate) close_to_tray: bool,
    #[serde(flatten)]
    other: Map<String, Value>,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            launch_at_login: default_launch_at_login(),
            silent_launch: default_silent_launch(),
            close_to_tray: default_close_to_tray(),
            other: Map::new(),
        }
    }
}

impl DesktopSettings {
    fn set(&mut self, key: DesktopSettingKey, value: bool) {
        match key {
            DesktopSettingKey::LaunchAtLogin => self.launch_at_login = value,
            DesktopSettingKey::SilentLaunch => self.silent_launch = value,
            DesktopSettingKey::CloseToTray => self.close_to_tray = value,
        }
    }
}

fn load_state(path: &Path) -> Result<DesktopSettings, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DesktopSettings::default());
        }
        Err(error) => {
            return Err(format!(
                "Failed to read desktop settings state {}: {}",
                path.display(),
                error
            ));
        }
    };

    match serde_json::from_str::<DesktopSettings>(&raw) {
        Ok(state) => Ok(state),
        Err(error) => {
            crate::append_desktop_log(&format!(
                "failed to parse desktop settings state {}: {}. resetting state file",
                path.display(),
                error
            ));
            let default_state = DesktopSettings::default();
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

fn save_state(path: &Path, state: &DesktopSettings) -> Result<(), String> {
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
        Ok(state) => state,
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
    state.set(key, value);
    save_state(&state_path, &state)?;
    Ok(state)
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

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn clear(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    fn settings(
        launch_at_login: bool,
        silent_launch: bool,
        close_to_tray: bool,
    ) -> DesktopSettings {
        DesktopSettings {
            launch_at_login,
            silent_launch,
            close_to_tray,
            ..DesktopSettings::default()
        }
    }

    #[test]
    fn desktop_settings_default_preserves_existing_close_to_tray_behavior() {
        assert_eq!(DesktopSettings::default(), settings(false, false, true));
    }

    #[test]
    fn read_desktop_settings_maps_camel_case_fields() {
        let _root_guard = EnvVarGuard::clear(crate::ASTRBOT_ROOT_ENV);
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
            settings(true, true, false)
        );
    }

    #[test]
    fn read_desktop_settings_applies_field_defaults_for_missing_values() {
        let _root_guard = EnvVarGuard::clear(crate::ASTRBOT_ROOT_ENV);
        let root = create_temp_case_dir("defaults");
        let path = state_path(&root);
        fs::create_dir_all(path.parent().expect("state parent")).expect("create state parent");
        fs::write(&path, r#"{"silentLaunch":true}"#).expect("write state");

        assert_eq!(
            read_desktop_settings(Some(&root)),
            settings(false, true, true)
        );
    }

    #[test]
    fn desktop_settings_cache_returns_updated_value_without_reloading_file() {
        let cache = DesktopSettingsCache::new(DesktopSettings::default());
        let updated = settings(true, true, false);

        cache.set(updated.clone());

        assert_eq!(cache.get(), updated);
    }

    #[test]
    fn write_desktop_setting_preserves_unknown_fields() {
        let _root_guard = EnvVarGuard::clear(crate::ASTRBOT_ROOT_ENV);
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
        let _root_guard = EnvVarGuard::clear(crate::ASTRBOT_ROOT_ENV);
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
        let _root_guard = EnvVarGuard::clear(crate::ASTRBOT_ROOT_ENV);
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

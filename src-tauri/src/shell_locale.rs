use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde_json::{Map, Value};

const LOCALE_FIELD: &str = "locale";

fn empty_state_object() -> Value {
    Value::Object(Map::new())
}

#[derive(Debug, Clone, Copy)]
pub struct ShellTexts {
    pub tray_hide: &'static str,
    pub tray_show: &'static str,
    pub tray_reload: &'static str,
    pub tray_restart_backend: &'static str,
    pub tray_quit: &'static str,
}

pub fn shell_texts_for_locale(locale: &str) -> ShellTexts {
    if locale == "en-US" {
        return ShellTexts {
            tray_hide: "Hide AstrBot",
            tray_show: "Show AstrBot",
            tray_reload: "Reload UI",
            tray_restart_backend: "Restart Backend",
            tray_quit: "Quit",
        };
    }

    ShellTexts {
        tray_hide: "隐藏 AstrBot",
        tray_show: "显示 AstrBot",
        tray_reload: "重载界面",
        tray_restart_backend: "重启后端",
        tray_quit: "退出",
    }
}

pub fn resolve_shell_locale(
    default_shell_locale: &'static str,
    packaged_root_dir: Option<PathBuf>,
) -> &'static str {
    if let Some(locale) = read_cached_shell_locale(packaged_root_dir.as_deref()) {
        return locale;
    }

    for env_key in ["ASTRBOT_DESKTOP_LOCALE", "LC_ALL", "LANG"] {
        if let Ok(value) = env::var(env_key) {
            if let Some(locale) = normalize_shell_locale(&value) {
                return locale;
            }
        }
    }

    default_shell_locale
}

pub(crate) fn normalize_shell_locale(raw: &str) -> Option<&'static str> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw == "zh-CN" {
        return Some("zh-CN");
    }
    if raw == "en-US" {
        return Some("en-US");
    }

    let lowered = raw.to_ascii_lowercase();
    if lowered.starts_with("zh") {
        return Some("zh-CN");
    }
    if lowered.starts_with("en") {
        return Some("en-US");
    }
    None
}

fn desktop_state_path_for_locale(packaged_root_dir: Option<&Path>) -> Option<PathBuf> {
    if let Ok(root) = env::var("ASTRBOT_ROOT") {
        let path = PathBuf::from(root.trim());
        if !path.as_os_str().is_empty() {
            return Some(path.join("data").join("desktop_state.json"));
        }
    }

    packaged_root_dir.map(|root| root.join("data").join("desktop_state.json"))
}

fn read_cached_shell_locale(packaged_root_dir: Option<&Path>) -> Option<&'static str> {
    let state_path = desktop_state_path_for_locale(packaged_root_dir)?;
    let raw = fs::read_to_string(state_path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let locale = parsed.get(LOCALE_FIELD)?.as_str()?;
    normalize_shell_locale(locale)
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if let Value::Object(map) = value {
        return map;
    }

    *value = empty_state_object();
    // Safe because `value` was just replaced with an object.
    value
        .as_object_mut()
        .expect("value was just normalized into a JSON object")
}

pub(crate) fn write_cached_shell_locale(
    locale: Option<&str>,
    packaged_root_dir: Option<&Path>,
) -> Result<(), String> {
    let normalized_locale = locale.and_then(normalize_shell_locale);
    if let Some(raw_locale) = locale {
        if normalized_locale.is_none() {
            crate::append_desktop_log(&format!(
                "unsupported shell locale '{}'; clearing cached locale",
                raw_locale
            ));
        }
    }

    let Some(state_path) = desktop_state_path_for_locale(packaged_root_dir) else {
        crate::append_desktop_log(
            "shell locale state path is unavailable; skipping locale persistence",
        );
        return Ok(());
    };

    if let Some(parent_dir) = state_path.parent() {
        fs::create_dir_all(parent_dir).map_err(|error| {
            format!(
                "Failed to create shell locale directory {}: {}",
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
                    "failed to parse shell locale state {}: {}. resetting state file",
                    state_path.display(),
                    error
                ));
                empty_state_object()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => empty_state_object(),
        Err(error) => {
            return Err(format!(
                "Failed to read shell locale state {}: {}",
                state_path.display(),
                error
            ));
        }
    };
    if !parsed.is_object() {
        crate::append_desktop_log(&format!(
            "shell locale state {} has non-object root; resetting state file",
            state_path.display()
        ));
    }
    let object = ensure_object(&mut parsed);

    if let Some(normalized_locale) = normalized_locale {
        object.insert(
            LOCALE_FIELD.to_string(),
            Value::String(normalized_locale.to_string()),
        );
    } else {
        object.remove(LOCALE_FIELD);
    }

    let serialized = serde_json::to_string_pretty(&parsed)
        .map_err(|error| format!("Failed to serialize shell locale state: {error}"))?;
    fs::write(&state_path, serialized).map_err(|error| {
        format!(
            "Failed to write shell locale state {}: {}",
            state_path.display(),
            error
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_texts_for_locale_returns_english_copy() {
        let texts = shell_texts_for_locale("en-US");
        assert_eq!(texts.tray_hide, "Hide AstrBot");
        assert_eq!(texts.tray_quit, "Quit");
    }

    #[test]
    fn shell_texts_for_locale_falls_back_to_zh_cn_copy() {
        let texts = shell_texts_for_locale("zh-CN");
        assert_eq!(texts.tray_hide, "隐藏 AstrBot");
        assert_eq!(texts.tray_quit, "退出");
    }

    #[test]
    fn normalize_shell_locale_accepts_language_prefixes() {
        assert_eq!(normalize_shell_locale("EN_us"), Some("en-US"));
        assert_eq!(normalize_shell_locale("zh_TW"), Some("zh-CN"));
        assert_eq!(normalize_shell_locale("fr-FR"), None);
    }
}

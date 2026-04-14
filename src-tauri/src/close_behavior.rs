use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const CLOSE_ACTION_FIELD: &str = "closeActionOnWindowClose";

fn empty_state_object() -> Value {
    Value::Object(Map::new())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CloseAction {
    Tray,
    Exit,
}

impl CloseAction {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "tray" => Some(Self::Tray),
            "exit" => Some(Self::Exit),
            _ => None,
        }
    }

    fn as_state_value(self) -> &'static str {
        match self {
            Self::Tray => "tray",
            Self::Exit => "exit",
        }
    }
}

pub(crate) fn parse_close_action(raw: &str) -> Option<CloseAction> {
    CloseAction::parse(raw)
}

pub(crate) fn read_cached_close_action(packaged_root_dir: Option<&Path>) -> Option<CloseAction> {
    read_cached_close_action_from_state_path(crate::desktop_state::resolve_desktop_state_path(
        packaged_root_dir,
    ))
}

fn read_cached_close_action_from_state_path(state_path: Option<PathBuf>) -> Option<CloseAction> {
    let raw = fs::read_to_string(state_path?).ok()?;
    let parsed = load_state_value(&raw, "desktop close behavior state");
    let action = parsed.get(CLOSE_ACTION_FIELD)?.as_str()?;
    parse_close_action(action)
}

fn load_state_value(raw: &str, log_subject: &str) -> Value {
    match serde_json::from_str::<Value>(raw) {
        Ok(value) if value.is_object() => value,
        Ok(_) => {
            crate::append_desktop_log(&format!(
                "{log_subject} has non-object root; resetting state semantics"
            ));
            empty_state_object()
        }
        Err(error) => {
            crate::append_desktop_log(&format!(
                "failed to parse {log_subject}: {error}. resetting state semantics"
            ));
            empty_state_object()
        }
    }
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

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent_dir) = path.parent() {
        fs::create_dir_all(parent_dir).map_err(|error| {
            format!(
                "Failed to create close behavior directory {}: {}",
                parent_dir.display(),
                error
            )
        })?;
    }

    Ok(())
}

fn save_state(path: &Path, state: &Value) -> Result<(), String> {
    ensure_parent_dir(path)?;

    let serialized = serde_json::to_string_pretty(state)
        .map_err(|error| format!("Failed to serialize close behavior state: {error}"))?;
    let tmp_name = format!(
        "{}.tmp",
        path.file_name()
            .map(|value| value.to_string_lossy())
            .unwrap_or_default()
    );
    let tmp_path = path.with_file_name(tmp_name);

    let mut file = fs::File::create(&tmp_path).map_err(|error| {
        format!(
            "Failed to create temporary close behavior state file {}: {}",
            tmp_path.display(),
            error
        )
    })?;
    file.write_all(serialized.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|error| {
            format!(
                "Failed to write temporary close behavior state file {}: {}",
                tmp_path.display(),
                error
            )
        })?;
    fs::rename(&tmp_path, path).map_err(|error| {
        format!(
            "Failed to atomically replace close behavior state file {}: {}",
            path.display(),
            error
        )
    })
}

pub(crate) fn write_cached_close_action(
    action: Option<CloseAction>,
    packaged_root_dir: Option<&Path>,
) -> Result<(), String> {
    write_cached_close_action_to_state_path(
        action,
        crate::desktop_state::resolve_desktop_state_path(packaged_root_dir),
    )
}

fn write_cached_close_action_to_state_path(
    action: Option<CloseAction>,
    state_path: Option<PathBuf>,
) -> Result<(), String> {
    let Some(state_path) = state_path else {
        crate::append_desktop_log(
            "close behavior state path is unavailable; skipping close action persistence",
        );
        return Ok(());
    };

    let mut parsed = match fs::read_to_string(&state_path) {
        Ok(raw) => load_state_value(
            &raw,
            &format!("close behavior state {}", state_path.display()),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => empty_state_object(),
        Err(error) => {
            return Err(format!(
                "Failed to read close behavior state {}: {}",
                state_path.display(),
                error
            ));
        }
    };
    let object = ensure_object(&mut parsed);

    if let Some(action) = action {
        object.insert(
            CLOSE_ACTION_FIELD.to_string(),
            Value::String(action.as_state_value().to_string()),
        );
    } else {
        object.remove(CLOSE_ACTION_FIELD);
    }

    save_state(&state_path, &parsed)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        parse_close_action, read_cached_close_action_from_state_path,
        write_cached_close_action_to_state_path, CloseAction,
    };
    use serde_json::json;
    use std::{fs, path::PathBuf};

    fn state_path(temp_dir: &tempfile::TempDir) -> PathBuf {
        temp_dir.path().join("data").join("desktop_state.json")
    }

    #[test]
    fn read_cached_close_action_returns_none_when_state_file_is_missing() {
        let temp_dir = tempfile::tempdir().expect("temp dir");

        assert_eq!(
            read_cached_close_action_from_state_path(Some(state_path(&temp_dir))),
            None
        );
    }

    #[test]
    fn parse_close_action_accepts_tray_and_exit_only() {
        assert_eq!(parse_close_action("tray"), Some(CloseAction::Tray));
        assert_eq!(parse_close_action("exit"), Some(CloseAction::Exit));
    }

    #[test]
    fn parse_close_action_rejects_invalid_values() {
        assert_eq!(parse_close_action(""), None);
        assert_eq!(parse_close_action(" tray "), None);
        assert_eq!(parse_close_action("minimize"), None);
        assert_eq!(parse_close_action("TRAY"), None);
    }

    #[test]
    fn write_cached_close_action_preserves_unrelated_state_fields() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = state_path(&temp_dir);
        fs::create_dir_all(state_path.parent().expect("state parent")).expect("create state dir");
        fs::write(
            &state_path,
            serde_json::to_string_pretty(&json!({
                "locale": "en-US",
                "nested": { "enabled": true }
            }))
            .expect("serialize state"),
        )
        .expect("write state");

        write_cached_close_action_to_state_path(Some(CloseAction::Tray), Some(state_path.clone()))
            .expect("write close action");

        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&state_path).expect("read updated state"))
                .expect("parse updated state");

        assert_eq!(saved.get("closeActionOnWindowClose"), Some(&json!("tray")));
        assert_eq!(saved.get("locale"), Some(&json!("en-US")));
        assert_eq!(saved.get("nested"), Some(&json!({ "enabled": true })));
    }

    #[test]
    fn write_cached_close_action_resets_malformed_state_to_object() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = state_path(&temp_dir);
        fs::create_dir_all(state_path.parent().expect("state parent")).expect("create state dir");
        fs::write(&state_path, "[").expect("write malformed state");

        write_cached_close_action_to_state_path(Some(CloseAction::Exit), Some(state_path.clone()))
            .expect("write close action");

        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&state_path).expect("read updated state"))
                .expect("parse updated state");

        assert_eq!(saved, json!({ "closeActionOnWindowClose": "exit" }));
    }

    #[test]
    fn read_cached_close_action_returns_saved_value() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = state_path(&temp_dir);

        write_cached_close_action_to_state_path(Some(CloseAction::Tray), Some(state_path.clone()))
            .expect("write close action");

        assert_eq!(
            read_cached_close_action_from_state_path(Some(state_path)),
            Some(CloseAction::Tray)
        );
    }

    #[test]
    fn write_cached_close_action_none_removes_only_close_action_field() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = state_path(&temp_dir);
        fs::create_dir_all(state_path.parent().expect("state parent")).expect("create state dir");
        fs::write(
            &state_path,
            serde_json::to_string_pretty(&json!({
                "closeActionOnWindowClose": "exit",
                "locale": "zh-CN"
            }))
            .expect("serialize state"),
        )
        .expect("write state");

        write_cached_close_action_to_state_path(None, Some(state_path.clone()))
            .expect("clear close action");

        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&state_path).expect("read updated state"))
                .expect("parse updated state");

        assert_eq!(saved.get("closeActionOnWindowClose"), None);
        assert_eq!(saved.get("locale"), Some(&json!("zh-CN")));
    }

    #[test]
    fn read_cached_close_action_treats_malformed_state_as_empty_object() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = state_path(&temp_dir);
        fs::create_dir_all(state_path.parent().expect("state parent")).expect("create state dir");
        fs::write(&state_path, "[").expect("write malformed state");

        assert_eq!(
            read_cached_close_action_from_state_path(Some(state_path)),
            None
        );
    }
}

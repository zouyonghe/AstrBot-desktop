use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use std::{fs, io::Write, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CloseAction {
    Tray,
    Exit,
}

fn deserialize_close_action_option<'de, D>(deserializer: D) -> Result<Option<CloseAction>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.as_deref().and_then(parse_close_action))
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DesktopState {
    #[serde(
        rename = "closeActionOnWindowClose",
        default,
        deserialize_with = "deserialize_close_action_option",
        skip_serializing_if = "Option::is_none"
    )]
    close_action: Option<CloseAction>,

    #[serde(flatten)]
    rest: Map<String, Value>,
}

pub(crate) fn parse_close_action(raw: &str) -> Option<CloseAction> {
    match raw {
        "tray" => Some(CloseAction::Tray),
        "exit" => Some(CloseAction::Exit),
        _ => None,
    }
}

fn load_desktop_state(raw: &str, log_subject: &str) -> DesktopState {
    match serde_json::from_str::<DesktopState>(raw) {
        Ok(state) => state,
        Err(error) => {
            crate::append_desktop_log(&format!(
                "failed to parse {log_subject}: {error}. resetting state semantics"
            ));
            DesktopState::default()
        }
    }
}

pub(crate) fn read_cached_close_action(packaged_root_dir: Option<&Path>) -> Option<CloseAction> {
    let state_path = crate::desktop_state::resolve_desktop_state_path(packaged_root_dir)?;
    read_cached_close_action_at_path(&state_path)
}

fn read_cached_close_action_at_path(state_path: &Path) -> Option<CloseAction> {
    let raw = fs::read_to_string(state_path).ok()?;
    let state = load_desktop_state(&raw, "desktop close behavior state");
    state.close_action
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

fn save_state<T: Serialize>(path: &Path, state: &T) -> Result<(), String> {
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
    let Some(state_path) = crate::desktop_state::resolve_desktop_state_path(packaged_root_dir)
    else {
        crate::append_desktop_log(
            "close behavior state path is unavailable; skipping close action persistence",
        );
        return Ok(());
    };

    write_cached_close_action_at_path(action, &state_path)
}

fn write_cached_close_action_at_path(
    action: Option<CloseAction>,
    state_path: &Path,
) -> Result<(), String> {
    let mut state = match fs::read_to_string(state_path) {
        Ok(raw) => load_desktop_state(
            &raw,
            &format!("close behavior state {}", state_path.display()),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => DesktopState::default(),
        Err(error) => {
            return Err(format!(
                "Failed to read close behavior state {}: {}",
                state_path.display(),
                error
            ));
        }
    };
    state.close_action = action;

    save_state(state_path, &state)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        load_desktop_state, parse_close_action, read_cached_close_action_at_path,
        write_cached_close_action_at_path, CloseAction, DesktopState,
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
            read_cached_close_action_at_path(&state_path(&temp_dir)),
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
    fn load_desktop_state_deserializes_close_action_and_preserves_other_fields() {
        let state = load_desktop_state(
            r#"{"closeActionOnWindowClose":"tray","locale":"zh-CN"}"#,
            "test desktop state",
        );

        assert_eq!(state.close_action, Some(CloseAction::Tray));
        assert_eq!(state.rest.get("locale"), Some(&json!("zh-CN")));
    }

    #[test]
    fn load_desktop_state_treats_invalid_close_action_as_none_without_dropping_rest() {
        let state = load_desktop_state(
            r#"{"closeActionOnWindowClose":"bogus","locale":"en-US"}"#,
            "test desktop state",
        );

        assert_eq!(state.close_action, None);
        assert_eq!(state.rest.get("locale"), Some(&json!("en-US")));
    }

    #[test]
    fn desktop_state_serialization_omits_close_action_when_none() {
        let mut state = DesktopState::default();
        state.rest.insert("locale".to_string(), json!("en-US"));

        let serialized = serde_json::to_value(&state).expect("serialize desktop state");

        assert_eq!(serialized, json!({ "locale": "en-US" }));
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

        write_cached_close_action_at_path(Some(CloseAction::Tray), &state_path)
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

        write_cached_close_action_at_path(Some(CloseAction::Exit), &state_path)
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

        write_cached_close_action_at_path(Some(CloseAction::Tray), &state_path)
            .expect("write close action");

        assert_eq!(
            read_cached_close_action_at_path(&state_path),
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

        write_cached_close_action_at_path(None, &state_path).expect("clear close action");

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

        assert_eq!(read_cached_close_action_at_path(&state_path), None);
    }
}

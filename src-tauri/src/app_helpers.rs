use std::{
    ffi::OsString,
    sync::{Mutex, OnceLock},
};

use tauri::{AppHandle, Manager};

use crate::{
    backend, bridge, logging, runtime_paths, window, BackendState, LaunchPlan, DESKTOP_LOG_FILE,
    DESKTOP_LOG_MAX_BYTES, LOG_BACKUP_COUNT, TRAY_RESTART_BACKEND_EVENT,
};

static DESKTOP_LOG_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static BACKEND_PATH_OVERRIDE: OnceLock<Option<OsString>> = OnceLock::new();

pub(crate) fn navigate_main_window_to_backend(app_handle: &AppHandle) -> Result<(), String> {
    let state = app_handle.state::<BackendState>();
    window::main_window::navigate_main_window_to_backend(app_handle, &state.backend_url)
}

pub(crate) fn inject_desktop_bridge(webview: &tauri::Webview<tauri::Wry>) {
    bridge::desktop::inject_desktop_bridge(webview, TRAY_RESTART_BACKEND_EVENT, append_desktop_log);
}

pub(crate) fn backend_path_override() -> Option<OsString> {
    BACKEND_PATH_OVERRIDE
        .get_or_init(|| {
            backend::path::build_backend_path_override(|message| append_desktop_log(&message))
        })
        .clone()
}

pub(crate) fn build_debug_command(plan: &LaunchPlan) -> Vec<String> {
    let mut parts = vec![plan.cmd.clone()];
    parts.extend(plan.args.clone());
    parts
}

pub(crate) fn append_desktop_log(message: &str) {
    append_desktop_log_with_category(logging::DesktopLogCategory::Runtime, message);
}

pub(crate) fn append_startup_log(message: &str) {
    append_desktop_log_with_category(logging::DesktopLogCategory::Startup, message);
}

pub(crate) fn append_restart_log(message: &str) {
    append_desktop_log_with_category(logging::DesktopLogCategory::Restart, message);
}

pub(crate) fn append_shutdown_log(message: &str) {
    append_desktop_log_with_category(logging::DesktopLogCategory::Shutdown, message);
}

fn append_desktop_log_with_category(category: logging::DesktopLogCategory, message: &str) {
    logging::append_desktop_log(
        category,
        message,
        runtime_paths::default_packaged_root_dir(),
        DESKTOP_LOG_FILE,
        DESKTOP_LOG_MAX_BYTES,
        LOG_BACKUP_COUNT,
        &DESKTOP_LOG_WRITE_LOCK,
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::build_debug_command;
    use crate::LaunchPlan;

    #[test]
    fn build_debug_command_returns_cmd_followed_by_args() {
        let plan = LaunchPlan {
            cmd: "python".to_string(),
            args: vec!["main.py".to_string(), "--flag".to_string()],
            cwd: PathBuf::from("."),
            root_dir: None,
            webui_dir: None,
            startup_heartbeat_path: None,
            packaged_mode: false,
        };

        assert_eq!(
            build_debug_command(&plan),
            vec![
                "python".to_string(),
                "main.py".to_string(),
                "--flag".to_string()
            ]
        );
    }
}

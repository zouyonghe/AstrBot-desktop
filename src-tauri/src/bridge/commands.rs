use std::process::{Command, Stdio};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;
use url::Url;

use crate::{
    append_desktop_log, restart_backend_flow, runtime_paths, shell_locale, tray,
    BackendBridgeResult, BackendBridgeState, BackendState, DEFAULT_SHELL_LOCALE,
};
use crate::bridge::updater_types::{
    map_no_update_result, map_update_available_result, map_update_check_error,
    map_update_install_error, map_update_install_ok, DesktopAppUpdateCheckResult,
    DesktopAppUpdateResult,
};

const DESKTOP_UPDATER_UNSUPPORTED_REASON: &str =
    "Desktop app updater is not available on this platform yet.";

fn desktop_updater_supported() -> bool {
    cfg!(target_os = "windows") || cfg!(target_os = "macos")
}

fn parse_openable_url(raw_url: &str) -> Result<Url, String> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return Err("Missing external URL.".to_string());
    }

    let parsed = Url::parse(trimmed).map_err(|error| format!("Invalid URL: {error}"))?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        scheme => Err(format!(
            "Unsupported URL scheme '{scheme}', only http/https are allowed."
        )),
    }
}

#[cfg(target_os = "macos")]
fn open_url_with_system_browser(url: &str) -> Result<(), String> {
    Command::new("open")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Failed to run 'open': {error}"))
}

#[cfg(target_os = "windows")]
fn open_url_with_system_browser(url: &str) -> Result<(), String> {
    Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Failed to run 'rundll32': {error}"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_url_with_system_browser(url: &str) -> Result<(), String> {
    Command::new("xdg-open")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Failed to run 'xdg-open': {error}"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows", unix)))]
fn open_url_with_system_browser(_url: &str) -> Result<(), String> {
    Err("Opening external URLs is not supported on this platform.".to_string())
}

#[tauri::command]
pub(crate) fn desktop_bridge_is_desktop_runtime() -> bool {
    true
}

#[tauri::command]
pub(crate) fn desktop_bridge_get_backend_state(app_handle: AppHandle) -> BackendBridgeState {
    let state = app_handle.state::<BackendState>();
    state.bridge_state(&app_handle)
}

#[tauri::command]
pub(crate) fn desktop_bridge_set_auth_token(
    app_handle: AppHandle,
    auth_token: Option<String>,
) -> BackendBridgeResult {
    let state = app_handle.state::<BackendState>();
    state.set_restart_auth_token(auth_token.as_deref());
    BackendBridgeResult {
        ok: true,
        reason: None,
    }
}

#[tauri::command]
pub(crate) async fn desktop_bridge_restart_backend(
    app_handle: AppHandle,
    auth_token: Option<String>,
) -> BackendBridgeResult {
    let state = app_handle.state::<BackendState>();
    if restart_backend_flow::is_backend_action_in_progress(&state) {
        return BackendBridgeResult {
            ok: false,
            reason: Some("Backend action already in progress.".to_string()),
        };
    }

    restart_backend_flow::run_restart_backend_task(app_handle, auth_token).await
}

#[tauri::command]
pub(crate) fn desktop_bridge_stop_backend(app_handle: AppHandle) -> BackendBridgeResult {
    let state = app_handle.state::<BackendState>();
    if restart_backend_flow::is_backend_action_in_progress(&state) {
        return BackendBridgeResult {
            ok: false,
            reason: Some("Backend action already in progress.".to_string()),
        };
    }

    match state.stop_backend_for_bridge() {
        Ok(()) => BackendBridgeResult {
            ok: true,
            reason: None,
        },
        Err(error) => BackendBridgeResult {
            ok: false,
            reason: Some(error),
        },
    }
}

#[tauri::command]
pub(crate) fn desktop_bridge_open_external_url(url: String) -> BackendBridgeResult {
    let parsed = match parse_openable_url(&url) {
        Ok(parsed) => parsed,
        Err(error) => {
            return BackendBridgeResult {
                ok: false,
                reason: Some(error),
            };
        }
    };

    match open_url_with_system_browser(parsed.as_ref()) {
        Ok(()) => BackendBridgeResult {
            ok: true,
            reason: None,
        },
        Err(error) => BackendBridgeResult {
            ok: false,
            reason: Some(error),
        },
    }
}

#[tauri::command]
pub(crate) fn desktop_bridge_set_shell_locale(
    app_handle: AppHandle,
    locale: Option<String>,
) -> BackendBridgeResult {
    let packaged_root_dir = runtime_paths::default_packaged_root_dir();
    match shell_locale::write_cached_shell_locale(locale.as_deref(), packaged_root_dir.as_deref()) {
        Ok(()) => {
            tray::labels::update_tray_menu_labels(
                &app_handle,
                DEFAULT_SHELL_LOCALE,
                append_desktop_log,
            );
            BackendBridgeResult {
                ok: true,
                reason: None,
            }
        }
        Err(error) => {
            append_desktop_log(&format!("failed to persist shell locale: {error}"));
            BackendBridgeResult {
                ok: false,
                reason: Some(error),
            }
        }
    }
}

#[tauri::command]
pub(crate) async fn desktop_bridge_check_app_update(
    app_handle: AppHandle,
) -> DesktopAppUpdateCheckResult {
    let current_version = app_handle.package_info().version.to_string();
    if !desktop_updater_supported() {
        return map_update_check_error(Some(current_version), DESKTOP_UPDATER_UNSUPPORTED_REASON);
    }

    let updater = match app_handle.updater() {
        Ok(updater) => updater,
        Err(error) => {
            return map_update_check_error(
                Some(current_version),
                format!("Failed to initialize updater: {error}"),
            )
        }
    };

    match updater.check().await {
        Ok(Some(update)) => {
            map_update_available_result(current_version, update.version.clone().to_string())
        }
        Ok(None) => map_no_update_result(current_version),
        Err(error) => {
            map_update_check_error(Some(current_version), format!("Failed to check updates: {error}"))
        }
    }
}

#[tauri::command]
pub(crate) async fn desktop_bridge_install_app_update(
    app_handle: AppHandle,
) -> DesktopAppUpdateResult {
    if !desktop_updater_supported() {
        return map_update_install_error(DESKTOP_UPDATER_UNSUPPORTED_REASON);
    }

    let updater = match app_handle.updater() {
        Ok(updater) => updater,
        Err(error) => return map_update_install_error(format!("Failed to initialize updater: {error}")),
    };

    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => return map_update_install_error("No update available."),
        Err(error) => return map_update_install_error(format!("Failed to check updates: {error}")),
    };

    match update.download_and_install(|_, _| {}, || {}).await {
        Ok(()) => {
            app_handle.request_restart();
            map_update_install_ok()
        }
        Err(error) => map_update_install_error(format!("Failed to install update: {error}")),
    }
}

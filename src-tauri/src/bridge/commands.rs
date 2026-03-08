use std::process::{Command, Stdio};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;
use url::Url;

use crate::bridge::updater_messages::{
    desktop_manual_download_reason, DESKTOP_UPDATER_UNSUPPORTED_REASON,
};
use crate::bridge::updater_mode::{resolve_desktop_update_mode, DesktopUpdateMode};
use crate::bridge::updater_types::{
    map_manual_download_no_update_result, map_manual_download_update_available_result,
    map_no_update_result, map_update_available_result, map_update_channel_error,
    map_update_channel_ok, map_update_check_error, map_update_install_error, map_update_install_ok,
    DesktopAppUpdateChannelResult, DesktopAppUpdateCheckResult, DesktopAppUpdateResult,
};
use crate::{
    append_desktop_log, restart_backend_flow, runtime_paths, shell_locale, tray, update_channel,
    BackendBridgeResult, BackendBridgeState, BackendState, DEFAULT_SHELL_LOCALE,
};

fn resolve_update_channel(app_handle: &AppHandle) -> update_channel::UpdateChannel {
    let packaged_root_dir = runtime_paths::default_packaged_root_dir();
    update_channel::resolve_preferred_channel(
        &app_handle.package_info().version,
        packaged_root_dir.as_deref(),
    )
}

fn updater_manifest_log_message(channel: update_channel::UpdateChannel, endpoint: &Url) -> String {
    format!(
        "Using updater manifest for {} channel: {}",
        channel.config_key(),
        endpoint
    )
}

fn build_channel_aware_updater(
    app_handle: &AppHandle,
) -> Result<tauri_plugin_updater::Updater, String> {
    let preferred_channel = resolve_update_channel(app_handle);
    let raw_endpoint = update_channel::resolve_manifest_endpoint(
        &app_handle.config().plugins.0,
        preferred_channel,
    )?;
    let endpoint =
        Url::parse(&raw_endpoint).map_err(|error| format!("Invalid updater endpoint: {error}"))?;
    append_desktop_log(&updater_manifest_log_message(preferred_channel, &endpoint));

    app_handle
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|error| format!("Failed to configure updater endpoint: {error}"))?
        .version_comparator(move |current_version, remote_release| {
            update_channel::should_offer_update(
                &current_version,
                preferred_channel,
                &remote_release.version,
            )
        })
        .build()
        .map_err(|error| format!("Failed to initialize updater: {error}"))
}

fn update_check_short_circuit_result(
    mode: DesktopUpdateMode,
    current_version: &str,
) -> Option<(&'static str, DesktopAppUpdateCheckResult)> {
    match mode {
        DesktopUpdateMode::NativeUpdater => None,
        DesktopUpdateMode::ManualDownload => None,
        DesktopUpdateMode::Unsupported => Some((
            "desktop updater check is unsupported on the current platform/runtime mode",
            map_update_check_error(
                Some(current_version.to_string()),
                DESKTOP_UPDATER_UNSUPPORTED_REASON,
            ),
        )),
    }
}

fn short_circuit_update_install(
    mode: DesktopUpdateMode,
) -> Option<(&'static str, DesktopAppUpdateResult)> {
    match mode {
        DesktopUpdateMode::NativeUpdater => None,
        DesktopUpdateMode::ManualDownload => Some((
            "desktop updater install routed to manual-download mode for current Linux install",
            map_update_install_error(desktop_manual_download_reason()),
        )),
        DesktopUpdateMode::Unsupported => Some((
            "desktop updater install is unsupported on the current platform/runtime mode",
            map_update_install_error(DESKTOP_UPDATER_UNSUPPORTED_REASON),
        )),
    }
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
pub(crate) fn desktop_bridge_get_app_update_channel(
    app_handle: AppHandle,
) -> DesktopAppUpdateChannelResult {
    map_update_channel_ok(resolve_update_channel(&app_handle))
}

#[tauri::command]
pub(crate) fn desktop_bridge_set_app_update_channel(
    app_handle: AppHandle,
    channel: String,
) -> DesktopAppUpdateChannelResult {
    let Some(channel) = update_channel::UpdateChannel::parse(&channel) else {
        return map_update_channel_error("Invalid update channel. Expected 'stable' or 'nightly'.");
    };

    let packaged_root_dir = runtime_paths::default_packaged_root_dir();
    match update_channel::write_cached_update_channel(Some(channel), packaged_root_dir.as_deref()) {
        Ok(()) => {
            append_desktop_log(&format!("update channel set to {:?}", channel));
            let _ = app_handle;
            map_update_channel_ok(channel)
        }
        Err(error) => {
            append_desktop_log(&format!("failed to persist update channel: {error}"));
            map_update_channel_error(error)
        }
    }
}

#[tauri::command]
pub(crate) async fn desktop_bridge_check_app_update(
    app_handle: AppHandle,
) -> DesktopAppUpdateCheckResult {
    let current_version = app_handle.package_info().version.to_string();
    let update_mode = resolve_desktop_update_mode();
    if let Some((log_message, result)) =
        update_check_short_circuit_result(update_mode, &current_version)
    {
        append_desktop_log(log_message);
        return result;
    }

    let updater = match build_channel_aware_updater(&app_handle) {
        Ok(updater) => updater,
        Err(error) => return map_update_check_error(Some(current_version), error),
    };

    match updater.check().await {
        Ok(Some(update)) => match update_mode {
            DesktopUpdateMode::ManualDownload => map_manual_download_update_available_result(
                &current_version,
                &update.version,
                desktop_manual_download_reason(),
            ),
            _ => map_update_available_result(&current_version, &update.version),
        },
        Ok(None) => match update_mode {
            DesktopUpdateMode::ManualDownload => map_manual_download_no_update_result(
                &current_version,
                desktop_manual_download_reason(),
            ),
            _ => map_no_update_result(&current_version),
        },
        Err(error) => map_update_check_error(
            Some(current_version),
            format!("Failed to check updates: {error}"),
        ),
    }
}

#[tauri::command]
pub(crate) async fn desktop_bridge_install_app_update(
    app_handle: AppHandle,
) -> DesktopAppUpdateResult {
    if let Some((log_message, result)) = short_circuit_update_install(resolve_desktop_update_mode())
    {
        append_desktop_log(log_message);
        return result;
    }

    let updater = match build_channel_aware_updater(&app_handle) {
        Ok(updater) => updater,
        Err(error) => return map_update_install_error(error),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_check_short_circuit_only_applies_to_unsupported_mode() {
        assert!(
            update_check_short_circuit_result(DesktopUpdateMode::ManualDownload, "4.19.2")
                .is_none(),
            "manual-download mode should keep running the update check"
        );
    }

    #[test]
    fn updater_check_manual_download_mode_only_reports_reason_without_forced_update_flag() {
        let result = crate::bridge::updater_types::map_manual_download_no_update_result(
            "4.19.2",
            crate::bridge::updater_messages::desktop_manual_download_reason(),
        );

        assert_eq!(
            result,
            crate::bridge::updater_types::DesktopAppUpdateCheckResult {
                ok: true,
                reason: Some(crate::bridge::updater_messages::desktop_manual_download_reason()),
                current_version: Some("4.19.2".to_string()),
                latest_version: Some("4.19.2".to_string()),
                has_update: false,
                manual_download_required: false,
            }
        );
    }

    #[test]
    fn updater_check_mode_returns_unsupported_result() {
        let (log_message, result) =
            update_check_short_circuit_result(DesktopUpdateMode::Unsupported, "4.19.2")
                .expect("unsupported mode should short-circuit update checks");

        assert_eq!(
            log_message,
            "desktop updater check is unsupported on the current platform/runtime mode"
        );
        assert_eq!(
            result,
            crate::bridge::updater_types::DesktopAppUpdateCheckResult {
                ok: false,
                reason: Some(
                    crate::bridge::updater_messages::DESKTOP_UPDATER_UNSUPPORTED_REASON.to_string(),
                ),
                current_version: Some("4.19.2".to_string()),
                latest_version: Some("4.19.2".to_string()),
                has_update: false,
                manual_download_required: false,
            }
        );
    }

    #[test]
    fn updater_install_mode_returns_manual_download_result() {
        let (log_message, result) = short_circuit_update_install(DesktopUpdateMode::ManualDownload)
            .expect("manual-download mode should short-circuit update installs");

        assert_eq!(
            log_message,
            "desktop updater install routed to manual-download mode for current Linux install"
        );
        assert_eq!(
            result,
            crate::bridge::updater_types::DesktopAppUpdateResult {
                ok: false,
                reason: Some(crate::bridge::updater_messages::desktop_manual_download_reason()),
            }
        );
    }

    #[test]
    fn updater_install_mode_returns_unsupported_result() {
        let (log_message, result) = short_circuit_update_install(DesktopUpdateMode::Unsupported)
            .expect("unsupported mode should short-circuit update installs");

        assert_eq!(
            log_message,
            "desktop updater install is unsupported on the current platform/runtime mode"
        );
        assert_eq!(
            result,
            crate::bridge::updater_types::DesktopAppUpdateResult {
                ok: false,
                reason: Some(
                    crate::bridge::updater_messages::DESKTOP_UPDATER_UNSUPPORTED_REASON.to_string(),
                ),
            }
        );
    }

    #[test]
    fn updater_manifest_log_message_includes_channel_and_endpoint() {
        let endpoint = Url::parse("https://example.com/nightly.json").expect("url should parse");
        assert_eq!(
            updater_manifest_log_message(update_channel::UpdateChannel::Nightly, &endpoint),
            "Using updater manifest for nightly channel: https://example.com/nightly.json"
        );
    }
}

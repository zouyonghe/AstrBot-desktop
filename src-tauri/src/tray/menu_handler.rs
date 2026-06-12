use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    append_desktop_log, append_restart_log, desktop_settings, lifecycle, restart_backend_flow,
    runtime_paths,
    tray::{actions, bridge_event},
    ui_dispatch, window, BackendState, DesktopSettingsCache, TrayMenuState, DEFAULT_SHELL_LOCALE,
    TRAY_RESTART_BACKEND_EVENT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayRestartDecision {
    IgnoreBecauseBackendActionInProgress,
    ProceedWithRestart,
}

fn decide_tray_restart(backend_action_in_progress: bool) -> TrayRestartDecision {
    if backend_action_in_progress {
        TrayRestartDecision::IgnoreBecauseBackendActionInProgress
    } else {
        TrayRestartDecision::ProceedWithRestart
    }
}

fn set_checked_safe(item: &tauri::menu::CheckMenuItem<tauri::Wry>, checked: bool, item_name: &str) {
    if let Err(error) = item.set_checked(checked) {
        append_desktop_log(&format!(
            "failed to update tray menu check state for {}: {}",
            item_name, error
        ));
    }
}

fn handle_launch_at_login_toggle(app_handle: &AppHandle) {
    let current_enabled = match app_handle.autolaunch().is_enabled() {
        Ok(value) => value,
        Err(error) => {
            append_desktop_log(&format!(
                "failed to read launch-at-login state, using cached setting: {error}"
            ));
            app_handle
                .state::<DesktopSettingsCache>()
                .get()
                .launch_at_login
        }
    };
    let desired_enabled = !current_enabled;

    let operation_result = if desired_enabled {
        app_handle.autolaunch().enable()
    } else {
        app_handle.autolaunch().disable()
    };

    if let Err(error) = operation_result {
        append_desktop_log(&format!(
            "failed to {} launch at login: {}",
            if desired_enabled { "enable" } else { "disable" },
            error
        ));
        if let Some(tray_state) = app_handle.try_state::<TrayMenuState>() {
            set_checked_safe(
                &tray_state.launch_at_login_item,
                current_enabled,
                actions::TRAY_MENU_LAUNCH_AT_LOGIN,
            );
        }
        return;
    }

    if let Err(error) = desktop_settings::write_desktop_setting(
        runtime_paths::default_packaged_root_dir().as_deref(),
        desktop_settings::DesktopSettingKey::LaunchAtLogin,
        desired_enabled,
    ) {
        append_desktop_log(&format!(
            "failed to persist launch-at-login setting: {error}"
        ));
    } else {
        let mut updated = app_handle.state::<DesktopSettingsCache>().get();
        updated.launch_at_login = desired_enabled;
        app_handle.state::<DesktopSettingsCache>().set(updated);
    }

    if let Some(tray_state) = app_handle.try_state::<TrayMenuState>() {
        set_checked_safe(
            &tray_state.launch_at_login_item,
            desired_enabled,
            actions::TRAY_MENU_LAUNCH_AT_LOGIN,
        );
    }
}

fn persist_bool_setting(
    app_handle: &AppHandle,
    key: desktop_settings::DesktopSettingKey,
    value: bool,
    previous_value: bool,
    item: &tauri::menu::CheckMenuItem<tauri::Wry>,
    item_name: &str,
) {
    match desktop_settings::write_desktop_setting(
        runtime_paths::default_packaged_root_dir().as_deref(),
        key,
        value,
    ) {
        Ok(updated_settings) => {
            app_handle
                .state::<DesktopSettingsCache>()
                .set(updated_settings);
            set_checked_safe(item, value, item_name);
        }
        Err(error) => {
            append_desktop_log(&format!(
                "failed to persist {} setting: {}",
                item_name, error
            ));
            set_checked_safe(item, previous_value, item_name);
        }
    }
}

fn handle_silent_launch_toggle(app_handle: &AppHandle) {
    let Some(tray_state) = app_handle.try_state::<TrayMenuState>() else {
        return;
    };
    let current_settings = app_handle.state::<DesktopSettingsCache>().get();
    persist_bool_setting(
        app_handle,
        desktop_settings::DesktopSettingKey::SilentLaunch,
        !current_settings.silent_launch,
        current_settings.silent_launch,
        &tray_state.silent_launch_item,
        actions::TRAY_MENU_SILENT_LAUNCH,
    );
}

fn handle_close_to_tray_toggle(app_handle: &AppHandle) {
    let Some(tray_state) = app_handle.try_state::<TrayMenuState>() else {
        return;
    };
    let current_settings = app_handle.state::<DesktopSettingsCache>().get();
    persist_bool_setting(
        app_handle,
        desktop_settings::DesktopSettingKey::CloseToTray,
        !current_settings.close_to_tray,
        current_settings.close_to_tray,
        &tray_state.close_to_tray_item,
        actions::TRAY_MENU_CLOSE_TO_TRAY,
    );
}

pub fn handle_tray_menu_event(app_handle: &AppHandle, menu_id: &str) {
    match actions::action_from_menu_id(menu_id) {
        Some(actions::TrayMenuAction::ToggleWindow) => window::actions::toggle_main_window(
            app_handle,
            DEFAULT_SHELL_LOCALE,
            append_desktop_log,
        ),
        Some(actions::TrayMenuAction::ReloadWindow) => {
            window::actions::reload_main_window(app_handle, append_desktop_log)
        }
        Some(actions::TrayMenuAction::RestartBackend) => {
            let state = app_handle.state::<BackendState>();
            match decide_tray_restart(restart_backend_flow::is_backend_action_in_progress(&state)) {
                TrayRestartDecision::IgnoreBecauseBackendActionInProgress => {
                    append_restart_log("tray restart ignored: backend action already in progress");
                    return;
                }
                TrayRestartDecision::ProceedWithRestart => {}
            }
            append_restart_log("tray requested backend restart");
            window::actions::show_main_window(app_handle, DEFAULT_SHELL_LOCALE, append_desktop_log);
            bridge_event::emit_tray_restart_backend_event(
                app_handle,
                TRAY_RESTART_BACKEND_EVENT,
                append_restart_log,
            );

            let app_handle_cloned = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let result =
                    restart_backend_flow::run_restart_backend_task(app_handle_cloned.clone(), None)
                        .await;
                if result.ok {
                    append_restart_log("backend restarted from tray menu");
                    if let Err(error) = ui_dispatch::run_on_main_thread_dispatch(
                        &app_handle_cloned,
                        "reload main window after tray restart",
                        move |main_app| {
                            window::actions::reload_main_window(main_app, append_desktop_log);
                        },
                    ) {
                        append_restart_log(&format!(
                            "failed to schedule main window reload after tray restart: {error}"
                        ));
                    }
                } else {
                    let reason = result.reason.unwrap_or_else(|| "unknown error".to_string());
                    append_restart_log(&format!("backend restart from tray menu failed: {reason}"));
                }
            });
        }
        Some(actions::TrayMenuAction::LaunchAtLogin) => handle_launch_at_login_toggle(app_handle),
        Some(actions::TrayMenuAction::SilentLaunch) => handle_silent_launch_toggle(app_handle),
        Some(actions::TrayMenuAction::CloseToTray) => handle_close_to_tray_toggle(app_handle),
        Some(actions::TrayMenuAction::Quit) => {
            lifecycle::events::handle_tray_quit(app_handle);
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{decide_tray_restart, TrayRestartDecision};

    #[test]
    fn decide_tray_restart_blocks_when_backend_action_in_progress() {
        assert_eq!(
            decide_tray_restart(true),
            TrayRestartDecision::IgnoreBecauseBackendActionInProgress
        );
    }

    #[test]
    fn decide_tray_restart_allows_when_no_backend_action_in_progress() {
        assert_eq!(
            decide_tray_restart(false),
            TrayRestartDecision::ProceedWithRestart
        );
    }
}

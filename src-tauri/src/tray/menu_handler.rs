use tauri::{AppHandle, Manager};

use crate::{
    append_desktop_log, append_restart_log, lifecycle, restart_backend_flow,
    tray::{actions, bridge_event},
    ui_dispatch, window, BackendState, DEFAULT_SHELL_LOCALE, TRAY_RESTART_BACKEND_EVENT,
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
        Some(actions::TrayMenuAction::Quit) => {
            lifecycle::events::request_immediate_exit(
                app_handle,
                lifecycle::events::ImmediateExitTrigger::TrayQuitRequest,
            );
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

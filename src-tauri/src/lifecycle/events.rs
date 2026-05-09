use tauri::{AppHandle, Manager};

use crate::{append_shutdown_log, lifecycle::cleanup, BackendState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitRequestedDecision {
    AllowImmediateExit,
    RunBackendCleanupFirst,
}

fn decide_exit_requested_flow(has_exit_request_allowance: bool) -> ExitRequestedDecision {
    if has_exit_request_allowance {
        ExitRequestedDecision::AllowImmediateExit
    } else {
        ExitRequestedDecision::RunBackendCleanupFirst
    }
}

fn stop_backend_then_exit(app_handle: &AppHandle, trigger: cleanup::ExitTrigger) {
    let app_handle_cloned = app_handle.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle_cloned.state::<BackendState>();
        cleanup::stop_backend_for_exit(&state, trigger, append_shutdown_log);
        state.allow_next_exit_request();
        app_handle_cloned.exit(0);
    });
}

pub fn handle_exit_requested(app_handle: &AppHandle, api: &tauri::ExitRequestApi) {
    let state = app_handle.state::<BackendState>();
    match decide_exit_requested_flow(state.take_exit_request_allowance()) {
        ExitRequestedDecision::AllowImmediateExit => {
            append_shutdown_log("exit request allowed to pass through after backend cleanup");
            return;
        }
        ExitRequestedDecision::RunBackendCleanupFirst => {}
    }
    api.prevent_exit();
    if !cleanup::try_begin_exit_cleanup(
        &state,
        cleanup::ExitTrigger::ExitRequested,
        append_shutdown_log,
    ) {
        return;
    }

    append_shutdown_log("exit requested, stopping backend asynchronously");
    stop_backend_then_exit(app_handle, cleanup::ExitTrigger::ExitRequested);
}

pub fn handle_tray_quit(app_handle: &AppHandle) {
    let state = app_handle.state::<BackendState>();
    state.mark_quitting();
    if !cleanup::try_begin_exit_cleanup(&state, cleanup::ExitTrigger::TrayQuit, append_shutdown_log)
    {
        return;
    }

    append_shutdown_log("tray quit requested, stopping backend asynchronously");
    stop_backend_then_exit(app_handle, cleanup::ExitTrigger::TrayQuit);
}

pub fn handle_exit_event(app_handle: &AppHandle) {
    let state = app_handle.state::<BackendState>();
    if !cleanup::try_begin_exit_cleanup(
        &state,
        cleanup::ExitTrigger::ExitFallback,
        append_shutdown_log,
    ) {
        return;
    }

    append_shutdown_log("exit event triggered fallback backend cleanup");
    cleanup::stop_backend_for_exit(
        &state,
        cleanup::ExitTrigger::ExitFallback,
        append_shutdown_log,
    );
}

#[cfg(test)]
mod tests {
    use super::{decide_exit_requested_flow, ExitRequestedDecision};

    #[test]
    fn decide_exit_requested_flow_allows_immediate_exit_when_allowance_exists() {
        assert_eq!(
            decide_exit_requested_flow(true),
            ExitRequestedDecision::AllowImmediateExit
        );
    }

    #[test]
    fn decide_exit_requested_flow_requires_cleanup_when_allowance_missing() {
        assert_eq!(
            decide_exit_requested_flow(false),
            ExitRequestedDecision::RunBackendCleanupFirst
        );
    }
}

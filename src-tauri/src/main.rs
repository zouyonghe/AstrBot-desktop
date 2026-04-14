#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_constants;
mod app_helpers;
mod app_runtime;
mod app_runtime_events;
mod app_types;

mod backend;
mod bridge;
mod close_behavior;
mod desktop_state;

mod exit_state;
mod lifecycle;

mod launch_plan;
mod logging;
mod packaged_webui;
mod process_control;
mod restart_backend_flow;
mod runtime_paths;
mod shell_locale;
mod startup_mode;
mod startup_task;

mod tray;
mod ui_dispatch;
mod update_channel;
mod webui_paths;
mod window;

pub(crate) use app_constants::*;
pub(crate) use app_helpers::{
    append_desktop_log, append_restart_log, append_shutdown_log, append_startup_log,
    backend_path_override, build_debug_command, inject_desktop_bridge,
    navigate_main_window_to_backend,
};
pub(crate) use app_types::{
    AtomicFlagGuard, BackendBridgeResult, BackendBridgeState, BackendState, LaunchPlan,
    RuntimeManifest, TrayMenuState,
};

fn main() {
    app_runtime::run();
}

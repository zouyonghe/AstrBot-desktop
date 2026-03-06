use tauri::{
    webview::{PageLoadEvent, PageLoadPayload},
    Builder, Manager, RunEvent, WindowEvent,
};

use crate::{
    app_runtime_events, append_desktop_log, append_startup_log, bridge, lifecycle, startup_task,
    tray, window, BackendState, DEFAULT_SHELL_LOCALE, DESKTOP_LOG_FILE, STARTUP_MODE_ENV,
};

fn configure_plugins(builder: Builder<tauri::Wry>) -> Builder<tauri::Wry> {
    builder
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            append_desktop_log("detected second instance launch, focusing existing main window");
            window::actions::show_main_window(app, DEFAULT_SHELL_LOCALE, append_desktop_log);
        }))
}

fn configure_window_events(builder: Builder<tauri::Wry>) -> Builder<tauri::Wry> {
    builder.on_window_event(|window, event| {
        let is_quitting = window.app_handle().state::<BackendState>().is_quitting();
        let action = match &event {
            WindowEvent::CloseRequested { .. } => app_runtime_events::main_window_action(
                window.label(),
                is_quitting,
                false,
                true,
                false,
            ),
            WindowEvent::Focused(false) => app_runtime_events::main_window_action(
                window.label(),
                is_quitting,
                matches!(window.is_minimized(), Ok(true)),
                false,
                true,
            ),
            _ => app_runtime_events::MainWindowAction::None,
        };

        match action {
            app_runtime_events::MainWindowAction::PreventCloseAndHide => {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                }
                window::actions::hide_main_window(
                    window.app_handle(),
                    DEFAULT_SHELL_LOCALE,
                    append_desktop_log,
                );
            }
            app_runtime_events::MainWindowAction::HideIfMinimized => {
                window::actions::hide_main_window(
                    window.app_handle(),
                    DEFAULT_SHELL_LOCALE,
                    append_desktop_log,
                );
            }
            app_runtime_events::MainWindowAction::None => {}
        }
    })
}

fn handle_page_load_started(webview: &tauri::Webview<tauri::Wry>, payload: &PageLoadPayload<'_>) {
    append_desktop_log(&format!("page-load started: {}", payload.url()));
    let state = webview.app_handle().state::<BackendState>();
    let action = app_runtime_events::page_load_action(
        PageLoadEvent::Started,
        bridge::desktop::should_inject_desktop_bridge(&state.backend_url, payload.url()),
        false,
    );

    if action == app_runtime_events::PageLoadAction::InjectDesktopBridge {
        crate::inject_desktop_bridge(webview);
    }
}

fn handle_page_load_finished(webview: &tauri::Webview<tauri::Wry>, payload: &PageLoadPayload<'_>) {
    append_desktop_log(&format!("page-load finished: {}", payload.url()));
    let state = webview.app_handle().state::<BackendState>();
    let action = app_runtime_events::page_load_action(
        PageLoadEvent::Finished,
        bridge::desktop::should_inject_desktop_bridge(&state.backend_url, payload.url()),
        window::startup_loading::should_apply_startup_loading_mode(
            webview.window().label(),
            payload.url(),
        ),
    );

    match action {
        app_runtime_events::PageLoadAction::InjectDesktopBridge => {
            crate::inject_desktop_bridge(webview);
        }
        app_runtime_events::PageLoadAction::ApplyStartupLoadingMode => {
            window::startup_loading::apply_startup_loading_mode(
                webview.app_handle(),
                webview,
                STARTUP_MODE_ENV,
                append_startup_log,
            );
        }
        app_runtime_events::PageLoadAction::None => {}
    }
}

fn configure_page_load_events(builder: Builder<tauri::Wry>) -> Builder<tauri::Wry> {
    builder.on_page_load(|webview, payload| match payload.event() {
        PageLoadEvent::Started => handle_page_load_started(webview, payload),
        PageLoadEvent::Finished => handle_page_load_finished(webview, payload),
    })
}

fn configure_setup(builder: Builder<tauri::Wry>) -> Builder<tauri::Wry> {
    builder.setup(|app| {
        let app_handle = app.handle().clone();
        if let Err(error) = tray::setup::setup_tray(&app_handle) {
            append_startup_log(&format!("failed to initialize tray: {error}"));
        }

        startup_task::spawn_startup_task(app_handle.clone(), append_startup_log);
        Ok(())
    })
}

fn handle_run_event(app_handle: &tauri::AppHandle, event: RunEvent) {
    match app_runtime_events::run_event_action(&event) {
        app_runtime_events::RunEventAction::HandleExitRequested => {
            if let RunEvent::ExitRequested { api, .. } = event {
                lifecycle::events::handle_exit_requested(app_handle, &api);
            }
        }
        app_runtime_events::RunEventAction::HandleExit => {
            lifecycle::events::handle_exit_event(app_handle);
        }
        app_runtime_events::RunEventAction::None => {}
    }
}

pub(crate) fn run() {
    append_startup_log("desktop process starting");
    append_startup_log(&format!(
        "desktop log path: {}",
        crate::logging::resolve_desktop_log_path(
            crate::runtime_paths::default_packaged_root_dir(),
            DESKTOP_LOG_FILE,
        )
        .display()
    ));
    let builder = tauri::Builder::default();
    let builder = configure_plugins(builder);
    let builder = configure_window_events(builder);
    let builder = configure_page_load_events(builder);
    let builder = configure_setup(builder);

    builder
        .manage(BackendState::default())
        .invoke_handler(tauri::generate_handler![
            crate::bridge::commands::desktop_bridge_is_desktop_runtime,
            crate::bridge::commands::desktop_bridge_get_backend_state,
            crate::bridge::commands::desktop_bridge_set_auth_token,
            crate::bridge::commands::desktop_bridge_set_shell_locale,
            crate::bridge::commands::desktop_bridge_restart_backend,
            crate::bridge::commands::desktop_bridge_stop_backend,
            crate::bridge::commands::desktop_bridge_open_external_url,
            crate::bridge::commands::desktop_bridge_check_app_update,
            crate::bridge::commands::desktop_bridge_install_app_update
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(handle_run_event);
}

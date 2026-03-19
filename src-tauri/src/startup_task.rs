use std::path::Path;

use tauri::{AppHandle, Manager};

use crate::{
    logging, navigate_main_window_to_backend, runtime_paths, ui_dispatch, BackendState,
    DESKTOP_LOG_FILE,
};

fn prepare_startup_panel_for_attempt(state: &BackendState, desktop_log_path: &Path) {
    crate::window::startup_panel::remember_desktop_log_start(state, desktop_log_path);
}

pub fn spawn_startup_task<F>(app_handle: AppHandle, log: F)
where
    F: Fn(&str) + Copy + Send + 'static,
{
    let desktop_log_path = logging::resolve_desktop_log_path(
        runtime_paths::default_packaged_root_dir(),
        DESKTOP_LOG_FILE,
    );
    prepare_startup_panel_for_attempt(
        app_handle.state::<BackendState>().inner(),
        &desktop_log_path,
    );

    let startup_app_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let startup_worker_handle = startup_app_handle.clone();
        let startup_result = tauri::async_runtime::spawn_blocking(move || {
            let state = startup_worker_handle.state::<BackendState>();
            state.ensure_backend_ready(&startup_worker_handle)
        })
        .await
        .map_err(|error| format!("Backend startup task failed: {error}"))
        .and_then(|result| result);

        match startup_result {
            Ok(()) => {
                if let Err(error) = ui_dispatch::run_on_main_thread_dispatch(
                    &startup_app_handle,
                    "navigate backend",
                    move |main_app| match navigate_main_window_to_backend(main_app) {
                        Ok(()) => {}
                        Err(navigate_error) => {
                            crate::window::startup_panel::set_failed(
                                main_app.state::<BackendState>().inner(),
                                &navigate_error,
                            );
                            ui_dispatch::show_startup_error(main_app, &navigate_error, log);
                        }
                    },
                ) {
                    crate::window::startup_panel::set_failed(
                        startup_app_handle.state::<BackendState>().inner(),
                        &error,
                    );
                    ui_dispatch::show_startup_error_on_main_thread(
                        &startup_app_handle,
                        &error,
                        log,
                    );
                }
            }
            Err(error) => {
                crate::window::startup_panel::set_failed(
                    startup_app_handle.state::<BackendState>().inner(),
                    &error,
                );
                ui_dispatch::show_startup_error_on_main_thread(&startup_app_handle, &error, log);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::prepare_startup_panel_for_attempt;
    use crate::BackendState;

    fn create_temp_case_dir(name: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "astrbot-desktop-startup-task-test-{}-{}-{}",
            std::process::id(),
            ts,
            name
        ));
        fs::create_dir_all(&dir).expect("create temp case dir");
        dir
    }

    #[test]
    fn prepare_startup_panel_for_attempt_records_current_desktop_log_end() {
        let state = BackendState::default();
        let root = create_temp_case_dir("desktop-log-start-offset");
        let desktop_log_path = root.join(crate::DESKTOP_LOG_FILE);

        fs::write(
            &desktop_log_path,
            "previous launch line\nold readiness note\n",
        )
        .expect("write existing desktop log");
        let expected_offset = fs::metadata(&desktop_log_path)
            .expect("read desktop log metadata")
            .len();

        prepare_startup_panel_for_attempt(&state, &desktop_log_path);

        let panel = match state.startup_panel.lock() {
            Ok(guard) => guard.clone(),
            Err(error) => error.into_inner().clone(),
        };
        assert_eq!(panel.desktop_log_start_offset, expected_offset);

        fs::remove_dir_all(&root).expect("cleanup temp case dir");
    }
}

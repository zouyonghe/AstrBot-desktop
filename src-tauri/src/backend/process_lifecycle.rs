use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use tauri::{AppHandle, Manager};

use crate::{
    append_desktop_log, logging, process_control, BackendState, BACKEND_LOG_MAX_BYTES,
    BACKEND_LOG_ROTATION_CHECK_INTERVAL, GRACEFUL_STOP_TIMEOUT_MS, LOG_BACKUP_COUNT,
};

impl BackendState {
    pub(crate) fn stop_backend(&self) -> Result<(), String> {
        self.stop_backend_with_timeout(Duration::from_millis(GRACEFUL_STOP_TIMEOUT_MS))
    }

    pub(crate) fn stop_backend_with_timeout(&self, timeout: Duration) -> Result<(), String> {
        self.stop_backend_log_rotation_worker();
        let mut guard = self
            .child
            .lock()
            .map_err(|_| "Backend process lock poisoned.".to_string())?;

        let Some(child) = guard.as_mut() else {
            return Ok(());
        };

        if process_control::stop_child_process_gracefully(child, timeout, append_desktop_log) {
            *guard = None;
            return Ok(());
        }

        Err(format!(
            "Backend process did not exit after {}ms graceful stop timeout.",
            timeout.as_millis()
        ))
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn stop_backend_for_system_shutdown(&self, timeout: Duration) -> Result<(), String> {
        self.stop_backend_log_rotation_worker();
        let mut guard = self
            .child
            .lock()
            .map_err(|_| "Backend process lock poisoned.".to_string())?;

        let Some(child) = guard.as_mut() else {
            return Ok(());
        };

        if process_control::stop_child_process_for_system_shutdown(
            child,
            timeout,
            append_desktop_log,
        ) {
            *guard = None;
            return Ok(());
        }

        Err(format!(
            "Backend process did not exit after {}ms Windows shutdown stop timeout.",
            timeout.as_millis()
        ))
    }

    pub(crate) fn stop_backend_log_rotation_worker(&self) {
        match self.log_rotator_stop.lock() {
            Ok(mut guard) => {
                if let Some(flag) = guard.take() {
                    flag.store(true, Ordering::Relaxed);
                }
            }
            Err(error) => {
                append_desktop_log(&format!(
                    "backend log rotator stop flag lock poisoned: {error}"
                ));
            }
        }
    }

    fn child_matches_pid_and_alive(&self, child_pid: u32) -> bool {
        let mut guard = match self.child.lock() {
            Ok(guard) => guard,
            Err(error) => {
                append_desktop_log(&format!(
                    "backend child lock poisoned while checking log rotator worker pid={child_pid}: {error}"
                ));
                return false;
            }
        };

        let Some(child) = guard.as_mut() else {
            return false;
        };
        if child.id() != child_pid {
            return false;
        }

        match child.try_wait() {
            Ok(None) => true,
            Ok(Some(status)) => {
                append_desktop_log(&format!(
                    "backend process exited, stop log rotator worker: pid={child_pid}, status={status}"
                ));
                false
            }
            Err(error) => {
                append_desktop_log(&format!(
                    "failed to poll backend process status for log rotator worker pid={child_pid}: {error}"
                ));
                false
            }
        }
    }

    pub(crate) fn start_backend_log_rotation_worker(
        &self,
        app: &AppHandle,
        log_path: PathBuf,
        child_pid: u32,
    ) {
        self.stop_backend_log_rotation_worker();
        let stop_flag = Arc::new(AtomicBool::new(false));
        match self.log_rotator_stop.lock() {
            Ok(mut guard) => {
                *guard = Some(stop_flag.clone());
            }
            Err(error) => {
                append_desktop_log(&format!(
                    "backend log rotator stop flag lock poisoned on start: {error}"
                ));
                return;
            }
        }

        let app_handle = app.clone();
        thread::spawn(move || {
            let log_scope = format!("backend(pid={child_pid})");
            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                thread::sleep(BACKEND_LOG_ROTATION_CHECK_INTERVAL);
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                let state = app_handle.state::<BackendState>();
                if !state.child_matches_pid_and_alive(child_pid) {
                    break;
                }
                logging::rotate_log_if_needed(
                    &log_path,
                    BACKEND_LOG_MAX_BYTES,
                    LOG_BACKUP_COUNT,
                    &log_scope,
                    true,
                );
            }
        });
    }
}

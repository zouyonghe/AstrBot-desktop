use std::{
    env, thread,
    time::{Duration, Instant},
};

use tauri::AppHandle;

use crate::{
    append_desktop_log, backend, AtomicFlagGuard, BackendState, BACKEND_TIMEOUT_ENV,
    PACKAGED_BACKEND_TIMEOUT_FALLBACK_MS,
};

impl BackendState {
    fn existing_backend_is_ready_for_startup(&self) -> bool {
        let readiness = backend::runtime::backend_readiness_config(append_desktop_log);
        matches!(
            self.request_backend_status_code(
                "GET",
                &readiness.path,
                readiness.probe_timeout_ms,
                None,
                None,
            ),
            Some(status_code) if (200..400).contains(&status_code)
        )
    }

    pub(crate) fn ensure_backend_ready(&self, app: &AppHandle) -> Result<(), String> {
        if self.existing_backend_is_ready_for_startup() {
            crate::window::startup_panel::set_stage(
                self,
                crate::app_types::StartupPanelStage::HttpReady,
            );
            append_desktop_log("backend already HTTP ready, skip spawn");
            return Ok(());
        }

        if env::var("ASTRBOT_BACKEND_AUTO_START").unwrap_or_else(|_| "1".to_string()) == "0" {
            append_desktop_log("backend auto-start disabled by ASTRBOT_BACKEND_AUTO_START=0");
            return Err(
                "Backend auto-start is disabled (ASTRBOT_BACKEND_AUTO_START=0).".to_string(),
            );
        }

        let _spawn_guard = AtomicFlagGuard::try_set(&self.is_spawning)
            .ok_or_else(|| "Backend action already in progress.".to_string())?;
        crate::window::startup_panel::set_stage(
            self,
            crate::app_types::StartupPanelStage::ResolveLaunchPlan,
        );
        let plan = self.resolve_launch_plan(app)?;
        self.start_backend_process(app, &plan)?;
        self.wait_for_backend(&plan)
    }

    pub(crate) fn wait_for_backend(&self, plan: &crate::LaunchPlan) -> Result<(), String> {
        let timeout_ms = backend::config::resolve_backend_timeout_ms(
            plan.packaged_mode,
            BACKEND_TIMEOUT_ENV,
            20_000,
            PACKAGED_BACKEND_TIMEOUT_FALLBACK_MS,
        );
        let readiness = backend::runtime::backend_readiness_config(append_desktop_log);
        let start_time = Instant::now();
        let mut tcp_ready_logged = false;
        let mut ever_tcp_reachable = false;

        loop {
            let (http_status, tcp_reachable) =
                self.probe_backend_readiness(&readiness.path, readiness.probe_timeout_ms);
            if matches!(http_status, Some(status_code) if (200..400).contains(&status_code)) {
                crate::window::startup_panel::set_stage(
                    self,
                    crate::app_types::StartupPanelStage::HttpReady,
                );
                return Ok(());
            }

            if tcp_reachable {
                ever_tcp_reachable = true;
                if !tcp_ready_logged {
                    crate::window::startup_panel::set_stage(
                        self,
                        crate::app_types::StartupPanelStage::TcpReachable,
                    );
                    append_desktop_log(
                        "backend TCP port is reachable but HTTP dashboard is not ready yet; waiting",
                    );
                    tcp_ready_logged = true;
                }
            }

            {
                let mut guard = self
                    .child
                    .lock()
                    .map_err(|_| "Backend process lock poisoned.".to_string())?;
                if let Some(child) = guard.as_mut() {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            *guard = None;
                            return Err(format!(
                                "Backend process exited before becoming reachable: {status}"
                            ));
                        }
                        Ok(None) => {}
                        Err(error) => {
                            return Err(format!("Failed to poll backend process status: {error}"));
                        }
                    }
                } else {
                    return Err("Backend process is not running.".to_string());
                }
            }

            if let Some(limit) = timeout_ms {
                if start_time.elapsed() >= limit {
                    self.log_backend_readiness_timeout(
                        limit,
                        &readiness.path,
                        readiness.probe_timeout_ms,
                        http_status,
                        ever_tcp_reachable,
                    );
                    return Err(format!(
                        "Timed out after {}ms waiting for backend startup.",
                        limit.as_millis()
                    ));
                }
            }

            thread::sleep(Duration::from_millis(readiness.poll_interval_ms));
        }
    }

    fn probe_backend_readiness(
        &self,
        ready_http_path: &str,
        probe_timeout_ms: u64,
    ) -> (Option<u16>, bool) {
        let http_status =
            self.request_backend_status_code("GET", ready_http_path, probe_timeout_ms, None, None);
        let tcp_timeout_ms = probe_timeout_ms.min(crate::BACKEND_READY_TCP_PROBE_TIMEOUT_MAX_MS);
        let tcp_reachable = self.ping_backend(tcp_timeout_ms);
        (http_status, tcp_reachable)
    }

    fn log_backend_readiness_timeout(
        &self,
        timeout: Duration,
        ready_http_path: &str,
        probe_timeout_ms: u64,
        last_http_status: Option<u16>,
        tcp_reachable: bool,
    ) {
        let last_http_status_text = last_http_status
            .map(|status| status.to_string())
            .unwrap_or_else(|| "none".to_string());
        append_desktop_log(&format!(
            "backend HTTP readiness check timed out after {}ms: backend_url={}, path={}, probe_timeout_ms={}, tcp_reachable={}, last_http_status={}",
            timeout.as_millis(),
            self.backend_url,
            ready_http_path,
            probe_timeout_ms,
            tcp_reachable,
            last_http_status_text
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::ErrorKind,
        net::TcpListener,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread,
        time::Duration,
    };

    use super::BackendState;

    struct TcpOnlyServer {
        url: String,
        stop: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl TcpOnlyServer {
        fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind tcp-only listener");
            listener
                .set_nonblocking(true)
                .expect("set nonblocking listener");
            let address = listener.local_addr().expect("listener local addr");
            let stop = Arc::new(AtomicBool::new(false));
            let stop_for_thread = Arc::clone(&stop);
            let handle = thread::spawn(move || {
                while !stop_for_thread.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => drop(stream),
                        Err(error) if error.kind() == ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(_) => break,
                    }
                }
            });

            Self {
                url: format!("http://{address}"),
                stop,
                handle: Some(handle),
            }
        }
    }

    impl Drop for TcpOnlyServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(handle) = self.handle.take() {
                handle.join().expect("join tcp-only listener thread");
            }
        }
    }

    #[test]
    fn existing_backend_startup_skip_requires_http_readiness() {
        let server = TcpOnlyServer::start();
        let state = BackendState {
            backend_url: server.url.clone(),
            ..BackendState::default()
        };

        assert!(!state.existing_backend_is_ready_for_startup());
    }
}

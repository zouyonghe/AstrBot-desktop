use std::{
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};

use tauri::AppHandle;

use crate::{
    append_desktop_log, append_restart_log, backend, AtomicFlagGuard, BackendBridgeState,
    BackendState, LaunchPlan, GRACEFUL_RESTART_POLL_INTERVAL_MS,
    GRACEFUL_RESTART_REQUEST_TIMEOUT_MS,
};

impl BackendState {
    fn sanitize_auth_token(auth_token: Option<&str>) -> Option<String> {
        let token = auth_token?;
        if token.contains('\r') || token.contains('\n') {
            return None;
        }
        let token = token.trim();
        if token.is_empty() {
            return None;
        }
        Some(token.to_string())
    }

    fn get_restart_auth_token(&self) -> Option<String> {
        match self.restart_auth_token.lock() {
            Ok(guard) => guard.clone(),
            Err(error) => {
                append_restart_log(&format!(
                    "restart auth token lock poisoned when reading: {error}"
                ));
                None
            }
        }
    }

    pub(crate) fn set_restart_auth_token(&self, provided_auth_token: Option<&str>) {
        let normalized = Self::sanitize_auth_token(provided_auth_token);
        match self.restart_auth_token.lock() {
            Ok(mut guard) => {
                *guard = normalized;
            }
            Err(error) => append_restart_log(&format!(
                "restart auth token lock poisoned when writing: {error}"
            )),
        }
    }

    fn request_graceful_restart(&self, auth_token: Option<&str>) -> bool {
        let status_code = self.request_backend_status_code(
            "POST",
            "/api/stat/restart-core",
            GRACEFUL_RESTART_REQUEST_TIMEOUT_MS,
            Some("{}"),
            auth_token,
        );
        match status_code {
            Some(code) if (200..300).contains(&code) => true,
            Some(code) => {
                append_restart_log(&format!(
                    "graceful restart request rejected with HTTP status {code}"
                ));
                false
            }
            None => {
                append_restart_log(
                    "graceful restart request returned no HTTP status; will verify restart by polling backend",
                );
                true
            }
        }
    }

    fn wait_for_graceful_restart(
        &self,
        previous_start_time: Option<i64>,
        packaged_mode: bool,
    ) -> Result<(), String> {
        let max_wait = backend::runtime::backend_wait_timeout(packaged_mode);
        let start = Instant::now();
        let mut saw_backend_down = false;

        loop {
            let reachable = self.ping_backend(700);
            if !reachable {
                saw_backend_down = true;
            } else {
                let current_start_time = self.fetch_backend_start_time();
                if let (Some(previous), Some(current)) = (previous_start_time, current_start_time) {
                    if current != previous {
                        return Ok(());
                    }
                } else if previous_start_time.is_none() && saw_backend_down {
                    return Ok(());
                }
            }

            if start.elapsed() >= max_wait {
                return Err(format!(
                    "Timed out after {}ms waiting for graceful restart.",
                    max_wait.as_millis()
                ));
            }

            thread::sleep(Duration::from_millis(GRACEFUL_RESTART_POLL_INTERVAL_MS));
        }
    }

    pub(crate) fn stop_backend_for_bridge(&self) -> Result<(), String> {
        let has_managed_child = self
            .child
            .lock()
            .map_err(|_| "Backend process lock poisoned.".to_string())?
            .is_some();
        if has_managed_child {
            return self.stop_backend();
        }

        if self.ping_backend(backend::runtime::backend_ping_timeout_ms(
            append_desktop_log,
        )) {
            return Err("Backend is running but not managed by desktop process.".to_string());
        }
        Ok(())
    }

    fn has_managed_child(&self) -> Result<bool, String> {
        self.child
            .lock()
            .map(|guard| guard.is_some())
            .map_err(|error| {
                let message = format!(
                    "backend child lock poisoned while resolving restart strategy: {error}"
                );
                append_desktop_log(&message);
                message
            })
    }

    fn restart_strategy(
        &self,
        plan: &LaunchPlan,
        has_managed_child: bool,
    ) -> backend::restart_strategy::RestartStrategy {
        backend::restart_strategy::compute_restart_strategy(
            cfg!(target_os = "windows"),
            plan.packaged_mode,
            has_managed_child,
        )
    }

    fn try_graceful_restart_and_wait(
        &self,
        auth_token: Option<&str>,
        previous_start_time: Option<i64>,
        packaged_mode: bool,
    ) -> backend::restart_strategy::GracefulRestartOutcome {
        let request_accepted = self.request_graceful_restart(auth_token);
        let wait_result = if request_accepted {
            self.wait_for_graceful_restart(previous_start_time, packaged_mode)
        } else {
            Ok(())
        };
        backend::restart_strategy::map_graceful_restart_outcome(request_accepted, wait_result)
    }

    fn execute_graceful_restart_strategy(
        &self,
        strategy: backend::restart_strategy::RestartStrategy,
        auth_token: Option<&str>,
        previous_start_time: Option<i64>,
        packaged_mode: bool,
    ) -> Result<(), String> {
        // Contract: this function interprets the pure (strategy, outcome) pair and either
        // returns early on a completed graceful restart or performs the managed fallback path.
        let outcome = match strategy {
            backend::restart_strategy::RestartStrategy::ManagedSkipGraceful => {
                backend::restart_strategy::GracefulRestartOutcome::RequestRejected
            }
            _ => self.try_graceful_restart_and_wait(auth_token, previous_start_time, packaged_mode),
        };

        match (strategy, outcome) {
            (
                backend::restart_strategy::RestartStrategy::ManagedWithGracefulFallback,
                backend::restart_strategy::GracefulRestartOutcome::Completed,
            )
            | (
                backend::restart_strategy::RestartStrategy::UnmanagedWithGracefulProbe,
                backend::restart_strategy::GracefulRestartOutcome::Completed,
            ) => {
                append_restart_log("graceful restart completed via backend api");
                Ok(())
            }
            (backend::restart_strategy::RestartStrategy::ManagedSkipGraceful, _) => {
                append_restart_log(
                    "skip graceful restart for packaged windows managed backend; using managed restart",
                );
                self.stop_backend_for_restart_flow()
            }
            (
                backend::restart_strategy::RestartStrategy::ManagedWithGracefulFallback,
                backend::restart_strategy::GracefulRestartOutcome::WaitFailed(error),
            ) => {
                append_restart_log(&format!(
                    "graceful restart did not complete, fallback to managed restart: {error}"
                ));
                self.stop_backend_for_restart_flow()
            }
            (
                backend::restart_strategy::RestartStrategy::ManagedWithGracefulFallback,
                backend::restart_strategy::GracefulRestartOutcome::RequestRejected,
            ) => {
                append_restart_log(
                    "graceful restart request was rejected, fallback to managed restart",
                );
                self.stop_backend_for_restart_flow()
            }
            (
                backend::restart_strategy::RestartStrategy::UnmanagedWithGracefulProbe,
                backend::restart_strategy::GracefulRestartOutcome::WaitFailed(error),
            ) => {
                append_restart_log(&format!(
                    "graceful restart did not complete for unmanaged backend, bootstrap managed restart: {error}"
                ));
                self.stop_backend_for_restart_flow()
            }
            (
                backend::restart_strategy::RestartStrategy::UnmanagedWithGracefulProbe,
                backend::restart_strategy::GracefulRestartOutcome::RequestRejected,
            ) => Err(
                "graceful restart request was rejected and backend is not desktop-managed."
                    .to_string(),
            ),
        }
    }

    fn stop_backend_for_restart_flow(&self) -> Result<(), String> {
        self.stop_backend()
    }

    fn launch_backend_after_restart(
        &self,
        app: &AppHandle,
        plan: &LaunchPlan,
    ) -> Result<(), String> {
        let _spawn_guard = AtomicFlagGuard::set(&self.is_spawning);
        self.start_backend_process(app, plan)?;
        self.wait_for_backend(plan)
    }

    pub(crate) fn restart_backend(
        &self,
        app: &AppHandle,
        auth_token: Option<&str>,
    ) -> Result<(), String> {
        append_restart_log("backend restart requested");

        let _restart_guard = AtomicFlagGuard::try_set(&self.is_restarting)
            .ok_or_else(|| "Backend action already in progress.".to_string())?;
        let plan = self.resolve_launch_plan(app)?;
        let has_managed_child = self.has_managed_child()?;
        let strategy = self.restart_strategy(&plan, has_managed_child);
        let normalized_param = Self::sanitize_auth_token(auth_token);
        if let Some(token) = normalized_param.as_deref() {
            self.set_restart_auth_token(Some(token));
        }
        let restart_auth_token = normalized_param.or_else(|| self.get_restart_auth_token());
        let previous_start_time = self.fetch_backend_start_time();
        match self.execute_graceful_restart_strategy(
            strategy,
            restart_auth_token.as_deref(),
            previous_start_time,
            plan.packaged_mode,
        ) {
            Ok(())
                if strategy != backend::restart_strategy::RestartStrategy::ManagedSkipGraceful =>
            {
                return Ok(());
            }
            Ok(()) => {}
            Err(error) => return Err(error),
        }

        self.launch_backend_after_restart(app, &plan)
    }

    pub(crate) fn bridge_state(&self, app: &AppHandle) -> BackendBridgeState {
        let has_managed_child = self
            .child
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or_else(|error| {
                append_desktop_log(&format!(
                    "backend bridge: child process mutex poisoned in bridge_state: {error}"
                ));
                false
            });
        let can_manage = has_managed_child || self.resolve_launch_plan(app).is_ok();
        BackendBridgeState {
            running: self.ping_backend(backend::runtime::bridge_backend_ping_timeout_ms(
                append_desktop_log,
            )),
            spawning: self.is_spawning.load(Ordering::Relaxed),
            restarting: self.is_restarting.load(Ordering::Relaxed),
            can_manage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BackendState;
    use crate::backend::restart_strategy::RestartStrategy;

    #[test]
    fn sanitize_auth_token_rejects_empty_and_newline_tokens() {
        assert_eq!(BackendState::sanitize_auth_token(None), None);
        assert_eq!(BackendState::sanitize_auth_token(Some("   ")), None);
        assert_eq!(BackendState::sanitize_auth_token(Some("abc\r\ndef")), None);
        assert_eq!(BackendState::sanitize_auth_token(Some("abc\ndef")), None);
    }

    #[test]
    fn sanitize_auth_token_trims_valid_token() {
        assert_eq!(
            BackendState::sanitize_auth_token(Some("  token-123  ")),
            Some("token-123".to_string())
        );
    }

    #[test]
    fn restart_strategy_delegates_to_strategy_module() {
        let plan = crate::LaunchPlan {
            cmd: "python".to_string(),
            args: vec![],
            cwd: std::path::PathBuf::from("."),
            root_dir: None,
            webui_dir: None,
            startup_heartbeat_path: None,
            packaged_mode: true,
        };
        let state = BackendState::default();

        assert_eq!(
            state.restart_strategy(&plan, true),
            RestartStrategy::ManagedWithGracefulFallback
        );
    }
}

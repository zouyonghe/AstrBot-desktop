use std::{
    env, fs,
    path::Path,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use tauri::AppHandle;

use crate::{
    append_desktop_log, backend, AtomicFlagGuard, BackendState, BACKEND_TIMEOUT_ENV,
    PACKAGED_BACKEND_TIMEOUT_FALLBACK_MS,
};

impl BackendState {
    pub(crate) fn ensure_backend_ready(&self, app: &AppHandle) -> Result<(), String> {
        if self.ping_backend(backend::runtime::backend_ping_timeout_ms(
            append_desktop_log,
        )) {
            append_desktop_log("backend already reachable, skip spawn");
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
        let readiness = backend::runtime::backend_readiness_config(plan, append_desktop_log);
        let startup_idle_timeout = Duration::from_millis(readiness.startup_idle_timeout_ms);
        let start_time = Instant::now();
        let mut tcp_ready_logged = false;
        let mut ever_tcp_reachable = false;
        let mut startup_heartbeat_logged = false;
        let mut last_startup_heartbeat_at = None;

        loop {
            let (http_status, tcp_reachable) =
                self.probe_backend_readiness(&readiness.path, readiness.probe_timeout_ms);
            if matches!(http_status, Some(status_code) if (200..400).contains(&status_code)) {
                return Ok(());
            }
            let now = SystemTime::now();

            let child_pid = {
                let mut guard = self
                    .child
                    .lock()
                    .map_err(|_| "Backend process lock poisoned.".to_string())?;
                if let Some(child) = guard.as_mut() {
                    let child_pid = child.id();
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            *guard = None;
                            return Err(format!(
                                "Backend process exited before becoming reachable: {status}"
                            ));
                        }
                        Ok(None) => child_pid,
                        Err(error) => {
                            return Err(format!("Failed to poll backend process status: {error}"));
                        }
                    }
                } else {
                    return Err("Backend process is not running.".to_string());
                }
            };

            if let Some(heartbeat_path) = readiness.startup_heartbeat_path.as_deref() {
                match next_startup_heartbeat_at(
                    last_startup_heartbeat_at,
                    read_startup_heartbeat_updated_at(heartbeat_path, child_pid),
                ) {
                    StartupHeartbeatObservation::Missing => {
                        last_startup_heartbeat_at = None;
                    }
                    StartupHeartbeatObservation::Observed(updated_at) => {
                        last_startup_heartbeat_at = Some(updated_at);
                    }
                    StartupHeartbeatObservation::Invalidated(previous) => {
                        let heartbeat_age_ms = now
                            .duration_since(previous)
                            .ok()
                            .map(|age| age.as_millis().to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        append_desktop_log(&format!(
                            "backend startup heartbeat disappeared or became invalid before HTTP dashboard became ready: last_valid_age_ms={heartbeat_age_ms}"
                        ));
                        return Err(
                            "Backend startup heartbeat disappeared or became invalid before HTTP readiness."
                                .to_string(),
                        );
                    }
                }

                if startup_heartbeat_timestamp_is_fresh(
                    last_startup_heartbeat_at,
                    now,
                    startup_idle_timeout,
                ) {
                    if !startup_heartbeat_logged {
                        append_desktop_log(
                            "backend startup heartbeat is fresh while HTTP dashboard is not ready yet; waiting",
                        );
                        startup_heartbeat_logged = true;
                    }
                } else if last_startup_heartbeat_at.is_some() {
                    append_desktop_log(
                        "backend startup heartbeat went stale before HTTP dashboard became ready",
                    );
                    return Err(format!(
                        "Backend startup heartbeat went stale after {}ms without HTTP readiness.",
                        readiness.startup_idle_timeout_ms
                    ));
                }
            }

            if tcp_reachable {
                ever_tcp_reachable = true;
                if !tcp_ready_logged {
                    append_desktop_log(
                        "backend TCP port is reachable but HTTP dashboard is not ready yet; waiting",
                    );
                    tcp_ready_logged = true;
                }
            }

            if let Some(limit) = timeout_ms {
                if start_time.elapsed() >= limit {
                    self.log_backend_readiness_timeout(
                        limit,
                        &readiness.path,
                        readiness.probe_timeout_ms,
                        ReadinessTimeoutSnapshot {
                            now,
                            last_http_status: http_status,
                            tcp_reachable: ever_tcp_reachable,
                            last_startup_heartbeat_at,
                        },
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
        snapshot: ReadinessTimeoutSnapshot,
    ) {
        let last_http_status_text = snapshot
            .last_http_status
            .map(|status| status.to_string())
            .unwrap_or_else(|| "none".to_string());
        let startup_heartbeat_age_ms = snapshot
            .last_startup_heartbeat_at
            .and_then(|updated_at| snapshot.now.duration_since(updated_at).ok())
            .map(|age| age.as_millis().to_string())
            .unwrap_or_else(|| "none".to_string());
        append_desktop_log(&format!(
            "backend HTTP readiness check timed out after {}ms: backend_url={}, path={}, probe_timeout_ms={}, tcp_reachable={}, last_http_status={}, startup_heartbeat_age_ms={}",
            timeout.as_millis(),
            self.backend_url,
            ready_http_path,
            probe_timeout_ms,
            snapshot.tcp_reachable,
            last_http_status_text,
            startup_heartbeat_age_ms
        ));
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StartupHeartbeatFile {
    pid: u32,
    state: StartupHeartbeatState,
    updated_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum StartupHeartbeatState {
    Starting,
    Stopping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupHeartbeatObservation {
    Missing,
    Observed(SystemTime),
    Invalidated(SystemTime),
}

#[derive(Debug, Clone, Copy)]
struct ReadinessTimeoutSnapshot {
    now: SystemTime,
    last_http_status: Option<u16>,
    tcp_reachable: bool,
    last_startup_heartbeat_at: Option<SystemTime>,
}

fn read_startup_heartbeat_updated_at(path: &Path, expected_pid: u32) -> Option<SystemTime> {
    let payload = fs::read_to_string(path).ok()?;
    let heartbeat: StartupHeartbeatFile = serde_json::from_str(&payload).ok()?;
    if heartbeat.pid != expected_pid || heartbeat.state != StartupHeartbeatState::Starting {
        return None;
    }
    heartbeat_updated_at_ms_to_system_time(heartbeat.updated_at_ms)
}

fn heartbeat_updated_at_ms_to_system_time(updated_at_ms: u64) -> Option<SystemTime> {
    UNIX_EPOCH.checked_add(Duration::from_millis(updated_at_ms))
}

fn startup_heartbeat_timestamp_is_fresh(
    updated_at: Option<SystemTime>,
    now: SystemTime,
    max_age: Duration,
) -> bool {
    updated_at
        .and_then(|updated_at| now.duration_since(updated_at).ok())
        .is_some_and(|age| age <= max_age)
}

fn next_startup_heartbeat_at(
    previous: Option<SystemTime>,
    current: Option<SystemTime>,
) -> StartupHeartbeatObservation {
    match (previous, current) {
        (Some(previous), None) => StartupHeartbeatObservation::Invalidated(previous),
        (None, None) => StartupHeartbeatObservation::Missing,
        (Some(previous), Some(current)) if current <= previous => {
            StartupHeartbeatObservation::Observed(previous)
        }
        (_, Some(current)) => StartupHeartbeatObservation::Observed(current),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn startup_heartbeat_is_fresh_for_recent_timestamp() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let heartbeat_path = temp_dir.path().join("startup-heartbeat.json");
        std::fs::write(
            &heartbeat_path,
            r#"{"pid":42,"state":"starting","updated_at_ms":5000}"#,
        )
        .expect("write heartbeat file");

        let updated_at =
            read_startup_heartbeat_updated_at(&heartbeat_path, 42).expect("heartbeat timestamp");

        assert!(startup_heartbeat_timestamp_is_fresh(
            Some(updated_at),
            UNIX_EPOCH + Duration::from_millis(5500),
            Duration::from_secs(1),
        ));
    }

    #[test]
    fn startup_heartbeat_is_not_fresh_for_stale_timestamp() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let heartbeat_path = temp_dir.path().join("startup-heartbeat.json");
        std::fs::write(
            &heartbeat_path,
            r#"{"pid":42,"state":"starting","updated_at_ms":1000}"#,
        )
        .expect("write heartbeat file");

        let updated_at =
            read_startup_heartbeat_updated_at(&heartbeat_path, 42).expect("heartbeat timestamp");

        assert!(!startup_heartbeat_timestamp_is_fresh(
            Some(updated_at),
            SystemTime::UNIX_EPOCH + Duration::from_millis(5000),
            Duration::from_secs(1),
        ));
    }

    #[test]
    fn startup_heartbeat_is_not_fresh_for_mismatched_pid() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let heartbeat_path = temp_dir.path().join("startup-heartbeat.json");
        std::fs::write(
            &heartbeat_path,
            r#"{"pid":7,"state":"starting","updated_at_ms":5000}"#,
        )
        .expect("write heartbeat file");

        assert_eq!(read_startup_heartbeat_updated_at(&heartbeat_path, 42), None);
    }

    #[test]
    fn startup_heartbeat_is_not_fresh_for_future_timestamp() {
        assert!(!startup_heartbeat_timestamp_is_fresh(
            Some(UNIX_EPOCH + Duration::from_millis(6000)),
            UNIX_EPOCH + Duration::from_millis(5500),
            Duration::from_secs(1),
        ));
    }

    #[test]
    fn next_startup_heartbeat_at_marks_previous_timestamp_invalid_when_current_is_missing() {
        assert_eq!(
            next_startup_heartbeat_at(Some(UNIX_EPOCH + Duration::from_millis(5000)), None),
            StartupHeartbeatObservation::Invalidated(UNIX_EPOCH + Duration::from_millis(5000))
        );
    }

    #[test]
    fn startup_heartbeat_file_rejects_unknown_state() {
        assert!(serde_json::from_str::<StartupHeartbeatFile>(
            r#"{"pid":42,"state":"unexpected","updated_at_ms":5000}"#
        )
        .is_err());
    }

    #[test]
    fn startup_heartbeat_file_rejects_unknown_fields() {
        assert!(serde_json::from_str::<StartupHeartbeatFile>(
            r#"{"pid":42,"state":"starting","updated_at_ms":5000,"unexpected":true}"#
        )
        .is_err());
    }

    #[test]
    fn heartbeat_updated_at_ms_to_system_time_matches_checked_add() {
        assert_eq!(
            heartbeat_updated_at_ms_to_system_time(u64::MAX),
            UNIX_EPOCH.checked_add(Duration::from_millis(u64::MAX))
        );
    }
}

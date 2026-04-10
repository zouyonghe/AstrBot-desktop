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
        let mut startup_heartbeat_state = StartupHeartbeatTracker::new();

        loop {
            let (http_status, tcp_reachable) =
                self.probe_backend_readiness(&readiness.path, readiness.probe_timeout_ms);
            if matches!(http_status, Some(status_code) if (200..400).contains(&status_code)) {
                return Ok(());
            }
            let wall_now = SystemTime::now();
            let monotonic_now = Instant::now();

            let child_pid = self.live_child_pid()?;

            if let Some(heartbeat_path) = readiness.startup_heartbeat_path.as_deref() {
                step_startup_heartbeat(
                    heartbeat_path,
                    child_pid,
                    wall_now,
                    monotonic_now,
                    startup_idle_timeout,
                    &mut startup_heartbeat_state,
                )?;
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
                        &readiness,
                        wall_now,
                        http_status,
                        ever_tcp_reachable,
                        startup_heartbeat_state.last_seen_at,
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

    fn live_child_pid(&self) -> Result<u32, String> {
        let mut guard = self
            .child
            .lock()
            .map_err(|_| "Backend process lock poisoned.".to_string())?;

        if let Some(child) = guard.as_mut() {
            let pid = child.id();
            match child.try_wait() {
                Ok(Some(status)) => {
                    *guard = None;
                    Err(format!(
                        "Backend process exited before becoming reachable: {status}"
                    ))
                }
                Ok(None) => Ok(pid),
                Err(error) => Err(format!("Failed to poll backend process status: {error}")),
            }
        } else {
            Err("Backend process is not running.".to_string())
        }
    }

    fn log_backend_readiness_timeout(
        &self,
        timeout: Duration,
        readiness: &backend::config::BackendReadinessConfig,
        now: SystemTime,
        last_http_status: Option<u16>,
        tcp_reachable: bool,
        last_startup_heartbeat_at: Option<SystemTime>,
    ) {
        let last_http_status_text = last_http_status
            .map(|status| status.to_string())
            .unwrap_or_else(|| "none".to_string());
        let startup_heartbeat_age_ms = describe_heartbeat_age(last_startup_heartbeat_at, now);
        append_desktop_log(&format!(
            "backend HTTP readiness check timed out after {}ms: backend_url={}, path={}, probe_timeout_ms={}, tcp_reachable={}, last_http_status={}, startup_heartbeat_age_ms={}",
            timeout.as_millis(),
            self.backend_url,
            readiness.path,
            readiness.probe_timeout_ms,
            tcp_reachable,
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

#[derive(Debug, Clone, Copy)]
struct StartupHeartbeatTracker {
    last_seen_at: Option<SystemTime>,
    last_progress_at: Option<Instant>,
    consecutive_invalid_reads: u8,
    logged_fresh: bool,
}

impl StartupHeartbeatTracker {
    fn new() -> Self {
        Self {
            last_seen_at: None,
            last_progress_at: None,
            consecutive_invalid_reads: 0,
            logged_fresh: false,
        }
    }
}

const STARTUP_HEARTBEAT_INVALID_READ_THRESHOLD: u8 = 2;

fn read_startup_heartbeat_updated_at(path: &Path, expected_pid: u32) -> Option<SystemTime> {
    let payload = fs::read_to_string(path).ok()?;
    let heartbeat: StartupHeartbeatFile = serde_json::from_str(&payload).ok()?;
    if heartbeat.pid != expected_pid || heartbeat.state != StartupHeartbeatState::Starting {
        return None;
    }
    UNIX_EPOCH.checked_add(Duration::from_millis(heartbeat.updated_at_ms))
}

fn startup_heartbeat_progress_is_fresh(
    last_progress_at: Option<Instant>,
    now: Instant,
    max_age: Duration,
) -> bool {
    last_progress_at.is_some_and(|updated_at| now.duration_since(updated_at) <= max_age)
}

fn ms_since(earlier: SystemTime, now: SystemTime) -> Option<u128> {
    now.duration_since(earlier)
        .ok()
        .map(|duration| duration.as_millis())
}

fn describe_heartbeat_age(
    last_startup_heartbeat_at: Option<SystemTime>,
    now: SystemTime,
) -> String {
    match last_startup_heartbeat_at {
        Some(updated_at) => match ms_since(updated_at, now) {
            Some(age) => age.to_string(),
            None => format!("future ({updated_at:?})"),
        },
        None => "none".to_string(),
    }
}

fn step_startup_heartbeat(
    heartbeat_path: &Path,
    child_pid: u32,
    wall_now: SystemTime,
    monotonic_now: Instant,
    idle_timeout: Duration,
    state: &mut StartupHeartbeatTracker,
) -> Result<(), String> {
    let previous = state.last_seen_at;
    let current = read_startup_heartbeat_updated_at(heartbeat_path, child_pid);

    match (previous, current) {
        (Some(previous), None) => {
            state.consecutive_invalid_reads = state.consecutive_invalid_reads.saturating_add(1);
            if state.consecutive_invalid_reads < STARTUP_HEARTBEAT_INVALID_READ_THRESHOLD {
                return Ok(());
            }

            let heartbeat_age_ms = describe_heartbeat_age(Some(previous), wall_now);
            append_desktop_log(&format!(
                "backend startup heartbeat disappeared or became invalid before HTTP dashboard became ready: last_valid_age_ms={heartbeat_age_ms}"
            ));
            Err(
                "Backend startup heartbeat disappeared or became invalid before HTTP readiness."
                    .to_string(),
            )
        }
        (None, None) => {
            state.consecutive_invalid_reads = 0;
            Ok(())
        }
        (_, Some(current)) => {
            state.consecutive_invalid_reads = 0;
            let updated_at = match previous {
                Some(previous) if current <= previous => previous,
                _ => current,
            };
            state.last_seen_at = Some(updated_at);

            if previous.is_none()
                || Some(updated_at) != previous
                || state.last_progress_at.is_none()
            {
                state.last_progress_at = Some(monotonic_now);
            }

            if startup_heartbeat_progress_is_fresh(
                state.last_progress_at,
                monotonic_now,
                idle_timeout,
            ) {
                if !state.logged_fresh {
                    append_desktop_log(
                        "backend startup heartbeat is fresh while HTTP dashboard is not ready yet; waiting",
                    );
                    state.logged_fresh = true;
                }
                Ok(())
            } else {
                append_desktop_log(
                    "backend startup heartbeat went stale before HTTP dashboard became ready",
                );
                Err(format!(
                    "Backend startup heartbeat went stale after {}ms without HTTP readiness.",
                    idle_timeout.as_millis()
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant, UNIX_EPOCH};

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn startup_heartbeat_progress_is_fresh_for_recent_instant() {
        assert!(startup_heartbeat_progress_is_fresh(
            Some(Instant::now()),
            Instant::now() + Duration::from_millis(500),
            Duration::from_secs(1),
        ));
    }

    #[test]
    fn startup_heartbeat_progress_is_not_fresh_when_stale() {
        assert!(!startup_heartbeat_progress_is_fresh(
            Some(Instant::now()),
            Instant::now() + Duration::from_millis(1500),
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
    fn step_startup_heartbeat_fails_when_existing_heartbeat_disappears() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let heartbeat_path = temp_dir.path().join("missing-startup-heartbeat.json");
        let monotonic_now = Instant::now();
        let mut tracker = StartupHeartbeatTracker {
            last_seen_at: Some(UNIX_EPOCH + Duration::from_millis(5000)),
            last_progress_at: Some(monotonic_now),
            consecutive_invalid_reads: 0,
            logged_fresh: false,
        };

        let first_result = step_startup_heartbeat(
            &heartbeat_path,
            42,
            UNIX_EPOCH + Duration::from_millis(5500),
            monotonic_now,
            Duration::from_secs(1),
            &mut tracker,
        );

        let result = step_startup_heartbeat(
            &heartbeat_path,
            42,
            UNIX_EPOCH + Duration::from_millis(5600),
            monotonic_now + Duration::from_millis(100),
            Duration::from_secs(1),
            &mut tracker,
        );

        assert_eq!(first_result, Ok(()));
        assert_eq!(
            result,
            Err(
                "Backend startup heartbeat disappeared or became invalid before HTTP readiness."
                    .to_string()
            )
        );
    }

    #[test]
    fn step_startup_heartbeat_tolerates_single_missing_read_after_valid_heartbeat() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let heartbeat_path = temp_dir.path().join("missing-startup-heartbeat.json");
        let monotonic_now = Instant::now();
        let mut tracker = StartupHeartbeatTracker {
            last_seen_at: Some(UNIX_EPOCH + Duration::from_millis(5000)),
            last_progress_at: Some(monotonic_now),
            consecutive_invalid_reads: 0,
            logged_fresh: false,
        };

        let result = step_startup_heartbeat(
            &heartbeat_path,
            42,
            UNIX_EPOCH + Duration::from_millis(5500),
            monotonic_now,
            Duration::from_secs(1),
            &mut tracker,
        );

        assert_eq!(result, Ok(()));
        assert_eq!(tracker.consecutive_invalid_reads, 1);
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
    fn read_startup_heartbeat_updated_at_handles_large_timestamp_without_panic() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let heartbeat_path = temp_dir.path().join("startup-heartbeat.json");
        std::fs::write(
            &heartbeat_path,
            format!(
                r#"{{"pid":42,"state":"starting","updated_at_ms":{}}}"#,
                u64::MAX
            ),
        )
        .expect("write heartbeat file");

        assert_eq!(
            read_startup_heartbeat_updated_at(&heartbeat_path, 42),
            UNIX_EPOCH.checked_add(Duration::from_millis(u64::MAX))
        );
    }

    #[test]
    fn describe_heartbeat_age_distinguishes_future_timestamp_from_missing() {
        assert_eq!(
            describe_heartbeat_age(
                Some(UNIX_EPOCH + Duration::from_millis(6_000)),
                UNIX_EPOCH + Duration::from_millis(5_500)
            ),
            format!("future ({:?})", UNIX_EPOCH + Duration::from_millis(6_000))
        );
        assert_eq!(
            describe_heartbeat_age(None, UNIX_EPOCH + Duration::from_millis(5_500)),
            "none"
        );
    }
}

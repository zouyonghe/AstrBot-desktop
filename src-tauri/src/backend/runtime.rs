use std::{env, sync::OnceLock, time::Duration};

use crate::backend;

static BACKEND_PING_TIMEOUT_MS: OnceLock<u64> = OnceLock::new();
static BRIDGE_BACKEND_PING_TIMEOUT_MS: OnceLock<u64> = OnceLock::new();

pub fn backend_wait_timeout(packaged_mode: bool) -> Duration {
    backend::config::resolve_backend_timeout_ms(
        packaged_mode,
        crate::BACKEND_TIMEOUT_ENV,
        20_000,
        crate::PACKAGED_BACKEND_TIMEOUT_FALLBACK_MS,
    )
    .unwrap_or(Duration::from_millis(20_000))
}

pub fn backend_readiness_config<F>(
    plan: &crate::LaunchPlan,
    log: F,
) -> backend::config::BackendReadinessConfig
where
    F: Fn(&str) + Copy,
{
    let probe_timeout_fallback = backend_ping_timeout_ms(log);
    let mut readiness = backend::config::backend_readiness_config(
        crate::BACKEND_READY_HTTP_PATH_ENV,
        crate::DEFAULT_BACKEND_READY_HTTP_PATH,
        crate::BACKEND_READY_PROBE_TIMEOUT_ENV,
        probe_timeout_fallback,
        crate::BACKEND_READY_PROBE_TIMEOUT_MIN_MS,
        crate::BACKEND_READY_PROBE_TIMEOUT_MAX_MS,
        crate::BACKEND_READY_POLL_INTERVAL_ENV,
        crate::DEFAULT_BACKEND_READY_POLL_INTERVAL_MS,
        crate::BACKEND_READY_POLL_INTERVAL_MIN_MS,
        crate::BACKEND_READY_POLL_INTERVAL_MAX_MS,
        |message| log(&message),
    );
    readiness.startup_idle_timeout_ms = match env::var(crate::BACKEND_STARTUP_IDLE_TIMEOUT_ENV) {
        Ok(raw) => backend::config::resolve_backend_startup_idle_timeout_ms(
            &raw,
            crate::BACKEND_STARTUP_IDLE_TIMEOUT_ENV,
            crate::DEFAULT_BACKEND_STARTUP_IDLE_TIMEOUT_MS,
            crate::BACKEND_STARTUP_IDLE_TIMEOUT_MIN_MS,
            crate::BACKEND_STARTUP_IDLE_TIMEOUT_MAX_MS,
            |message| log(&message),
        ),
        Err(_) => crate::DEFAULT_BACKEND_STARTUP_IDLE_TIMEOUT_MS,
    };
    readiness.startup_heartbeat_path = backend::config::resolve_backend_startup_heartbeat_path(
        plan.root_dir.as_deref(),
        plan.packaged_mode
            .then(crate::runtime_paths::default_packaged_root_dir)
            .flatten(),
        crate::DEFAULT_BACKEND_STARTUP_HEARTBEAT_RELATIVE_PATH,
    );
    readiness
}

pub fn backend_ping_timeout_ms<F>(log: F) -> u64
where
    F: Fn(&str) + Copy,
{
    *BACKEND_PING_TIMEOUT_MS.get_or_init(|| match env::var(crate::BACKEND_PING_TIMEOUT_ENV) {
        Ok(raw) => backend::config::parse_ping_timeout_env(
            &raw,
            crate::BACKEND_PING_TIMEOUT_ENV,
            crate::DEFAULT_BACKEND_PING_TIMEOUT_MS,
            crate::BACKEND_PING_TIMEOUT_MIN_MS,
            crate::BACKEND_PING_TIMEOUT_MAX_MS,
            |message| log(&message),
        ),
        Err(_) => crate::DEFAULT_BACKEND_PING_TIMEOUT_MS,
    })
}

pub fn bridge_backend_ping_timeout_ms<F>(log: F) -> u64
where
    F: Fn(&str) + Copy,
{
    *BRIDGE_BACKEND_PING_TIMEOUT_MS.get_or_init(|| {
        let fallback = backend_ping_timeout_ms(log);
        match env::var(crate::BRIDGE_BACKEND_PING_TIMEOUT_ENV) {
            Ok(raw) => backend::config::parse_ping_timeout_env(
                &raw,
                crate::BRIDGE_BACKEND_PING_TIMEOUT_ENV,
                fallback,
                crate::BACKEND_PING_TIMEOUT_MIN_MS,
                crate::BACKEND_PING_TIMEOUT_MAX_MS,
                |message| log(&message),
            ),
            Err(_) => fallback,
        }
    })
}

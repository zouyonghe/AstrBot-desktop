use crate::BackendState;

#[derive(Debug, Clone, Copy)]
pub enum ExitTrigger {
    ExitRequested,
    ExitFallback,
    TrayQuit,
}

fn duplicate_cleanup_message(trigger: ExitTrigger) -> &'static str {
    match trigger {
        ExitTrigger::ExitRequested => "exit requested while backend cleanup is already running",
        ExitTrigger::ExitFallback => {
            "exit fallback cleanup skipped: backend cleanup already running"
        }
        ExitTrigger::TrayQuit => "tray quit while backend cleanup is already running",
    }
}

fn stop_failure_prefix(trigger: ExitTrigger) -> &'static str {
    match trigger {
        ExitTrigger::ExitRequested => "backend graceful stop on ExitRequested failed",
        ExitTrigger::ExitFallback => "backend fallback stop on Exit failed",
        ExitTrigger::TrayQuit => "backend graceful stop on tray quit failed",
    }
}

pub fn try_begin_exit_cleanup<F>(state: &BackendState, trigger: ExitTrigger, log: F) -> bool
where
    F: Fn(&str),
{
    if state.try_begin_exit_cleanup() {
        return true;
    }

    log(duplicate_cleanup_message(trigger));
    false
}

pub fn stop_backend_for_exit<F>(state: &BackendState, trigger: ExitTrigger, log: F)
where
    F: Fn(&str),
{
    let failure_prefix = stop_failure_prefix(trigger);
    if let Err(error) = state.stop_backend() {
        log(&format!("{failure_prefix}: {error}"));
    }

    if matches!(trigger, ExitTrigger::ExitRequested | ExitTrigger::TrayQuit) {
        log("backend stop finished, exiting desktop process");
    }
}

#[cfg(test)]
mod tests {
    use super::{duplicate_cleanup_message, ExitTrigger};

    #[test]
    fn duplicate_cleanup_message_describes_tray_quit_trigger() {
        assert_eq!(
            duplicate_cleanup_message(ExitTrigger::TrayQuit),
            "tray quit while backend cleanup is already running"
        );
    }

    #[test]
    fn stop_failure_prefix_describes_tray_quit_trigger() {
        assert_eq!(
            super::stop_failure_prefix(ExitTrigger::TrayQuit),
            "backend graceful stop on tray quit failed"
        );
    }
}

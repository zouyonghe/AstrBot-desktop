#[cfg(any(target_os = "windows", test))]
use std::collections::{HashMap, HashSet};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::{
    io,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, ERROR_NOT_FOUND,
        ERROR_NO_MORE_FILES, HANDLE, INVALID_HANDLE_VALUE,
    },
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
        Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE},
    },
};

const FORCE_STOP_WAIT_MIN_MS: u64 = 200;
#[cfg(target_os = "windows")]
const WINDOWS_GRACEFUL_STOP_NONZERO_WAIT_MS: u64 = 350;
#[cfg(target_os = "windows")]
const FORCE_STOP_WAIT_MAX_WINDOWS_MS: u64 = 2_200;
#[cfg(not(target_os = "windows"))]
const FORCE_STOP_WAIT_MAX_NON_WINDOWS_MS: u64 = 1_500;
#[cfg(target_os = "windows")]
const WINDOWS_CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn wait_for_child_exit(child: &mut Child, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => {
                if start.elapsed() >= timeout {
                    return false;
                }
                thread::sleep(Duration::from_millis(120));
            }
            Err(_) => return false,
        }
    }
}

fn run_stop_command<F>(
    pid: u32,
    label: &str,
    program: &str,
    args: &[&str],
    log: F,
) -> io::Result<ExitStatus>
where
    F: Fn(&str) + Copy,
{
    let mut command = Command::new(program);
    command
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        // Avoid flashing transient black console windows when invoking taskkill.
        command.creation_flags(WINDOWS_CREATE_NO_WINDOW);
    }
    let status = command.status();

    match &status {
        Ok(exit_status) if exit_status.success() => {}
        Ok(exit_status) => log(&format!(
            "{label} returned non-zero: pid={pid}, status={exit_status:?}"
        )),
        Err(error) => log(&format!(
            "{label} failed to start: pid={pid}, error={error}"
        )),
    }

    status
}

fn compute_followup_wait(timeout: Duration, max_extra_wait: Duration) -> Duration {
    if timeout.is_zero() {
        Duration::ZERO
    } else {
        (timeout / 4)
            .max(Duration::from_millis(FORCE_STOP_WAIT_MIN_MS))
            .min(max_extra_wait)
    }
}

fn resolve_graceful_wait_timeout<F>(
    pid: u32,
    timeout: Duration,
    non_success_wait_cap: Duration,
    graceful_status: &io::Result<ExitStatus>,
    command_label: &str,
    log: F,
) -> Duration
where
    F: Fn(&str) + Copy,
{
    match graceful_status {
        Ok(status) if status.success() => timeout,
        _ => {
            let shortened_wait = timeout.min(non_success_wait_cap);
            if shortened_wait < timeout {
                let outcome = match graceful_status {
                    Ok(status) => format!("status={status:?}"),
                    Err(error) => format!("error={error}"),
                };
                log(&format!(
                    "{command_label} not successful; shorten graceful wait: pid={pid}, {outcome}, requested_wait_ms={}, effective_wait_ms={}",
                    timeout.as_millis(),
                    shortened_wait.as_millis()
                ));
            }
            shortened_wait
        }
    }
}

#[cfg(target_os = "windows")]
fn close_handle(handle: HANDLE) {
    unsafe {
        CloseHandle(handle);
    }
}

#[cfg(any(target_os = "windows", test))]
fn collect_descendant_processes(root_pid: u32, entries: &[(u32, u32)]) -> Vec<u32> {
    let mut children_by_parent: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, parent_pid) in entries {
        children_by_parent
            .entry(*parent_pid)
            .or_default()
            .push(*pid);
    }

    let mut tree = Vec::new();
    let mut discovered = HashSet::from([root_pid]);
    let mut stack = vec![root_pid];
    while let Some(parent_pid) = stack.pop() {
        let Some(children) = children_by_parent.get(&parent_pid) else {
            continue;
        };
        for pid in children {
            if discovered.insert(*pid) {
                tree.push(*pid);
                stack.push(*pid);
            }
        }
    }
    tree.push(root_pid);
    tree
}

#[cfg(target_os = "windows")]
fn snapshot_process_tree(root_pid: u32) -> io::Result<Vec<u32>> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }

    let mut entries = Vec::new();
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    let first_ok = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    if !first_ok {
        let error = io::Error::last_os_error();
        close_handle(snapshot);
        return Err(error);
    }

    loop {
        entries.push((entry.th32ProcessID, entry.th32ParentProcessID));
        let next_ok = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
        if !next_ok {
            let error = io::Error::last_os_error();
            if is_process_snapshot_end_error(&error) {
                break;
            }
            close_handle(snapshot);
            return Err(error);
        }
    }
    close_handle(snapshot);

    Ok(collect_descendant_processes(root_pid, &entries))
}

#[cfg(all(test, not(target_os = "windows")))]
const ERROR_ACCESS_DENIED: u32 = 5;
#[cfg(all(test, not(target_os = "windows")))]
const ERROR_INVALID_PARAMETER: u32 = 87;
#[cfg(all(test, not(target_os = "windows")))]
const ERROR_NO_MORE_FILES: u32 = 18;
#[cfg(all(test, not(target_os = "windows")))]
const ERROR_NOT_FOUND: u32 = 1168;

#[cfg(any(target_os = "windows", test))]
fn is_process_snapshot_end_error(error: &io::Error) -> bool {
    let Some(code) = error.raw_os_error() else {
        return false;
    };
    code as u32 == ERROR_NO_MORE_FILES
}

#[cfg(target_os = "windows")]
fn terminate_process_by_pid(pid: u32) -> io::Result<()> {
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
    if handle.is_null() {
        let error = io::Error::last_os_error();
        if is_expected_shutdown_termination_error(&error) {
            return Ok(());
        }
        return Err(error);
    }

    let result = unsafe { TerminateProcess(handle, 1) };
    let error = if result == 0 {
        Some(io::Error::last_os_error())
    } else {
        None
    };
    close_handle(handle);

    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(any(target_os = "windows", test))]
fn is_expected_shutdown_termination_error(error: &io::Error) -> bool {
    let Some(code) = error.raw_os_error() else {
        return false;
    };
    matches!(
        code as u32,
        ERROR_ACCESS_DENIED | ERROR_INVALID_PARAMETER | ERROR_NOT_FOUND
    )
}

#[cfg(target_os = "windows")]
fn terminate_process_tree_native(root_pid: u32, log: &dyn Fn(&str)) {
    let process_tree = match snapshot_process_tree(root_pid) {
        Ok(process_tree) => process_tree,
        Err(error) => {
            log(&format!(
                "native Windows process tree snapshot failed: pid={root_pid}, error={error}"
            ));
            vec![root_pid]
        }
    };

    for pid in process_tree {
        if let Err(error) = terminate_process_by_pid(pid) {
            if !is_expected_shutdown_termination_error(&error) {
                log(&format!(
                    "native Windows process termination failed: pid={pid}, error={error}"
                ));
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub fn stop_child_process_for_system_shutdown<F>(
    child: &mut Child,
    timeout: Duration,
    log: F,
) -> bool
where
    F: Fn(&str) + Copy,
{
    let pid = child.id();
    terminate_process_tree_native(pid, &log);
    wait_for_child_exit(child, timeout)
}

#[cfg(target_os = "windows")]
pub fn stop_child_process_gracefully<F>(child: &mut Child, timeout: Duration, log: F) -> bool
where
    F: Fn(&str) + Copy,
{
    // Normal Windows app exits intentionally keep the existing taskkill-based
    // process-tree cleanup. The system-shutdown path uses native Win32
    // termination instead so it does not launch taskkill.exe during shutdown.
    let pid = child.id();
    let pid_arg = pid.to_string();

    let graceful_status = run_stop_command(
        pid,
        "taskkill graceful stop",
        "taskkill",
        &["/pid", &pid_arg, "/t"],
        log,
    );

    let graceful_wait_timeout = resolve_graceful_wait_timeout(
        pid,
        timeout,
        Duration::from_millis(WINDOWS_GRACEFUL_STOP_NONZERO_WAIT_MS),
        &graceful_status,
        "taskkill graceful stop",
        log,
    );

    if wait_for_child_exit(child, graceful_wait_timeout) {
        return true;
    }

    let force_status = run_stop_command(
        pid,
        "taskkill force stop",
        "taskkill",
        &["/pid", &pid_arg, "/t", "/f"],
        log,
    );

    let followup_wait = compute_followup_wait(
        timeout,
        Duration::from_millis(FORCE_STOP_WAIT_MAX_WINDOWS_MS),
    );
    log(&format!(
        "child graceful stop timed out, force-kill issued: pid={pid}, graceful={graceful_status:?}, force={force_status:?}, followup_wait_ms={}",
        followup_wait.as_millis(),
    ));
    wait_for_child_exit(child, followup_wait)
}

#[cfg(not(target_os = "windows"))]
pub fn stop_child_process_gracefully<F>(child: &mut Child, timeout: Duration, log: F) -> bool
where
    F: Fn(&str) + Copy,
{
    // Normal Unix app exits intentionally keep the existing external kill-based
    // process cleanup.
    let pid = child.id();
    let pid_arg = pid.to_string();

    let graceful_status = run_stop_command(pid, "kill -TERM", "kill", &["-TERM", &pid_arg], log);

    let graceful_wait_timeout =
        resolve_graceful_wait_timeout(pid, timeout, timeout, &graceful_status, "kill -TERM", log);
    if wait_for_child_exit(child, graceful_wait_timeout) {
        return true;
    }

    let force_status = run_stop_command(pid, "kill -KILL", "kill", &["-KILL", &pid_arg], log);

    let followup_wait = compute_followup_wait(
        timeout,
        Duration::from_millis(FORCE_STOP_WAIT_MAX_NON_WINDOWS_MS),
    );
    log(&format!(
        "child graceful stop timed out, force-kill issued: pid={pid}, graceful={graceful_status:?}, force={force_status:?}, followup_wait_ms={}",
        followup_wait.as_millis(),
    ));

    wait_for_child_exit(child, followup_wait)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn collect_descendant_processes_deduplicates_with_cycles() {
        let tree = collect_descendant_processes(
            10,
            &[(11, 10), (12, 10), (13, 11), (13, 11), (10, 13), (99, 98)],
        );

        assert_eq!(tree.len(), 4);
        assert!(tree.contains(&10));
        assert!(tree.contains(&11));
        assert!(tree.contains(&12));
        assert!(tree.contains(&13));
        assert!(!tree.contains(&99));
        assert_eq!(tree.last(), Some(&10));
    }

    #[test]
    fn expected_shutdown_termination_errors_match_windows_race_codes() {
        for code in [
            ERROR_ACCESS_DENIED,
            ERROR_INVALID_PARAMETER,
            ERROR_NOT_FOUND,
        ] {
            let error = io::Error::from_raw_os_error(code as i32);
            assert!(is_expected_shutdown_termination_error(&error));
        }

        let unexpected = io::Error::from_raw_os_error(123_456);
        assert!(!is_expected_shutdown_termination_error(&unexpected));
        assert!(!is_expected_shutdown_termination_error(&io::Error::other(
            "no os code"
        )));
    }

    #[test]
    fn process_snapshot_end_error_only_matches_no_more_files() {
        let end = io::Error::from_raw_os_error(ERROR_NO_MORE_FILES as i32);
        assert!(is_process_snapshot_end_error(&end));

        let unexpected = io::Error::from_raw_os_error(ERROR_ACCESS_DENIED as i32);
        assert!(!is_process_snapshot_end_error(&unexpected));
        assert!(!is_process_snapshot_end_error(&io::Error::other(
            "no os code"
        )));
    }

    #[test]
    fn compute_followup_wait_respects_min_and_cap() {
        assert_eq!(
            compute_followup_wait(Duration::from_millis(0), Duration::from_millis(900)),
            Duration::ZERO
        );
        assert_eq!(
            compute_followup_wait(Duration::from_millis(100), Duration::from_millis(900)),
            Duration::from_millis(200)
        );
        assert_eq!(
            compute_followup_wait(Duration::from_millis(9_000), Duration::from_millis(900)),
            Duration::from_millis(900)
        );
    }

    #[test]
    fn resolve_graceful_wait_timeout_shortens_and_logs_on_failure() {
        let logs = Mutex::new(Vec::new());
        let graceful_status: io::Result<ExitStatus> = Err(io::Error::other("simulated failure"));
        let wait = resolve_graceful_wait_timeout(
            42,
            Duration::from_millis(2_000),
            Duration::from_millis(350),
            &graceful_status,
            "taskkill graceful stop",
            |message| logs.lock().expect("lock logs").push(message.to_string()),
        );

        assert_eq!(wait, Duration::from_millis(350));
        let snapshot = logs.lock().expect("lock logs");
        assert_eq!(snapshot.len(), 1);
        assert!(snapshot[0].contains("shorten graceful wait"));
    }
}

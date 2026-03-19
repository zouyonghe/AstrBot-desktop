use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use crate::{
    app_types::{
        StartupPanelSnapshot, StartupPanelStage, StartupPanelStageItem, StartupPanelState,
    },
    logging, runtime_paths, BackendState, DESKTOP_LOG_FILE,
};

const SNAPSHOT_LOG_READ_BYTES: usize = 8 * 1024;
const SNAPSHOT_LOG_LINES: usize = 8;

struct CompactStageDefinition {
    stage: StartupPanelStage,
    key: &'static str,
    label: &'static str,
}

const COMPACT_STAGE_DEFINITIONS: [CompactStageDefinition; 4] = [
    CompactStageDefinition {
        stage: StartupPanelStage::ResolveLaunchPlan,
        key: "plan",
        label: "Plan",
    },
    CompactStageDefinition {
        stage: StartupPanelStage::SpawnBackend,
        key: "spawn",
        label: "Spawn",
    },
    CompactStageDefinition {
        stage: StartupPanelStage::TcpReachable,
        key: "tcp",
        label: "TCP",
    },
    CompactStageDefinition {
        stage: StartupPanelStage::HttpReady,
        key: "http",
        label: "HTTP",
    },
];

fn with_startup_panel_state<R, F>(state: &BackendState, update: F) -> R
where
    F: FnOnce(&mut StartupPanelState) -> R,
{
    match state.startup_panel.lock() {
        Ok(mut guard) => update(&mut guard),
        Err(error) => {
            let mut guard = error.into_inner();
            update(&mut guard)
        }
    }
}

fn current_startup_panel_state(state: &BackendState) -> StartupPanelState {
    match state.startup_panel.lock() {
        Ok(guard) => guard.clone(),
        Err(error) => error.into_inner().clone(),
    }
}

pub(crate) fn set_stage(state: &BackendState, stage: StartupPanelStage) {
    with_startup_panel_state(state, |panel| {
        panel.stage = stage;
        panel.last_non_failed_stage = stage;
        panel.failure = None;
    });
}

pub(crate) fn set_failed(state: &BackendState, reason: &str) {
    let failure = reason.trim();
    with_startup_panel_state(state, |panel| {
        panel.stage = StartupPanelStage::Failed;
        panel.failure = if failure.is_empty() {
            None
        } else {
            Some(failure.to_string())
        };
    });
}

pub(crate) fn remember_backend_log_path(state: &BackendState, path: PathBuf) {
    let start_offset = current_log_end(&path);
    with_startup_panel_state(state, |panel| {
        panel.backend_log_path = Some(path);
        panel.backend_log_start_offset = start_offset;
    });
}

pub(crate) fn remember_desktop_log_start(state: &BackendState, path: &Path) {
    let start_offset = current_log_end(path);
    with_startup_panel_state(state, |panel| {
        panel.desktop_log_start_offset = start_offset;
    });
}

pub(crate) fn snapshot(state: &BackendState) -> StartupPanelSnapshot {
    let panel = current_startup_panel_state(state);
    let desktop_log_path = logging::resolve_desktop_log_path(
        runtime_paths::default_packaged_root_dir(),
        DESKTOP_LOG_FILE,
    );

    snapshot_for_panel_state(&panel, &desktop_log_path)
}

fn snapshot_for_panel_state(
    panel: &StartupPanelState,
    desktop_log_path: &Path,
) -> StartupPanelSnapshot {
    let desktop_log_tail = read_recent_log_tail_after(
        desktop_log_path,
        panel.desktop_log_start_offset,
        SNAPSHOT_LOG_READ_BYTES,
    );
    let backend_log_tail = panel
        .backend_log_path
        .as_deref()
        .map(|path| {
            read_recent_log_tail_after(
                path,
                panel.backend_log_start_offset,
                SNAPSHOT_LOG_READ_BYTES,
            )
        })
        .unwrap_or_default();

    build_snapshot(
        panel.stage,
        panel.last_non_failed_stage,
        panel.failure.as_deref(),
        &desktop_log_tail,
        &backend_log_tail,
    )
}

pub(crate) fn build_stage_items(stage: StartupPanelStage) -> Option<[StartupPanelStageItem; 4]> {
    let current_index = COMPACT_STAGE_DEFINITIONS
        .iter()
        .position(|definition| definition.stage == stage)?;

    Some(std::array::from_fn(|index| {
        let definition = &COMPACT_STAGE_DEFINITIONS[index];

        StartupPanelStageItem {
            key: definition.key,
            label: definition.label,
            done: index < current_index,
            active: index == current_index,
        }
    }))
}

fn stage_summary(stage: StartupPanelStage, failure: Option<&str>) -> String {
    match stage {
        StartupPanelStage::ResolveLaunchPlan => "Resolving launch plan".to_string(),
        StartupPanelStage::SpawnBackend => "Spawning backend".to_string(),
        StartupPanelStage::TcpReachable => "TCP ready, waiting for HTTP".to_string(),
        StartupPanelStage::HttpReady => "Backend ready".to_string(),
        StartupPanelStage::Failed => failure
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Startup failed")
            .to_string(),
    }
}

fn current_log_end(path: &Path) -> u64 {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn read_recent_log_tail_after(path: &Path, start_offset: u64, max_bytes: usize) -> String {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return String::new(),
    };

    read_recent_log_tail_from_reader_after(&mut file, start_offset, max_bytes).unwrap_or_default()
}

#[cfg(test)]
fn read_recent_log_tail_from_reader<R>(reader: &mut R, max_bytes: usize) -> io::Result<String>
where
    R: Read + Seek,
{
    read_recent_log_tail_from_reader_after(reader, 0, max_bytes)
}

fn read_recent_log_tail_from_reader_after<R>(
    reader: &mut R,
    start_offset: u64,
    max_bytes: usize,
) -> io::Result<String>
where
    R: Read + Seek,
{
    if max_bytes == 0 {
        return Ok(String::new());
    }

    let end = reader.seek(SeekFrom::End(0))?;
    if end == 0 {
        return Ok(String::new());
    }

    let effective_start = if start_offset > end { 0 } else { start_offset };
    if effective_start == end {
        return Ok(String::new());
    }

    let read_len = (end - effective_start).min(max_bytes as u64);
    let start = end - read_len;
    reader.seek(SeekFrom::Start(start))?;

    let mut raw = Vec::with_capacity(read_len as usize);
    let mut limited_reader = reader.take(read_len);
    limited_reader.read_to_end(&mut raw)?;
    if raw.is_empty() {
        return Ok(String::new());
    }

    let mut tail = String::from_utf8_lossy(&raw).into_owned();
    if start > effective_start {
        if let Some(index) = tail.find('\n') {
            tail.drain(..=index);
        } else {
            tail.clear();
        }
    }
    Ok(tail)
}

fn build_snapshot(
    stage: StartupPanelStage,
    compact_stage: StartupPanelStage,
    failure: Option<&str>,
    desktop_log_tail: &str,
    backend_log_tail: &str,
) -> StartupPanelSnapshot {
    StartupPanelSnapshot {
        stage,
        summary: stage_summary(stage, failure),
        items: build_stage_items(compact_stage)
            .map(Vec::from)
            .unwrap_or_default(),
        desktop_log: trim_non_empty_log_tail(desktop_log_tail, SNAPSHOT_LOG_LINES),
        backend_log: trim_non_empty_log_tail(backend_log_tail, SNAPSHOT_LOG_LINES),
    }
}

pub(crate) fn trim_non_empty_log_tail(log_tail: &str, max_lines: usize) -> Vec<String> {
    let mut trimmed = log_tail
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .filter(|line| !line.trim().is_empty())
        .rev()
        .take(max_lines)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    trimmed.reverse();
    trimmed
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{self, Cursor, Read, Seek, SeekFrom, Write},
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        build_snapshot, build_stage_items, current_startup_panel_state,
        read_recent_log_tail_from_reader, remember_backend_log_path, remember_desktop_log_start,
        set_failed, set_stage, snapshot_for_panel_state, trim_non_empty_log_tail,
    };
    use crate::{app_types::StartupPanelStage, BackendState};

    fn create_temp_case_dir(name: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "astrbot-desktop-startup-panel-test-{}-{}-{}",
            std::process::id(),
            ts,
            name
        ));
        fs::create_dir_all(&dir).expect("create temp case dir");
        dir
    }

    #[test]
    fn build_stage_items_marks_previous_done_and_current_active_at_tcp_reachable() {
        let items = build_stage_items(StartupPanelStage::TcpReachable)
            .expect("non-failed stages should produce stage items");

        assert_eq!(items.len(), 4);
        assert!(items[0].done);
        assert!(!items[0].active);
        assert!(items[1].done);
        assert!(!items[1].active);
        assert!(!items[2].done);
        assert!(items[2].active);
        assert!(!items[3].done);
        assert!(!items[3].active);
    }

    #[test]
    fn build_stage_items_covers_each_non_failed_stage() {
        let cases = [
            (StartupPanelStage::ResolveLaunchPlan, 0),
            (StartupPanelStage::SpawnBackend, 1),
            (StartupPanelStage::TcpReachable, 2),
            (StartupPanelStage::HttpReady, 3),
        ];

        for (stage, active_index) in cases {
            let items = build_stage_items(stage).expect("non-failed stages should produce items");
            assert!(items[active_index].active);
        }
    }

    #[test]
    fn build_stage_items_use_expected_keys_and_labels() {
        let items = build_stage_items(StartupPanelStage::ResolveLaunchPlan)
            .expect("non-failed stages should produce stage items");

        let stage_meta = items.map(|item| (item.key, item.label));
        assert_eq!(
            stage_meta,
            [
                ("plan", "Plan"),
                ("spawn", "Spawn"),
                ("tcp", "TCP"),
                ("http", "HTTP")
            ]
        );
    }

    #[test]
    fn build_stage_items_skips_compact_flow_for_failed_stage() {
        assert_eq!(build_stage_items(StartupPanelStage::Failed), None);
    }

    #[test]
    fn snapshot_failed_stage_keeps_last_non_failed_stage_items() {
        let state = BackendState::default();
        set_stage(&state, StartupPanelStage::TcpReachable);
        set_failed(&state, "backend crashed");

        let root = create_temp_case_dir("snapshot-failed-stage-items");
        let desktop_log_path = root.join(crate::DESKTOP_LOG_FILE);
        fs::write(&desktop_log_path, "tcp reachable\nbackend crashed\n")
            .expect("write desktop log");

        let snapshot =
            snapshot_for_panel_state(&current_startup_panel_state(&state), &desktop_log_path);

        assert_eq!(snapshot.stage, StartupPanelStage::Failed);
        assert_eq!(snapshot.summary, "backend crashed");
        assert_eq!(snapshot.items.len(), 4);
        assert!(snapshot.items[0].done);
        assert!(snapshot.items[1].done);
        assert!(snapshot.items[2].active);
        assert!(!snapshot.items[3].done);
        assert!(!snapshot.items[3].active);

        fs::remove_dir_all(&root).expect("cleanup temp case dir");
    }

    #[test]
    fn trim_non_empty_log_tail_keeps_latest_non_empty_lines() {
        let trimmed =
            trim_non_empty_log_tail("\nplan ready\n\nbackend spawned\n  \nhttp ready\n", 2);

        assert_eq!(trimmed, vec!["backend spawned", "http ready"]);
    }

    #[test]
    fn trim_non_empty_log_tail_normalizes_crlf_before_filtering_blank_lines() {
        let trimmed = trim_non_empty_log_tail("plan ready\r\n\r\nbackend spawned\r\n", 3);

        assert_eq!(trimmed, vec!["plan ready", "backend spawned"]);
    }

    #[test]
    fn build_snapshot_includes_active_stage_summary_and_trimmed_logs() {
        let snapshot = build_snapshot(
            StartupPanelStage::TcpReachable,
            StartupPanelStage::TcpReachable,
            None,
            "\nresolve launch plan\n\nbackend spawned\n",
            "\nlistening on 6185\nhttp still warming up\n",
        );

        assert_eq!(snapshot.stage, StartupPanelStage::TcpReachable);
        assert_eq!(snapshot.summary, "TCP ready, waiting for HTTP");
        assert_eq!(snapshot.items.len(), 4);
        assert!(snapshot.items[2].active);
        assert_eq!(
            snapshot.desktop_log,
            vec!["resolve launch plan", "backend spawned"]
        );
        assert_eq!(
            snapshot.backend_log,
            vec!["listening on 6185", "http still warming up"]
        );
    }

    #[test]
    fn snapshot_keeps_backend_log_empty_when_backend_path_is_unknown() {
        let root = create_temp_case_dir("snapshot-no-backend-path");
        let logs_dir = root.join("logs");
        fs::create_dir_all(&logs_dir).expect("create logs dir");
        let desktop_log_path = logs_dir.join(crate::DESKTOP_LOG_FILE);
        fs::write(&desktop_log_path, "resolve launch plan\nbackend spawned\n")
            .expect("write desktop log");
        fs::write(logs_dir.join("backend.log"), "unexpected backend log\n")
            .expect("write backend log");

        let snapshot = snapshot_for_panel_state(
            &current_startup_panel_state(&BackendState::default()),
            &desktop_log_path,
        );

        assert_eq!(
            snapshot.desktop_log,
            vec!["resolve launch plan", "backend spawned"]
        );
        assert!(snapshot.backend_log.is_empty());

        fs::remove_dir_all(&root).expect("cleanup temp case dir");
    }

    #[test]
    fn remember_backend_log_path_marks_current_end_so_snapshot_skips_previous_launch_lines() {
        let state = BackendState::default();
        let root = create_temp_case_dir("snapshot-backend-log-start-offset");
        let desktop_log_path = root.join(crate::DESKTOP_LOG_FILE);
        let backend_log_path = root.join("backend.log");

        fs::write(&desktop_log_path, "resolve launch plan\nbackend spawned\n")
            .expect("write desktop log");
        fs::write(&backend_log_path, "previous launch line\nold warmup\n")
            .expect("write existing backend log");

        remember_backend_log_path(&state, backend_log_path.clone());

        let mut backend_log = fs::OpenOptions::new()
            .append(true)
            .open(&backend_log_path)
            .expect("open backend log for append");
        backend_log
            .write_all(b"current launch line\nhttp ready\n")
            .expect("append current backend log");

        let snapshot =
            snapshot_for_panel_state(&current_startup_panel_state(&state), &desktop_log_path);

        assert_eq!(
            snapshot.backend_log,
            vec!["current launch line", "http ready"]
        );

        fs::remove_dir_all(&root).expect("cleanup temp case dir");
    }

    #[test]
    fn remember_desktop_log_start_marks_current_end_so_snapshot_skips_previous_launch_lines() {
        let state = BackendState::default();
        let root = create_temp_case_dir("snapshot-desktop-log-start-offset");
        let desktop_log_path = root.join(crate::DESKTOP_LOG_FILE);

        fs::write(
            &desktop_log_path,
            "previous desktop line\nold readiness note\n",
        )
        .expect("write existing desktop log");

        remember_desktop_log_start(&state, &desktop_log_path);

        let mut desktop_log = fs::OpenOptions::new()
            .append(true)
            .open(&desktop_log_path)
            .expect("open desktop log for append");
        desktop_log
            .write_all(b"current startup line\nhttp ready\n")
            .expect("append current desktop log");

        let snapshot =
            snapshot_for_panel_state(&current_startup_panel_state(&state), &desktop_log_path);

        assert_eq!(
            snapshot.desktop_log,
            vec!["current startup line", "http ready"]
        );

        fs::remove_dir_all(&root).expect("cleanup temp case dir");
    }

    struct CountingCursor {
        inner: Cursor<Vec<u8>>,
        bytes_read: usize,
    }

    impl CountingCursor {
        fn new(bytes: Vec<u8>) -> Self {
            Self {
                inner: Cursor::new(bytes),
                bytes_read: 0,
            }
        }
    }

    impl Read for CountingCursor {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let bytes_read = self.inner.read(buf)?;
            self.bytes_read += bytes_read;
            Ok(bytes_read)
        }
    }

    impl Seek for CountingCursor {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    #[test]
    fn read_recent_log_tail_reads_only_requested_tail_bytes() {
        let bytes = b"prefix that should be skipped\nbackend ready\nhttp ready\n".to_vec();
        let max_bytes = b" skipped\nbackend ready\nhttp ready\n".len();
        let mut reader = CountingCursor::new(bytes);

        let tail = read_recent_log_tail_from_reader(&mut reader, max_bytes)
            .expect("read recent log tail from reader");

        assert_eq!(tail, "backend ready\nhttp ready\n");
        assert_eq!(reader.bytes_read, max_bytes);
    }
}

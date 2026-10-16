use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

pub const CANCELLED_ERROR: &str = "__S3CC_OPERATION_CANCELLED__";

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OperationStatus {
    pub kind: String,
    pub running: bool,
    pub cancel_requested: bool,
    pub processed: usize,
    pub total: usize,
    pub current: Option<String>,
    pub phase: String,
    pub message: Option<String>,
    pub started_unix_ms: u64,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationRun {
    kind: String,
    generation: u64,
}

#[derive(Debug, Clone)]
struct ActiveMutation {
    run: OperationRun,
    scopes: Vec<String>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

fn registry() -> &'static Mutex<HashMap<String, OperationStatus>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, OperationStatus>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lifecycle_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn mutation_owner() -> &'static Mutex<Vec<ActiveMutation>> {
    static OWNER: OnceLock<Mutex<Vec<ActiveMutation>>> = OnceLock::new();
    OWNER.get_or_init(|| Mutex::new(Vec::new()))
}

fn active_runs() -> &'static Mutex<HashMap<String, u64>> {
    static RUNS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    RUNS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A long operation may consist of several real phases owned by different
/// modules.  Keep their counters on one monotonic scale so the UI never has
/// to reset from 100% to 0% merely because it moved from scanning to a
/// duplicate or conflict pass.
#[derive(Debug, Clone, Copy)]
struct ProgressSegment {
    offset: usize,
    span: usize,
    overall_total: usize,
}

fn progress_segments() -> &'static Mutex<HashMap<String, ProgressSegment>> {
    static SEGMENTS: OnceLock<Mutex<HashMap<String, ProgressSegment>>> = OnceLock::new();
    SEGMENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn set_progress_segment(kind: &str, offset: usize, span: usize, overall_total: usize) {
    let mut segments = progress_segments()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    segments.insert(
        kind.to_string(),
        ProgressSegment {
            offset,
            span,
            overall_total: overall_total.max(1),
        },
    );
}

pub fn clear_progress_segment(kind: &str) {
    if let Ok(mut segments) = progress_segments().lock() {
        segments.remove(kind);
    }
}

fn progress_segment(kind: &str) -> Option<ProgressSegment> {
    progress_segments()
        .lock()
        .ok()
        .and_then(|segments| segments.get(kind).copied())
}

fn next_generation() -> u64 {
    static NEXT: OnceLock<Mutex<u64>> = OnceLock::new();
    let mut next = NEXT
        .get_or_init(|| Mutex::new(0))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *next = next.wrapping_add(1).max(1);
    *next
}

fn with_status<F>(kind: &str, mut f: F)
where
    F: FnMut(&mut OperationStatus),
{
    let mut map = registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let status = map
        .entry(kind.to_string())
        .or_insert_with(|| OperationStatus {
            kind: kind.to_string(),
            ..OperationStatus::default()
        });
    f(status);
}

fn begin_locked(kind: &str, phase: &str) -> Result<OperationRun, String> {
    let generation = next_generation();
    let mut runs = active_runs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if runs.contains_key(kind) {
        return Err(format!("An operation of type '{kind}' is already running."));
    }
    runs.insert(kind.to_string(), generation);
    drop(runs);
    with_status(kind, |status| {
        *status = OperationStatus {
            kind: kind.to_string(),
            running: true,
            cancel_requested: false,
            processed: 0,
            total: 0,
            current: None,
            phase: phase.to_string(),
            message: None,
            started_unix_ms: now_ms(),
            elapsed_ms: 0,
        };
    });
    Ok(OperationRun {
        kind: kind.to_string(),
        generation,
    })
}

pub fn begin(kind: &str, phase: &str) -> Result<OperationRun, String> {
    let _lifecycle = lifecycle_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    begin_locked(kind, phase)
}

/// Start an operation that can mutate files.  The UI normally serializes
/// these actions, but the backend must also reject concurrent callers so a
/// stale second invocation cannot interleave with a transaction or rollback.
pub fn begin_mutating(kind: &str, phase: &str) -> Result<OperationRun, String> {
    begin_mutating_scoped(kind, phase, &["*"])
}

pub fn begin_mutating_scoped(
    kind: &str,
    phase: &str,
    scopes: &[&str],
) -> Result<OperationRun, String> {
    let _lifecycle = lifecycle_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let normalized_scopes = scopes
        .iter()
        .map(|scope| normalize_scope(scope))
        .collect::<Vec<_>>();
    let mut mutations = mutation_owner()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(current) = mutations
        .iter()
        .find(|current| scopes_conflict(&current.scopes, &normalized_scopes))
    {
        return Err(format!(
            "Another file operation is already using the requested scope ({kind} conflicts with {}).",
            current.run.kind
        ));
    }
    // Holding the lifecycle and mutation locks while reserving the per-kind
    // run prevents two mutating commands from passing their checks together.
    let run = begin_locked(kind, phase)?;
    mutations.push(ActiveMutation {
        run: run.clone(),
        scopes: normalized_scopes,
    });
    Ok(run)
}

fn normalize_scope(scope: &str) -> String {
    let normalized = scope
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase();
    if normalized.is_empty() {
        "*".to_string()
    } else {
        normalized
    }
}

fn scopes_conflict(left: &[String], right: &[String]) -> bool {
    left.iter().any(|a| {
        right.iter().any(|b| {
            a == "*"
                || b == "*"
                || a == b
                || a.strip_prefix(b).is_some_and(|rest| rest.starts_with('/'))
                || b.strip_prefix(a).is_some_and(|rest| rest.starts_with('/'))
        })
    })
}

fn release_mutation(run: &OperationRun) {
    let mut mutations = mutation_owner()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    mutations.retain(|active| active.run != *run);
}

fn release_run(run: &OperationRun, phase: &str, message: Option<String>, cancelled: bool) -> bool {
    let _lifecycle = lifecycle_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    {
        let mut runs = active_runs()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if runs.get(&run.kind) != Some(&run.generation) {
            return false;
        }
        runs.remove(&run.kind);
    }
    #[cfg(test)]
    pause_after_active_release(&run.kind);
    release_mutation(run);
    clear_progress_segment(&run.kind);
    with_status(&run.kind, |status| {
        status.running = false;
        status.cancel_requested = cancelled;
        status.current = None;
        status.phase = phase.to_string();
        status.message = message.clone();
        status.elapsed_ms = now_ms().saturating_sub(status.started_unix_ms);
    });
    true
}

#[cfg(test)]
fn completion_pause() -> &'static Mutex<Option<(String, std::sync::Arc<std::sync::Barrier>)>> {
    static PAUSE: OnceLock<Mutex<Option<(String, std::sync::Arc<std::sync::Barrier>)>>> =
        OnceLock::new();
    PAUSE.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
fn pause_after_active_release(kind: &str) {
    let barrier = completion_pause()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .filter(|(paused_kind, _)| paused_kind == kind)
        .map(|(_, barrier)| barrier.clone());
    if let Some(barrier) = barrier {
        barrier.wait();
        barrier.wait();
    }
}

pub fn set_total(kind: &str, total: usize) {
    let total = progress_segment(kind)
        .map(|segment| segment.overall_total)
        .unwrap_or(total);
    with_status(kind, |status| status.total = total);
}

pub fn update(kind: &str, processed: usize, current: Option<String>, phase: &str) {
    let processed = progress_segment(kind)
        .map(|segment| segment.offset.saturating_add(processed.min(segment.span)))
        .unwrap_or(processed);
    with_status(kind, |status| {
        status.processed = processed;
        status.current = current.clone();
        status.phase = phase.to_string();
    });
}

pub fn set_message(kind: &str, message: Option<String>) {
    with_status(kind, |status| status.message = message.clone());
}

pub fn is_cancelled(kind: &str) -> bool {
    let map = registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    map.get(kind)
        .map(|status| status.running && status.cancel_requested)
        .unwrap_or(false)
}

pub fn finish(run: &OperationRun, phase: &str, message: Option<String>) {
    let _ = release_run(run, phase, message, false);
}

pub fn mark_cancelled(run: &OperationRun) {
    let _ = release_run(
        run,
        "cancelled",
        Some("Cancelled by user.".to_string()),
        true,
    );
}

/// Run CPU/I/O-heavy read-only work on Tauri's blocking pool and keep the
/// operation status consistent for polling and cancellation.
pub async fn run_blocking<T, F>(
    kind: &'static str,
    phase: &'static str,
    work: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let run = begin(kind, phase)?;
    let joined = tauri::async_runtime::spawn_blocking(work).await;
    let result = match joined {
        Ok(result) => result,
        Err(error) => Err(format!("{kind} worker failed: {error}")),
    };
    match &result {
        Ok(_) => finish(&run, "complete", None),
        Err(error) if error == CANCELLED_ERROR => mark_cancelled(&run),
        Err(error) => finish(&run, "error", Some(error.clone())),
    }
    result
}

/// Variant used by commands that copy/move/delete files.  It combines the
/// same worker-pool behavior with a backend mutation lock.
pub async fn run_mutating_blocking<T, F>(
    kind: &'static str,
    phase: &'static str,
    work: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    run_mutating_blocking_scoped(kind, phase, &["*"], work).await
}

pub async fn run_mutating_blocking_scoped<T, F>(
    kind: &'static str,
    phase: &'static str,
    scopes: &[&str],
    work: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let run = begin_mutating_scoped(kind, phase, scopes)?;
    let joined = tauri::async_runtime::spawn_blocking(work).await;
    let result = match joined {
        Ok(result) => result,
        Err(error) => Err(format!("{kind} worker failed: {error}")),
    };
    match &result {
        Ok(_) => finish(&run, "complete", None),
        Err(error) if error == CANCELLED_ERROR => mark_cancelled(&run),
        Err(error) => finish(&run, "error", Some(error.clone())),
    }
    result
}

#[tauri::command]
pub fn get_operation_status(kind: String) -> OperationStatus {
    let map = registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut status = map.get(&kind).cloned().unwrap_or_else(|| OperationStatus {
        kind,
        phase: "idle".to_string(),
        ..OperationStatus::default()
    });
    if status.running && status.started_unix_ms > 0 {
        status.elapsed_ms = now_ms().saturating_sub(status.started_unix_ms);
    }
    status
}

#[tauri::command]
pub fn cancel_operation(kind: String) -> bool {
    let mut changed = false;
    with_status(&kind, |status| {
        if status.running {
            status.cancel_requested = true;
            status.phase = "cancelling".to_string();
            status.message = Some("Cancellation requested.".to_string());
            changed = true;
        }
    });
    changed
}

pub fn run_mutating_sync<T, F>(
    kind: &str,
    phase: &str,
    scopes: &[&str],
    work: F,
) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    let run = begin_mutating_scoped(kind, phase, scopes)?;
    let result = work();
    match &result {
        Ok(_) => finish(&run, "complete", None),
        Err(error) if error == CANCELLED_ERROR => mark_cancelled(&run),
        Err(error) => finish(&run, "error", Some(error.clone())),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_scoped_by_operation_kind() {
        let scan = begin("scan-test", "starting").unwrap();
        let duplicates = begin("duplicates-test", "starting").unwrap();
        assert!(cancel_operation("scan-test".to_string()));
        assert!(is_cancelled("scan-test"));
        assert!(!is_cancelled("duplicates-test"));
        finish(&duplicates, "complete", None);
        finish(&scan, "complete", None);
    }

    #[test]
    fn completed_cancellation_does_not_leak_into_a_future_run() {
        let kind = "planning-cancellation-leak";
        let first = begin(kind, "building").unwrap();
        assert!(cancel_operation(kind.to_string()));
        assert!(is_cancelled(kind));
        mark_cancelled(&first);
        assert!(!is_cancelled(kind));

        let second = begin(kind, "building").unwrap();
        assert!(!is_cancelled(kind));
        finish(&second, "complete", None);
    }

    #[test]
    fn mutating_operations_are_serialized() {
        let scope = "C:/s3cc-operation-test/serialized";
        let first = begin_mutating_scoped("mutation-a", "starting", &[scope]).unwrap();
        assert!(begin_mutating_scoped("mutation-b", "starting", &[scope]).is_err());
        finish(&first, "complete", None);
        let second = begin_mutating_scoped("mutation-b", "starting", &[scope]).unwrap();
        finish(&second, "complete", None);
    }

    #[test]
    fn same_kind_runs_are_rejected_and_stale_completion_is_ignored() {
        let first = begin("same-kind", "starting").unwrap();
        assert!(begin("same-kind", "starting").is_err());
        finish(&first, "complete", None);

        let second = begin("same-kind", "starting").unwrap();
        finish(&first, "error", Some("stale".into()));
        let status = get_operation_status("same-kind".into());
        assert!(status.running);
        finish(&second, "complete", None);
        assert!(!get_operation_status("same-kind".into()).running);
    }

    #[test]
    fn finalization_and_new_begin_are_atomic_under_interleaving() {
        use std::sync::{mpsc, Arc, Barrier};
        use std::thread;
        use std::time::Duration;

        let kind = "atomic-finalization-race";
        let first = begin(kind, "starting").unwrap();
        let pause = Arc::new(Barrier::new(2));
        *completion_pause()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            Some((kind.to_string(), pause.clone()));

        let finish_thread = thread::spawn(move || {
            finish(&first, "complete", None);
        });

        // release_run has removed the active generation but has not yet
        // published the terminal status.  A new begin must remain blocked by
        // the lifecycle lock during this interval.
        pause.wait();

        let (started_tx, started_rx) = mpsc::channel();
        let begin_thread = thread::spawn(move || {
            let result = begin(kind, "starting");
            started_tx.send(result.is_ok()).unwrap();
            if let Ok(run) = result {
                finish(&run, "complete", None);
            }
        });

        let blocked_until_completion = started_rx.recv_timeout(Duration::from_millis(50)).is_err();

        // Publish the first terminal state and release the lifecycle lock.
        pause.wait();
        *completion_pause()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        finish_thread.join().unwrap();
        begin_thread.join().unwrap();

        let second_started = started_rx.recv_timeout(Duration::from_secs(1)).unwrap();

        assert!(blocked_until_completion);
        assert!(second_started);
        assert!(!get_operation_status(kind.to_string()).running);
    }

    #[test]
    fn mutation_scopes_allow_independent_writes_but_block_overlaps() {
        let first = begin_mutating_scoped("catalog-a", "saving", &["C:/s3cc-operation-test/a.csv"])
            .unwrap();
        let independent =
            begin_mutating_scoped("catalog-b", "saving", &["C:/s3cc-operation-test/b.csv"]);
        assert!(independent.is_ok());
        let independent = independent.unwrap();
        assert!(begin_mutating_scoped("catalog-c", "saving", &["C:/s3cc-operation-test"]).is_err());
        finish(&independent, "complete", None);
        finish(&first, "complete", None);
    }

    #[test]
    fn failed_mutation_releases_scope_for_retry() {
        let first = begin_mutating_scoped(
            "failed-write",
            "saving",
            &["C:/s3cc-operation-test/failure.csv"],
        )
        .unwrap();
        finish(&first, "error", Some("simulated write failure".into()));
        let retry = begin_mutating_scoped(
            "failed-write-retry",
            "saving",
            &["C:/s3cc-operation-test/failure.csv"],
        )
        .unwrap();
        finish(&retry, "complete", None);
    }
}

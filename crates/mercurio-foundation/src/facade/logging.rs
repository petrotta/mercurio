use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static LOG_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[cfg(not(target_arch = "wasm32"))]
pub type CompileTimer = Instant;

#[cfg(target_arch = "wasm32")]
pub type CompileTimer = ();

#[cfg(not(target_arch = "wasm32"))]
pub fn compile_timer_start() -> CompileTimer {
    Instant::now()
}

#[cfg(target_arch = "wasm32")]
pub fn compile_timer_start() -> CompileTimer {}

pub fn log_runtime_event(message: impl AsRef<str>) {
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let sequence = LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let line = format!(
        "[{} #{} pid={}] {}\n",
        timestamp_ms,
        sequence,
        std::process::id(),
        message.as_ref()
    );

    let _ = std::io::stderr().write_all(line.as_bytes());

    if let Some(path) = runtime_log_path() {
        if let Some(parent) = path.parent() {
            let _ = create_dir_all(parent);
        }

        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }
}

pub fn log_timed_event(operation: &str, start: Instant, outcome: &str, details: impl AsRef<str>) {
    log_runtime_event(format!(
        "{} {} elapsed_ms={} {}",
        operation,
        outcome,
        start.elapsed().as_millis(),
        details.as_ref()
    ));
}

pub fn log_compile_timed_event(
    operation: &str,
    start: CompileTimer,
    outcome: &str,
    details: impl AsRef<str>,
) {
    if compile_trace_enabled() {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = start;
            log_runtime_event(format!("{} {} {}", operation, outcome, details.as_ref()));
        }
        #[cfg(not(target_arch = "wasm32"))]
        log_timed_event(operation, start, outcome, details);
    }
}

fn compile_trace_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();

    *ENABLED.get_or_init(|| {
        std::env::var_os("MERCURIO_COMPILE_TRACE")
            .map(|value| value != "0" && value != "false")
            .unwrap_or(false)
    })
}

fn runtime_log_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MERCURIO_LOG_PATH") {
        return Some(PathBuf::from(path));
    }

    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return Some(
            PathBuf::from(local_app_data)
                .join("Mercurio")
                .join("logs")
                .join("desktop-ui.log"),
        );
    }

    Some(std::env::temp_dir().join("mercurio-desktop-ui.log"))
}

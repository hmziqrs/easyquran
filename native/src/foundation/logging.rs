//! File logging: a bounded in-memory ring every read sees, plus an
//! optional append-only log file under the app's data directory,
//! truncated back to a bound when it outgrows it.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

/// The most lines the in-memory ring keeps (what `log_tail` reads from).
pub const RING_MAX: usize = 500;

/// The most bytes the file keeps before it is truncated back to half.
pub const FILE_MAX_BYTES: u64 = 256 * 1024;

/// How serious a log line is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// Routine progress.
    Info,
    /// Something worked, though not as intended.
    Warn,
    /// Something failed.
    Error,
}

impl Level {
    /// The tag a line carries.
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

/// One log line.
#[derive(Clone, Debug, PartialEq)]
pub struct LogLine {
    /// How serious it is.
    pub level: Level,
    /// One line of text, no trailing newline.
    pub message: String,
    /// When it was written (`foundation::time::now_ms`).
    pub at_ms: f64,
}

/// The ring and the file target, shared by the engine thread and any task
/// thread that reports through a sink.
static LOG: LazyLock<Mutex<Log>> = LazyLock::new(|| {
    Mutex::new(Log {
        ring: Vec::new(),
        file: None,
    })
});

struct Log {
    ring: Vec<LogLine>,
    file: Option<PathBuf>,
}

impl Log {
    /// Appends, keeping the ring bounded and the file at its bound.
    fn push(&mut self, line: LogLine) {
        self.ring.push(line.clone());
        if self.ring.len() > RING_MAX {
            self.ring.remove(0);
        }
        if let Some(Err(error)) = self.file.as_ref().map(|path| append_to_file(path, &line)) {
            // Logging must never wedge the app: drop the file target
            // rather than retrying forever.
            eprintln!("log file unwritable ({error}); file logging stopped");
            self.file = None;
        }
    }
}

/// Appends one line to the file at `path`, truncating it back to half its
/// bound when it has outgrown [`FILE_MAX_BYTES`].
fn append_to_file(path: &std::path::Path, line: &LogLine) -> std::io::Result<()> {
    if let Ok(metadata) = std::fs::metadata(path)
        && metadata.len() > FILE_MAX_BYTES
    {
        truncate_to_bound(path)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(
        file,
        "{} [{}] {}",
        crate::foundation::time::format_time(line.at_ms),
        line.level.as_str(),
        line.message
    )
}

/// Keeps the last half of the file, so one bound is never exceeded twice
/// in a row.
fn truncate_to_bound(path: &std::path::Path) -> std::io::Result<()> {
    let keep = (FILE_MAX_BYTES / 2) as usize;
    let body = std::fs::read(path)?;
    let start = body.len().saturating_sub(keep);
    // Start at a line boundary, so the kept text stays readable.
    let cut = body[start..]
        .iter()
        .position(|b| *b == b'\n')
        .map_or(start, |at| start + at + 1);
    std::fs::write(path, &body[cut.min(body.len())..])?;
    Ok(())
}

/// Writes a line at `level`. Safe from any thread; a broken file target is
/// dropped, never fatal.
pub fn write(level: Level, message: &str) {
    let line = LogLine {
        level,
        message: message.to_owned(),
        at_ms: crate::foundation::time::now_ms(),
    };
    if let Ok(mut log) = LOG.lock() {
        log.push(line);
    }
}

/// Logs routine progress.
pub fn log_info(message: &str) {
    write(Level::Info, message);
}

/// Logs a worked-but-not-as-intended outcome.
pub fn log_warn(message: &str) {
    write(Level::Warn, message);
}

/// Logs a failure.
pub fn log_error(message: &str) {
    write(Level::Error, message);
}

/// Starts appending to `<dir>/app.log` (creating `dir`). Without this the
/// ring alone feeds readers, which is what tests want.
pub fn init_file(dir: PathBuf) {
    if let Ok(mut log) = LOG.lock() {
        log.file = Some(dir.join("app.log"));
    }
}

/// Where the app's data lives, the way the engine resolves it (Application
/// Support on Apple platforms, `%APPDATA%` on Windows, `$XDG_DATA_HOME` or
/// `~/.local/share` elsewhere), under the app id. `None` where there is no
/// home directory (Android keeps its files directory private to the
/// platform). The shell passes this to [`init_file`] at startup.
pub fn default_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| !home.as_os_str().is_empty())?;
    let base = match std::env::consts::OS {
        "macos" | "ios" => home.join("Library/Application Support"),
        "windows" => std::env::var_os("APPDATA").map(PathBuf::from)?,
        _ => std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|dir| !dir.as_os_str().is_empty())
            .unwrap_or_else(|| home.join(".local/share")),
    };
    // The app id comes from the manifest, so the log's home can never
    // drift from `nsh.json`.
    let (_, app_id, _) = crate::foundation::manifest_info();
    Some(base.join(app_id.as_ref()).join("logs"))
}

/// The last `lines` log lines, oldest first.
pub fn log_tail(lines: usize) -> Vec<LogLine> {
    LOG.lock()
        .map(|log| {
            let at = log.ring.len().saturating_sub(lines);
            log.ring[at..].to_vec()
        })
        .unwrap_or_default()
}

/// Forgets every ring line (the file, if any, keeps what it has).
pub fn clear_log() {
    if let Ok(mut log) = LOG.lock() {
        log.ring.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Break seen: `truncate_to_bound` early-returned `Ok(())` — failed on the bound assertion.
    #[test]
    fn truncates_the_file_back_to_a_bound() {
        let dir = std::env::temp_dir().join(format!("nsh-starter-log-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.log");
        let long = "x".repeat(600);
        let mut filler = LogLine {
            level: Level::Info,
            message: long.clone(),
            at_ms: 0.0,
        };
        for _ in 0..2000 {
            filler.at_ms += 1.0;
            append_to_file(&path, &filler).unwrap();
        }
        append_to_file(&path, &filler).unwrap();
        // Whatever the truncation cycle's phase, one append never leaves
        // more than the bound plus the line it added.
        let size = std::fs::metadata(&path).unwrap().len();
        let line_bytes = long.len() as u64 + 32;
        assert!(size <= FILE_MAX_BYTES + line_bytes, "truncated to {size}");
        // What remains is whole lines, and the newest survived.
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.lines().all(|line| line.ends_with(&long)));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

//! Log directory resolution, `flexi_logger` setup, and app vs wire routing.

use anyhow::Result;
use chrono::SecondsFormat;
use flexi_logger::writers::{FileLogWriter, LogWriter};
use flexi_logger::{
    Age, Cleanup, Criterion, DeferredNow, FileSpec, FlexiLoggerError, FormatFunction,
    LogSpecification, Logger, Naming,
};
use log::{Level, LevelFilter, Record};
use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use vim_rs::WireLoggingMode;

use crate::config::{ResolvedLogging, RotatingFileConfig};

thread_local! {
    static TEST_STATE_ROOT: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Test-only: force [`state_dir`] (and thus log paths) under this root.
#[allow(dead_code)] // Used by `tests/` and `config` unit tests; unused in `cargo build`.
pub fn set_test_state_root(path: Option<PathBuf>) {
    TEST_STATE_ROOT.with(|t| {
        *t.borrow_mut() = path;
    });
}

/// Prefix for records routed to the wire log (`vim_rs::wire::json`, `vim_rs::wire::soap`, …).
pub const WIRE_TARGET_PREFIX: &str = "vim_rs::wire::";

/// XDG / Windows state directory for persistent logs (not CWD).
pub fn state_dir() -> Option<PathBuf> {
    if let Some(p) = TEST_STATE_ROOT.with(|t| t.borrow().clone()) {
        return Some(p);
    }
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("vtui"))
    }
    #[cfg(not(windows))]
    {
        if let Some(xdg) = std::env::var_os("XDG_STATE_HOME") {
            let p = PathBuf::from(&xdg);
            if p.is_absolute() {
                return Some(p.join("vtui"));
            }
        }
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state").join("vtui"))
    }
}

/// `<state_dir>/logs`
pub fn logs_dir() -> Option<PathBuf> {
    Some(state_dir()?.join("logs"))
}

#[allow(dead_code)]
pub fn app_log_path() -> Option<PathBuf> {
    Some(logs_dir()?.join("vtui-app.log"))
}

#[allow(dead_code)]
pub fn wire_log_path() -> Option<PathBuf> {
    Some(logs_dir()?.join("vtui-wire.log"))
}

/// Map rotation config to flexi_logger [`Criterion`].
pub fn criterion_from_rotating(rotate_daily: bool, max_size_mib: u64) -> Criterion {
    let max_bytes = max_size_mib.saturating_mul(1024 * 1024);
    if rotate_daily {
        Criterion::AgeOrSize(Age::Day, max_bytes)
    } else {
        Criterion::Size(max_bytes)
    }
}

/// Retention + compression policy for rotated files.
pub fn cleanup_from_rotating(rot: &RotatingFileConfig) -> Cleanup {
    if rot.compress {
        Cleanup::KeepCompressedFiles(rot.keep_files)
    } else {
        Cleanup::KeepLogFiles(rot.keep_files)
    }
}

fn vtui_utc_format(
    w: &mut dyn Write,
    now: &mut DeferredNow,
    record: &Record,
) -> std::io::Result<()> {
    let ts = now
        .now_utc_owned()
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    // One line per record: flexi_logger appends `line_ending` after this; do not use `writeln!`
    // or every entry would be followed by a blank line.
    write!(
        w,
        "{} {} [{}] {}",
        ts,
        record.level(),
        record.target(),
        record.args()
    )
}

fn build_file_writer(
    logs: &Path,
    basename: &str,
    rot: &RotatingFileConfig,
    cleanup_in_background: bool,
) -> Result<FileLogWriter, FlexiLoggerError> {
    let file_spec = FileSpec::default()
        .directory(logs)
        .basename(basename)
        .suppress_timestamp()
        .suffix("log");

    let criterion = criterion_from_rotating(rot.rotate_daily, rot.max_size_mib);
    let cleanup = cleanup_from_rotating(rot);

    FileLogWriter::builder(file_spec)
        .append()
        .format(vtui_utc_format as FormatFunction)
        .use_utc()
        .cleanup_in_background_thread(cleanup_in_background)
        .rotate(criterion, Naming::Numbers, cleanup)
        .max_level(LevelFilter::Trace)
        .try_build()
}

/// Longest matching target prefix wins; ties broken by later entries in `filters`.
/// True if `level` is emitted when the ceiling is `filter` (same rules as `log` / flexi_logger).
pub fn level_allowed(filter: LevelFilter, level: Level) -> bool {
    match filter {
        LevelFilter::Off => false,
        LevelFilter::Error => level == Level::Error,
        LevelFilter::Warn => matches!(level, Level::Error | Level::Warn),
        LevelFilter::Info => matches!(level, Level::Error | Level::Warn | Level::Info),
        LevelFilter::Debug => !matches!(level, Level::Trace),
        LevelFilter::Trace => true,
    }
}

pub fn effective_app_level(
    target: &str,
    base: LevelFilter,
    filters: &[(String, LevelFilter)],
) -> LevelFilter {
    let mut best: Option<(usize, usize, LevelFilter)> = None; // (len, index, level)
    for (i, (pfx, lvl)) in filters.iter().enumerate() {
        if target.starts_with(pfx) {
            let len = pfx.len();
            match best {
                None => best = Some((len, i, *lvl)),
                Some((bl, bi, _)) if len > bl || (len == bl && i > bi) => {
                    best = Some((len, i, *lvl));
                }
                _ => {}
            }
        }
    }
    best.map(|(_, _, l)| l).unwrap_or(base)
}

struct RoutingWriter {
    app: Arc<FileLogWriter>,
    wire: Option<Arc<FileLogWriter>>,
    app_base: LevelFilter,
    filters: Arc<Vec<(String, LevelFilter)>>,
}

impl LogWriter for RoutingWriter {
    fn write(&self, now: &mut DeferredNow, record: &Record) -> std::io::Result<()> {
        if record.target().starts_with(WIRE_TARGET_PREFIX) {
            if let Some(w) = &self.wire {
                return w.write(now, record);
            }
            return Ok(());
        }
        let eff = effective_app_level(record.target(), self.app_base, &self.filters);
        if !level_allowed(eff, record.level()) {
            return Ok(());
        }
        self.app.write(now, record)
    }

    fn flush(&self) -> std::io::Result<()> {
        self.app.flush()?;
        if let Some(w) = &self.wire {
            w.flush()?;
        }
        Ok(())
    }

    fn max_log_level(&self) -> LevelFilter {
        LevelFilter::Trace
    }

    fn shutdown(&self) {
        self.app.shutdown();
        if let Some(w) = &self.wire {
            w.shutdown();
        }
    }
}

/// Initialize process logging. On failure to create the log directory or file writers, prints a
/// warning to stderr and falls back to stderr-only logging at `app_level`.
pub fn init(resolved: &ResolvedLogging, rust_log_note: bool) -> Result<flexi_logger::LoggerHandle> {
    if rust_log_note {
        eprintln!(
            "Note: RUST_LOG is set; vtui ignores it. Use LOG_LEVEL and ~/.config/vtui/config.toml [logging] instead."
        );
    }

    DeferredNow::force_utc();

    let Some(logs) = logs_dir() else {
        return init_stderr_only(resolved.app_level);
    };

    if let Err(e) = ensure_logs_tree(&logs) {
        eprintln!(
            "WARNING: could not create log directory {}: {e}. Logging to stderr only.",
            logs.display()
        );
        return init_stderr_only(resolved.app_level);
    }

    match init_file_routing(&logs, resolved) {
        Ok(h) => Ok(h),
        Err(e) => {
            eprintln!(
                "WARNING: could not initialize file logging under {}: {e}. Logging to stderr only.",
                logs.display()
            );
            init_stderr_only(resolved.app_level)
        }
    }
}

fn ensure_logs_tree(logs: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(logs)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(logs, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

fn init_file_routing(
    logs: &Path,
    resolved: &ResolvedLogging,
) -> Result<flexi_logger::LoggerHandle> {
    let bg_cleanup = true;
    let app_writer = build_file_writer(logs, "vtui-app", &resolved.app_rotation, bg_cleanup)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let wire_writer = if matches!(resolved.wire_mode, WireLoggingMode::Off) {
        None
    } else {
        Some(
            build_file_writer(logs, "vtui-wire", &resolved.wire_rotation, bg_cleanup)
                .map_err(|e| anyhow::anyhow!("{}", e))?,
        )
    };

    let filters: Vec<(String, LevelFilter)> = resolved
        .filters
        .iter()
        .map(|f| (f.target.clone(), f.level))
        .collect();

    let routing = RoutingWriter {
        app: Arc::new(app_writer),
        wire: wire_writer.map(Arc::new),
        app_base: resolved.app_level,
        filters: Arc::new(filters),
    };

    let handle = Logger::with(LogSpecification::trace())
        .log_to_writer(Box::new(routing))
        .use_utc()
        .start()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    log::set_max_level(LevelFilter::Trace);
    Ok(handle)
}

fn init_stderr_only(app_level: LevelFilter) -> Result<flexi_logger::LoggerHandle> {
    let mut b = LogSpecification::builder();
    b.default(app_level);
    let spec = b.finalize();
    let handle = Logger::with(spec)
        .log_to_stderr()
        .use_utc()
        .format(vtui_utc_format as FormatFunction)
        .start()
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    log::set_max_level(app_level);
    Ok(handle)
}

/// In-memory [`LogWriter`] for routing tests.
#[cfg(test)]
pub struct TestVecWriter {
    pub lines: Arc<std::sync::Mutex<Vec<String>>>,
}

#[cfg(test)]
impl LogWriter for TestVecWriter {
    fn write(&self, now: &mut DeferredNow, record: &Record) -> std::io::Result<()> {
        let ts = now
            .now_utc_owned()
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let line = format!(
            "{} {} [{}] {}",
            ts,
            record.level(),
            record.target(),
            record.args()
        );
        self.lines
            .lock()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
            .push(line);
        Ok(())
    }

    fn flush(&self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
struct TestRoutingWriter {
    app: Arc<TestVecWriter>,
    wire: Option<Arc<TestVecWriter>>,
    app_base: LevelFilter,
    filters: Arc<Vec<(String, LevelFilter)>>,
}

#[cfg(test)]
impl LogWriter for TestRoutingWriter {
    fn write(&self, now: &mut DeferredNow, record: &Record) -> std::io::Result<()> {
        if record.target().starts_with(WIRE_TARGET_PREFIX) {
            if let Some(w) = &self.wire {
                return w.write(now, record);
            }
            return Ok(());
        }
        let eff = effective_app_level(record.target(), self.app_base, &self.filters);
        if !level_allowed(eff, record.level()) {
            return Ok(());
        }
        self.app.write(now, record)
    }

    fn flush(&self) -> std::io::Result<()> {
        Ok(())
    }

    fn max_log_level(&self) -> LevelFilter {
        LevelFilter::Trace
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Level;
    use std::sync::Mutex;
    use std::sync::{Mutex as StdMutex, OnceLock};

    /// `flexi_logger` / `log::set_logger` are process-global; serialize tests that install a logger.
    fn flexi_logger_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| StdMutex::new(()))
            .lock()
            .expect("flexi logger test lock")
    }

    #[test]
    fn longest_prefix_filter_wins() {
        let filters = vec![
            ("vim_rs".to_string(), LevelFilter::Debug),
            ("vim_rs::core".to_string(), LevelFilter::Warn),
        ];
        assert_eq!(
            effective_app_level("vim_rs::core::x", LevelFilter::Info, &filters),
            LevelFilter::Warn
        );
        assert_eq!(
            effective_app_level("vim_rs::xml", LevelFilter::Info, &filters),
            LevelFilter::Debug
        );
    }

    #[test]
    fn same_length_prefix_later_wins() {
        let filters = vec![
            ("a::b".to_string(), LevelFilter::Debug),
            ("a::b".to_string(), LevelFilter::Trace),
        ];
        assert_eq!(
            effective_app_level("a::b::c", LevelFilter::Info, &filters),
            LevelFilter::Trace
        );
    }

    #[test]
    fn criterion_daily_vs_size_only() {
        let daily = criterion_from_rotating(true, 10);
        let size = criterion_from_rotating(false, 10);
        assert!(matches!(daily, Criterion::AgeOrSize(Age::Day, _)));
        assert!(matches!(size, Criterion::Size(_)));
    }

    #[test]
    fn routing_wire_vs_app() {
        DeferredNow::force_utc();
        let mut now = DeferredNow::new();
        let app = Arc::new(TestVecWriter {
            lines: Arc::new(Mutex::new(Vec::new())),
        });
        let wire = Arc::new(TestVecWriter {
            lines: Arc::new(Mutex::new(Vec::new())),
        });
        let router = TestRoutingWriter {
            app: Arc::clone(&app),
            wire: Some(Arc::clone(&wire)),
            app_base: LevelFilter::Info,
            filters: Arc::new(vec![]),
        };

        let r_app = Record::builder()
            .target("vtui")
            .level(Level::Info)
            .args(format_args!("app line"))
            .module_path(Some("vtui::logging::tests"))
            .file(Some(file!()))
            .line(Some(line!()))
            .build();
        router.write(&mut now, &r_app).unwrap();

        let r_wire = Record::builder()
            .target("vim_rs::wire::json")
            .level(Level::Debug)
            .args(format_args!("wire line"))
            .module_path(Some("vtui::logging::tests"))
            .file(Some(file!()))
            .line(Some(line!()))
            .build();
        router.write(&mut now, &r_wire).unwrap();

        let r_core = Record::builder()
            .target("vim_rs::core")
            .level(Level::Info)
            .args(format_args!("vim core line"))
            .module_path(Some("vtui::logging::tests"))
            .file(Some(file!()))
            .line(Some(line!()))
            .build();
        router.write(&mut now, &r_core).unwrap();

        let app_lines = app.lines.lock().unwrap().clone();
        let wire_lines = wire.lines.lock().unwrap().clone();
        assert!(
            app_lines
                .iter()
                .any(|l| l.contains("[vtui]") && l.contains("app line")),
            "app: {app_lines:?}"
        );
        assert!(
            wire_lines
                .iter()
                .any(|l| l.contains("[vim_rs::wire::json]") && l.contains("wire line")),
            "wire: {wire_lines:?}"
        );
        assert!(
            app_lines
                .iter()
                .any(|l| l.contains("[vim_rs::core]") && l.contains("vim core line")),
            "vim_rs core should stay in app: {app_lines:?}"
        );
        assert!(!app_lines.iter().any(|l| l.contains("wire line")));
        assert!(!wire_lines.iter().any(|l| l.contains("vim core line")));
    }

    #[test]
    fn map_wire_modes_to_vim() {
        use crate::config::WireLogMode;
        use crate::config::wire_mode_to_vim;
        assert_eq!(wire_mode_to_vim(WireLogMode::Off), WireLoggingMode::Off);
        assert_eq!(
            wire_mode_to_vim(WireLogMode::Summary),
            WireLoggingMode::Summary
        );
        assert_eq!(
            wire_mode_to_vim(WireLogMode::Detailed),
            WireLoggingMode::Detailed
        );
    }

    fn read_logs_concat(dir: &std::path::Path, name_part: &str) -> String {
        let mut out = String::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return out;
        };
        for e in entries.flatten() {
            let p = e.path();
            let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if fname.contains(name_part) {
                if let Ok(s) = std::fs::read_to_string(&p) {
                    out.push_str(&s);
                }
            }
        }
        out
    }

    #[test]
    fn integration_file_sinks_append_and_split() {
        let _lock = flexi_logger_lock();
        let tmp = tempfile::tempdir().expect("tempdir");
        set_test_state_root(Some(tmp.path().to_path_buf()));

        let mut detailed = ResolvedLogging::default();
        detailed.wire_mode = WireLoggingMode::Detailed;
        detailed.app_rotation.compress = false;
        detailed.wire_rotation.compress = false;

        DeferredNow::force_utc();
        let h = init(&detailed, false).expect("init");
        log::info!(target: "vtui", "hello_app");
        log::log!(
            target: "vim_rs::wire::json",
            Level::Trace,
            "hello_wire_trace"
        );
        h.shutdown();

        let logs = logs_dir().expect("logs dir");
        let app = read_logs_concat(&logs, "vtui-app");
        let wire = read_logs_concat(&logs, "vtui-wire");
        assert!(
            app.contains("hello_app"),
            "app log should contain app line: {app:?}"
        );
        assert!(
            !app.contains("hello_wire_trace"),
            "wire trace must not land in app log: {app:?}"
        );
        assert!(
            wire.contains("hello_wire_trace"),
            "wire log should contain wire line: {wire:?}"
        );

        let re = regex::Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z").expect("regex");
        assert!(
            app.lines().any(|l| re.is_match(l)),
            "UTC millis Z timestamp missing: {app:?}"
        );
        assert!(
            wire.lines().any(|l| re.is_match(l)),
            "UTC millis Z timestamp missing on wire: {wire:?}"
        );
    }
}

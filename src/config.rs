//! Multi-environment TOML config, env overrides, password commands, and interactive prompt.

use anyhow::{Context, Result, bail, ensure};
use log::LevelFilter;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use vim_rs::WireLoggingMode;

#[derive(Deserialize, Default)]
pub struct ConfigFile {
    pub default_env: Option<String>,
    #[serde(default)]
    pub environments: HashMap<String, EnvironmentConfig>,
    #[serde(default)]
    pub logging: Option<RawLogging>,
}

#[derive(Deserialize, Clone)]
pub struct EnvironmentConfig {
    pub server: String,
    pub username: String,
    pub password: Option<String>,
    pub password_cmd: Option<String>,
    #[serde(default)]
    pub insecure: bool,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    pub log_level: Option<String>,
}

fn default_protocol() -> String {
    "auto".into()
}

/// Wire capture mode in config (`[logging.wire]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireLogMode {
    Off,
    Summary,
    Detailed,
}

/// Rotation / retention for one log file (app or wire).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotatingFileConfig {
    pub rotate_daily: bool,
    pub max_size_mib: u64,
    pub keep_files: usize,
    pub compress: bool,
}

#[derive(Debug, Clone)]
pub struct TargetLogFilter {
    pub target: String,
    pub level: LevelFilter,
}

/// Fully resolved logging settings for this process.
#[derive(Debug, Clone)]
pub struct ResolvedLogging {
    pub app_level: LevelFilter,
    pub wire_mode: WireLoggingMode,
    pub app_rotation: RotatingFileConfig,
    pub wire_rotation: RotatingFileConfig,
    pub filters: Vec<TargetLogFilter>,
}

impl Default for ResolvedLogging {
    fn default() -> Self {
        Self {
            app_level: LevelFilter::Info,
            wire_mode: WireLoggingMode::Off,
            app_rotation: RotatingFileConfig {
                rotate_daily: true,
                max_size_mib: 10,
                keep_files: 21,
                compress: true,
            },
            wire_rotation: RotatingFileConfig {
                rotate_daily: true,
                max_size_mib: 1024,
                keep_files: 2,
                compress: true,
            },
            filters: Vec::new(),
        }
    }
}

pub struct ResolvedConfig {
    pub server: String,
    pub username: String,
    pub password: String,
    pub insecure: bool,
    pub protocol: String,
    pub logging: ResolvedLogging,
}

pub enum CliAction {
    Connect(ResolvedConfig),
    ListEnvironments { path: PathBuf, config: ConfigFile },
    ShowHelp,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct RawLogging {
    pub level: Option<String>,
    #[serde(default)]
    pub app: RawAppRotation,
    #[serde(default)]
    pub wire: RawWireSection,
    #[serde(default)]
    pub filters: Vec<RawTargetFilter>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawAppRotation {
    #[serde(default = "default_true")]
    pub rotate_daily: bool,
    #[serde(default = "default_app_mib")]
    pub max_size_mib: u64,
    #[serde(default = "default_app_keep")]
    pub keep_files: usize,
    #[serde(default = "default_true")]
    pub compress: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawWireSection {
    #[serde(default = "default_wire_mode_str")]
    pub mode: String,
    #[serde(default)]
    pub rotate_daily: bool,
    #[serde(default = "default_wire_mib")]
    pub max_size_mib: u64,
    #[serde(default = "default_wire_keep")]
    pub keep_files: usize,
    #[serde(default = "default_true")]
    pub compress: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawTargetFilter {
    pub target: String,
    pub level: String,
}

fn default_true() -> bool {
    true
}

fn default_app_mib() -> u64 {
    10
}

fn default_app_keep() -> usize {
    21
}

fn default_wire_mib() -> u64 {
    1024
}

fn default_wire_keep() -> usize {
    2
}

fn default_wire_mode_str() -> String {
    "off".into()
}

impl Default for RawAppRotation {
    fn default() -> Self {
        Self {
            rotate_daily: true,
            max_size_mib: default_app_mib(),
            keep_files: default_app_keep(),
            compress: true,
        }
    }
}

impl Default for RawWireSection {
    fn default() -> Self {
        Self {
            mode: default_wire_mode_str(),
            rotate_daily: true,
            max_size_mib: default_wire_mib(),
            keep_files: default_wire_keep(),
            compress: true,
        }
    }
}

#[derive(Debug)]
enum CliParse {
    Run { env_name: Option<String> },
    List,
    Help,
}

fn parse_cli_args() -> Result<CliParse> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [] => Ok(CliParse::Run { env_name: None }),
        [a] if a == "--help" || a == "-h" => Ok(CliParse::Help),
        [a] if a == "--list" || a == "-l" => Ok(CliParse::List),
        [name] if !name.starts_with('-') => Ok(CliParse::Run {
            env_name: Some(name.clone()),
        }),
        _ => bail!(
            "Unexpected arguments: {}. Try `vtui --help`.",
            args.join(" ")
        ),
    }
}

fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var_os("APPDATA").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }
}

pub fn config_path() -> Option<PathBuf> {
    Some(config_dir()?.join("vtui").join("config.toml"))
}

fn load_config_file(path: &Path) -> Result<ConfigFile> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file {}", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("Failed to parse {}", path.display()))
}

fn shell_command(cmd: &str) -> std::process::Command {
    #[cfg(windows)]
    {
        let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
        let mut c = std::process::Command::new(comspec);
        c.args(["/C", cmd]);
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    }
}

fn execute_password_cmd(cmd: &str) -> Result<String> {
    let output = shell_command(cmd)
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("Failed to execute password command: {cmd}"))?;

    if !output.status.success() {
        bail!("Password command `{cmd}` exited with {}", output.status);
    }

    let pwd = String::from_utf8(output.stdout)
        .context("Password command output is not valid UTF-8")?
        .trim_end_matches(['\r', '\n'])
        .to_string();

    ensure!(
        !pwd.is_empty(),
        "Password command `{cmd}` returned empty output"
    );
    Ok(pwd)
}

fn prompt_password(server: &str, username: &str) -> Result<String> {
    eprintln!("Connecting to {server} as {username}");
    let pwd = rpassword::prompt_password("Password: ").context("Failed to read password")?;
    ensure!(!pwd.is_empty(), "Empty password");
    Ok(pwd)
}

fn resolve_password(
    server: &str,
    username: &str,
    cfg_password: Option<&str>,
    cfg_password_cmd: Option<&str>,
) -> Result<String> {
    if let Ok(p) = env::var("VIM_PASSWORD") {
        ensure!(!p.is_empty(), "VIM_PASSWORD is set but empty");
        return Ok(p);
    }
    if let Ok(cmd) = env::var("VIM_PWD_CMD") {
        return execute_password_cmd(&cmd);
    }
    if let Some(p) = cfg_password {
        ensure!(!p.is_empty(), "password in config is empty");
        return Ok(p.to_string());
    }
    if let Some(cmd) = cfg_password_cmd {
        return execute_password_cmd(cmd);
    }
    prompt_password(server, username)
}

#[cfg(unix)]
fn warn_loose_config_permissions(path: &Path, has_plaintext_password: bool) {
    use std::os::unix::fs::PermissionsExt;
    if !has_plaintext_password {
        return;
    }
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    let mode = meta.permissions().mode();
    if mode & 0o077 != 0 {
        eprintln!(
            "WARNING: {} contains plaintext passwords but is readable by group or others (mode {:o}).\n\
             Run: chmod 600 {}",
            path.display(),
            mode & 0o777,
            path.display()
        );
    }
}

#[cfg(not(unix))]
fn warn_loose_config_permissions(_path: &Path, _has_plaintext_password: bool) {}

fn parse_transport(protocol: &str) -> Result<&str> {
    match protocol {
        "auto" | "json" | "soap" => Ok(protocol),
        _ => bail!("Invalid protocol: {protocol} (expected auto, json, soap)"),
    }
}

fn parse_level_filter(name: &str) -> Result<LevelFilter> {
    match name.to_ascii_lowercase().as_str() {
        "trace" => Ok(LevelFilter::Trace),
        "debug" => Ok(LevelFilter::Debug),
        "info" => Ok(LevelFilter::Info),
        "warn" => Ok(LevelFilter::Warn),
        "error" => Ok(LevelFilter::Error),
        "off" => Ok(LevelFilter::Off),
        _ => bail!("invalid log level: {name}"),
    }
}

fn parse_level_filter_opt(s: &str) -> Option<LevelFilter> {
    parse_level_filter(s).ok()
}

fn parse_wire_mode(s: &str) -> Result<WireLogMode> {
    match s.to_ascii_lowercase().as_str() {
        "off" => Ok(WireLogMode::Off),
        "summary" => Ok(WireLogMode::Summary),
        "detailed" => Ok(WireLogMode::Detailed),
        _ => bail!("invalid logging.wire.mode: {s} (expected off, summary, detailed)"),
    }
}

pub(crate) fn wire_mode_to_vim(m: WireLogMode) -> WireLoggingMode {
    match m {
        WireLogMode::Off => WireLoggingMode::Off,
        WireLogMode::Summary => WireLoggingMode::Summary,
        WireLogMode::Detailed => WireLoggingMode::Detailed,
    }
}

/// Resolve `[logging]` + `LOG_LEVEL` + optional legacy per-environment `log_level`.
pub fn resolve_logging(
    raw: Option<&RawLogging>,
    legacy_env_log_level: Option<&str>,
) -> Result<ResolvedLogging> {
    let mut out = ResolvedLogging::default();

    let raw = raw.cloned().unwrap_or_default();

    // --- app level (LOG_LEVEL > logging.level > legacy env log_level > info)
    let mut from_env = false;
    match env::var("LOG_LEVEL") {
        Ok(s) if s.is_empty() => {
            eprintln!("WARNING: LOG_LEVEL is set but empty; ignoring.");
        }
        Ok(s) => match parse_level_filter_opt(&s) {
            Some(lvl) => {
                out.app_level = lvl;
                from_env = true;
            }
            None => eprintln!("WARNING: invalid LOG_LEVEL={s:?}; ignoring."),
        },
        Err(_) => {}
    }

    if !from_env {
        if let Some(ref ls) = raw.level {
            out.app_level =
                parse_level_filter(ls).with_context(|| format!("invalid logging.level: {ls}"))?;
        } else if let Some(leg) = legacy_env_log_level {
            if let Some(lvl) = parse_level_filter_opt(leg) {
                eprintln!(
                    "DEPRECATION: [environments.*].log_level is deprecated; move to a global [logging] section with level = \"{}\" (see README).",
                    match leg.to_ascii_lowercase().as_str() {
                        "trace" => "trace",
                        "debug" => "debug",
                        "info" => "info",
                        "warn" => "warn",
                        "error" => "error",
                        "off" => "off",
                        _ => "info",
                    }
                );
                out.app_level = lvl;
            } else {
                bail!("invalid legacy log_level in environment profile: {leg}");
            }
        }
    }

    let wm = parse_wire_mode(&raw.wire.mode)?;
    out.wire_mode = wire_mode_to_vim(wm);

    out.app_rotation = RotatingFileConfig {
        rotate_daily: raw.app.rotate_daily,
        max_size_mib: raw.app.max_size_mib,
        keep_files: raw.app.keep_files,
        compress: raw.app.compress,
    };

    out.wire_rotation = RotatingFileConfig {
        rotate_daily: raw.wire.rotate_daily,
        max_size_mib: raw.wire.max_size_mib,
        keep_files: raw.wire.keep_files,
        compress: raw.wire.compress,
    };

    out.filters.clear();
    for f in &raw.filters {
        if f.target.is_empty() {
            bail!("logging.filters entry has empty target");
        }
        let lvl = parse_level_filter(&f.level).with_context(|| {
            format!("invalid level in logging.filters for target {:?}", f.target)
        })?;
        out.filters.push(TargetLogFilter {
            target: f.target.clone(),
            level: lvl,
        });
    }

    Ok(out)
}

fn merge_and_resolve(
    base: Option<&EnvironmentConfig>,
    file: Option<&ConfigFile>,
    config_path_for_warn: Option<&Path>,
) -> Result<ResolvedConfig> {
    let server = env::var("VIM_SERVER")
        .ok()
        .or_else(|| base.map(|e| e.server.clone()))
        .with_context(
            || "VIM_SERVER is not set and no server was found in the selected environment",
        )?;

    let username = env::var("VIM_USERNAME")
        .ok()
        .or_else(|| base.map(|e| e.username.clone()))
        .with_context(
            || "VIM_USERNAME is not set and no username was found in the selected environment",
        )?;

    let insecure = match env::var("VIM_INSECURE") {
        Ok(s) => s != "false",
        Err(_) => base.map(|e| e.insecure).unwrap_or(false),
    };

    let protocol = env::var("VIM_PROTOCOL").unwrap_or_else(|_| {
        base.map(|e| e.protocol.clone())
            .unwrap_or_else(default_protocol)
    });

    let protocol = parse_transport(&protocol)
        .context("transport protocol")?
        .to_string();

    let legacy = base.and_then(|e| e.log_level.as_deref());
    let logging = resolve_logging(file.and_then(|f| f.logging.as_ref()), legacy)?;

    if let (Some(path), Some(b)) = (config_path_for_warn, base) {
        warn_loose_config_permissions(path, b.password.is_some());
    }

    let password = resolve_password(
        &server,
        &username,
        base.and_then(|e| e.password.as_deref()),
        base.and_then(|e| e.password_cmd.as_deref()),
    )?;

    Ok(ResolvedConfig {
        server,
        username,
        password,
        insecure,
        protocol,
        logging,
    })
}

fn resolve_run(env_name: Option<String>) -> Result<CliAction> {
    let path_opt = config_path();

    let loaded: Option<ConfigFile> = path_opt
        .as_ref()
        .filter(|p| p.exists())
        .map(|p| load_config_file(p))
        .transpose()?;

    match env_name.as_deref() {
        Some(name) => {
            let Some(path) = path_opt else {
                bail!("Cannot determine config directory");
            };
            if !path.exists() {
                bail!(
                    "Config file not found at {}.\n\
                     Create ~/.config/vtui/config.toml (or %APPDATA%\\vtui\\config.toml on Windows) \\
                     or set VIM_* environment variables.",
                    path.display()
                );
            }
            let file = loaded.as_ref().expect("loaded when path exists");
            let env_cfg = file.environments.get(name).with_context(|| {
                format!(
                    "Environment '{}' not found. Available: {}",
                    name,
                    if file.environments.is_empty() {
                        "(none)".into()
                    } else {
                        let mut k: Vec<_> = file.environments.keys().cloned().collect();
                        k.sort();
                        k.join(", ")
                    }
                )
            })?;
            let resolved = merge_and_resolve(Some(env_cfg), Some(file), Some(path.as_path()))?;
            Ok(CliAction::Connect(resolved))
        }
        None => {
            if let Some(file) = loaded.as_ref()
                && let Some(def) = file.default_env.as_ref()
            {
                let path = path_opt.as_ref().expect("file implies path");
                let env_cfg = file.environments.get(def).with_context(|| {
                    format!(
                        "default_env '{}' not found. Available: {}",
                        def,
                        if file.environments.is_empty() {
                            "(none)".into()
                        } else {
                            let mut k: Vec<_> = file.environments.keys().cloned().collect();
                            k.sort();
                            k.join(", ")
                        }
                    )
                })?;
                let resolved = merge_and_resolve(Some(env_cfg), Some(file), Some(path.as_path()))?;
                return Ok(CliAction::Connect(resolved));
            }
            let resolved = merge_and_resolve(None, loaded.as_ref(), path_opt.as_deref())?;
            Ok(CliAction::Connect(resolved))
        }
    }
}

/// Resolve CLI + config + env into a connect action, list, or help.
pub fn resolve() -> Result<CliAction> {
    match parse_cli_args()? {
        CliParse::Help => Ok(CliAction::ShowHelp),
        CliParse::List => {
            let Some(path) = config_path() else {
                bail!("Cannot determine config directory (HOME / APPDATA / XDG_CONFIG_HOME)");
            };
            if !path.exists() {
                bail!(
                    "Config file not found at {}.\n\
                     Create it or set VIM_* environment variables for legacy mode.",
                    path.display()
                );
            }
            let config = load_config_file(&path)?;
            Ok(CliAction::ListEnvironments { path, config })
        }
        CliParse::Run { env_name } => resolve_run(env_name),
    }
}

pub fn print_environment_list(path: &Path, config: &ConfigFile) {
    println!("Environments (from {}):", path.display());
    if config.environments.is_empty() {
        println!("  (none defined)");
        return;
    }
    let mut names: Vec<_> = config.environments.keys().cloned().collect();
    names.sort();
    for name in names {
        let e = &config.environments[&name];
        let default = config
            .default_env
            .as_deref()
            .is_some_and(|d| d == name.as_str());
        let suffix = if default { "  [default]" } else { "" };
        println!("  {name}  {}  {}{suffix}", e.server, e.username);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_log_level<T>(val: Option<&str>, f: impl FnOnce() -> T) -> T {
        let _g = ENV_LOCK.lock().expect("env lock");
        let prev = std::env::var("LOG_LEVEL").ok();
        unsafe {
            match val {
                Some(v) => std::env::set_var("LOG_LEVEL", v),
                None => std::env::remove_var("LOG_LEVEL"),
            }
        }
        let out = f();
        unsafe {
            match prev {
                Some(p) => std::env::set_var("LOG_LEVEL", p),
                None => std::env::remove_var("LOG_LEVEL"),
            }
        }
        out
    }

    fn with_extra_env<T>(vars: &[(&str, Option<&str>)], f: impl FnOnce() -> T) -> T {
        let _g = ENV_LOCK.lock().expect("env lock");
        let mut saved: Vec<(String, Option<String>)> = Vec::new();
        for (k, v) in vars {
            saved.push((
                (*k).to_string(),
                std::env::var_os(*k).map(|o| o.to_string_lossy().into_owned()),
            ));
            unsafe {
                match v {
                    Some(val) => std::env::set_var(k, val),
                    None => std::env::remove_var(k),
                }
            }
        }
        let out = f();
        for (k, prev) in saved {
            unsafe {
                match prev {
                    Some(p) => std::env::set_var(&k, p),
                    None => std::env::remove_var(&k),
                }
            }
        }
        out
    }

    #[test]
    fn parse_sample_toml() {
        let s = r#"
default_env = "prod"

[environments.prod]
server = "vc.example"
username = "u@local"
password_cmd = "echo secret"
insecure = false
protocol = "json"
log_level = "warn"

[environments.lab]
server = "10.0.0.1"
username = "root"
insecure = true
"#;
        let c: ConfigFile = toml::from_str(s).unwrap();
        assert_eq!(c.default_env.as_deref(), Some("prod"));
        assert_eq!(c.environments["prod"].server, "vc.example");
        assert_eq!(c.environments["lab"].protocol, "auto");
        assert!(!c.environments["prod"].insecure);
        assert!(c.environments["lab"].insecure);
    }

    #[test]
    fn transport_validation() {
        assert!(parse_transport("auto").is_ok());
        assert!(parse_transport("bad").is_err());
    }

    #[test]
    fn log_level_env_overrides_file() {
        let raw: RawLogging = toml::from_str(
            r#"
level = "warn"
[app]
[wire]
"#,
        )
        .unwrap();
        with_log_level(Some("debug"), || {
            let r = resolve_logging(Some(&raw), None).unwrap();
            assert_eq!(r.app_level, LevelFilter::Debug);
        });
    }

    #[test]
    fn log_level_env_does_not_change_wire_mode() {
        let raw: RawLogging = toml::from_str(
            r#"
level = "info"
[app]
[wire]
mode = "summary"
"#,
        )
        .unwrap();
        with_log_level(Some("trace"), || {
            let r = resolve_logging(Some(&raw), None).unwrap();
            assert_eq!(r.wire_mode, WireLoggingMode::Summary);
        });
    }

    #[test]
    fn invalid_log_level_env_warn_continues() {
        let raw: RawLogging = toml::from_str("level = \"info\"\n[app]\n[wire]\n").unwrap();
        with_log_level(Some("bogus"), || {
            let r = resolve_logging(Some(&raw), None).unwrap();
            assert_eq!(r.app_level, LevelFilter::Info);
        });
    }

    #[test]
    fn empty_log_level_env_warns() {
        let raw: RawLogging = toml::from_str("level = \"debug\"\n[app]\n[wire]\n").unwrap();
        with_log_level(Some(""), || {
            let r = resolve_logging(Some(&raw), None).unwrap();
            assert_eq!(r.app_level, LevelFilter::Debug);
        });
    }

    #[test]
    fn legacy_env_log_level_migration() {
        let raw: RawLogging = toml::from_str("[app]\n[wire]\n").unwrap();
        with_log_level(None, || {
            let r = resolve_logging(Some(&raw), Some("debug")).unwrap();
            assert_eq!(r.app_level, LevelFilter::Debug);
        });
    }

    #[test]
    fn longest_prefix_override() {
        let raw: RawLogging = toml::from_str(
            r#"
level = "info"
[app]
[wire]
[[filters]]
target = "vim_rs::core"
level = "debug"
[[filters]]
target = "vim_rs"
level = "warn"
"#,
        )
        .unwrap();
        let r = resolve_logging(Some(&raw), None).unwrap();
        use crate::logging::effective_app_level;
        let pairs: Vec<(String, LevelFilter)> = r
            .filters
            .iter()
            .map(|f| (f.target.clone(), f.level))
            .collect();
        assert_eq!(
            effective_app_level("vim_rs::core::x", r.app_level, &pairs),
            LevelFilter::Debug
        );
        assert_eq!(
            effective_app_level("vim_rs::xml", r.app_level, &pairs),
            LevelFilter::Warn
        );
    }

    #[test]
    fn invalid_wire_mode_errors() {
        let raw: RawLogging = toml::from_str(
            r#"
[app]
[wire]
mode = "banana"
"#,
        )
        .unwrap();
        assert!(resolve_logging(Some(&raw), None).is_err());
    }

    #[test]
    fn invalid_filter_level_errors() {
        let raw: RawLogging = toml::from_str(
            r#"
[app]
[wire]
[[filters]]
target = "x"
level = "nope"
"#,
        )
        .unwrap();
        assert!(resolve_logging(Some(&raw), None).is_err());
    }

    #[test]
    fn empty_filter_target_errors() {
        let raw: RawLogging = toml::from_str(
            r#"
[app]
[wire]
[[filters]]
target = ""
level = "debug"
"#,
        )
        .unwrap();
        assert!(resolve_logging(Some(&raw), None).is_err());
    }

    #[test]
    fn default_app_policy_matches_spec() {
        let r = resolve_logging(None, None).unwrap();
        assert_eq!(r.app_level, LevelFilter::Info);
        assert!(r.app_rotation.rotate_daily);
        assert_eq!(r.app_rotation.max_size_mib, 10);
        assert_eq!(r.app_rotation.keep_files, 21);
        assert!(r.app_rotation.compress);
    }

    #[test]
    fn default_wire_policy_matches_spec() {
        let r = resolve_logging(None, None).unwrap();
        assert_eq!(r.wire_mode, WireLoggingMode::Off);
        assert!(r.wire_rotation.rotate_daily);
        assert_eq!(r.wire_rotation.max_size_mib, 1024);
        assert_eq!(r.wire_rotation.keep_files, 2);
        assert!(r.wire_rotation.compress);
    }

    #[test]
    fn omitted_logging_sections_use_defaults() {
        let c: ConfigFile =
            toml::from_str("default_env=\"a\"\n[environments.a]\nserver=\"s\"\nusername=\"u\"\n")
                .unwrap();
        assert!(c.logging.is_none());
        let r = resolve_logging(c.logging.as_ref(), None).unwrap();
        assert_eq!(r.app_level, LevelFilter::Info);
    }

    #[test]
    fn test_state_dir_override() {
        use crate::logging;
        let tmp = std::env::temp_dir().join("vtui-state-override");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        logging::set_test_state_root(Some(tmp.join("forced")));
        assert!(logging::state_dir().unwrap().ends_with("forced"));
        logging::set_test_state_root(None);
    }

    #[test]
    fn relative_xdg_state_home_is_ignored() {
        use crate::logging;
        let _g = ENV_LOCK.lock().expect("env lock");
        let tmp = std::env::temp_dir().join("vtui-xdg-rel");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let home = tmp.join("home");
        fs::create_dir_all(&home).unwrap();
        let prev_xdg = std::env::var_os("XDG_STATE_HOME");
        let prev_home = std::env::var_os("HOME");
        unsafe {
            std::env::set_var("XDG_STATE_HOME", "relative/path");
            std::env::set_var("HOME", home.as_os_str());
        }
        logging::set_test_state_root(None);
        let expected = home.join(".local").join("state").join("vtui");
        assert_eq!(logging::state_dir().unwrap(), expected);
        unsafe {
            match prev_xdg {
                Some(p) => std::env::set_var("XDG_STATE_HOME", p),
                None => std::env::remove_var("XDG_STATE_HOME"),
            }
            match prev_home {
                Some(p) => std::env::set_var("HOME", p),
                None => std::env::remove_var("HOME"),
            }
        }
    }

    #[test]
    fn rust_log_does_not_change_resolution() {
        let raw: RawLogging = toml::from_str("level = \"info\"\n[app]\n[wire]\n").unwrap();
        with_extra_env(&[("RUST_LOG", Some("trace"))], || {
            let r = resolve_logging(Some(&raw), None).unwrap();
            assert_eq!(r.app_level, LevelFilter::Info);
        });
    }
}

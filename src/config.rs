//! Multi-environment TOML config, env overrides, password commands, and interactive prompt.

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

#[derive(Deserialize, Default)]
pub struct ConfigFile {
    pub default_env: Option<String>,
    #[serde(default)]
    pub environments: HashMap<String, EnvironmentConfig>,
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

pub struct ResolvedConfig {
    pub server: String,
    pub username: String,
    pub password: String,
    pub insecure: bool,
    pub protocol: String,
    pub log_level: String,
}

pub enum CliAction {
    Connect(ResolvedConfig),
    ListEnvironments { path: PathBuf, config: ConfigFile },
    ShowHelp,
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
        [name] if !name.starts_with("-") => Ok(CliParse::Run {
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

fn merge_and_resolve(
    base: Option<&EnvironmentConfig>,
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

    let log_level = env::var("LOG_LEVEL")
        .ok()
        .or_else(|| base.and_then(|e| e.log_level.clone()))
        .unwrap_or_else(|| "info".into());

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
        log_level,
    })
}

fn resolve_run(env_name: Option<String>) -> Result<CliAction> {
    let path_opt = config_path();

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
            let file = load_config_file(&path)?;
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
            let resolved = merge_and_resolve(Some(env_cfg), Some(path.as_path()))?;
            Ok(CliAction::Connect(resolved))
        }
        None => {
            if let Some(ref path) = path_opt
                && path.exists()
            {
                let file = load_config_file(path)?;
                if let Some(ref def) = file.default_env {
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
                    let resolved = merge_and_resolve(Some(env_cfg), Some(path.as_path()))?;
                    return Ok(CliAction::Connect(resolved));
                }
            }
            let resolved = merge_and_resolve(None, None)?;
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
}

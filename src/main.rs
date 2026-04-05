use crate::event::EventHandler;
use anyhow::{Context, Result};
use app::App;
use log::{LevelFilter, info};
use simplelog::{Config, WriteLogger};
use std::cell::RefCell;
use std::fs::File;
use std::path::Path;
use std::rc::Rc;
use vim_rs::core::client::{ClientBuilder, TransportMode, VimClientHandle};
use vim_rs::core::pc_cache::CacheManager;

mod app;
mod body_pane;
mod config;
mod event;
mod hints;
mod history;
mod inventory_path;
mod perf_worker;
mod prop_browser;
mod resource_browser;
mod resource_type;
mod search;
mod vm_action_ui;
mod vm_power_actions;

#[allow(clippy::await_holding_refcell_ref)]
#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from a local `.env` file if present.
    // This is optional; real environment variables still take precedence.
    let _ = dotenvy::dotenv();

    let resolved = match config::resolve() {
        Ok(config::CliAction::ShowHelp) => {
            print_usage();
            return Ok(());
        }
        Ok(config::CliAction::ListEnvironments { path, config }) => {
            config::print_environment_list(&path, &config);
            return Ok(());
        }
        Ok(config::CliAction::Connect(c)) => c,
        Err(err) => {
            print_usage();
            eprintln!("Error: {:#}", err);
            return Err(err);
        }
    };

    setup_logging(&resolved.log_level)?;

    info!("Starting vtui application!");

    let client = match init_vim_client(&resolved).await {
        Ok(client) => client,
        Err(err) => {
            print_usage();
            eprintln!("Error initializing client: {}", err);
            return Err(err);
        }
    };
    let cache_manager = Rc::new(RefCell::new(CacheManager::new(client.clone())?));
    let monitor = cache_manager.borrow().create_monitor()?;
    let event_handler = EventHandler::new(monitor);
    let terminal = ratatui::init();

    let app_result = App::new(event_handler, cache_manager.clone(), client.clone())
        .await?
        .run(terminal)
        .await;
    ratatui::restore();
    cache_manager.borrow_mut().destroy().await?;
    app_result
}

async fn init_vim_client(cfg: &config::ResolvedConfig) -> Result<VimClientHandle> {
    let transport = match cfg.protocol.as_str() {
        "auto" => TransportMode::Auto,
        "json" => TransportMode::Json,
        "soap" => TransportMode::Soap,
        _ => return Err(anyhow::anyhow!("Invalid protocol: {}", cfg.protocol)),
    };

    let client = ClientBuilder::new(cfg.server.as_str())
        .insecure(cfg.insecure)
        .basic_authn(cfg.username.as_str(), cfg.password.as_str())
        .app_details(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
        .transport(transport)
        .build()
        .await
        .with_context(|| format!("Failed to connect to {}", cfg.server))?;
    Ok(client)
}

fn print_usage() {
    println!("Usage: vtui [ENV_NAME | --list | --help]");
    println!();
    println!("  vtui              Use default_env from config, or VIM_* environment variables");
    println!("  vtui ENV_NAME     Use the named environment from the config file");
    println!("  vtui --list, -l   List configured environments and exit");
    println!("  vtui --help, -h   Show this help and exit");
    println!();
    println!("Config file (optional):");
    if let Some(p) = config::config_path() {
        println!("  {}", p.display());
    } else {
        println!("  (see XDG_CONFIG_HOME, HOME/.config, or APPDATA on Windows)");
    }
    println!();
    println!("Environment variables (highest precedence; override config file):");
    println!("VIM_SERVER: vCenter or ESXi address (FQDN or IP)");
    println!("VIM_USERNAME: vSphere login");
    println!("VIM_PASSWORD: password (optional if VIM_PWD_CMD, config password_cmd, or prompt)");
    println!("VIM_PWD_CMD: shell command whose stdout is the password (e.g. 1Password CLI)");
    println!(
        "VIM_INSECURE: if set, only 'false' verifies TLS; other values skip verification; if unset, use config or verify (env-only)"
    );
    println!("VIM_PROTOCOL: auto, json, or soap (default: auto)");
    println!(
        "LOG_LEVEL: trace, debug, info, warn, error, off (default: info). Use 'trace' for wire logging."
    );
    println!();
    println!(
        "A `.env` file in the current or a parent directory can set variables; process env wins."
    );
}

fn setup_logging(level: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all("logs")?;

    let log_file_path = Path::new("logs/vtui.log");

    WriteLogger::init(
        parse_log_level(level),
        Config::default(),
        File::create(log_file_path)?,
    )
    .map_err(|e| anyhow::anyhow!("Failed to initialize logger: {}", e))?;

    info!("Logging system initialized");
    Ok(())
}

fn parse_log_level(level: &str) -> LevelFilter {
    match level {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        "off" => LevelFilter::Off,
        _ => LevelFilter::Info,
    }
}

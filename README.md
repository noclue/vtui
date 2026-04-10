# vTUI: VMware vSphere inventory in the terminal

vTUI is a terminal UI for browsing VMware vSphere inventory from vCenter and standalone ESXi hosts.
It uses `vim_rs` and the vSphere PropertyCollector API to render live inventory data in a Ratatui
interface.

## Install

### Homebrew

```bash
brew install noclue/tap/vtui
```

### winget

```powershell
winget install noclue.vtui
```

### Command line

macOS and Linux:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/noclue/vtui/releases/download/v0.2.4/vtui-installer.sh | sh
```

Windows PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/noclue/vtui/releases/download/v0.2.4/vtui-installer.ps1 | iex"
```

## Supported Platforms

- macOS: Apple Silicon (`aarch64`) and Intel (`x86_64`)
- Windows: ARM64 (`aarch64`) and x64 (`x86_64`)
- Linux: ARM64 (`aarch64`) and x64 (`x86_64`)

## Features

- Browse vCenter and standalone ESXi inventory directly in your terminal
- Real-time inventory updates using the PropertyCollector API
- Full-text search with `/`
- Sort columns by pressing the column index key (`0`-`9`)
- Drill into child collections with shortcuts: `v` VMs, `h` Hosts, `n` Networks, `d` Datastores, `t` Tasks, `e` Events (where shown in the footer)
- **Events** (`e`): live recent-events table; **Enter** opens a read-only JSON tree for the selected event payload (not a managed-object property view). History and **Backspace** work across resource, live property, and static event-detail panes.
- **VM and Host tables**: CPU and memory **sparklines** (PerformanceManager samples), refreshed about every 20 seconds and when the visible set changes (e.g. search)
- Connection/about line shows **API version** and transport (**JSON** or **SOAP**)
- **VM power actions** (`x` on the Virtual Machine list): open a menu of power operations gated by the server’s `disabledMethod` list. Inventory path is resolved (govmomi-style) before the menu opens. All actions except **Power On / Start** require a confirmation showing VM name, path, and action. The UI only **starts** each operation (no task-wait or success banner); the live grid reflects state, and `t` still opens tasks for the selected VM. API failures show an error dialog (dismiss with `Esc` or `Enter`).
- Inspect raw vSphere properties for any object
- Export object properties to a timestamped JSON file with `j`
- Navigate backward through browsing history with `Backspace`
- Switch resource types with `r`
- File logging under the platform state directory (see **Logging** below): separate **application** and **wire** logs with rotation and retention

## Configuration

vTUI connects to vCenter or standalone ESXi. You can configure it in two ways:

1. **Environment variables** (and an optional `.env` file in the current or a parent directory)—good for a single connection or CI.
2. **A TOML config file** under your user config directory—good when you switch between labs, production, or several vCenters.

Both can be used together: **anything set in the process environment overrides the config file** for that run.

`VIM_PROTOCOL` defaults to `auto`, so vTUI picks a sensible transport for the endpoint.

### Config file (multiple connections)

**Locations**

| Platform | Path |
| -------- | ---- |
| macOS / Linux | `~/.config/vtui/config.toml`, or `$XDG_CONFIG_HOME/vtui/config.toml` if set |
| Windows | `%APPDATA%\vtui\config.toml` (usually `...\AppData\Roaming\vtui\config.toml`) |

**What goes in the file**

- Optional top-level **`default_env`**: profile name used when you run plain `vtui` (no extra arguments).
- One **`[environments.<name>]`** table per profile. Each profile needs at least **`server`** and **`username`**.

| Field | Required | Meaning |
| ----- | -------- | ------- |
| `server` | yes | vCenter or ESXi hostname or IP |
| `username` | yes | Login (e.g. `administrator@vsphere.local` or `root`) |
| `password` | no | Plain password (avoid in shared systems; on Unix, use `chmod 600` on the file if you use this) |
| `password_cmd` | no | Shell command whose **standard output** (first line, trailing newline stripped) is the password |
| `insecure` | no | If `true`, TLS certificate verification is skipped (default `false`) |
| `protocol` | no | `auto`, `json`, or `soap` (default `auto`) |
| `log_level` | no | **Deprecated:** use global `[logging].level` instead (see **Logging**). Still read for one release as a migration aid when `[logging].level` is absent. |

**Commands**

- `vtui` — use `default_env` from the file, **or** fall back to environment variables only (same as before) if there is no config or no `default_env`.
- `vtui <name>` — use the `[environments.<name>]` block (the config file must exist).
- `vtui --list` (or `-l`) — print defined profiles and exit.
- `vtui --help` (or `-h`) — usage and variable list.

**Passwords without putting them in the file**

If you do not set `password` or `password_cmd`, vTUI can **prompt for a password** in the terminal before the UI starts (when `server` and `username` are known).

**`password_cmd` in practice**

vTUI runs the string with your system shell (`sh -c` on Unix, `cmd /C` on Windows). It inherits your terminal for stdin/stderr (so tools can prompt or use the macOS keychain) and reads **only stdout** as the password. Examples:

- **envchain** (macOS Keychain-backed env vars): store secrets under a namespace, then expose one variable to stdout:

  ```toml
  default_env = "vc"

  [environments.vc]
  server = "vc.home"
  username = "peter@vsphere.local"
  password_cmd = "envchain VIM printenv VC8"
  log_level = "debug"
  ```

  Here `envchain VIM` unlocks the `VIM` namespace; `printenv VC8` prints the password variable you stored (e.g. after `envchain --set VIM VC8`).

- **1Password CLI**: e.g. `password_cmd = "op read op://Vault/item/password"`.
- **Bitwarden CLI**: e.g. `bw get password <id>` (ensure the CLI is logged in).

- **Get-Secret** (Windows PowerShell SecretManagement):

Install Microsoft.PowerShell.SecretManagement and Microsoft.PowerShell.SecretStore modules. From Administrator's powershell console run:
```pwsh
Install-Module -Name Microsoft.PowerShell.SecretManagement
Install-Module -Name Microsoft.PowerShell.SecretStore
```
Setup local secret store
```pwsh
Register-SecretVault -Name "MyLocalVault" -ModuleName Microsoft.PowerShell.SecretStore -DefaultVault
Set-SecretStoreConfiguration -Authentication None -Confirm:$false
```
The last command disables constant nagging to re-enter current account authentication.

Below is a sample config for Windows (Note that using quotes or curly braces does not seem play well with Windows. File vtui a ticket if you feel this should be better handled)
```toml
default_env="vc"

[environments.vc]
server="vc.home"
username="peter@vsphere.local"
password_cmd = "powershell -NoProfile -Command Get-Secret 'vtui-vc' -AsPlainText"
log_level="debug"
```

For a **one-off** session without editing the file, set **`VIM_PWD_CMD`** to the same kind of command; it overrides `password_cmd` from the file unless `VIM_PASSWORD` is set.

### `.env` file (single connection or overrides)

Create a `.env` file in your working directory (or a parent). Variables here behave like normal environment variables after load; real environment variables still win.

```bash
VIM_SERVER=vcsa.example.com
VIM_USERNAME=administrator@vsphere.local
VIM_PASSWORD=your-password
VIM_INSECURE=true
VIM_PROTOCOL=auto
LOG_LEVEL=info
```

### Environment variables

These apply whether or not you use a config file. When both are set, **environment variables override the selected profile** in the file.

- `VIM_SERVER` — Address of the vCenter or ESXi host
- `VIM_USERNAME` — Username for authentication
- `VIM_PASSWORD` — Password (optional if `VIM_PWD_CMD`, `password_cmd`, or interactive prompt applies)
- `VIM_PWD_CMD` — Shell command whose stdout is the password (same idea as `password_cmd` in TOML)
- `VIM_INSECURE` — If set, only the literal value `false` enables TLS verification; any other value skips verification. If **unset**, the profile’s `insecure` from the file is used, or in env-only mode verification is enabled by default.
- `VIM_PROTOCOL` — `auto`, `json`, or `soap` (default `auto`)
- `LOG_LEVEL` — `trace`, `debug`, `info`, `warn`, `error`, or `off` — **application log verbosity only** (default `info`). Invalid or empty values are ignored with a warning; resolution then follows `config.toml` and defaults. Wire capture is **not** controlled by `LOG_LEVEL`; use `[logging.wire]` in `config.toml` (see **Logging**).
  - With `LOG_LEVEL=debug` (or a `[logging]` / legacy profile level of `debug`), VM action prefetch logs under targets **`vm_actions`** (steps: `name()`, `disabled_method()`, `resolve_inventory_path`) and **`inventory_path`** (PropertyCollector retrieve + path build). The error popup also includes `anyhow` context naming the failing step.
  - Dedicated **wire** logs (`vim_rs::wire::json` / `vim_rs::wire::soap`) use `[logging.wire] mode = summary|detailed` and land in `vtui-wire.log`. At `detailed`, full bodies may appear for non-session traffic; `SessionManager` remains summary-only. SOAP payloads may contain NUL bytes; they are written as the two-character escape `\0` in log files.

### Logging

Logs are written under a **per-user state directory** (not the process current working directory):

| Platform | Directory |
| -------- | --------- |
| macOS / Linux | `$XDG_STATE_HOME/vtui/logs/` if `XDG_STATE_HOME` is set and absolute, otherwise `~/.local/state/vtui/logs/` |
| Windows | `%LOCALAPPDATA%\vtui\logs\` |

On disk, the active files use flexi_logger’s rotation naming, for example **`vtui-app_rCURRENT.log`** and **`vtui-wire_rCURRENT.log`**: the **`r`** is part of the library’s **`rCURRENT`** infix (the file currently receiving writes). Older rotated files get different infixes (e.g. numbered or timestamped). Logs **append** across restarts; rotation, retention, and optional **gzip** of rotated files follow your `[logging.app]` / `[logging.wire]` settings.

Configure in **`config.toml`** (global, not per profile):

```toml
[logging]
level = "info"  # application level; omit to default to info (after LOG_LEVEL / legacy migration)

[logging.app]
rotate_daily = true
max_size_mib = 10
keep_files = 21
compress = true

[logging.wire]
mode = "off"    # off | summary | detailed — maps to vim_rs::WireLoggingMode for the client
rotate_daily = true
max_size_mib = 1024
keep_files = 2
compress = true

# Optional: raise verbosity for specific log targets (app sink only; prefix match, longest prefix wins)
[[logging.filters]]
target = "vim_rs::core"
level = "debug"
```

**Precedence:** `LOG_LEVEL` (env) overrides `[logging].level` for the app only. Legacy per-environment `log_level` in a profile is used only when the global `[logging].level` key is absent and `LOG_LEVEL` is unset; a deprecation message is printed.

**Note:** `RUST_LOG` is **not** used for vTUI logger configuration; if set, a startup note explains that explicit vTUI settings apply instead.

## Usage

After installing a release build:

```bash
vtui              # default profile from config, or env / .env only
vtui prod         # profile [environments.prod] from config
vtui --list       # show profile names
```

To build and run from source, ensure Rust `1.85` or newer is installed and run:

```bash
cargo run --bin vtui
```

## Contributing

Contributions are welcome. Please fork the repository and submit a pull request with your
improvements.

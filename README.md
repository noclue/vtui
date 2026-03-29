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
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/noclue/vtui/releases/download/v0.2.3/vtui-installer.sh | sh
```

Windows PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/noclue/vtui/releases/download/v0.2.3/vtui-installer.ps1 | iex"
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
- File logging to `logs/vtui.log`

## Configuration

vTUI can connect to both vCenter and standalone ESXi hosts. `vim_rs` `0.4.1` adds XML support, and
`VIM_PROTOCOL` defaults to `auto`, so vTUI can use the right transport for the target endpoint.

### `.env` file

Create a `.env` file in your working directory:

```bash
VIM_SERVER=vcsa.example.com
VIM_USERNAME=administrator@vsphere.local
VIM_PASSWORD=your-password
VIM_INSECURE=true
VIM_PROTOCOL=auto
LOG_LEVEL=info
```

### Environment variables

- `VIM_SERVER` - Address of the vCenter or ESXi host
- `VIM_USERNAME` - Username for authentication
- `VIM_PASSWORD` - Password for authentication
- `VIM_INSECURE` - Set to `true` to ignore TLS certificate validation
- `VIM_PROTOCOL` - Transport mode: `auto`, `json`, or `soap` (defaults to `auto`)
- `LOG_LEVEL` - Optional log level: `trace`, `debug`, `info`, `warn`, `error`, or `off`
  - With `LOG_LEVEL=debug`, VM action prefetch logs to `logs/vtui.log` under targets **`vm_actions`** (steps: `name()`, `disabled_method()`, `resolve_inventory_path`) and **`inventory_path`** (PropertyCollector retrieve + path build). The error popup also includes `anyhow` context naming the failing step.
  - At `LOG_LEVEL=trace`, vSphere wire logs can include embedded NUL bytes in SOAP payloads; vTUI writes those as the two-character escape `\0` so the log file stays plain text–friendly.

## Usage

After installing a release build:

```bash
vtui
```

To build and run from source, ensure Rust `1.85` or newer is installed and run:

```bash
cargo run --bin vtui
```

## Contributing

Contributions are welcome. Please fork the repository and submit a pull request with your
improvements.

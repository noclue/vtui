# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- **Cluster inventory table:** `summary_ex.effective_cpu`, `effective_memory`, and `num_hosts` are now optional in the PropertyCollector cache. vcsim sometimes omits these fields even though the API marks them required; previously the whole cluster row disappeared from the table. Missing values render as empty cells instead of dropping the row.
- **Managed-object property browser:** when the server sends an `Enter`/`Modify` with no property values (empty or missing `change_set`), the body no longer renders as a blank black area. You see either **Loading properties…** while waiting for the first update, or a centered explanation when the server truly returned no properties (common with vcsim facade objects such as `EnvironmentBrowser`, or heavily permission-stripped objects). WARN-level logs explain the empty-data case; empty `change_set` after data already arrived is logged at INFO without clearing the tree.

## [0.2.5] - 2026-04-12

### Added

- **VM Summary** (`s` on the **Virtual Machine** inventory table): opens a centered popup with a consolidated view of the selected VM—name and MO id, primary IP(s), overall status, power state and uptime, guest OS, vCPU count and CPU usage (MHz), a one-line **VMware Tools** summary, host memory in use versus configured size, resolved **host** name, provisioned **disk** usage, plus scrollable **Networking** and **Disks** tables. Networking lists each NIC (label, network name when it can be resolved from the backing, MAC, guest IPs); disks list virtual disks with path/backing and datastore names when available. The vSphere work runs **asynchronously** so typing, search, and redraws stay responsive; you briefly see “Loading VM summary…” until data arrives. **Esc** or **q** closes the popup; while it is open, **↑**/**↓**, **j**/**k**, **Page Up**/**Page Down**, **Home**/**End**, **g**/**G**, and **Ctrl+B**/**Ctrl+F** scroll the content.
- **VM Summary networking labels** include standard port groups, distributed port groups (with a clear placeholder when the port group name is not yet resolved), NSX-style **opaque** networks (type and opaque network id), and **SR-IOV** (physical or virtual function device name, or a PCI-style id when the name is empty). If the backing cannot be interpreted, the network cell may show **-** as before.

### Changed

- **Demand-driven PropertyCollector and performance polling:** terminal input and PropertyCollector `WaitForUpdatesEx` long-polls no longer share one `tokio::select!`, so keystrokes no longer cancel and restart in-flight waits (which previously amplified server load during UI activity). A `watch` channel gates when long-polls run: **on** for the resource inventory table and the managed-object property browser, **off** for the static/event JSON property browser and during shutdown. Long-polls use a **60-second** server-side wait window instead of a short timeout loop. Performance (`QueryPerf`) requests are **explicitly cleared** when perf should stop (property browsers, **VM summary** popup open, or other pauses) so the background worker does not keep polling with a stale visible-row set. `refresh_polling_demand()` centralizes these rules; `polling_policy` unit tests document PropertyCollector vs perf demand.
- **Suppress redundant ad-hoc perf refreshes:** changing the selected row within the same visible VM/Host window no longer re-arms the background `PerformanceManager` worker. vTUI now only sends an immediate perf refresh when the effective observed entity set, pause/running state, or perf generation actually changes.
- **Logging (breaking operational change):** vTUI no longer writes `logs/vtui.log` under the current working directory. Application and wire diagnostics use **flexi_logger** with append, rotation, retention, and optional compression under the platform state directory: on Unix-like systems `$XDG_STATE_HOME/vtui/logs/` (or `~/.local/state/vtui/logs/` when `XDG_STATE_HOME` is unset or not absolute), and on Windows `%LOCALAPPDATA%\vtui\logs\`. Filenames follow flexi_logger conventions (e.g. current `vtui-app` / `vtui-wire` logs plus rotated siblings). Timestamps are **UTC** with millisecond precision and a `Z` suffix. Configure verbosity and rotation in a global `[logging]` section of `config.toml`; `LOG_LEVEL` remains supported and applies application logging only. Wire capture is configured with `[logging.wire] mode` (`off`, `summary`, `detailed`) and is passed to `vim_rs::WireLoggingMode` on `ClientBuilder::wire_logging`; targets `vim_rs::wire::json` and `vim_rs::wire::soap` are routed to `vtui-wire.log` without flooding `vtui-app.log`. Per-target overrides use `[[logging.filters]]`. Legacy `[environments.*].log_level` is still read for one release when no global `[logging].level` and no `LOG_LEVEL` apply, with a deprecation warning. **`RUST_LOG`** is intentionally ignored for vTUI’s own logger setup (a note is printed if set). If file logging cannot be initialized, vTUI warns and falls back to stderr-only logging instead of aborting.
- **Background VM action prefetch and execution off the UI loop:** introduce an async operations facility under `src/ops` so vSphere work for the VM power-action flow no longer blocks the main event loop (terminal input, property updates, redraws).

### Fixed

- Preserve Unicode in pretty-printed JSON dumps (previously corrupted non-ASCII content).

## [0.2.4] - 2026-04-05

### Added

- **Non-blocking sparkline UI**: comprehensive architecture for moving all vSphere network operations off the main UI loop into background workers.

Previously, a slow or hung vCenter response could freeze the entire UI — keystrokes would queue up and even quitting could stall. This is especially problematic during the exact infrastructure stress scenarios where operators need vtui most. Performance sparkline queries now run in a dedicated background worker instead of blocking the main event loop. Scrolling, filtering, searching, and periodic refreshes all remain responsive regardless of network latency. Sparklines populate within roughly 500ms of navigating to a new view, and stale results from a previous view are automatically discarded. This is the first phase of a broader effort to move all network operations off the UI thread.

- **Saved connection profiles**: you can keep several vSphere connections in one config file instead of juggling environment variables. Pick a profile with `vtui <profile-name>`, set a default for plain `vtui`, or list profiles with `vtui --list`. Passwords can come from a small shell command (for example 1Password CLI, Bitwarden, or envchain) so secrets stay out of the file and off the command line; if you omit a password, vTUI asks for it when you start. Your existing `.env` / variable setup still works unchanged.
- **Windows password integration**: documented and improved support for Windows PowerShell SecretManagement (`Get-Secret`) as a `password_cmd`, including setup steps for `Microsoft.PowerShell.SecretStore` and a working sample config.

## [0.2.3] - 2026-03-29

### Added

- **Events** resource view: open from supported parents with `e` (hosts, datastores, VMs; shown in the footer when available). Live table backed by `EventHistoryCollector` and PropertyCollector; **Enter** opens a **static property browser** for the event data object (JSON tree, not a managed-object PropertyCollector view), with history and **Backspace** integration.
- **CPU and memory performance sparklines** on Virtual Machine and Host inventory tables: samples from PerformanceManager (`cpu.usage`, `mem.usage`), six points per visible row, 0–10000 (hundredths-of-percent) scale; capacity columns show vCPU count / configured memory (MiB) for VMs and hardware totals for hosts. Refreshes on a ~20s timer and when search or resource context changes.
- About / connection line shows the active wire format next to the API version: **JSON** or **SOAP**.

### Changed

- Host table layout and version label alignment in resource and property browsers (footer spacing, left-aligned version strip).
- Clippy and rustfmt cleanups.

## [0.2.2] - 2026-03-23

### Added

- VM power actions from the Virtual Machine resource view: press `x` to open actions (Power On, Shutdown Guest, Hard Power Off, Guest Reboot, Hard Reset, Suspend). Actions are hidden when listed in `VirtualMachine.disabledMethod`.
- Confirmation dialog for destructive/guest operations, showing VM name and govmomi-style inventory path (batched PropertyCollector ancestry + `parentVApp` handling).
- Error popup for prefetch or action RPC failures (`Esc` / `Enter` to dismiss).
- Debug logging for VM action prefetch (`vm_actions`, `inventory_path` log targets) and clearer error context when a prefetch step fails (SOAP/XML vs inventory path).

### Changed

- Tasks now show the description id if a description string is not found.
- Fixed a critical bug with tasks by moving to vim_Rs 0.4.2. Tasks were not working in the prior release.
- moved the verison to bottom left of the screen
- changed dialogs to consistent scheme (bg: DarkGray, border: Yellow, fg: White)
- revised column widths to better utilize screen estate

### Notes (roadmap)

- **Milestone 2 (planned):** object-scoped **events** view (e.g. `EventManager::query_events` filtered by entity) is not implemented in this release.

## [0.2.1] - 2026-03-21

### Added

- Homebrew and winget distribution for published releases
- `VIM_PROTOCOL` configuration with `auto`, `json`, and `soap` transport modes

### Changed

- Bumped `vim_rs` to `0.4.1` with XML support
- Added documentation for Homebrew, winget, and command-line installation
- Added documentation for standalone ESXi connectivity with `VIM_PROTOCOL=auto`

## [0.2.0] - 2026-03-05

### Added

- Initial public release
- Browse VMware vCenter inventory (VMs, Hosts, Clusters, Datastores, Networks, Tasks)
- Real-time updates via the vSphere PropertyCollector API
- Full-text search (`/`) across any resource list
- Sort columns by pressing the column index key (0–9)
- Drill into child collections (`v` VMs, `h` Hosts, `n` Networks, `d` Datastores, `t` Tasks)
- Property browser: inspect all raw vSphere properties of any object
- JSON dump (`j`) exports object properties to a timestamped file
- Back navigation (`Backspace`) through browsing history
- Resource type switcher (`r`)
- Configurable via environment variables or `.env` file (`VIM_SERVER`, `VIM_USERNAME`, `VIM_PASSWORD`, `VIM_INSECURE`, `LOG_LEVEL`)
- File logging to `logs/vtui.log`
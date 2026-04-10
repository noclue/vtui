# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

* fix(prop-browser): preserve unicode in pretty-printed JSON

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

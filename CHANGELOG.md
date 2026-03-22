# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

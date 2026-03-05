# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

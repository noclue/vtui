# Contract: Host Summary UI

## Scope

This contract defines the observable behavior for the ESX Host summary popup. It is a TUI contract,
not a network API contract.

## Entry Points

### Host Resource Row

- **Context**: Resource browser is showing `ResourceType::Host`.
- **Action hint**: Footer/hint list includes `s summary`.
- **Key**: `s`.
- **Behavior**: Sends `AppEvent::OpenHostSummary(selected_host_ref)` for the selected Host row.

### VM Resource Row

- **Context**: Resource browser is showing `ResourceType::VirtualMachine`.
- **Action hint**: Existing `s summary` behavior remains VM summary.
- **Key**: `s`.
- **Behavior**: Sends existing `AppEvent::OpenVmSummary(selected_vm_ref)`.

## Popup States

### Loading

- **Title**: `Host summary`.
- **Body**: Centered loading message.
- **Footer**: `Esc / q close`.
- **Keys**:
  - `Esc`, `q`, `Q`: close popup.
  - Other keys: consumed so underlying resource browser does not act while modal is active.

### Ready

- **Title**: `Host summary — {inventory_path}` when path is available.
- **Fallback title**: `Host summary — {host_name}` if implementation chooses graceful fallback for
  path resolution failures.
- **Footer**: `Esc/q close  ↑/↓ scroll  PgUp/PgDn page  g/G top/bottom  Ctrl-b/f page`.
- **Sections**:
  - Summary
  - Memory Tiering, only when non-empty and configured
  - Graphics, only when non-empty
  - Physical NICs
  - Disks
  - Virtual Machines
- **Keys**:
  - `Esc`, `q`, `Q`: close popup.
  - Up, `k`: scroll up one line.
  - Down, `j`: scroll down one line.
  - Page Up, Ctrl+B: scroll up one page.
  - Page Down, Ctrl+F: scroll down one page.
  - Home, `g`: top.
  - End, `G`: bottom.
  - Other keys: consumed while modal is active.

## Async Events

### OpenHostSummary

Creates an operation id, starts `HostSummaryUi::start_loading(request_id)`, and queues:

```rust
OperationRequest::PrefetchHostSummary {
    request_id,
    host,
}
```

### HostSummarySucceeded

Applies the summary only when `request_id` matches the pending request. Stale completions are ignored
and logged.

### HostSummaryFailed

Closes the loading popup and sets the existing app error popup only when `request_id` matches the
pending request. Stale failures are ignored and logged.

## Data Retrieval Contract

The background fetch performs:

1. One `ObjectRetriever::retrieve_object::<HostSummaryProps>(&host)` call for host properties.
2. One inventory path resolution call for the title.
3. Zero or one `ObjectRetriever::retrieve_objects_from_list::<HostVmInfo>(&vm_refs[..cap])` call.

The VM cap is 300. When `total_vm_count > 300`, the VM section header must communicate
`Showing 300 of {total_vm_count}`.

## Rendering Contract

- The popup uses the existing VM summary centered-rectangle behavior.
- The popup uses a dark background, high-contrast border, readable labels/values, and a visible
  scrollbar gutter.
- Content rebuilds when the available content width changes.
- Scroll offset is clamped after resize and after content rebuild.
- Empty optional sections are omitted.
- Empty required tables show concise empty text, e.g. `No physical NICs`, `No disks`, or `No VMs`.

## Error Contract

- Missing optional fields do not fail the entire summary.
- Missing host core identity, empty retrieve result, or vSphere retrieval failure fails the fetch.
- Failures include context identifying the host and failing step where practical.

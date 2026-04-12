//! Popup-specific CPU and VMware Tools string formatting.

use crate::resource_browser::formatting::format_compact_mhz;

/// Same numeric compaction as [`format_compact_mhz`], but with spelled-out `MHz` / `GHz` units.
pub fn format_popup_cpu_mhz(mhz: i32) -> String {
    let compact = format_compact_mhz(mhz as i64);
    if compact.ends_with('M') {
        let num = compact.trim().trim_end_matches('M').trim();
        format!("{} MHz", num)
    } else if compact.ends_with('G') {
        let num = compact.trim().trim_end_matches('G').trim();
        format!("{} GHz", num)
    } else if compact == "   -" {
        "-".to_string()
    } else if compact.trim() == "0" {
        "0 MHz".to_string()
    } else {
        compact
    }
}

/// [`VirtualMachineGuestState`](https://developer.broadcom.com/xapis/virtual-infrastructure-json-api/latest/data-structures/VirtualMachineGuestState/) — short, readable phrases (sentence-style).
pub fn format_guest_state_label(raw: &str) -> String {
    match raw.trim() {
        "" => String::new(),
        "running" => "running".to_string(),
        "shuttingDown" => "shutting down".to_string(),
        "resetting" => "resetting".to_string(),
        "standby" => "standby".to_string(),
        "notRunning" => "not running".to_string(),
        "unknown" => "unknown".to_string(),
        other => {
            let o = other.trim();
            if o.is_empty() {
                String::new()
            } else if o.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                o.replace('_', " ")
            } else {
                // e.g. unexpected casing from API
                o.to_string()
            }
        }
    }
}

/// Maps [`VirtualMachineToolsVersionStatus`](https://developer.broadcom.com/xapis/virtual-infrastructure-json-api/latest/data-structures/VirtualMachineToolsVersionStatus_enum/) API strings to brief English.
/// Unrecognized non-empty values are returned as-is for forward compatibility.
pub fn map_tools_version_status_display(raw: &str) -> String {
    match raw.trim() {
        "guestToolsCurrent" => "Current".to_string(),
        "guestToolsNeedUpgrade" => "Upgrade needed".to_string(),
        "guestToolsBlacklisted" => "Blacklisted".to_string(),
        "guestToolsNotInstalled" => "Not installed".to_string(),
        "guestToolsSupportedNew" => "Supported (new tools)".to_string(),
        "guestToolsTooOld" => "Too old".to_string(),
        "guestToolsTooNew" => "Too new".to_string(),
        "guestToolsSupportedOld" => "Supported (old tools)".to_string(),
        "guestToolsUnmanaged" => "Unmanaged".to_string(),
        "" => "Unknown".to_string(),
        other => other.to_string(),
    }
}

/// One-line VMware Tools summary for the VM popup: guest OS state, optional tools build/version, optional support status.
///
/// Example: `running, version 12389 (Unmanaged)`
pub fn format_vmware_tools_summary(
    guest_state: &str,
    tools_version: Option<&str>,
    tools_version_status_2: Option<&str>,
) -> Option<String> {
    let state_fmt = format_guest_state_label(guest_state);
    let ver = tools_version.map(str::trim).filter(|s| !s.is_empty());
    let status = tools_version_status_2
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(map_tools_version_status_display);

    if state_fmt.is_empty() && ver.is_none() && status.is_none() {
        return None;
    }

    let mut out = if state_fmt.is_empty() {
        "unknown".to_string()
    } else {
        state_fmt
    };

    if let Some(v) = ver {
        out.push_str(", version ");
        out.push_str(v);
    }
    if let Some(st) = status {
        out.push_str(" (");
        out.push_str(&st);
        out.push(')');
    }

    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_cpu_mhz_spells_units() {
        assert_eq!(format_popup_cpu_mhz(243), "243 MHz");
        assert_eq!(format_popup_cpu_mhz(2300), "2.3 GHz");
    }

    #[test]
    fn guest_state_labels() {
        assert_eq!(format_guest_state_label("running"), "running");
        assert_eq!(format_guest_state_label("notRunning"), "not running");
        assert_eq!(format_guest_state_label("shuttingDown"), "shutting down");
    }

    #[test]
    fn vmware_tools_summary_example() {
        assert_eq!(
            format_vmware_tools_summary("running", Some("12389"), Some("guestToolsUnmanaged")),
            Some("running, version 12389 (Unmanaged)".to_string())
        );
    }

    #[test]
    fn vmware_tools_summary_state_only() {
        assert_eq!(
            format_vmware_tools_summary("standby", None, None),
            Some("standby".to_string())
        );
    }
}

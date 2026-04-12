//! Pure demand rules for PropertyCollector vs perf polling (unit-tested).

/// Whether PropertyCollector long-polls should run for the current body pane class.
pub(crate) fn property_collector_wanted(
    resource_browser: bool,
    managed_property_browser: bool,
) -> bool {
    resource_browser || managed_property_browser
}

/// Whether `QueryPerf` polling should run (resource grid visible and VM summary modal closed).
pub(crate) fn perf_polling_wanted(resource_browser: bool, vm_summary_open: bool) -> bool {
    resource_browser && !vm_summary_open
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_collector_resource_or_managed_property_only() {
        assert!(property_collector_wanted(true, false));
        assert!(property_collector_wanted(false, true));
        assert!(!property_collector_wanted(false, false));
        assert!(property_collector_wanted(true, true));
    }

    #[test]
    fn perf_only_when_resource_browser_and_vm_summary_closed() {
        assert!(perf_polling_wanted(true, false));
        assert!(!perf_polling_wanted(true, true));
        assert!(!perf_polling_wanted(false, false));
        assert!(!perf_polling_wanted(false, true));
    }
}

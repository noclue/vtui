//! Pure demand rules for PropertyCollector vs perf polling (unit-tested).

/// Whether PropertyCollector long-polls should run for the current body pane class.
pub(crate) fn property_collector_wanted(
    resource_browser: bool,
    managed_property_browser: bool,
) -> bool {
    resource_browser || managed_property_browser
}

/// Which `QueryPerf` request the worker should be running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PerfPollMode {
    Off,
    Table,
    Overview,
}

/// Whether table `QueryPerf` polling should run (resource grid visible and summary modals closed).
pub(crate) fn perf_polling_wanted(resource_browser: bool, summary_modal_open: bool) -> bool {
    resource_browser && !summary_modal_open
}

/// Overview wins over the table. A summary modal pauses both.
pub(crate) fn perf_poll_mode(
    resource_browser: bool,
    summary_modal_open: bool,
    perf_overview: bool,
) -> PerfPollMode {
    if perf_overview && !summary_modal_open {
        PerfPollMode::Overview
    } else if perf_polling_wanted(resource_browser, summary_modal_open) {
        PerfPollMode::Table
    } else {
        PerfPollMode::Off
    }
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
    fn perf_only_when_resource_browser_and_summary_modal_closed() {
        assert!(perf_polling_wanted(true, false));
        assert!(!perf_polling_wanted(true, true));
        assert!(!perf_polling_wanted(false, false));
        assert!(!perf_polling_wanted(false, true));
    }

    #[test]
    fn overview_does_not_request_property_collector_and_table_stays_for_inventory() {
        assert_eq!(perf_poll_mode(false, false, true), PerfPollMode::Overview);
        assert!(!property_collector_wanted(false, false));
        assert_eq!(perf_poll_mode(true, false, false), PerfPollMode::Table);
        assert_eq!(perf_poll_mode(true, true, false), PerfPollMode::Off);
        assert_eq!(perf_poll_mode(false, true, true), PerfPollMode::Off);
    }
}

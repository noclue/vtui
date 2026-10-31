//! Host and virtual-machine performance overview.

mod layout;
mod metrics;
mod render;
mod store;
mod ui;

pub use metrics::{apply_fetch, prepare_overview};
pub use store::{PerfOverviewShare, new_overview_store, record_overview_failure};
pub use ui::PerfOverview;

use vim_rs::types::enums::MoTypesEnum;
use vim_rs::types::structs::ManagedObjectReference;

pub(crate) const EXPAND_HINTS: &[&str] = &["Tab focus", "1 cpu", "2 mem", "3 net", "4 disk"];
pub(crate) const HELP_HINTS: &[&str] = &["Esc back", "z header", "q quit"];

pub(crate) fn hints() -> (&'static [&'static str], &'static [&'static str]) {
    (EXPAND_HINTS, HELP_HINTS)
}

pub fn kind_label(moref: &ManagedObjectReference) -> &'static str {
    if moref.r#type == MoTypesEnum::HostSystem {
        "Host"
    } else {
        "VM"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_hints_list_focus_and_back() {
        assert_eq!(
            EXPAND_HINTS,
            &["Tab focus", "1 cpu", "2 mem", "3 net", "4 disk"][..]
        );
        assert_eq!(HELP_HINTS, &["Esc back", "z header", "q quit"][..]);
    }
}

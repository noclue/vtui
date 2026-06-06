use crate::resource_browser::formatting::{self, format_compact_mem_bytes, format_compact_mhz};
use crate::resource_browser::formatting::{STATUS, format_byte_size, sparkline_from_perf_samples};
use crate::resource_browser::perf::PerfRowsSnapshot;
use crate::resource_browser::tabular_data::{InventoryRowBuilder, SortFn, TabularData};
use crate::resource_browser::vm_layout::{
    VM_CPU_WIDTH, VM_ID_COLUMN_WIDTH, VM_MEMORY_WIDTH, VM_OS_WIDTH, VM_POWER_WIDTH,
    VM_STATUS_WIDTH, VM_USED_SPACE_WIDTH,
};
use crate::resource_type::ResourceType;
use ratatui::layout::Constraint;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Span;
use ratatui::widgets::{Cell, Row};
use vim_rs::types::enums::VirtualMachinePowerStateEnum;
use vim_rs::vim_updatable;

vim_updatable!(
    struct VmData: VirtualMachine {
        name = "name",
        os = "summary.guest.guest_full_name",
        storage = "summary.storage",
        status = "overall_status",
        power_state = "runtime.power_state",
    }
);

const POWER_ON: &str = "● ";
// U+25CF
const POWER_OFF: &str = "○ ";
// U+25CB
const SUSPENDED: &str = "◐ ";

impl InventoryRowBuilder for VmData {
    fn table_cells(&self, perf: Option<&PerfRowsSnapshot>) -> Vec<Cell<'static>> {
        let color = formatting::status_color(&self.status);
        let power_state = match self.power_state {
            VirtualMachinePowerStateEnum::PoweredOn => {
                Span::styled(POWER_ON, Style::default().fg(Color::Green))
            }
            VirtualMachinePowerStateEnum::PoweredOff => {
                Span::styled(POWER_OFF, Style::default().fg(Color::Red))
            }
            VirtualMachinePowerStateEnum::Suspended => {
                Span::styled(SUSPENDED, Style::default().fg(Color::Yellow))
            }
            _ => Span::from("?").gray(),
        };
        let used_space = if let Some(ref storage) = self.storage {
            format_byte_size(storage.committed)
        } else {
            Cell::default()
        };

        let (cpu_slots, mem_slots) = perf
            .map(|p| p.cpu_mem_slots(&self.id))
            .unwrap_or(([None; 6], [None; 6]));

        let spark_cpu = sparkline_from_perf_samples(&cpu_slots);
        let spark_mem = sparkline_from_perf_samples(&mem_slots);

        let cap_cpu = perf
            .and_then(|p| p.latest_cpu_mhz(&self.id))
            .map(format_compact_mhz)
            .unwrap_or_else(|| "    ".to_string());
        let cap_mem = perf
            .and_then(|p| p.latest_mem_bytes(&self.id))
            .map(format_compact_mem_bytes)
            .unwrap_or_else(|| "    ".to_string());

        let host_cpu = Cell::from(format!("{spark_cpu}{cap_cpu}"));
        let host_memory = Cell::from(format!("{spark_mem}{cap_mem}"));

        vec![
            Cell::from(self.id.value.clone()),
            Cell::from(Span::from(STATUS).style(color)),
            Cell::from(power_state),
            Cell::from(self.name.clone()),
            Cell::from(self.os.clone().unwrap_or("<unknown>".to_string())),
            used_space,
            host_cpu,
            host_memory,
        ]
    }

    fn inventory_row(&self, perf: Option<&PerfRowsSnapshot>) -> Row<'static> {
        Row::new(self.table_cells(perf))
    }
}

impl TabularData for VmData {
    fn get_title() -> &'static str {
        "Virtual Machines"
    }
    fn column_sizes() -> Vec<Constraint> {
        vec![
            Constraint::Length(VM_ID_COLUMN_WIDTH),
            Constraint::Length(VM_STATUS_WIDTH),
            Constraint::Length(VM_POWER_WIDTH),
            Constraint::Fill(1),
            Constraint::Length(VM_OS_WIDTH),
            Constraint::Length(VM_USED_SPACE_WIDTH),
            Constraint::Length(VM_CPU_WIDTH),
            Constraint::Length(VM_MEMORY_WIDTH),
        ]
    }

    fn header_row() -> Vec<&'static str> {
        vec![
            "ID ",
            "S ",
            "P ",
            "Name ",
            "OS ",
            "Used Space ",
            "CPU ",
            "Memory ",
        ]
    }

    fn sortable_columns() -> Vec<usize> {
        // CPU/Memory (6,7) are not sortable: perf samples exist only for visible rows.
        vec![0, 3, 4, 5]
    }

    fn sort_by_column(column_idx: usize, descending: bool) -> Option<SortFn<Self>> {
        let mut f: SortFn<Self> = match column_idx {
            0 => Box::new(|a, b| a.id.value.cmp(&b.id.value)),
            3 => Box::new(|a, b| a.name.cmp(&b.name)),
            4 => Box::new(|a, b| a.os.cmp(&b.os)),
            5 => Box::new(|a, b| {
                a.storage
                    .as_ref()
                    .map_or(0, |s| s.committed)
                    .cmp(&b.storage.as_ref().map_or(0, |s| s.committed))
            }),
            _ => return None,
        };
        if descending {
            Some(Box::new(move |a: &Self, b: &Self| f(b, a)))
        } else {
            Some(f)
        }
    }

    fn matches_filter(&self, filter: &str) -> bool {
        // Check if the filter matches any of the ID, Name, OS fields
        let filter = filter.to_lowercase();
        self.id.value.to_lowercase().contains(&filter)
            || self.name.to_lowercase().contains(&filter)
            || self
                .os
                .as_ref()
                .unwrap_or(&"".to_string())
                .to_lowercase()
                .contains(&filter)
    }

    fn name(&self) -> String {
        self.name.clone()
    }
    fn resource_type() -> ResourceType {
        ResourceType::VirtualMachine
    }
}

#[cfg(test)]
mod tests {
    use super::VmData;
    use crate::resource_browser::tabular_data::TabularData;
    use vim_rs::types::enums::{
        ManagedEntityStatusEnum, MoTypesEnum, VirtualMachinePowerStateEnum,
    };
    use vim_rs::types::structs::{ManagedObjectReference, VirtualMachineStorageSummary};

    fn sample_vm(value: &str, name: &str, os: Option<&str>, committed: Option<i64>) -> VmData {
        VmData {
            id: ManagedObjectReference {
                r#type: MoTypesEnum::VirtualMachine,
                value: value.into(),
            },
            name: name.into(),
            os: os.map(String::from),
            storage: committed.map(|c| VirtualMachineStorageSummary {
                committed: c,
                uncommitted: 0,
                unshared: 0,
                timestamp: String::new(),
            }),
            status: ManagedEntityStatusEnum::Green,
            power_state: VirtualMachinePowerStateEnum::PoweredOn,
        }
    }

    #[test]
    fn matches_filter_name_id_and_os() {
        let vm = sample_vm("vm-42", "db-01", Some("Linux"), None);
        assert!(vm.matches_filter("db"));
        assert!(vm.matches_filter("VM-42"));
        assert!(vm.matches_filter("linux"));
        assert!(!vm.matches_filter("zzz"));
    }

    #[test]
    fn sort_by_name_column_orders_lexicographically() {
        let a = sample_vm("1", "antelope", None, None);
        let z = sample_vm("2", "zebra", None, None);
        let mut cmp = VmData::sort_by_column(3, false).expect("column 3 sortable");
        assert_eq!(cmp(&a, &z), std::cmp::Ordering::Less);
    }

    #[test]
    fn sort_by_storage_column_uses_committed_bytes() {
        let small = sample_vm("1", "a", None, Some(100));
        let large = sample_vm("2", "b", None, Some(9_000));
        let mut cmp = VmData::sort_by_column(5, false).expect("column 5 sortable");
        assert_eq!(cmp(&small, &large), std::cmp::Ordering::Less);
    }

    #[test]
    fn matches_filter_by_id_when_id_column_hidden_in_layout() {
        let vm = sample_vm("vm-hidden-99", "short", Some("Windows"), None);
        assert!(vm.matches_filter("hidden-99"));
    }

    #[test]
    fn matches_filter_by_os_when_os_column_hidden_in_layout() {
        let vm = sample_vm("vm-1", "short", Some("Ubuntu Linux"), None);
        assert!(vm.matches_filter("ubuntu"));
    }
}

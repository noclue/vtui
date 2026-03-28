use crate::resource_browser::formatting;
use crate::resource_browser::formatting::{
    ID_COLUMN_WIDTH, STATUS, STATUS_COLUMN_WIDTH, format_byte_size,
};
use crate::resource_browser::tabular_data::{SortFn, TabularData};
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
        host_cpu = "summary.quick_stats.overall_cpu_usage",
        host_memory = "summary.quick_stats.host_memory_usage",
        status = "overall_status",
        power_state = "runtime.power_state",
    }
);

const POWER_ON: &str = "● ";
// U+25CF
const POWER_OFF: &str = "○ ";
// U+25CB
const SUSPENDED: &str = "◐ ";

impl From<&VmData> for Row<'_> {
    fn from(vm: &VmData) -> Self {
        let color = formatting::status_color(&vm.status);
        let power_state = match vm.power_state {
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
        let used_space = if let Some(ref storage) = vm.storage {
            format_byte_size(storage.committed)
        } else {
            Cell::default()
        };
        let host_cpu = if let Some(host_cpu) = vm.host_cpu {
            Cell::from(format!("{:.2} MHz", host_cpu as f32))
        } else {
            Cell::default()
        };
        let host_memory = if let Some(host_memory) = vm.host_memory {
            if host_memory > 1024 {
                Cell::from(format!("{:.2} GiB", host_memory as f32 / 1024.0))
            } else {
                Cell::from(format!("{:.2} MiB", host_memory as f32))
            }
        } else {
            Cell::default()
        };

        Row::new(vec![
            Cell::from(vm.id.value.clone()),
            Cell::from(Span::from(STATUS).style(color)),
            Cell::from(power_state),
            Cell::from(vm.name.clone()),
            Cell::from(vm.os.clone().unwrap_or("<unknown>".to_string())),
            used_space,
            host_cpu,
            host_memory,
        ])
    }
}

impl TabularData for VmData {
    fn get_title() -> &'static str {
        "Virtual Machines"
    }
    fn column_sizes() -> Vec<Constraint> {
        vec![
            Constraint::Length(ID_COLUMN_WIDTH),
            Constraint::Length(STATUS_COLUMN_WIDTH),
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Max(15),
            Constraint::Max(12),
            Constraint::Max(12),
            Constraint::Max(12),
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
        // ID, Name, OS, Used Space, CPU and Memory are sortable
        vec![0, 3, 4, 5, 6, 7]
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
            6 => Box::new(|a, b| a.host_cpu.cmp(&b.host_cpu)),
            7 => Box::new(|a, b| a.host_memory.cmp(&b.host_memory)),
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
            host_cpu: None,
            host_memory: None,
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
}

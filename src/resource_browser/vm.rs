use crate::resource_browser::formatting::{self, format_compact_memory_size};
use crate::resource_browser::formatting::{
    ID_COLUMN_WIDTH, STATUS, STATUS_COLUMN_WIDTH, format_byte_size, sparkline_from_perf_samples,
};
use crate::resource_browser::perf::PerfRowsSnapshot;
use crate::resource_browser::tabular_data::{InventoryRowBuilder, SortFn, TabularData};
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
        num_cpu = "summary.config.num_cpu",
        memory_size_mb = "summary.config.memory_size_mb",
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
    fn inventory_row(&self, perf: Option<&PerfRowsSnapshot>) -> Row<'static> {
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

        let cap_cpu = self
            .num_cpu
            .map(|n| format!("{:>3}c", n))
            .unwrap_or_else(|| "    ".to_string());
        let cap_mem = self
            .memory_size_mb
            .map(|mb| format_compact_memory_size(mb as i64))
            .unwrap_or_else(|| "    ".to_string());

        let host_cpu = Cell::from(format!("{spark_cpu}{cap_cpu}"));
        let host_memory = Cell::from(format!("{spark_mem}{cap_mem}"));

        Row::new(vec![
            Cell::from(self.id.value.clone()),
            Cell::from(Span::from(STATUS).style(color)),
            Cell::from(power_state),
            Cell::from(self.name.clone()),
            Cell::from(self.os.clone().unwrap_or("<unknown>".to_string())),
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
            Constraint::Length(10),
            Constraint::Length(11),
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
            6 => Box::new(|a, b| a.num_cpu.cmp(&b.num_cpu)),
            7 => Box::new(|a, b| a.memory_size_mb.cmp(&b.memory_size_mb)),
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

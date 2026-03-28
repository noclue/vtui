use crate::resource_browser::formatting::{
    ID_COLUMN_WIDTH, STATUS, STATUS_COLUMN_WIDTH, format_compact_metric,
    sparkline_from_perf_samples, status_color,
};
use crate::resource_browser::perf::PerfRowsSnapshot;
use crate::resource_browser::tabular_data::{InventoryRowBuilder, SortFn, TabularData};
use crate::resource_type::ResourceType;
use ratatui::layout::Constraint;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Cell, Row};
use vim_rs::types::enums::HostSystemConnectionStateEnum;
use vim_rs::vim_updatable;
vim_updatable!(
    struct Host: HostSystem {
        overall_status = "summary.overall_status",
        connection_state = "runtime.connection_state",
        name = "name",
        version = "config.product.version",
        hw_cpu_mhz = "summary.hardware.cpu_mhz",
        hw_num_cpu_cores = "summary.hardware.num_cpu_cores",
        hw_memory_size = "summary.hardware.memory_size",
        uptime = "summary.quick_stats.uptime",
        vms = "vm.length",
        networks = "network.length",
        datastores = "datastore.length",
    }
);

impl Host {
    fn cpu_capacity_mhz(&self) -> Option<i32> {
        let mhz = self.hw_cpu_mhz?;
        let cores = self.hw_num_cpu_cores?;
        Some(mhz * i32::from(cores))
    }
}

impl InventoryRowBuilder for Host {
    fn inventory_row(&self, perf: Option<&PerfRowsSnapshot>) -> Row<'static> {
        let color = status_color(&self.overall_status);
        let version = if let Some(version) = self.version.as_ref() {
            Cell::from(version.to_string())
        } else {
            Cell::default()
        };

        let (cpu_slots, mem_slots) = perf
            .map(|p| p.cpu_mem_slots(&self.id))
            .unwrap_or(([None; 6], [None; 6]));

        let spark_cpu = sparkline_from_perf_samples(&cpu_slots);
        let spark_mem = sparkline_from_perf_samples(&mem_slots);

        let cap_cpu = self
            .cpu_capacity_mhz()
            .map(|mhz| format_compact_metric(mhz as f64))
            .unwrap_or_else(|| "    ".to_string());
        let cap_mem = self
            .hw_memory_size
            .map(|b| format_compact_metric(b as f64))
            .unwrap_or_else(|| "    ".to_string());

        let host_cpu = Cell::from(format!("{spark_cpu}{cap_cpu}"));
        let memory_cell = Cell::from(format!("{spark_mem}{cap_mem}"));

        // connected, not_responding, disconnected
        let (symbol, conn_color) = match self.connection_state {
            HostSystemConnectionStateEnum::Connected => ("✓", ratatui::style::Color::Green),
            HostSystemConnectionStateEnum::NotResponding => ("!", ratatui::style::Color::Yellow),
            HostSystemConnectionStateEnum::Disconnected => ("✗", ratatui::style::Color::Red),
            _ => ("?", ratatui::style::Color::Gray),
        };

        let vms = if let Some(vms) = self.vms {
            Cell::from(vms.to_string())
        } else {
            Cell::default()
        };
        let networks = if let Some(networks) = self.networks {
            Cell::from(networks.to_string())
        } else {
            Cell::default()
        };
        let datastores = if let Some(datastores) = self.datastores {
            Cell::from(datastores.to_string())
        } else {
            Cell::default()
        };

        Row::new(vec![
            Cell::from(self.id.value.clone()),
            Cell::from(Span::from(STATUS).style(color)),
            Cell::from(Span::styled(symbol, Style::new().fg(conn_color))),
            Cell::from(Span::from(self.name.clone())),
            version,
            host_cpu,
            memory_cell,
            vms,
            networks,
            datastores,
        ])
    }
}

impl TabularData for Host {
    fn get_title() -> &'static str {
        "Hosts"
    }
    fn column_sizes() -> Vec<Constraint> {
        vec![
            Constraint::Length(ID_COLUMN_WIDTH),
            Constraint::Length(STATUS_COLUMN_WIDTH),
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Max(7),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Max(5),
            Constraint::Max(5),
            Constraint::Max(5),
        ]
    }

    fn header_row() -> Vec<&'static str> {
        vec![
            "ID ", "S ", "C ", "Name ", "Version ", "CPU ", "Memory ", "VMs ", "Net ", "DS ",
        ]
    }

    fn sortable_columns() -> Vec<usize> {
        // ID, Name, Version, CPU and Memory are sortable
        vec![0, 3, 4, 5, 6, 7, 8]
    }

    fn sort_by_column(column_idx: usize, descending: bool) -> Option<SortFn<Self>> {
        let mut f: SortFn<Self> = match column_idx {
            0 => Box::new(|a, b| a.id.value.cmp(&b.id.value)),
            3 => Box::new(|a, b| a.name.cmp(&b.name)),
            4 => Box::new(|a, b| a.version.cmp(&b.version)),
            5 => Box::new(|a, b| a.cpu_capacity_mhz().cmp(&b.cpu_capacity_mhz())),
            6 => Box::new(|a, b| a.hw_memory_size.cmp(&b.hw_memory_size)),
            7 => Box::new(|a, b| a.vms.cmp(&b.vms)),
            8 => Box::new(|a, b| a.networks.cmp(&b.networks)),
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
                .version
                .as_ref()
                .unwrap_or(&"".to_string())
                .to_lowercase()
                .contains(&filter)
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn resource_type() -> ResourceType {
        ResourceType::Host
    }
}

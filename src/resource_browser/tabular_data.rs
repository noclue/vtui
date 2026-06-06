use crate::resource_browser::perf::{PerfRowsSnapshot, PerfSnapshotShare};
use crate::resource_type::ResourceType;
use ratatui::layout::Constraint;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Row};
use vim_rs::types::structs::ManagedObjectReference;

pub type SortFn<T> = Box<dyn FnMut(&T, &T) -> std::cmp::Ordering>;

/// Render-time column visibility and Ratatui width constraints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnLayout {
    pub visible_indices: Vec<usize>,
    pub constraints: Vec<Constraint>,
}

/// Build header cells for visible logical columns; sort arrow uses logical `sort_column` index.
pub fn project_header(
    header_row: &[&str],
    visible_indices: &[usize],
    sort_setting: Option<(usize, bool)>,
) -> Vec<Cell<'static>> {
    visible_indices
        .iter()
        .map(|&logical_idx| {
            let label = header_row
                .get(logical_idx)
                .copied()
                .unwrap_or("")
                .to_string();
            if let Some((sort_col, descending)) = sort_setting
                && logical_idx == sort_col
            {
                let arrow_span = if descending {
                    Span::styled("▼", Style::default().fg(Color::Blue))
                } else {
                    Span::styled("▲", Style::default().fg(Color::Green))
                };
                return Cell::from(Line::from(vec![Span::from(label), arrow_span]));
            }
            Cell::from(label)
        })
        .collect()
}

/// Select cells by logical column index (caller supplies full cell vector).
pub fn project_cells(cells: &[Cell<'static>], visible_indices: &[usize]) -> Vec<Cell<'static>> {
    visible_indices
        .iter()
        .filter_map(|&i| cells.get(i).cloned())
        .collect()
}

/// Trait for objects that can be displayed in a table. This exposes the static information
/// about the table, such as the title, column sizes, and header row. It also provides
/// methods for sorting and filtering the data.
pub trait TabularData {
    // Get the title of the table
    fn get_title() -> &'static str;
    // Column constraints for the table
    fn column_sizes() -> Vec<Constraint>;

    // Header row with column titles
    fn header_row() -> Vec<&'static str>;

    // Which columns support sorting (by index)
    fn sortable_columns() -> Vec<usize>;

    // Get sorting function for a specific column
    fn sort_by_column(column_idx: usize, descending: bool) -> Option<SortFn<Self>>
    where
        Self: Sized;

    // Whether this item matches the given filter string
    fn matches_filter(&self, filter: &str) -> bool;

    // Get a display name for the item e.g. "admin_cluster"
    fn name(&self) -> String;

    fn resource_type() -> ResourceType;
}

/// Build a table row, optionally using live performance history (VM/Host CPU/mem sparklines).
pub trait InventoryRowBuilder: TabularData {
    fn inventory_row(&self, perf: Option<&PerfRowsSnapshot>) -> Row<'static>;

    /// Full-width cell vector in [`Self::header_row`] column order for layout projection.
    fn table_cells(&self, _perf: Option<&PerfRowsSnapshot>) -> Vec<Cell<'static>> {
        vec![]
    }

    fn inventory_row_for_layout(
        &self,
        perf: Option<&PerfRowsSnapshot>,
        layout: &ColumnLayout,
    ) -> Row<'static> {
        if layout.visible_indices.len() >= Self::header_row().len() {
            return self.inventory_row(perf);
        }
        let cells = self.table_cells(perf);
        if cells.is_empty() {
            return self.inventory_row(perf);
        }
        Row::new(project_cells(&cells, &layout.visible_indices))
    }
}

/// Trait for data sources that can be displayed in a table.
pub trait TableDataSource {
    fn get_title(&self) -> &'static str;
    fn set_filter(&mut self, filter: Option<String>);
    fn get_filter(&self) -> Option<String>;
    fn set_sort_column(&mut self, column: Option<usize>);

    fn get_sort_setting(&self) -> Option<(usize, bool)>;
    fn set_sort_setting(&mut self, column: usize, descending: bool);
    fn iter<'a>(&'a mut self) -> Box<dyn Iterator<Item = Row<'static>> + 'a>;
    fn is_empty(&mut self) -> bool;
    fn len(&mut self) -> usize;
    fn total_count(&self) -> usize;
    fn column_sizes(&self) -> Vec<Constraint>;
    fn header_row(&self) -> Vec<&'static str>;
    fn invalidate(&mut self);
    // Get the ID and Name of the object at the given index
    fn item_at_index(&mut self, index: usize) -> Option<(ManagedObjectReference, String)>;
    fn resource_type(&self) -> ResourceType;

    /// Event table only: **removes** the row at `index` and returns a payload for the static
    /// property browser. Used when leaving the events view; default is a no-op.
    fn take_event_browser_payload_at(
        &mut self,
        _index: usize,
    ) -> Option<super::events::EventBrowserPayload> {
        None
    }

    /// VM/Host tables pass a shared perf snapshot; other sources ignore it.
    fn set_perf_snapshot(&mut self, _perf: Option<PerfSnapshotShare>) {}

    /// Width-aware column set; default shows every column at static [`column_sizes`].
    fn column_layout(&self, _columns_budget: u16) -> ColumnLayout {
        let header_len = self.header_row().len();
        ColumnLayout {
            visible_indices: (0..header_len).collect(),
            constraints: self.column_sizes(),
        }
    }

    fn iter_for_layout<'a>(
        &'a mut self,
        layout: &ColumnLayout,
    ) -> Box<dyn Iterator<Item = Row<'static>> + 'a> {
        let _ = layout;
        self.iter()
    }
}

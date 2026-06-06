use crate::resource_browser::perf::PerfSnapshotShare;
use crate::resource_browser::tabular_data::{
    ColumnLayout, InventoryRowBuilder, TableDataSource, TabularData,
};
use crate::resource_browser::vm_layout::vm_column_layout;
use crate::resource_type::ResourceType;
use ratatui::layout::Constraint;
use ratatui::widgets::Row;
use std::ops::Index;
use std::sync::{Arc, RwLock};
use vim_rs::core::pc_cache::{Cacheable, ObjectCache};
use vim_rs::core::pc_helpers::BoxableError;
use vim_rs::types::structs::ManagedObjectReference;

/// The IndexedCache struct is a wrapper around an ObjectCache that provides interface to
/// filter and sort the data. It implements the TableDataSource trait, allowing it to be used
/// as a data source for tabular views.
///
/// It maintains a list of indices that point to the original cache, allowing for efficient
/// filtering and sorting without needing to copy the data.
///
/// The struct is generic over the type T, which must implement the Cacheable and TabularData traits.
/// It also requires that the error type of T is BoxableError, and that rows are built via
/// [`InventoryRowBuilder`].
pub struct IndexedCache<T>
where
    T: Cacheable + TabularData + InventoryRowBuilder,
    T::Error: BoxableError,
{
    cache: Arc<RwLock<ObjectCache<T>>>,
    indices: Option<Vec<usize>>, // Filtered/sorted indices into original cache
    filter: Option<String>,      // Current filter criteria
    sort_column: Option<usize>,  // Current sort column
    sort_descending: bool,       // Sort direction
    perf: Option<PerfSnapshotShare>,
}

impl<T> IndexedCache<T>
where
    T: Cacheable + TabularData + InventoryRowBuilder,
    T::Error: BoxableError,
{
    pub fn new(cache: Arc<RwLock<ObjectCache<T>>>) -> Self {
        IndexedCache {
            cache,
            indices: None,
            filter: None,
            sort_column: None,
            sort_descending: false,
            perf: None,
        }
    }

    fn ensure_indices_updated(&mut self) {
        if self.indices.is_none() {
            self.update_indices();
        }
    }

    fn update_indices(&mut self) {
        // Update the indices based on the current filter and sort criteria
        let cache = self.cache.read().expect("ObjectCache lock poisoned");
        let mut indices: Vec<usize> = (0..cache.len()).collect();

        if let Some(ref filter) = self.filter {
            indices.retain(|&i| cache[i].matches_filter(filter));
        }

        if let Some(column) = self.sort_column {
            let cmp = T::sort_by_column(column, self.sort_descending);
            if let Some(mut cmp) = cmp {
                indices.sort_by(|&a, &b| cmp(&cache[a], &cache[b]));
            }
        }

        self.indices = Some(indices);
    }
}

impl<T> TableDataSource for IndexedCache<T>
where
    T: Cacheable + TabularData + InventoryRowBuilder,
    T::Error: BoxableError,
{
    fn get_title(&self) -> &'static str {
        T::get_title()
    }
    fn set_filter(&mut self, filter: Option<String>) {
        self.filter = filter;
        self.invalidate();
    }

    fn get_filter(&self) -> Option<String> {
        self.filter.clone()
    }

    fn set_sort_column(&mut self, column: Option<usize>) {
        // If the column is not sortable, do nothing
        if let Some(sort_column) = column
            && !T::sortable_columns().contains(&sort_column)
        {
            return;
        }
        if self.sort_column != column {
            self.sort_descending = false;
        } else {
            self.sort_descending = !self.sort_descending;
        }
        self.sort_column = column;
        self.invalidate();
    }

    fn get_sort_setting(&self) -> Option<(usize, bool)> {
        self.sort_column
            .map(|column| (column, self.sort_descending))
    }
    fn set_sort_setting(&mut self, column: usize, descending: bool) {
        if !T::sortable_columns().contains(&column) {
            return;
        }
        self.sort_column = Some(column);
        self.sort_descending = descending;
        self.invalidate();
    }
    fn iter<'a>(&'a mut self) -> Box<dyn Iterator<Item = Row<'static>> + 'a> {
        self.ensure_indices_updated();
        let Some(indices) = &self.indices else {
            panic!("Internal error: No indices found after ensuring indices updated");
        };

        let perf_rows = self
            .perf
            .as_ref()
            .and_then(|p| p.read().ok())
            .map(|g| g.clone());

        let cache = self.cache.clone();
        Box::new(indices.iter().map(move |idx| {
            let cache = cache.read().expect("ObjectCache lock poisoned");
            let item = &cache[*idx];
            T::inventory_row(item, perf_rows.as_ref())
        }))
    }

    fn is_empty(&mut self) -> bool {
        self.ensure_indices_updated();
        let Some(indices) = &self.indices else {
            return true;
        };
        indices.is_empty()
    }

    fn len(&mut self) -> usize {
        self.ensure_indices_updated();
        let Some(indices) = &self.indices else {
            return 0;
        };
        indices.len()
    }

    fn total_count(&self) -> usize {
        self.cache.read().expect("ObjectCache lock poisoned").len()
    }

    fn column_sizes(&self) -> Vec<Constraint> {
        T::column_sizes()
    }

    fn column_layout(&self, columns_budget: u16) -> ColumnLayout {
        if T::resource_type() == ResourceType::VirtualMachine {
            vm_column_layout(columns_budget)
        } else {
            let header_len = T::header_row().len();
            ColumnLayout {
                visible_indices: (0..header_len).collect(),
                constraints: T::column_sizes(),
            }
        }
    }

    fn iter_for_layout<'a>(
        &'a mut self,
        layout: &ColumnLayout,
    ) -> Box<dyn Iterator<Item = Row<'static>> + 'a> {
        self.ensure_indices_updated();
        let Some(indices) = &self.indices else {
            panic!("Internal error: No indices found after ensuring indices updated");
        };

        let perf_rows = self
            .perf
            .as_ref()
            .and_then(|p| p.read().ok())
            .map(|g| g.clone());

        let cache = self.cache.clone();
        let layout = layout.clone();
        Box::new(indices.iter().map(move |idx| {
            let cache = cache.read().expect("ObjectCache lock poisoned");
            let item = &cache[*idx];
            T::inventory_row_for_layout(item, perf_rows.as_ref(), &layout)
        }))
    }

    fn header_row(&self) -> Vec<&'static str> {
        T::header_row()
    }
    fn invalidate(&mut self) {
        // Invalidate the cache to force update indices
        self.indices = None;
    }

    fn item_at_index(&mut self, index: usize) -> Option<(ManagedObjectReference, String)> {
        self.ensure_indices_updated();
        let Some(indices) = &self.indices else {
            return None;
        };
        if index >= indices.len() {
            return None;
        }
        let index = indices[index];
        let cache = self.cache.read().expect("ObjectCache lock poisoned");
        if index >= cache.len() {
            return None;
        }
        let item = cache.index(index);
        Some((item.id().clone(), item.name().clone()))
    }

    fn resource_type(&self) -> ResourceType {
        T::resource_type()
    }

    fn set_perf_snapshot(&mut self, perf: Option<PerfSnapshotShare>) {
        self.perf = perf;
    }
}

#[cfg(test)]
mod tests {
    use super::IndexedCache;
    use crate::resource_browser::host::Host;
    use crate::resource_browser::tabular_data::TableDataSource;
    use crate::resource_browser::vm::VmData;
    use std::sync::{Arc, RwLock};
    use vim_rs::core::pc_cache::{Cache, ObjectCache};
    use vim_rs::types::boxed_types::ValueElements;
    use vim_rs::types::enums::{
        HostSystemConnectionStateEnum, ManagedEntityStatusEnum, MoTypesEnum, ObjectUpdateKindEnum,
        PropertyChangeOpEnum, VirtualMachinePowerStateEnum,
    };
    use vim_rs::types::structs::{
        ManagedObjectReference, ObjectUpdate, PropertyChange, VirtualMachineStorageSummary,
    };
    use vim_rs::types::vim_any::VimAny;

    fn prop_assign(name: &str, val: VimAny) -> PropertyChange {
        PropertyChange {
            name: name.to_string(),
            op: PropertyChangeOpEnum::Assign,
            val: Some(val),
        }
    }

    fn vm_enter(mo_id: &str, display_name: &str, storage_committed: Option<i64>) -> ObjectUpdate {
        let mut changes = vec![
            prop_assign(
                "name",
                VimAny::Value(ValueElements::PrimitiveString(display_name.into())),
            ),
            prop_assign(
                "overallStatus",
                VimAny::Value(ValueElements::ManagedEntityStatus(
                    ManagedEntityStatusEnum::Green,
                )),
            ),
            prop_assign(
                "runtime.powerState",
                VimAny::Value(ValueElements::VirtualMachinePowerState(
                    VirtualMachinePowerStateEnum::PoweredOn,
                )),
            ),
        ];
        if let Some(c) = storage_committed {
            changes.push(prop_assign(
                "summary.storage",
                VimAny::Object(Box::new(VirtualMachineStorageSummary {
                    committed: c,
                    uncommitted: 0,
                    unshared: 0,
                    timestamp: String::new(),
                })),
            ));
        }
        ObjectUpdate {
            kind: ObjectUpdateKindEnum::Enter,
            obj: ManagedObjectReference {
                r#type: MoTypesEnum::VirtualMachine,
                value: mo_id.into(),
            },
            change_set: Some(changes),
            missing_set: None,
        }
    }

    fn host_enter(mo_id: &str, display_name: &str) -> ObjectUpdate {
        ObjectUpdate {
            kind: ObjectUpdateKindEnum::Enter,
            obj: ManagedObjectReference {
                r#type: MoTypesEnum::HostSystem,
                value: mo_id.into(),
            },
            change_set: Some(vec![
                prop_assign(
                    "name",
                    VimAny::Value(ValueElements::PrimitiveString(display_name.into())),
                ),
                prop_assign(
                    "summary.overallStatus",
                    VimAny::Value(ValueElements::ManagedEntityStatus(
                        ManagedEntityStatusEnum::Green,
                    )),
                ),
                prop_assign(
                    "runtime.connectionState",
                    VimAny::Value(ValueElements::HostSystemConnectionState(
                        HostSystemConnectionStateEnum::Connected,
                    )),
                ),
            ]),
            missing_set: None,
        }
    }

    #[test]
    fn indexed_vm_cache_filter_len_and_total_count() {
        let cache = Arc::new(RwLock::new(ObjectCache::<VmData>::new()));
        cache
            .write()
            .expect("lock")
            .process_update(vec![
                vm_enter("vm-1", "antelope", None),
                vm_enter("vm-2", "bee", None),
                vm_enter("vm-3", "cat", None),
            ])
            .expect("process_update");

        let mut idx = IndexedCache::new(cache);
        assert_eq!(idx.total_count(), 3);
        assert_eq!(idx.len(), 3);

        idx.set_filter(Some("a".into()));
        assert_eq!(idx.total_count(), 3);
        assert_eq!(idx.len(), 2);

        idx.set_filter(None);
        assert_eq!(idx.len(), 3);
    }

    #[test]
    fn indexed_vm_cache_sort_by_name_and_item_at_index() {
        let cache = Arc::new(RwLock::new(ObjectCache::<VmData>::new()));
        cache
            .write()
            .expect("lock")
            .process_update(vec![
                vm_enter("vm-z", "zebra", None),
                vm_enter("vm-a", "antelope", None),
            ])
            .expect("process_update");

        let mut idx = IndexedCache::new(cache);
        idx.set_sort_setting(3, false);
        let (id, name) = idx.item_at_index(0).expect("row 0");
        assert_eq!(id.value, "vm-a");
        assert_eq!(name, "antelope");
        let (id2, _) = idx.item_at_index(1).expect("row 1");
        assert_eq!(id2.value, "vm-z");
    }

    #[test]
    fn indexed_vm_cache_sort_by_storage_column() {
        let cache = Arc::new(RwLock::new(ObjectCache::<VmData>::new()));
        cache
            .write()
            .expect("lock")
            .process_update(vec![
                vm_enter("vm-big", "b", Some(10_000)),
                vm_enter("vm-small", "s", Some(100)),
            ])
            .expect("process_update");

        let mut idx = IndexedCache::new(cache);
        idx.set_sort_setting(5, false);
        let (id, _) = idx.item_at_index(0).expect("row 0");
        assert_eq!(id.value, "vm-small");
    }

    #[test]
    fn indexed_host_cache_sort_by_id() {
        let cache = Arc::new(RwLock::new(ObjectCache::<Host>::new()));
        cache
            .write()
            .expect("lock")
            .process_update(vec![host_enter("host-b", "B"), host_enter("host-a", "A")])
            .expect("process_update");

        let mut idx = IndexedCache::new(cache);
        idx.set_sort_setting(0, false);
        let (id, _) = idx.item_at_index(0).expect("row 0");
        assert_eq!(id.value, "host-a");
    }
}

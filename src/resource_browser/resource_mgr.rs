use super::events;
use crate::event::{AppEvent, EventHandler};
use crate::resource_browser::cluster::ClusterDetails;
use crate::resource_browser::data_loaders;
use crate::resource_browser::datastore::{DatastoreDetails, get_datastore_hosts};
use crate::resource_browser::hints::{HELP_HINTS, HELP_HINTS_EVENTS, get_expand_hint};
use crate::resource_browser::host::Host;
use crate::resource_browser::network::NetworkDetails;
use crate::resource_browser::resource_table::ResourceTableWidget;
use crate::resource_browser::tabular_data::TableDataSource;
use crate::resource_browser::task::{TaskInfo, ensure_task_descriptions_initialized};
use crate::resource_browser::vm::VmData;
use crate::resource_type::ResourceType;
use anyhow::anyhow;
use crossterm::event::{KeyCode, KeyEvent};
use log::{debug, info, warn};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::TableState;
use std::cell::RefCell;
use std::rc::Rc;
use vim_rs::core::client::VimClientHandle;
use vim_rs::core::pc_cache::CacheManager;
use vim_rs::mo::EventHistoryCollector;
use vim_rs::types::enums::MoTypesEnum;
use vim_rs::types::structs::ManagedObjectReference;

/// Extra teardown beyond removing the PropertyCollector filter (e.g. session event collectors).
#[derive(Debug)]
pub(crate) enum ResourceCleanup {
    None,
    EventCollector(ManagedObjectReference),
}

pub struct ResourceManager {
    /// Cache manager for managing object caches.
    cache_mgr: Rc<RefCell<CacheManager>>,
    /// Client for interacting with the vSphere API.
    client: VimClientHandle,
    /// Data source for the table view.
    resources: Box<dyn TableDataSource>,
    /// PropertyCollector filter for the current view
    filter: ManagedObjectReference,
    /// Ratatui Table state for managing the current selection and scroll position.
    table_state: TableState,
    /// Parent object reference for the current view when expanding a sub collection e.g. VMs in host.
    parent: Option<(ManagedObjectReference, String)>,
    /// Table state to apply after data is loaded.
    pending_table_state: Option<(usize, Option<usize>)>, // (offset, selected_index)
    /// Session objects that must be destroyed when leaving this table source.
    source_cleanup: ResourceCleanup,
}

#[derive(Debug)]
pub struct HistoryRecord {
    resource_type: ResourceType,
    parent: Option<(ManagedObjectReference, String)>,
    selected_index: Option<usize>,
    offset: usize,
    search_filter: Option<String>,
    sort: Option<(usize, bool)>,
}

impl HistoryRecord {
    fn from_current_state(resource_mgr: &ResourceManager) -> Self {
        Self {
            resource_type: resource_mgr.resource_type(),
            parent: resource_mgr.parent.clone(),
            selected_index: resource_mgr.table_state.selected(),
            offset: resource_mgr.table_state.offset(),
            search_filter: resource_mgr.resources.get_filter(),
            sort: resource_mgr.resources.get_sort_setting(),
        }
    }
}

impl ResourceManager {
    /// Creates a new ResourceManager instance i.e. table view. It automatically loads virtual
    /// machine table at the start.
    ///
    /// # Arguments
    ///
    /// * `client` - A reference to the vSphere API client.
    /// * `cache_mgr` - A reference to the cache manager for managing object caches.
    pub async fn new(
        client: VimClientHandle,
        cache_mgr: Rc<RefCell<CacheManager>>,
        resource_type: ResourceType,
    ) -> anyhow::Result<Self> {
        debug!(
            "Creating resource manager for resource type: {}",
            resource_type
        );
        let (resources, filter, source_cleanup) =
            Self::load_from_container(resource_type, cache_mgr.clone(), &client).await?;

        Ok(Self {
            cache_mgr,
            client,
            resources,
            filter,
            table_state: TableState::default(),
            parent: None,
            pending_table_state: None,
            source_cleanup,
        })
    }

    pub async fn from_history_record(
        record: HistoryRecord,
        client: VimClientHandle,
        cache_mgr: Rc<RefCell<CacheManager>>,
    ) -> anyhow::Result<Self> {
        debug!(
            "Creating resource manager from history record. Resource type: {}",
            record.resource_type
        );
        let (mut resources, filter, source_cleanup) = if let Some(ref parent) = record.parent {
            Self::load_parent_collection(
                record.resource_type,
                &parent.0,
                cache_mgr.clone(),
                &client,
            )
            .await?
        } else {
            Self::load_from_container(record.resource_type, cache_mgr.clone(), &client).await?
        };

        // Apply text filter and sort settings
        resources.set_filter(record.search_filter);
        if let Some((column, descending)) = record.sort {
            resources.set_sort_setting(column, descending);
        } else {
            resources.set_sort_column(None);
        }

        // Make sure the data reflects our settings
        resources.invalidate();

        Ok(Self {
            cache_mgr,
            client,
            resources,
            filter,
            table_state: TableState::default(),
            parent: record.parent,
            pending_table_state: Some((record.offset, record.selected_index)),
            source_cleanup,
        })
    }

    pub async fn load_history_record(
        &mut self,
        previous_state: HistoryRecord,
    ) -> anyhow::Result<()> {
        if let Some(parent) = previous_state.parent {
            self.expand_parent_collection(previous_state.resource_type, &parent.0, parent.1)
                .await?;
        } else {
            self.parent = None;
            self.load_resource_type_int(previous_state.resource_type)
                .await?;
        }

        // Store the table state to be applied after data is loaded
        self.pending_table_state = Some((previous_state.offset, previous_state.selected_index));

        self.resources.set_filter(previous_state.search_filter);
        if let Some((column, descending)) = previous_state.sort {
            self.resources.set_sort_setting(column, descending);
        } else {
            self.resources.set_sort_column(None);
        }
        self.resources.invalidate();

        Ok(())
    }
    pub fn save_state(&mut self, events: &mut EventHandler) {
        let record = HistoryRecord::from_current_state(self);
        events.send(AppEvent::ResourceManagerHistory(record));
    }
    pub fn set_filter(&mut self, filter: Option<String>) {
        self.resources.set_filter(filter)
    }

    pub fn invalidate(&mut self) {
        self.resources.invalidate();
        // Apply any pending table state after data is loaded
        if let Some((offset, selected)) = self.pending_table_state.take() {
            self.table_state = TableState::default()
                .with_offset(offset)
                .with_selected(selected);
        }
    }

    pub fn resource_type(&self) -> ResourceType {
        self.resources.resource_type()
    }

    pub fn render(&mut self, frame: &mut Frame, body_area: Rect) {
        let table = ResourceTableWidget::new(self.resources.as_mut(), &self.parent);
        frame.render_stateful_widget(table, body_area, &mut self.table_state);
    }

    pub async fn handle_key(
        &mut self,
        key: &KeyEvent,
        events: &mut EventHandler,
    ) -> anyhow::Result<bool> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.table_state.scroll_down_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.table_state.scroll_up_by(1),

            KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                // Convert char to column index (0-based)
                let column_idx = c.to_digit(10).unwrap() as usize - 1;
                self.resources.set_sort_column(Some(column_idx));
            }
            KeyCode::Char('0') => self.resources.set_sort_column(None),

            // Add shortcut keys to sub-collections - (n)etwork, (d)atastore, (h)ost, (v)m, (c)luster
            KeyCode::Char('n') => {
                self.expand_collection(ResourceType::Network, events)
                    .await?
            }
            KeyCode::Char('d') => {
                self.expand_collection(ResourceType::Datastore, events)
                    .await?
            }
            KeyCode::Char('h') => self.expand_collection(ResourceType::Host, events).await?,
            KeyCode::Char('v') => {
                self.expand_collection(ResourceType::VirtualMachine, events)
                    .await?
            }
            //KeyCode::Char('c') => self.events.send(AppEvent::ExpandCollection(ResourceType::Cluster)),
            KeyCode::Char('t') => self.expand_collection(ResourceType::Task, events).await?,
            KeyCode::Char('e') => self.expand_collection(ResourceType::Event, events).await?,
            KeyCode::Char('x') if self.resource_type() == ResourceType::VirtualMachine => {
                if let Some((vm_ref, _)) = self.selected_item() {
                    events.send(AppEvent::OpenVmActions(vm_ref));
                }
            }
            KeyCode::Char('/') => events.send(AppEvent::OpenSearch),
            KeyCode::Esc => self.set_filter(None),
            KeyCode::Enter => {
                if self.resource_type() == ResourceType::Event {
                    if let Some(sel) = self.table_state.selected()
                        && let Some(payload) = self.resources.take_event_browser_payload_at(sel)
                    {
                        self.save_state(events);
                        events.send(AppEvent::LoadEventProperties(Box::new(payload)));
                    }
                } else if let Some((selected_id, _)) = self.selected_item() {
                    self.save_state(events);
                    events.send(AppEvent::LoadProperties(selected_id));
                }
            }
            _ => {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(crate) async fn load_resource_type(
        &mut self,
        resource_type: ResourceType,
        events: &mut EventHandler,
    ) -> anyhow::Result<()> {
        // Save the current navigation state
        self.save_state(events);
        self.load_resource_type_int(resource_type).await
    }

    /// Returns hints tuple for the current resource type. First element are the left column hints,
    /// second element are the right column hints.
    pub fn get_hints(&self) -> (&'static [&'static str], &'static [&'static str]) {
        let help = if self.resource_type() == ResourceType::Event {
            HELP_HINTS_EVENTS
        } else {
            HELP_HINTS
        };
        (get_expand_hint(self.resource_type()), help)
    }

    async fn expand_collection(
        &mut self,
        resource_type: ResourceType,
        events: &mut EventHandler,
    ) -> anyhow::Result<()> {
        // Save the current navigation state
        self.save_state(events);
        // Read the id of the currently selected resource
        let Some((selected_id, selected_name)) = self.selected_item() else {
            return Ok(());
        };

        self.expand_parent_collection(resource_type, &selected_id, selected_name)
            .await
    }

    fn selected_item(&mut self) -> Option<(ManagedObjectReference, String)> {
        let selected = self.table_state.selected()?;
        let (selected_id, selected_name) = self.resources.item_at_index(selected)?;
        Some((selected_id, selected_name))
    }

    async fn expand_parent_collection(
        &mut self,
        resource_type: ResourceType,
        parent_id: &ManagedObjectReference,
        parent_name: String,
    ) -> anyhow::Result<()> {
        let cache_mgr = self.cache_mgr.clone();
        let client = &self.client;

        info!(
            "Expanding collection: resource: {}, parent: {} [{:?}]",
            resource_type, parent_name, parent_id
        );
        let res = Self::load_parent_collection(resource_type, parent_id, cache_mgr, client).await;

        match res {
            Ok((resources, filter, cleanup)) => {
                self.apply_new_table_source(resources, filter, cleanup)
                    .await?;
                self.parent = Some((parent_id.clone(), parent_name));
                Ok(())
            }
            Err(err) => {
                // Check if it's our specific error type
                if let Some(ResourceError::UnsupportedExpansion {
                    resource_type,
                    parent_type,
                }) = err.downcast_ref::<ResourceError>()
                {
                    debug!(
                        "Ignoring unsupported expansion: resource: {}, prent: {}",
                        resource_type, parent_type
                    );
                    Ok(())
                } else {
                    // Unknown/network errors - propagate up
                    warn!(
                        "Failed to expand collection: resource: {}, parent: {} [{:?}]: {}",
                        resource_type, parent_name, parent_id, err
                    );
                    Err(err)
                }
            }
        }
    }

    async fn load_parent_collection(
        resource_type: ResourceType,
        parent_id: &ManagedObjectReference,
        cache_mgr: Rc<RefCell<CacheManager>>,
        client: &VimClientHandle,
    ) -> anyhow::Result<(
        Box<dyn TableDataSource>,
        ManagedObjectReference,
        ResourceCleanup,
    )> {
        match resource_type {
            ResourceType::VirtualMachine => match parent_id.r#type {
                MoTypesEnum::HostSystem
                | MoTypesEnum::Datastore
                | MoTypesEnum::Network
                | MoTypesEnum::DistributedVirtualPortgroup
                | MoTypesEnum::OpaqueNetwork => {
                    data_loaders::load_from_property::<VmData>(cache_mgr, parent_id, "vm")
                        .await
                        .map(|(a, b)| (a, b, ResourceCleanup::None))
                }
                MoTypesEnum::ClusterComputeResource => {
                    data_loaders::load_from_container::<VmData>(cache_mgr, parent_id)
                        .await
                        .map(|(a, b)| (a, b, ResourceCleanup::None))
                }
                _ => {
                    let r#type = parent_id.r#type.as_str();
                    Err(ResourceError::UnsupportedExpansion {
                        resource_type,
                        parent_type: r#type.to_string(),
                    }
                    .into())
                }
            },
            ResourceType::Host => match parent_id.r#type {
                MoTypesEnum::ClusterComputeResource
                | MoTypesEnum::Network
                | MoTypesEnum::DistributedVirtualPortgroup
                | MoTypesEnum::OpaqueNetwork => {
                    data_loaders::load_from_property::<Host>(cache_mgr, parent_id, "host")
                        .await
                        .map(|(a, b)| (a, b, ResourceCleanup::None))
                }
                MoTypesEnum::Datastore => {
                    let hosts = get_datastore_hosts(client.clone(), parent_id).await?;
                    data_loaders::load_from_list::<Host>(cache_mgr, &hosts)
                        .await
                        .map(|(a, b)| (a, b, ResourceCleanup::None))
                }
                _ => {
                    let r#type = parent_id.r#type.as_str();
                    Err(ResourceError::UnsupportedExpansion {
                        resource_type,
                        parent_type: r#type.to_string(),
                    }
                    .into())
                }
            },
            ResourceType::Datastore => match parent_id.r#type {
                MoTypesEnum::ClusterComputeResource | MoTypesEnum::HostSystem => {
                    data_loaders::load_from_property::<DatastoreDetails>(
                        cache_mgr,
                        parent_id,
                        "datastore",
                    )
                    .await
                    .map(|(a, b)| (a, b, ResourceCleanup::None))
                }
                _ => {
                    let r#type = parent_id.r#type.as_str();
                    Err(ResourceError::UnsupportedExpansion {
                        resource_type,
                        parent_type: r#type.to_string(),
                    }
                    .into())
                }
            },
            ResourceType::Cluster => {
                let r#type = parent_id.r#type.as_str();
                Err(ResourceError::UnsupportedExpansion {
                    resource_type,
                    parent_type: r#type.to_string(),
                }
                .into())
            }
            ResourceType::Network => match parent_id.r#type {
                MoTypesEnum::ClusterComputeResource | MoTypesEnum::HostSystem => {
                    data_loaders::load_from_property::<NetworkDetails>(
                        cache_mgr, parent_id, "network",
                    )
                    .await
                    .map(|(a, b)| (a, b, ResourceCleanup::None))
                }
                _ => {
                    let r#type = parent_id.r#type.as_str();
                    Err(ResourceError::UnsupportedExpansion {
                        resource_type,
                        parent_type: r#type.to_string(),
                    }
                    .into())
                }
            },
            ResourceType::Task => {
                ensure_task_descriptions_initialized(client.clone()).await?;
                match parent_id.r#type {
                    MoTypesEnum::ClusterComputeResource
                    | MoTypesEnum::HostSystem
                    | MoTypesEnum::VirtualMachine
                    | MoTypesEnum::Datastore
                    | MoTypesEnum::Network
                    | MoTypesEnum::DistributedVirtualPortgroup
                    | MoTypesEnum::OpaqueNetwork => data_loaders::load_from_property::<TaskInfo>(
                        cache_mgr,
                        parent_id,
                        "recentTask",
                    )
                    .await
                    .map(|(a, b)| (a, b, ResourceCleanup::None)),
                    _ => {
                        let r#type = parent_id.r#type.as_str();
                        Err(ResourceError::UnsupportedExpansion {
                            resource_type,
                            parent_type: r#type.to_string(),
                        }
                        .into())
                    }
                }
            }
            ResourceType::Event => match parent_id.r#type {
                MoTypesEnum::VirtualMachine | MoTypesEnum::HostSystem | MoTypesEnum::Datastore => {
                    events::create_entity_event_view(client.clone(), cache_mgr, parent_id)
                        .await
                        .map(|(a, b, c)| (a, b, ResourceCleanup::EventCollector(c)))
                }
                _ => {
                    let r#type = parent_id.r#type.as_str();
                    Err(ResourceError::UnsupportedExpansion {
                        resource_type,
                        parent_type: r#type.to_string(),
                    }
                    .into())
                }
            },
        }
    }

    async fn load_resource_type_int(&mut self, resource_type: ResourceType) -> anyhow::Result<()> {
        self.parent = None;
        let cache_mgr = self.cache_mgr.clone();
        let client = &self.client;

        let (resources, filter, cleanup) =
            Self::load_from_container(resource_type, cache_mgr, client).await?;
        self.apply_new_table_source(resources, filter, cleanup)
            .await
    }

    async fn load_from_container(
        resource_type: ResourceType,
        cache_mgr: Rc<RefCell<CacheManager>>,
        client: &VimClientHandle,
    ) -> anyhow::Result<(
        Box<dyn TableDataSource>,
        ManagedObjectReference,
        ResourceCleanup,
    )> {
        let parent = client.service_content().root_folder.clone();
        match resource_type {
            ResourceType::VirtualMachine => {
                data_loaders::load_from_container::<VmData>(cache_mgr, &parent)
                    .await
                    .map(|(a, b)| (a, b, ResourceCleanup::None))
            }
            ResourceType::Host => data_loaders::load_from_container::<Host>(cache_mgr, &parent)
                .await
                .map(|(a, b)| (a, b, ResourceCleanup::None)),
            ResourceType::Datastore => {
                data_loaders::load_from_container::<DatastoreDetails>(cache_mgr, &parent)
                    .await
                    .map(|(a, b)| (a, b, ResourceCleanup::None))
            }
            ResourceType::Cluster => {
                data_loaders::load_from_container::<ClusterDetails>(cache_mgr, &parent)
                    .await
                    .map(|(a, b)| (a, b, ResourceCleanup::None))
            }
            ResourceType::Network => {
                data_loaders::load_from_container::<NetworkDetails>(cache_mgr, &parent)
                    .await
                    .map(|(a, b)| (a, b, ResourceCleanup::None))
            }
            ResourceType::Task => {
                let task_manager = client.service_content().task_manager.as_ref();
                let Some(task_manager) = task_manager else {
                    return Err(anyhow!("Task manager not available"));
                };
                ensure_task_descriptions_initialized(client.clone()).await?;
                data_loaders::load_from_property::<TaskInfo>(cache_mgr, task_manager, "recentTask")
                    .await
                    .map(|(a, b)| (a, b, ResourceCleanup::None))
            }
            ResourceType::Event => events::create_global_event_view(client.clone(), cache_mgr)
                .await
                .map(|(a, b, c)| (a, b, ResourceCleanup::EventCollector(c))),
        }
    }

    #[allow(clippy::await_holding_refcell_ref)]
    async fn apply_new_table_source(
        &mut self,
        resources: Box<dyn TableDataSource>,
        filter: ManagedObjectReference,
        new_cleanup: ResourceCleanup,
    ) -> anyhow::Result<()> {
        let old_cleanup = std::mem::replace(&mut self.source_cleanup, new_cleanup);
        self.cache_mgr
            .borrow_mut()
            .remove_cache(&self.filter)
            .await?;
        if let ResourceCleanup::EventCollector(moref) = old_cleanup {
            let ec = EventHistoryCollector::new(self.client.clone(), &moref.value);
            ec.destroy_collector()
                .await
                .map_err(|e| anyhow!("destroy event collector: {e}"))?;
        }
        self.table_state = TableState::default();
        self.resources = resources;
        self.filter = filter;
        Ok(())
    }
}

impl Drop for ResourceManager {
    fn drop(&mut self) {
        let cache_mgr = self.cache_mgr.clone();
        let filter = self.filter.clone();
        let client = self.client.clone();
        let cleanup = std::mem::replace(&mut self.source_cleanup, ResourceCleanup::None);
        tokio::task::block_in_place(|| {
            #[allow(clippy::await_holding_refcell_ref)]
            tokio::runtime::Handle::current().block_on(async move {
                debug!("Terminating ResourceManager. Releasing filter");
                cache_mgr
                    .borrow_mut()
                    .remove_cache(&filter)
                    .await
                    .unwrap_or_else(|e| {
                        warn!(
                            "Failed to remove ResourceManager filter: {:?}, {}",
                            filter, e
                        );
                    });
                if let ResourceCleanup::EventCollector(moref) = cleanup {
                    let ec = EventHistoryCollector::new(client, &moref.value);
                    if let Err(e) = ec.destroy_collector().await {
                        warn!("Failed to destroy event collector {:?}: {}", moref, e);
                    }
                }
            });
        });
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("Cannot expand {resource_type} from parent type {parent_type}")]
    UnsupportedExpansion {
        resource_type: ResourceType,
        parent_type: String,
    },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

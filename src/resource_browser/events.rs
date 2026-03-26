//! Live event table backed by `EventHistoryCollector.latestPage` and PropertyCollector updates.

use crate::resource_browser::formatting::ID_COLUMN_WIDTH;
use crate::resource_browser::tabular_data::TableDataSource;
use crate::resource_type::ResourceType;
use anyhow::Context;
use chrono::{DateTime, Local};
use log::warn;
use ratatui::layout::Constraint;
use ratatui::widgets::{Cell, Row};
use std::cmp::Ordering;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use vim_rs::core::client::Client;
use vim_rs::core::error::Result as VimResult;
use vim_rs::core::pc_cache::{Cache, CacheManager, ReadWriteCacheProxy};
use vim_rs::mo::{EventHistoryCollector, EventManager};
use vim_rs::types::boxed_types::ValueElements;
use vim_rs::types::enums::{EventFilterSpecRecursionOptionEnum, MoTypesEnum, PropertyChangeOpEnum};
use vim_rs::types::struct_enum::StructType;
use vim_rs::types::structs::{
    Event, EventFilterSpec, EventFilterSpecByEntity, ManagedObjectReference, ObjectSpec,
    ObjectUpdate, PropertyChange, PropertySpec,
};
use vim_rs::types::vim_any::VimAny;

const EVENT_PAGE_SIZE: i32 = 200;

/// Synthetic row id — not a server MoRef; never pass to `LoadProperties`.
fn synthetic_event_row_id(key: i32) -> ManagedObjectReference {
    ManagedObjectReference {
        r#type: MoTypesEnum::Other_("EventRow".to_string()),
        value: format!("event-{key}"),
    }
}

pub fn decode_event_type(event: &Event) -> String {
    let Some(type_) = event.type_ else {
        return "Event".to_string();
    };
    if (type_.child_of(StructType::EventEx) || type_.child_of(StructType::ExtendedEvent))
        && let Some(miniserde::json::Value::String(event_type_id)) =
            event.extra_fields_.get("eventTypeId")
    {
        return event_type_id.clone();
    }
    type_.as_str().to_string()
}

fn decode_event_description(event: &Event) -> String {
    if let Some(ref m) = event.full_formatted_message
        && !m.is_empty()
    {
        return m.clone();
    }
    if let Some(miniserde::json::Value::String(msg)) = event.extra_fields_.get("message") {
        return msg.clone();
    }
    "-".to_string()
}

#[derive(Debug, Clone)]
pub struct DecodedMainObject {
    pub type_label: String,
    pub id: String,
    pub name: Option<String>,
    pub moref: Option<ManagedObjectReference>,
}

fn entity_arg(
    type_label: &str,
    name: &str,
    moref: &ManagedObjectReference,
) -> DecodedMainObject {
    DecodedMainObject {
        type_label: type_label.to_string(),
        id: moref.value.clone(),
        name: if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        },
        moref: Some(moref.clone()),
    }
}

fn moref_from_json_value(v: &miniserde::json::Value) -> Option<ManagedObjectReference> {
    let miniserde::json::Value::Object(map) = v else {
        return None;
    };
    let t = map.get("type").and_then(|x| match x {
        miniserde::json::Value::String(s) => Some(MoTypesEnum::from_str(s)),
        _ => None,
    })?;
    let val = map.get("value").and_then(|x| match x {
        miniserde::json::Value::String(s) => Some(s.clone()),
        _ => None,
    })?;
    Some(ManagedObjectReference { r#type: t, value: val })
}

pub fn decode_main_object(event: &Event) -> DecodedMainObject {
    // ExtendedEvent.managedObject
    if let Some(v) = event.extra_fields_.get("managedObject")
        && let Some(moref) = moref_from_json_value(v)
    {
        return DecodedMainObject {
            type_label: moref.r#type.as_str().to_string(),
            id: moref.value.clone(),
            name: None,
            moref: Some(moref),
        };
    }
    // EventEx
    if let (
        Some(miniserde::json::Value::String(ot)),
        Some(miniserde::json::Value::String(oid)),
    ) = (
        event.extra_fields_.get("objectType"),
        event.extra_fields_.get("objectId"),
    ) {
        let name = event
            .extra_fields_
            .get("objectName")
            .and_then(|v| match v {
                miniserde::json::Value::String(s) => Some(s.clone()),
                _ => None,
            });
        let moref = Some(ManagedObjectReference {
            r#type: MoTypesEnum::from_str(ot),
            value: oid.clone(),
        });
        return DecodedMainObject {
            type_label: ot.clone(),
            id: oid.clone(),
            name,
            moref,
        };
    }
    if let Some(ref vm) = event.vm {
        return entity_arg("VM", &vm.name, &vm.vm);
    }
    if let Some(ref host) = event.host {
        return entity_arg("Host", &host.name, &host.host);
    }
    if let Some(ref ds) = event.ds {
        return entity_arg("DS", &ds.name, &ds.datastore);
    }
    if let Some(ref net) = event.net {
        return entity_arg("Net", &net.name, &net.network);
    }
    if let Some(ref cr) = event.compute_resource {
        return entity_arg("CR", &cr.name, &cr.compute_resource);
    }
    if let Some(ref dc) = event.datacenter {
        return entity_arg("DC", &dc.name, &dc.datacenter);
    }
    if let Some(ref dvs) = event.dvs {
        return entity_arg("DVS", &dvs.name, &dvs.dvs);
    }
    DecodedMainObject {
        type_label: "-".to_string(),
        id: "-".to_string(),
        name: None,
        moref: None,
    }
}

fn format_event_time(created_time: &str) -> Cell<'static> {
    let Ok(dt) = DateTime::parse_from_rfc3339(created_time) else {
        return Cell::from(created_time.to_string());
    };
    let local = dt.with_timezone(&Local);
    Cell::from(local.format("%b %d %H:%M:%S").to_string())
}

fn compare_event_time(a: &EventRow, b: &EventRow) -> Ordering {
    let pa = DateTime::parse_from_rfc3339(&a.created_time).ok();
    let pb = DateTime::parse_from_rfc3339(&b.created_time).ok();
    pa.cmp(&pb)
}

pub struct EventRow {
    pub id: ManagedObjectReference,
    pub event: Event,
    pub event_key: i32,
    pub event_type: String,
    pub description: String,
    pub created_time: String,
    pub main_object_type: String,
    pub main_object_id: String,
    pub main_object_name: Option<String>,
    pub main_object_ref: Option<ManagedObjectReference>,
}

impl EventRow {
    fn from_event(event: Event) -> Self {
        let key = event.key;
        let main = decode_main_object(&event);
        Self {
            id: synthetic_event_row_id(key),
            event_type: decode_event_type(&event),
            description: decode_event_description(&event),
            created_time: event.created_time.clone(),
            main_object_type: main.type_label,
            main_object_id: main.id,
            main_object_name: main.name,
            main_object_ref: main.moref,
            event,
            event_key: key,
        }
    }

    fn row_label(&self) -> String {
        format!("{} [{}]", self.event_type, self.event_key)
    }

    fn matches_filter(&self, filter: &str) -> bool {
        let f = filter.to_lowercase();
        self.event_key.to_string().contains(&f)
            || self.event_type.to_lowercase().contains(&f)
            || self.description.to_lowercase().contains(&f)
            || self.created_time.to_lowercase().contains(&f)
            || self.main_object_type.to_lowercase().contains(&f)
            || self.main_object_id.to_lowercase().contains(&f)
            || self
                .main_object_name
                .as_ref()
                .map(|n| n.to_lowercase().contains(&f))
                .unwrap_or(false)
    }
}

impl From<&EventRow> for Row<'static> {
    fn from(r: &EventRow) -> Self {
        let object_name = r
            .main_object_name
            .clone()
            .unwrap_or_else(|| "-".to_string());
        Row::new(vec![
            Cell::from(r.event_key.to_string()),
            Cell::from(r.event_type.clone()),
            Cell::from(r.description.clone()),
            format_event_time(&r.created_time),
            Cell::from(r.main_object_type.clone()),
            Cell::from(object_name),
            Cell::from(r.main_object_id.clone()),
        ])
    }
}

pub struct EventTableState {
    rows: Vec<EventRow>,
}

impl EventTableState {
    fn new() -> Self {
        Self { rows: Vec::new() }
    }

    fn replace_rows(&mut self, events: Vec<Event>) {
        self.rows = events.into_iter().map(EventRow::from_event).collect();
    }
}

pub struct EventCollectorCache {
    collector: ManagedObjectReference,
    state: Arc<RwLock<EventTableState>>,
}

impl EventCollectorCache {
    fn new(collector: ManagedObjectReference, state: Arc<RwLock<EventTableState>>) -> Self {
        Self { collector, state }
    }
}

impl Cache for EventCollectorCache {
    fn prop_spec(&self) -> VimResult<PropertySpec> {
        Ok(PropertySpec {
            r#type: MoTypesEnum::EventHistoryCollector.as_str().to_string(),
            all: Some(false),
            path_set: Some(vec!["latestPage".to_string()]),
        })
    }

    fn process_update(&mut self, updates: Vec<ObjectUpdate>) -> VimResult<()> {
        for update in updates {
            if update.obj.r#type != MoTypesEnum::EventHistoryCollector
                || update.obj.value != self.collector.value
            {
                continue;
            }
            let Some(changes) = update.change_set else {
                continue;
            };
            for ch in changes {
                Self::apply_latest_page_change(&self.state, ch);
            }
        }
        Ok(())
    }
}

impl EventCollectorCache {
    fn apply_latest_page_change(state: &Arc<RwLock<EventTableState>>, ch: PropertyChange) {
        if ch.name != "latestPage" {
            return;
        }
        match ch.op {
            PropertyChangeOpEnum::Assign | PropertyChangeOpEnum::Add => {
                let Some(val) = ch.val else {
                    return;
                };
                match val {
                    VimAny::Value(ValueElements::ArrayOfEvent(events)) => {
                        if let Ok(mut g) = state.write() {
                            g.replace_rows(events);
                        }
                    }
                    _ => warn!("latestPage: expected VimAny::Value(ArrayOfEvent)"),
                }
            }
            PropertyChangeOpEnum::Remove | PropertyChangeOpEnum::IndirectRemove => {
                if let Ok(mut g) = state.write() {
                    g.replace_rows(Vec::new());
                }
            }
            PropertyChangeOpEnum::Other_(..) => {}
        }
    }
}

pub struct EventTableDataSource {
    state: Arc<RwLock<EventTableState>>,
    indices: Option<Vec<usize>>,
    filter: Option<String>,
    sort_column: Option<usize>,
    sort_descending: bool,
}

impl EventTableDataSource {
    pub fn new(state: Arc<RwLock<EventTableState>>) -> Self {
        Self {
            state,
            indices: None,
            filter: None,
            sort_column: None,
            sort_descending: false,
        }
    }

    fn ensure_indices_updated(&mut self) {
        if self.indices.is_none() {
            self.update_indices();
        }
    }

    fn update_indices(&mut self) {
        let guard = self.state.read().expect("EventTableState lock poisoned");
        let mut indices: Vec<usize> = (0..guard.rows.len()).collect();
        if let Some(ref filter) = self.filter {
            let f = filter.as_str();
            indices.retain(|&i| guard.rows[i].matches_filter(f));
        }
        if let Some(column) = self.sort_column {
            let descending = self.sort_descending;
            let cmp = |a: &usize, b: &usize| -> Ordering {
                let ra = &guard.rows[*a];
                let rb = &guard.rows[*b];
                let o = match column {
                    0 => ra.event_key.cmp(&rb.event_key),
                    1 => ra.event_type.cmp(&rb.event_type),
                    2 => ra.description.cmp(&rb.description),
                    3 => compare_event_time(ra, rb),
                    4 => ra
                        .main_object_type
                        .cmp(&rb.main_object_type)
                        .then_with(|| {
                            ra.main_object_name
                                .as_deref()
                                .unwrap_or("")
                                .cmp(rb.main_object_name.as_deref().unwrap_or(""))
                        })
                        .then_with(|| ra.main_object_id.cmp(&rb.main_object_id)),
                    5 => ra
                        .main_object_name
                        .as_deref()
                        .unwrap_or("")
                        .cmp(rb.main_object_name.as_deref().unwrap_or(""))
                        .then_with(|| ra.main_object_id.cmp(&rb.main_object_id)),
                    6 => ra.main_object_id.cmp(&rb.main_object_id),
                    _ => Ordering::Equal,
                };
                if descending {
                    o.reverse()
                } else {
                    o
                }
            };
            indices.sort_by(cmp);
        }
        self.indices = Some(indices);
    }
}

impl TableDataSource for EventTableDataSource {
    fn get_title(&self) -> &'static str {
        "Events"
    }

    fn set_filter(&mut self, filter: Option<String>) {
        self.filter = filter;
        self.invalidate();
    }

    fn get_filter(&self) -> Option<String> {
        self.filter.clone()
    }

    fn set_sort_column(&mut self, column: Option<usize>) {
        if let Some(sort_column) = column
            && !Self::sortable_columns().contains(&sort_column)
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
        if !Self::sortable_columns().contains(&column) {
            return;
        }
        self.sort_column = Some(column);
        self.sort_descending = descending;
        self.invalidate();
    }

    fn iter<'a>(&'a mut self) -> Box<dyn Iterator<Item = Row<'static>> + 'a> {
        self.ensure_indices_updated();
        let indices = self.indices.clone().unwrap_or_default();
        let state = self.state.clone();
        Box::new(indices.into_iter().map(move |idx| {
            let g = state.read().expect("EventTableState lock poisoned");
            Row::from(&g.rows[idx])
        }))
    }

    fn is_empty(&mut self) -> bool {
        self.ensure_indices_updated();
        self.indices.as_ref().map(|i| i.is_empty()).unwrap_or(true)
    }

    fn len(&mut self) -> usize {
        self.ensure_indices_updated();
        self.indices.as_ref().map(|i| i.len()).unwrap_or(0)
    }

    fn total_count(&self) -> usize {
        self.state
            .read()
            .expect("EventTableState lock poisoned")
            .rows
            .len()
    }

    fn column_sizes(&self) -> Vec<Constraint> {
        vec![
            Constraint::Length(10),
            Constraint::Max(28),
            Constraint::Fill(1),
            Constraint::Length(18),
            Constraint::Max(12),
            Constraint::Max(30),
            Constraint::Length(ID_COLUMN_WIDTH),
        ]
    }

    fn header_row(&self) -> Vec<&'static str> {
        vec![
            "Key ",
            "Type ",
            "Description ",
            "Time ",
            "Object ",
            "Name ",
            "Object ID ",
        ]
    }

    fn invalidate(&mut self) {
        self.indices = None;
    }

    fn item_at_index(&mut self, index: usize) -> Option<(ManagedObjectReference, String)> {
        self.ensure_indices_updated();
        let indices = self.indices.as_ref()?;
        let idx = *indices.get(index)?;
        let g = self.state.read().ok()?;
        let row = g.rows.get(idx)?;
        Some((row.id.clone(), row.row_label()))
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Event
    }
}

impl EventTableDataSource {
    fn sortable_columns() -> Vec<usize> {
        vec![0, 1, 2, 3, 4, 5, 6]
    }
}

fn base_event_filter(
    entity: Option<EventFilterSpecByEntity>,
) -> EventFilterSpec {
    EventFilterSpec {
        entity,
        time: None,
        user_name: None,
        event_chain_id: None,
        alarm: None,
        scheduled_task: None,
        disable_full_message: Some(false),
        category: None,
        r#type: None,
        tag: None,
        event_type_id: None,
        max_count: Some(EVENT_PAGE_SIZE),
        delayed_init: Some(false),
    }
}

#[allow(clippy::await_holding_refcell_ref)]
async fn create_event_table(
    client: Arc<Client>,
    cache_mgr: Rc<RefCell<CacheManager>>,
    filter: EventFilterSpec,
) -> anyhow::Result<(
    Box<dyn TableDataSource>,
    ManagedObjectReference,
    ManagedObjectReference,
)> {
    let event_manager_moref = client
        .service_content()
        .event_manager
        .clone()
        .context("EventManager not available")?;
    let em = EventManager::new(client.clone(), &event_manager_moref.value);
    let collector_moref = em
        .create_collector_for_events(&filter)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let ehc = EventHistoryCollector::new(client.clone(), &collector_moref.value);
    ehc.set_collector_page_size(EVENT_PAGE_SIZE)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let state = Arc::new(RwLock::new(EventTableState::new()));
    if let Some(initial) = ehc
        .latest_page()
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
    {
        state.write().expect("lock").replace_rows(initial);
    }

    let cache = Arc::new(RwLock::new(EventCollectorCache::new(
        collector_moref.clone(),
        state.clone(),
    )));
    let pc_filter = cache_mgr
        .borrow_mut()
        .add_cache(
            Box::new(ReadWriteCacheProxy::new(cache)),
            vec![ObjectSpec {
                obj: collector_moref.clone(),
                skip: Some(false),
                select_set: None,
            }],
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    Ok((
        Box::new(EventTableDataSource::new(state)),
        pc_filter,
        collector_moref,
    ))
}

pub async fn create_global_event_view(
    client: Arc<Client>,
    cache_mgr: Rc<RefCell<CacheManager>>,
) -> anyhow::Result<(
    Box<dyn TableDataSource>,
    ManagedObjectReference,
    ManagedObjectReference,
)> {
    let root = client.service_content().root_folder.clone();
    let filter = base_event_filter(Some(EventFilterSpecByEntity {
        entity: root,
        recursion: EventFilterSpecRecursionOptionEnum::All,
    }));
    create_event_table(client, cache_mgr, filter).await
}

pub async fn create_entity_event_view(
    client: Arc<Client>,
    cache_mgr: Rc<RefCell<CacheManager>>,
    entity: &ManagedObjectReference,
) -> anyhow::Result<(
    Box<dyn TableDataSource>,
    ManagedObjectReference,
    ManagedObjectReference,
)> {
    let filter = base_event_filter(Some(EventFilterSpecByEntity {
        entity: entity.clone(),
        recursion: EventFilterSpecRecursionOptionEnum::Self_,
    }));
    create_event_table(client, cache_mgr, filter).await
}

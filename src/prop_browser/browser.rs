use super::json_to_tree::{get_type_name, property_to_tree_item};
use super::prop_utils::to_json_value;
use std::fs::File;

use chrono::{Local, SecondsFormat};
use indexmap::IndexMap;
use log::{debug, warn};
use miniserde::json::{Object, Value};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, ScrollbarOrientation, StatefulWidget};
use std::borrow::Cow;
use std::io::Write;
use std::mem;
use std::path::PathBuf;
use tui_tree_widget::{Scrollbar, Tree, TreeItem, TreeState};
use vim_rs::core::pc_cache::Cache;
use vim_rs::core::pc_helpers::Error;
use vim_rs::types::enums::{ObjectUpdateKindEnum, PropertyChangeOpEnum};
use vim_rs::types::structs::{ManagedObjectReference, ObjectUpdate, PropertyChange, PropertySpec};

/// Title line and JSON dump filename stem (see `PropertyBrowserState::generate_json_filename`).
#[derive(Debug, Clone)]
pub struct BrowserMetadata {
    pub title: String,
    pub dump_prefix: String,
}

impl BrowserMetadata {
    pub fn for_managed_object(obj: &ManagedObjectReference) -> Self {
        Self {
            title: String::new(),
            dump_prefix: format!("{}_{}", obj.r#type.as_str(), obj.value),
        }
    }
}

pub struct PropertyBrowserState {
    /// When `Some`, this view is backed by PropertyCollector for that managed object.
    obj: Option<ManagedObjectReference>,
    metadata: BrowserMetadata,
    /// Properties of the current view.
    properties: IndexMap<String, Value>,
    /// Data source for the tree view.
    items: Vec<TreeItem<'static, String>>,
    /// Tree state for managing the current selection and scroll position.
    state: TreeState<String>,
}

impl PropertyBrowserState {
    pub async fn new(
        obj: ManagedObjectReference,
        tree_state: Option<TreeState<String>>,
    ) -> anyhow::Result<Self> {
        let metadata = BrowserMetadata::for_managed_object(&obj);
        Ok(Self {
            obj: Some(obj),
            metadata,
            properties: IndexMap::new(),
            items: Vec::new(),
            state: tree_state.unwrap_or_default(),
        })
    }

    /// Static snapshot (e.g. event data object): no PropertyCollector, no managed-object `obj`.
    pub fn from_static_json(
        metadata: BrowserMetadata,
        root: Object,
        tree_state: Option<TreeState<String>>,
    ) -> anyhow::Result<Self> {
        let mut s = Self {
            obj: None,
            metadata: BrowserMetadata {
                title: String::new(),
                dump_prefix: String::new(),
            },
            properties: IndexMap::new(),
            items: Vec::new(),
            state: TreeState::default(),
        };
        let _ = s.load_json_root(metadata, None, root, tree_state);
        Ok(s)
    }

    /// Replace content from a JSON object (top-level keys become tree roots, like PC properties).
    pub fn load_json_root(
        &mut self,
        metadata: BrowserMetadata,
        obj: Option<ManagedObjectReference>,
        root: Object,
        new_tree_state: Option<TreeState<String>>,
    ) -> TreeState<String> {
        self.metadata = metadata;
        self.obj = obj;
        self.properties = root.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        self.items = self
            .properties
            .iter()
            .map(|(k, v)| property_to_tree_item(k.clone(), v))
            .collect();
        let prev = self.replace_tree_state(new_tree_state);
        if self.state.selected().is_empty() && !self.items.is_empty() {
            self.state = self.clean_state();
        }
        prev
    }

    pub fn set_obj(
        &mut self,
        obj: ManagedObjectReference,
        new_tree_state: Option<TreeState<String>>,
    ) -> anyhow::Result<TreeState<String>> {
        self.obj = Some(obj.clone());
        self.metadata = BrowserMetadata::for_managed_object(&obj);
        self.items = Vec::new();
        self.properties = IndexMap::new();
        Ok(self.replace_tree_state(new_tree_state))
    }

    pub fn replace_tree_state(
        &mut self,
        new_tree_state: Option<TreeState<String>>,
    ) -> TreeState<String> {
        let tree_state = new_tree_state.unwrap_or_default();
        mem::replace(&mut self.state, tree_state)
    }

    fn clean_state(&self) -> TreeState<String> {
        let mut state = TreeState::default();
        if let Some(first_key) = self.properties.keys().next() {
            state.select(vec![first_key.clone()]);
        }
        state
    }

    pub fn up(&mut self) {
        self.state.key_up();
    }

    pub fn down(&mut self) {
        self.state.key_down();
    }

    pub fn left(&mut self) {
        self.state.key_left();
    }

    pub fn right(&mut self) {
        self.state.key_right();
    }

    /// Get the ManagedObjectReference of the selected object in the tree if an object is selected.
    pub fn get_selected_object(&self) -> Option<ManagedObjectReference> {
        let Some(Value::Object(props)) = self.get_selected_node() else {
            return None;
        };

        let type_name = get_type_name(&props)?;

        if type_name != "ManagedObjectReference" {
            return None;
        }

        let (Some(Value::String(motype)), Some(Value::String(value))) =
            (props.get("type"), props.get("value"))
        else {
            return None;
        };

        let Ok(motype) = miniserde::json::from_str(&format!("\"{}\"", motype)) else {
            warn!(
                "PropertyBrowserState: Failed to parse type name: {}",
                motype
            );
            return None;
        };

        Some(ManagedObjectReference {
            r#type: motype,
            value: value.clone(),
        })
    }

    fn get_selected_node(&self) -> Option<Value> {
        let selected = self.state.selected();
        if selected.is_empty() {
            return None;
        }

        let properties = &self.properties;
        let first = selected.first()?;
        let mut value = properties.get(first)?;

        for item in selected.iter().skip(1) {
            match value {
                Value::Object(map) => {
                    value = map.get(item)?;
                }
                Value::Array(arr) => {
                    let index: usize = item.parse().ok()?;
                    if index < arr.len() {
                        value = &arr[index];
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        Some(value.clone())
    }

    fn apply_update(&mut self, changes: Vec<PropertyChange>) -> anyhow::Result<()> {
        let was_empty = self.items.is_empty();
        let change_count = changes.len();
        let change_names: Vec<String> = changes.iter().map(|change| change.name.clone()).collect();
        debug!(
            "PropertyBrowserState::apply_update obj={:?} change_count={} changes={:?}",
            self.obj.as_ref().map(|obj| obj.value.as_str()),
            change_count,
            change_names
        );
        for change in changes {
            let name = change.name;
            match change.op {
                PropertyChangeOpEnum::Assign => {
                    if let Some(value) = change.val {
                        let json_value = to_json_value(&value, &name)?;
                        self.update_item(&name, &json_value);
                        self.properties.insert(name.clone(), json_value);
                    } else {
                        debug!(
                            "PropertyBrowserState: Assign operation with no value for property: {}",
                            name
                        );
                    }
                }
                PropertyChangeOpEnum::IndirectRemove => {
                    self.properties.shift_remove_entry(name.as_str());
                    self.remove_item(&name)?;
                }
                _ => {
                    warn!(
                        "PropertyBrowserState: Unsupported property change operation: {:?}",
                        change.op
                    );
                    continue;
                }
            }
        }
        //self.items = map_to_tree(&self.properties);

        if was_empty && !self.items.is_empty() && self.state.selected().is_empty() {
            self.state = self.clean_state();
        }
        Ok(())
    }

    fn remove_item(&mut self, name: &str) -> anyhow::Result<()> {
        if let Some(pos) = self.items.iter().position(|item| item.identifier() == name) {
            self.items.remove(pos);
        } else {
            warn!(
                "PropertyBrowserState::remove_item: Item not found in tree: {}",
                name
            );
        }
        Ok(())
    }

    fn update_item(&mut self, name: &str, value: &Value) {
        let tree_item = property_to_tree_item(name.to_owned(), value);

        let item_name = name.to_owned();

        // If item with item.identifier == name already exists, update it else push new item at the end
        if let Some(pos) = self
            .items
            .iter()
            .position(|item| item.identifier() == &item_name)
        {
            self.items[pos] = tree_item;
        } else {
            self.items.push(tree_item);
        }
    }

    fn get_object_name(&self) -> Option<String> {
        if let Some(Value::String(name)) = self.properties.get("name") {
            Some(name.clone())
        } else {
            None
        }
    }

    pub fn static_history_snapshot(&self) -> (BrowserMetadata, Object) {
        let root: Object = self
            .properties
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        (self.metadata.clone(), root)
    }

    pub fn dump_to_json(&self) -> anyhow::Result<()> {
        // Convert IndexMap to miniserde Object for serialization
        let obj: Object = self
            .properties
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let json_str = miniserde::json::to_string(&obj);
        let json_content = pretty_print_json(&json_str);

        let filename = self.generate_json_filename()?;
        let path = PathBuf::from(&filename);

        let mut file = File::create(path)?;
        file.write_all(json_content.as_bytes())?;

        Ok(())
    }

    fn generate_json_filename(&self) -> anyhow::Result<String> {
        let name_prefix = self
            .get_object_name()
            .map(|name| name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_"))
            .unwrap_or_default();

        let type_id_part: Cow<'_, str> = if let Some(ref obj) = self.obj {
            format!("{}_{}", obj.r#type.as_str(), obj.value).into()
        } else {
            self.metadata.dump_prefix.as_str().into()
        };
        let type_id_part =
            type_id_part.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");

        let timestamp = Local::now()
            .to_rfc3339_opts(SecondsFormat::Secs, true)
            .replace([':', '-'], "");

        let mut filename = String::new();
        if !name_prefix.is_empty() {
            filename.push_str(&name_prefix);
            filename.push('_');
        }
        filename.push_str(&type_id_part);
        filename.push('_');
        filename.push_str(&timestamp);
        filename.push_str(".json");

        Ok(filename)
    }
}

impl Cache for PropertyBrowserState {
    fn prop_spec(&self) -> vim_rs::core::pc_helpers::Result<PropertySpec> {
        let Some(obj) = self.obj.as_ref() else {
            return Err(vim_rs::core::pc_helpers::Error::internal(
                "static property view is not attached to PropertyCollector".into(),
            ));
        };
        let s = obj.r#type.as_str();
        Ok(PropertySpec {
            r#type: s.to_string(),
            all: Some(true),
            path_set: None,
        })
    }

    fn process_update(
        &mut self,
        update: Vec<ObjectUpdate>,
    ) -> vim_rs::core::pc_helpers::Result<()> {
        if self.obj.is_none() {
            return Ok(());
        }
        if update.is_empty() {
            return Ok(());
        };
        let self_value = self.obj.as_ref().expect("checked").value.clone();
        for update in update {
            if update.obj.value == self_value {
                match update.kind {
                    ObjectUpdateKindEnum::Enter | ObjectUpdateKindEnum::Modify => {
                        let Some(changes) = update.change_set else {
                            debug!(
                                "PropertyBrowserState::process_update obj={} kind={:?} with empty change_set",
                                update.obj.value,
                                update.kind
                            );
                            continue;
                        };
                        debug!(
                            "PropertyBrowserState::process_update obj={} kind={:?} change_count={}",
                            update.obj.value,
                            update.kind,
                            changes.len()
                        );
                        self.apply_update(changes)
                            .map_err(|e| Error::internal(e.to_string()))?;
                        continue;
                    }
                    ObjectUpdateKindEnum::Leave => {
                        debug!("object {:?} left", update.obj);
                        // Clear the state and items
                        self.state = TreeState::default();
                        self.items = Vec::new();
                        self.properties = IndexMap::new();
                        continue;
                    }
                    _ => {
                        // Ignore other update types
                        continue;
                    }
                }
            } else {
                warn!(
                    "PropertyBrowserState: update for different object: {}",
                    update.obj.value
                );
                // Ignore updates for other objects
                continue;
            }
        }
        Ok(())
    }
}

pub struct PropertyBrowser<'a> {
    highlight_style: Style,
    highlight_symbol: &'a str,
    with_scrollbar: bool,
}

impl<'a> PropertyBrowser<'a> {
    pub fn new() -> Self {
        Self {
            highlight_style: Style::new()
                .fg(Color::Yellow)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            highlight_symbol: "> ",
            with_scrollbar: true,
        }
    }
}

/// Pretty-print compact JSON with indentation (miniserde has no to_string_pretty).
fn pretty_print_json(json: &str) -> String {
    let mut out = String::new();
    let mut indent = 0usize;
    let mut in_string = false;
    let mut escape_next = false;
    let chars: Vec<char> = json.chars().collect();
    let len = chars.len();
    let mut i = 0;

    fn write_indent(s: &mut String, level: usize) {
        for _ in 0..level {
            s.push_str("  ");
        }
    }

    while i < len {
        let ch = chars[i];

        if escape_next {
            out.push(ch);
            escape_next = false;
            i += 1;
            continue;
        }

        if in_string {
            out.push(ch);
            if ch == '\\' {
                escape_next = true;
            } else if ch == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        match ch {
            '"' => {
                in_string = true;
                out.push('"');
            }
            '{' | '[' => {
                indent += 1;
                out.push(ch);
                let next_meaningful = chars[i + 1..]
                    .iter()
                    .copied()
                    .find(|c| !c.is_ascii_whitespace());
                if let Some(next_ch) = next_meaningful
                    && next_ch != '}'
                    && next_ch != ']'
                {
                    out.push('\n');
                    write_indent(&mut out, indent);
                }
            }
            '}' | ']' => {
                indent = indent.saturating_sub(1);
                out.push('\n');
                write_indent(&mut out, indent);
                out.push(ch);
            }
            ',' => {
                out.push_str(",\n");
                write_indent(&mut out, indent);
            }
            ':' => {
                out.push_str(": ");
            }
            c if c.is_ascii_whitespace() => {}
            _ => {
                out.push(ch);
            }
        }
        i += 1;
    }
    out
}

impl StatefulWidget for PropertyBrowser<'_> {
    type State = PropertyBrowserState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let title = if let Some(obj) = state.obj.as_ref() {
            let object_type = obj.r#type.as_str();
            let object_id = &obj.value;
            let mut spans = Vec::new();
            if let Some(name) = state.get_object_name() {
                spans.push(Span::styled(name, Style::default().fg(Color::White)));
                spans.push(Span::raw(" "));
            }
            spans.extend_from_slice(&[
                Span::styled("[", Style::default().fg(Color::DarkGray)),
                Span::styled(object_type, Style::default().fg(Color::Cyan)),
                Span::styled(": ", Style::default().fg(Color::DarkGray)),
                Span::styled(object_id, Style::default().fg(Color::Cyan)),
                Span::styled("]", Style::default().fg(Color::DarkGray)),
            ]);
            Line::from(spans)
        } else {
            Line::from(vec![Span::styled(
                state.metadata.title.as_str(),
                Style::default().fg(Color::Cyan),
            )])
        };

        let mut widget = Tree::new(&state.items)
            .expect("all item identifiers are unique")
            .block(
                Block::bordered()
                    .title(title)
                    .title_alignment(Alignment::Center)
                    .title_bottom(
                        Line::from(vec![
                            Span::raw(" "),
                            Span::styled("vTUI version: ", Style::default().fg(Color::DarkGray)),
                            Span::styled(
                                env!("CARGO_PKG_VERSION"),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::raw(" "),
                        ])
                        .alignment(Alignment::Left),
                    )
                    .title_bottom(
                        Line::styled(
                            "→ - expand, ← - collapse, ↑↓ - scroll",
                            Style::default().fg(Color::Cyan),
                        )
                        .alignment(Alignment::Right),
                    )
                    .padding(Padding::right(1)),
            )
            .highlight_style(self.highlight_style)
            .highlight_symbol(self.highlight_symbol);

        if self.with_scrollbar {
            widget = widget.experimental_scrollbar(Some(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None)
                    .track_style(Style::default().bg(Color::DarkGray).fg(Color::DarkGray))
                    .thumb_style(Style::default().bg(Color::Gray).fg(Color::Gray)),
            ));
        }

        widget.render(area, buf, &mut state.state);
    }
}

/// Phase 5 (`testing_strategy.md`): `PropertyBrowserState` behavior with **hand-authored static JSON**
/// and synthetic `ObjectUpdate` payloads. No live vCenter or fixture files are required; recording
/// real `ObjectUpdate` JSON under `tests/fixtures/` is optional if you want heavier regression data.
#[cfg(test)]
mod property_browser_state_unit_tests {
    use super::{BrowserMetadata, PropertyBrowserState};
    use futures::executor::block_on;
    use miniserde::json::{Object, Value};
    use vim_rs::core::pc_cache::Cache;
    use vim_rs::types::boxed_types::ValueElements;
    use vim_rs::types::enums::{MoTypesEnum, ObjectUpdateKindEnum, PropertyChangeOpEnum};
    use vim_rs::types::structs::{ManagedObjectReference, ObjectUpdate, PropertyChange};
    use vim_rs::types::vim_any::VimAny;

    fn vm_moref() -> ManagedObjectReference {
        ManagedObjectReference {
            r#type: MoTypesEnum::VirtualMachine,
            value: "vm-unit-test".into(),
        }
    }

    fn json_object_str(s: &str) -> Object {
        match miniserde::json::from_str::<Value>(s).expect("fixture JSON") {
            Value::Object(o) => o,
            _ => panic!("expected JSON object"),
        }
    }

    #[test]
    fn load_json_root_sets_properties_and_selects_first_key_when_tree_was_empty() {
        let root = json_object_str(r#"{"zebra":1,"alpha":2}"#);
        let meta = BrowserMetadata {
            title: "t".into(),
            dump_prefix: "p".into(),
        };
        let state =
            PropertyBrowserState::from_static_json(meta.clone(), root, None).expect("state");
        assert_eq!(state.properties.len(), 2);
        assert_eq!(state.metadata.title, "t");
        assert!(!state.state.selected().is_empty());
    }

    #[test]
    fn load_json_root_returns_previous_tree_state() {
        let root1 = json_object_str(r#"{"a":true}"#);
        let root2 = json_object_str(r#"{"b":false}"#);
        let mut ts = tui_tree_widget::TreeState::default();
        let _ = ts.select(vec!["a".to_string()]);
        let mut state = PropertyBrowserState::from_static_json(
            BrowserMetadata {
                title: String::new(),
                dump_prefix: String::new(),
            },
            root1,
            Some(ts),
        )
        .expect("state");
        let prev = state.load_json_root(
            BrowserMetadata {
                title: "x".into(),
                dump_prefix: "y".into(),
            },
            None,
            root2,
            None,
        );
        assert_eq!(prev.selected(), &["a".to_string()]);
        assert_eq!(state.metadata.dump_prefix, "y");
    }

    #[test]
    fn static_history_snapshot_roundtrips_root_keys() {
        let root = json_object_str(r#"{"name":"x","nested":{"k":1}}"#);
        let meta = BrowserMetadata {
            title: "Event : 1".into(),
            dump_prefix: "E_1".into(),
        };
        let state =
            PropertyBrowserState::from_static_json(meta.clone(), root.clone(), None).expect("s");
        let (m2, r2) = state.static_history_snapshot();
        assert_eq!(m2.title, meta.title);
        assert_eq!(r2.len(), root.len());
        assert!(r2.contains_key("name"));
        assert!(r2.contains_key("nested"));
    }

    #[test]
    fn get_selected_object_resolves_managed_object_reference_leaf() {
        let moref_json = json_object_str(
            r#"{"_typeName":"ManagedObjectReference","type":"VirtualMachine","value":"vm-42"}"#,
        );
        let mut root = Object::new();
        root.insert("vm".into(), Value::Object(moref_json));
        let mut ts = tui_tree_widget::TreeState::default();
        let _ = ts.select(vec!["vm".to_string()]);
        let state = PropertyBrowserState::from_static_json(
            BrowserMetadata {
                title: String::new(),
                dump_prefix: String::new(),
            },
            root,
            Some(ts),
        )
        .expect("state");
        let mo = state.get_selected_object().expect("moref");
        assert_eq!(mo.r#type, MoTypesEnum::VirtualMachine);
        assert_eq!(mo.value, "vm-42");
    }

    #[test]
    fn get_selected_object_none_for_non_moref_leaf() {
        let root = json_object_str(r#"{"name":"only-a-string"}"#);
        let mut ts = tui_tree_widget::TreeState::default();
        let _ = ts.select(vec!["name".to_string()]);
        let state = PropertyBrowserState::from_static_json(
            BrowserMetadata {
                title: String::new(),
                dump_prefix: String::new(),
            },
            root,
            Some(ts),
        )
        .expect("state");
        assert!(state.get_selected_object().is_none());
    }

    #[test]
    fn set_obj_clears_properties_and_updates_metadata() {
        let root = json_object_str(r#"{"f":1}"#);
        let mut state = block_on(PropertyBrowserState::new(vm_moref(), None)).expect("new");
        let _ = state.load_json_root(
            BrowserMetadata::for_managed_object(&vm_moref()),
            Some(vm_moref()),
            root,
            None,
        );
        assert!(!state.properties.is_empty());
        let new_obj = ManagedObjectReference {
            r#type: MoTypesEnum::HostSystem,
            value: "host-9".into(),
        };
        let _ = state.set_obj(new_obj.clone(), None).expect("set_obj");
        assert!(state.properties.is_empty());
        assert!(state.items.is_empty());
        assert_eq!(state.obj.as_ref().expect("obj").value, "host-9");
    }

    #[tokio::test]
    async fn process_update_assign_updates_name_property() {
        let mut state = PropertyBrowserState::new(vm_moref(), None)
            .await
            .expect("new");
        let update = ObjectUpdate {
            kind: ObjectUpdateKindEnum::Modify,
            obj: vm_moref(),
            change_set: Some(vec![PropertyChange {
                name: "name".into(),
                op: PropertyChangeOpEnum::Assign,
                val: Some(VimAny::Value(ValueElements::PrimitiveString(
                    "updated-vm".into(),
                ))),
            }]),
            missing_set: None,
        };
        Cache::process_update(&mut state, vec![update]).expect("process_update");
        match state.properties.get("name") {
            Some(Value::String(s)) => assert_eq!(s, "updated-vm"),
            o => panic!("expected name string, got {:?}", o),
        }
        assert_eq!(state.get_object_name().as_deref(), Some("updated-vm"));
    }

    #[tokio::test]
    async fn process_update_leave_clears_view() {
        let mut state = PropertyBrowserState::new(vm_moref(), None)
            .await
            .expect("new");
        let fill = ObjectUpdate {
            kind: ObjectUpdateKindEnum::Modify,
            obj: vm_moref(),
            change_set: Some(vec![PropertyChange {
                name: "name".into(),
                op: PropertyChangeOpEnum::Assign,
                val: Some(VimAny::Value(ValueElements::PrimitiveString("v".into()))),
            }]),
            missing_set: None,
        };
        Cache::process_update(&mut state, vec![fill]).expect("fill");
        let leave = ObjectUpdate {
            kind: ObjectUpdateKindEnum::Leave,
            obj: vm_moref(),
            change_set: None,
            missing_set: None,
        };
        Cache::process_update(&mut state, vec![leave]).expect("leave");
        assert!(state.properties.is_empty());
        assert!(state.items.is_empty());
    }

    #[tokio::test]
    async fn process_update_ignores_wrong_object_id() {
        let mut state = PropertyBrowserState::new(vm_moref(), None)
            .await
            .expect("new");
        let wrong = ObjectUpdate {
            kind: ObjectUpdateKindEnum::Modify,
            obj: ManagedObjectReference {
                r#type: MoTypesEnum::VirtualMachine,
                value: "other-vm".into(),
            },
            change_set: Some(vec![PropertyChange {
                name: "name".into(),
                op: PropertyChangeOpEnum::Assign,
                val: Some(VimAny::Value(ValueElements::PrimitiveString(
                    "ghost".into(),
                ))),
            }]),
            missing_set: None,
        };
        Cache::process_update(&mut state, vec![wrong]).expect("process_update");
        assert!(state.properties.is_empty());
    }

    #[tokio::test]
    async fn static_view_process_update_is_no_op() {
        let root = json_object_str(r#"{"k":1}"#);
        let mut state = PropertyBrowserState::from_static_json(
            BrowserMetadata {
                title: String::new(),
                dump_prefix: String::new(),
            },
            root,
            None,
        )
        .expect("static");
        let update = ObjectUpdate {
            kind: ObjectUpdateKindEnum::Modify,
            obj: vm_moref(),
            change_set: Some(vec![PropertyChange {
                name: "name".into(),
                op: PropertyChangeOpEnum::Assign,
                val: Some(VimAny::Value(ValueElements::PrimitiveString("nope".into()))),
            }]),
            missing_set: None,
        };
        Cache::process_update(&mut state, vec![update]).expect("process_update");
        match state.properties.get("k") {
            Some(Value::Number(miniserde::json::Number::U64(1))) => {}
            o => panic!("expected k=1, got {:?}", o),
        }
    }

    #[test]
    fn generate_json_filename_static_uses_dump_prefix_and_json_extension() {
        let root = json_object_str("{}");
        let state = PropertyBrowserState::from_static_json(
            BrowserMetadata {
                title: "T".into(),
                dump_prefix: "My_Event_9".into(),
            },
            root,
            None,
        )
        .expect("state");
        let name = state.generate_json_filename().expect("filename");
        assert!(name.ends_with(".json"));
        assert!(name.contains("My_Event_9"));
    }
}

#[cfg(test)]
mod snapshot_tests {
    use super::{BrowserMetadata, PropertyBrowser, PropertyBrowserState};
    use insta::assert_snapshot;
    use miniserde::json::{Object, Value};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tui_tree_widget::TreeState;

    fn sample_root_object() -> Object {
        match miniserde::json::from_str::<Value>(
            r#"{"name":"demo-vm","config":{"numCpu":4,"memoryMB":8192}}"#,
        )
        .expect("fixture JSON")
        {
            Value::Object(o) => o,
            _ => panic!("fixture must be a JSON object"),
        }
    }

    fn sample_metadata() -> BrowserMetadata {
        BrowserMetadata {
            title: "VmPoweredOnEvent : 42".into(),
            dump_prefix: "VmPoweredOnEvent_42".into(),
        }
    }

    fn draw_property_browser(
        metadata: BrowserMetadata,
        root: Object,
        tree_state: Option<TreeState<String>>,
    ) -> String {
        let mut state =
            PropertyBrowserState::from_static_json(metadata, root, tree_state).expect("state");
        let mut term = Terminal::new(TestBackend::new(78, 20)).unwrap();
        term.draw(|f| {
            let w = PropertyBrowser::new();
            f.render_stateful_widget(w, f.area(), &mut state);
        })
        .unwrap();
        format!("{}", term.backend())
    }

    #[test]
    fn property_browser_static_default_selection_snapshot() {
        assert_snapshot!(draw_property_browser(
            sample_metadata(),
            sample_root_object(),
            None,
        ));
    }

    #[test]
    fn property_browser_static_expanded_config_snapshot() {
        let mut ts = TreeState::default();
        let _ = ts.select(vec!["config".to_string()]);
        let _ = ts.open(vec!["config".to_string()]);
        assert_snapshot!(draw_property_browser(
            sample_metadata(),
            sample_root_object(),
            Some(ts),
        ));
    }
}

#[cfg(test)]
mod unicode_tests {
    use super::*;
    use indexmap::IndexMap;
    use miniserde::json::{Array, Object, Value};
    use vim_rs::types::{enums::MoTypesEnum, structs::ManagedObjectReference};

    fn test_state() -> PropertyBrowserState {
        PropertyBrowserState {
            obj: Some(ManagedObjectReference {
                r#type: MoTypesEnum::VirtualMachine,
                value: "vm-42".to_string(),
            }),
            properties: IndexMap::new(),
            metadata: BrowserMetadata {
                title: String::new(),
                dump_prefix: String::new(),
            },
            items: Vec::new(),
            state: TreeState::default(),
        }
    }

    fn string_value(input: &str) -> Value {
        Value::String(input.to_string())
    }

    fn nested_vm_config_value() -> Value {
        let mut root = Object::new();
        let mut config = Object::new();
        let mut vapp_config = Object::new();
        let eula: Array = vec![
            string_value("Foundagtion Agreement © “quoted”"),
            string_value("Part 2: Copyright © 2026 Broadcom."),
        ]
        .into_iter()
        .collect();

        vapp_config.insert("_typeName".to_string(), string_value("VmConfigInfo"));
        vapp_config.insert("eula".to_string(), Value::Array(eula));

        config.insert(
            "_typeName".to_string(),
            string_value("VirtualMachineConfigInfo"),
        );
        config.insert("vAppConfig".to_string(), Value::Object(vapp_config));

        root.insert("config".to_string(), Value::Object(config));
        Value::Object(root)
    }

    #[test]
    fn pretty_print_preserves_copyright_symbol() {
        let compact = r#"{"eula":["Copyright © 2026 Broadcom."]}"#;

        let pretty = pretty_print_json(compact);

        assert!(pretty.contains("Copyright © 2026 Broadcom."));
        assert!(!pretty.contains("Â©"));
    }

    #[test]
    fn pretty_print_preserves_smart_quotes() {
        let compact = r#"{"eula":["“Foundation Agreement”"]}"#;

        let pretty = pretty_print_json(compact);

        assert!(pretty.contains("“Foundation Agreement”"));
        assert!(!pretty.contains("â€œ"));
        assert!(!pretty.contains("â€\u{9d}"));
    }

    #[test]
    fn pretty_print_keeps_escaped_quotes_inside_strings() {
        let compact = r#"{"text":"He said \"hello\"."}"#;

        let pretty = pretty_print_json(compact);

        assert!(pretty.contains(r#"He said \"hello\"."#));
    }

    #[test]
    fn pretty_print_keeps_backslashes_inside_strings() {
        let compact = r#"{"path":"C:\\Program Files\\Broadcom"}"#;

        let pretty = pretty_print_json(compact);

        assert!(pretty.contains(r#"C:\\Program Files\\Broadcom"#));
    }

    #[test]
    fn pretty_print_keeps_newline_escapes_inside_strings() {
        let compact = r#"{"eula":"Line 1\nLine 2\nLine 3"}"#;

        let pretty = pretty_print_json(compact);

        assert!(pretty.contains(r#"Line 1\nLine 2\nLine 3"#));
    }

    #[test]
    fn pretty_print_removes_whitespace_outside_strings() {
        let compact = "{  \"a\" : [ 1 , 2 ] , \"b\" : { \"c\" : true } }";

        let pretty = pretty_print_json(compact);

        assert_eq!(
            pretty,
            "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {\n    \"c\": true\n  }\n}"
        );
    }

    #[test]
    fn pretty_print_keeps_spaces_inside_strings() {
        let compact = r#"{"text":"  keep   inner   spaces  "}"#;

        let pretty = pretty_print_json(compact);

        assert!(pretty.contains(r#""  keep   inner   spaces  ""#));
    }

    #[test]
    fn pretty_print_handles_empty_arrays_and_objects() {
        let compact = r#"{"array":[],"object":{}}"#;

        let pretty = pretty_print_json(compact);

        assert_eq!(pretty, "{\n  \"array\": [\n  ],\n  \"object\": {\n  }\n}");
    }

    #[test]
    fn pretty_print_preserves_unicode_in_nested_vm_config_shape() {
        let value = nested_vm_config_value();
        let compact = miniserde::json::to_string(&value);

        let pretty = pretty_print_json(&compact);

        assert!(pretty.contains("Foundagtion Agreement © “quoted”"));
        assert!(pretty.contains("Part 2: Copyright © 2026 Broadcom."));
        assert!(!pretty.contains("Â©"));
        assert!(!pretty.contains("â€œ"));
        assert!(!pretty.contains("â€"));
    }

    #[test]
    fn pretty_printed_nested_vm_config_round_trips_without_mojibake() {
        let value = nested_vm_config_value();
        let compact = miniserde::json::to_string(&value);
        let pretty = pretty_print_json(&compact);

        let reparsed: Value = miniserde::json::from_str(&pretty).expect("pretty JSON parses");
        let Value::Object(root) = reparsed else {
            panic!("expected root object");
        };
        let Value::Object(config) = root.get("config").expect("config exists") else {
            panic!("expected config object");
        };
        let Value::Object(vapp_config) = config.get("vAppConfig").expect("vAppConfig exists")
        else {
            panic!("expected vAppConfig object");
        };
        let Value::Array(eula) = vapp_config.get("eula").expect("eula exists") else {
            panic!("expected eula array");
        };

        assert_eq!(eula.len(), 2);
        let Value::String(first) = &eula[0] else {
            panic!("expected first eula entry to be a string");
        };
        let Value::String(second) = &eula[1] else {
            panic!("expected second eula entry to be a string");
        };
        assert_eq!(first, "Foundagtion Agreement © “quoted”");
        assert_eq!(second, "Part 2: Copyright © 2026 Broadcom.");
    }

    #[test]
    fn generate_json_filename_sanitizes_reserved_characters() {
        let mut state = test_state();
        state.properties.insert(
            "name".to_string(),
            string_value(r#"bad/name\with:reserved*?"<>|chars"#),
        );

        let filename = state.generate_json_filename().expect("filename generated");

        assert!(filename.starts_with("bad_name_with_reserved______chars_VirtualMachine_vm-42_"));
        assert!(filename.ends_with(".json"));
        assert!(!filename.contains('/'));
        assert!(!filename.contains('\\'));
        assert!(!filename.contains(':'));
        assert!(!filename.contains('*'));
        assert!(!filename.contains('?'));
        assert!(!filename.contains('"'));
        assert!(!filename.contains('<'));
        assert!(!filename.contains('>'));
        assert!(!filename.contains('|'));
    }

    #[test]
    fn generate_json_filename_preserves_unicode_name() {
        let mut state = test_state();
        state
            .properties
            .insert("name".to_string(), string_value("Appliance © “quoted”"));

        let filename = state.generate_json_filename().expect("filename generated");

        assert!(filename.starts_with("Appliance © “quoted”_VirtualMachine_vm-42_"));
        assert!(filename.ends_with(".json"));
        assert!(!filename.contains("Â©"));
    }
}

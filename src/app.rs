use crate::body_pane::BodyPane;
use crate::event::{AppEvent, Event, EventHandler};
use crate::hints;
use crate::history::{History, HistoryManager};
use crate::operation_types::OperationId;
use crate::ops::types::{InventoryOperation, OperationRequest};
use crate::ops::{OpsHandle, spawn_ops_supervisor};
use crate::perf_worker::{PerfRequest, run_perf_worker};
use crate::polling_policy;
use crate::prop_browser::{
    BrowserMetadata, PropertyBrowserManager, PropertyHistoryRecord, StaticPropertyBrowserManager,
};
use crate::resource_browser::ResourceManager;
use crate::resource_browser::event_to_browser_object;
use crate::resource_type::{ResourceSelectionState, ResourceType};
use crate::search::SearchState;
use crate::vm_action_ui::{self, VmActionKeyOutcome, VmActionUi};
use crate::vm_power_actions::VmPowerAction;
use crate::vm_summary_ui::{VmSummaryKeyOutcome, VmSummaryUi};
use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEventKind};
use log::{debug, info, warn};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::{DefaultTerminal, Frame};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::watch;
use vim_rs::core::client::{Transport, VimClientHandle};
use vim_rs::core::pc_cache::CacheManager;
use vim_rs::types::structs::ManagedObjectReference;

use crate::resource_browser::perf::PerfSnapshotShare;

/// Main application object.
pub struct App {
    /// Flag to indicate if the application should quit.
    should_quit: bool,
    /// Cache manager for managing object caches.
    cache_mgr: Rc<RefCell<CacheManager>>,
    /// Client for interacting with the vSphere API.
    client: VimClientHandle,
    /// Body pane.
    body_pane: BodyPane,
    /// Event dispatcher for processing events.
    events: EventHandler,
    /// Search state for managing the search input and filter.
    search_state: SearchState,
    /// State for managing resource selection.
    resource_selection_state: ResourceSelectionState,
    /// History of previous states for back navigation.
    history: HistoryManager,
    /// VM power action modals (`x` from VM grid).
    vm_action_ui: VmActionUi,
    /// VM summary popup (`s` from VM grid).
    vm_summary_ui: VmSummaryUi,
    /// Outstanding VM power action submitted to [`crate::ops`] (modal already closed).
    vm_action_pending_execute: Option<OperationId>,
    /// Monotonic id generator for ops requests.
    next_operation_id: OperationId,
    /// Submit work to the ops supervisor (sender dropped on quit).
    ops: OpsHandle,
    ops_worker: tokio::task::JoinHandle<()>,
    /// Blocking error popup (e.g. prefetch or action failure).
    error_popup: Option<String>,
    /// Redraw once after async app work so modals appear without waiting for another event.
    pending_redraw: bool,
    /// Monotonic view id for discarding stale perf worker results.
    perf_generation: u64,
    perf_worker: tokio::task::JoinHandle<()>,
    /// Dropped last so the perf worker observes shutdown after other teardown begins.
    perf_tx: watch::Sender<PerfRequest>,
    /// Last perf demand sent to the worker; used to skip redundant ad-hoc refreshes.
    last_perf_demand: Option<PerfDemandState>,
}

const ASCII_ART: &str = r#"     ╭───────╮
 ╭─╮╭┴┬─╮ ╭──╯   ▐█▌
 \ \/ / │ │╔═╗╔═╗╭─╮
  \  /  │ │║ ╚╝ ║│ │
   ╰╯   ╰─╯╚════╝╰─╯"#;

#[derive(Clone)]
enum PerfDemandState {
    Paused {
        generation: u64,
    },
    Active {
        generation: u64,
        entities: Vec<ManagedObjectReference>,
        snapshot: PerfSnapshotShare,
    },
}

impl PerfDemandState {
    fn paused(generation: u64) -> Self {
        Self::Paused { generation }
    }

    fn active(req: &PerfRequest) -> Self {
        Self::Active {
            generation: req.generation,
            entities: req.entities.clone(),
            snapshot: req.snapshot.clone(),
        }
    }

    fn matches(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Paused {
                    generation: left_generation,
                },
                Self::Paused {
                    generation: right_generation,
                },
            ) => left_generation == right_generation,
            (
                Self::Active {
                    generation: left_generation,
                    entities: left_entities,
                    snapshot: left_snapshot,
                },
                Self::Active {
                    generation: right_generation,
                    entities: right_entities,
                    snapshot: right_snapshot,
                },
            ) => {
                left_generation == right_generation
                    && left_entities == right_entities
                    && Arc::ptr_eq(left_snapshot, right_snapshot)
            }
            _ => false,
        }
    }
}

impl App {
    pub async fn new(
        events: EventHandler,
        cache_mgr: Rc<RefCell<CacheManager>>,
        client: VimClientHandle,
    ) -> anyhow::Result<Self> {
        let (perf_tx, perf_rx) = watch::channel(PerfRequest::initial());
        let event_tx = events.clone_event_sender();
        let client_for_worker = client.clone();
        let perf_worker = tokio::spawn(async move {
            run_perf_worker(client_for_worker, perf_rx, event_tx).await;
        });

        let event_tx_ops = events.clone_event_sender();
        let (ops, ops_worker) = spawn_ops_supervisor(client.clone(), event_tx_ops);

        // Create a new ResourceManager instance
        let resource_mgr = ResourceManager::new(
            client.clone(),
            cache_mgr.clone(),
            ResourceType::VirtualMachine,
        )
        .await?;
        let mut app = Self {
            should_quit: false,
            cache_mgr,
            client,
            body_pane: BodyPane::ResourceBrowser(Box::new(resource_mgr)),
            events,
            search_state: SearchState::new(),
            resource_selection_state: ResourceSelectionState::new(),
            history: HistoryManager::new(20), // TODO: Make this configurable
            vm_action_ui: VmActionUi::default(),
            vm_summary_ui: VmSummaryUi::default(),
            vm_action_pending_execute: None,
            next_operation_id: 1,
            ops,
            ops_worker,
            error_popup: None,
            pending_redraw: false,
            perf_generation: 1,
            perf_worker,
            perf_tx,
            last_perf_demand: None,
        };
        app.refresh_polling_demand();
        Ok(app)
    }

    /// Recompute PropertyCollector demand and perf worker request from the current UI (body pane + modals).
    fn refresh_polling_demand(&mut self) {
        let pc = self.body_pane.wants_property_collector_waits();
        self.events.set_property_collector_demand(pc);

        let want_perf = polling_policy::perf_polling_wanted(
            matches!(self.body_pane, BodyPane::ResourceBrowser(_)),
            self.vm_summary_ui.is_active(),
        );
        let next_perf_demand = if want_perf {
            if let BodyPane::ResourceBrowser(ref mut resource_mgr) = self.body_pane {
                let req = resource_mgr.perf_request(self.perf_generation);
                let next = PerfDemandState::active(&req);
                let unchanged = self
                    .last_perf_demand
                    .as_ref()
                    .is_some_and(|prev| prev.matches(&next));
                if !unchanged {
                    let _ = self.perf_tx.send_replace(req);
                }
                next
            } else {
                PerfDemandState::paused(self.perf_generation)
            }
        } else {
            let next = PerfDemandState::paused(self.perf_generation);
            let unchanged = self
                .last_perf_demand
                .as_ref()
                .is_some_and(|prev| prev.matches(&next));
            if !unchanged {
                let _ = self
                    .perf_tx
                    .send_replace(PerfRequest::paused(self.perf_generation));
            }
            next
        };
        self.last_perf_demand = Some(next_perf_demand);
    }

    fn next_op_id(&mut self) -> OperationId {
        let id = self.next_operation_id;
        self.next_operation_id = self.next_operation_id.saturating_add(1);
        id
    }

    async fn submit_op(&mut self, req: OperationRequest) -> Result<(), crate::ops::OpsSubmitError> {
        self.ops.submit(req).await
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> anyhow::Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| self.draw(frame))?;
            match self.events.next().await? {
                Event::Crossterm(event) => self.handle_terminal_event(&event).await?,
                Event::App(app_event) => self.handle_app_event(*app_event).await?,
            }
            if self.pending_redraw {
                terminal.draw(|frame| self.draw(frame))?;
                self.pending_redraw = false;
            }
        }
        Ok(())
    }

    async fn handle_app_event(&mut self, event: AppEvent) -> anyhow::Result<()> {
        match event {
            AppEvent::PropertyCollector(update) => {
                let filter_ids: Vec<String> = update
                    .iter()
                    .map(|filter_update| filter_update.filter.value.clone())
                    .collect();
                debug!(
                    "PropertyCollector update. length={} filter_ids={:?}",
                    update.len(),
                    filter_ids
                );
                self.cache_mgr.borrow_mut().apply_updates(update)?;
                if let BodyPane::ResourceBrowser(ref mut resource_mgr) = self.body_pane {
                    resource_mgr.invalidate();
                }
                self.refresh_polling_demand();
                self.pending_redraw = true;
            }
            AppEvent::ErrorMessage(msg) => {
                warn!("Error from update loop: {}", msg);
            }
            AppEvent::Quit => {
                info!("Quitting...");
                self.events.set_property_collector_demand(false);
                let _ = self
                    .perf_tx
                    .send_replace(PerfRequest::paused(self.perf_generation));
                self.ops.shutdown();
                self.ops_worker.abort();
                self.perf_worker.abort();
                self.events.shutdown().await?;
                self.should_quit = true
            }
            AppEvent::OpenSearch => self.search_state.activate(),
            AppEvent::SearchCompleted(filter) => {
                debug!("SearchCompleted. filter: {:?}", filter);
                if let BodyPane::ResourceBrowser(ref mut resource_mgr) = self.body_pane {
                    resource_mgr.set_filter(Some(filter));
                }
                self.refresh_polling_demand();
                self.pending_redraw = true;
            }
            AppEvent::OpenResourceSelection => self.resource_selection_state.activate(),
            AppEvent::ResourceSelected(resource_type) => {
                info!("Resource Type Selected. resource_type: {:?}", resource_type);
                match &mut self.body_pane {
                    BodyPane::ResourceBrowser(resource_mgr) => {
                        resource_mgr
                            .load_resource_type(resource_type, &mut self.events)
                            .await?;
                        self.perf_generation = self.perf_generation.saturating_add(1);
                    }
                    BodyPane::PropertyBrowser(prop_mgr) => {
                        prop_mgr.save_state(&mut self.events);
                        let res_mgr = ResourceManager::new(
                            self.client.clone(),
                            self.cache_mgr.clone(),
                            resource_type,
                        )
                        .await?;
                        self.perf_generation = self.perf_generation.saturating_add(1);
                        self.body_pane = BodyPane::ResourceBrowser(Box::new(res_mgr));
                    }
                    BodyPane::StaticPropertyBrowser(static_mgr) => {
                        static_mgr.save_state(&mut self.events);
                        let res_mgr = ResourceManager::new(
                            self.client.clone(),
                            self.cache_mgr.clone(),
                            resource_type,
                        )
                        .await?;
                        self.perf_generation = self.perf_generation.saturating_add(1);
                        self.body_pane = BodyPane::ResourceBrowser(Box::new(res_mgr));
                    }
                }
                self.refresh_polling_demand();
            }
            AppEvent::OpenVmActions(vm_ref) => {
                let request_id = self.next_op_id();
                self.vm_action_ui.start_prefetch_loading(request_id);
                let req = OperationRequest::PrefetchVmActions {
                    request_id,
                    vm: vm_ref,
                };
                if self.submit_op(req).await.is_err() {
                    self.vm_action_ui.close();
                    self.error_popup =
                        Some("Could not queue VM action prefetch (ops worker unavailable).".into());
                }
                self.pending_redraw = true;
            }
            AppEvent::VmActionPrefetchSucceeded {
                request_id,
                context,
            } => {
                if self.vm_action_ui.prefetch_is_pending(request_id) {
                    let actions = VmPowerAction::visible(&context.disabled_method);
                    self.vm_action_ui.open_menu(context, actions);
                }
                self.pending_redraw = true;
            }
            AppEvent::VmActionPrefetchFailed { request_id, error } => {
                if self.vm_action_ui.prefetch_is_pending(request_id) {
                    self.vm_action_ui.close();
                    self.error_popup = Some(error);
                }
                self.pending_redraw = true;
            }
            AppEvent::OpenVmSummary(vm_ref) => {
                let request_id = self.next_op_id();
                let vm_label = format!("{}:{}", vm_ref.r#type.as_str(), vm_ref.value);
                info!(
                    target: "vm_summary",
                    "vm summary: open (queue fetch) request_id={request_id} vm={vm_label}"
                );
                self.vm_summary_ui.start_loading(request_id);
                let req = OperationRequest::PrefetchVmSummary {
                    request_id,
                    vm: vm_ref,
                };
                if self.submit_op(req).await.is_err() {
                    warn!(
                        target: "vm_summary",
                        "vm summary: could not queue fetch request_id={request_id} vm={vm_label} (ops worker unavailable)"
                    );
                    self.vm_summary_ui.close();
                    self.error_popup =
                        Some("Could not queue VM summary fetch (ops worker unavailable).".into());
                }
                self.refresh_polling_demand();
                self.pending_redraw = true;
            }
            AppEvent::VmSummarySucceeded {
                request_id,
                summary,
            } => {
                if self.vm_summary_ui.pending_matches(request_id) {
                    self.vm_summary_ui.apply_success(request_id, summary);
                } else {
                    debug!(
                        target: "vm_summary",
                        "vm summary: ignoring VmSummarySucceeded (stale request_id={} name={})",
                        request_id,
                        summary.vm_name
                    );
                }
                self.pending_redraw = true;
            }
            AppEvent::VmSummaryFailed { request_id, error } => {
                if self.vm_summary_ui.pending_matches(request_id) {
                    self.vm_summary_ui.close();
                    self.error_popup = Some(error);
                    self.refresh_polling_demand();
                } else {
                    debug!(
                        target: "vm_summary",
                        "vm summary: ignoring VmSummaryFailed (stale request_id={request_id}): {error}"
                    );
                }
                self.pending_redraw = true;
            }
            AppEvent::OperationSucceeded { op_id, message } => {
                if self.vm_action_pending_execute == Some(op_id) {
                    self.vm_action_pending_execute = None;
                    debug!("VM op succeeded: {}", message);
                }
                self.pending_redraw = true;
            }
            AppEvent::OperationFailed { op_id, error } => {
                if self.vm_action_pending_execute == Some(op_id) {
                    self.vm_action_pending_execute = None;
                    self.error_popup = Some(error);
                }
                self.pending_redraw = true;
            }
            AppEvent::LoadProperties(moref) => {
                info!("LoadProperties. moref: {:?}", moref);
                self.body_pane = BodyPane::PropertyBrowser(
                    PropertyBrowserManager::new(self.cache_mgr.clone(), moref).await?,
                );
                self.refresh_polling_demand();
            }
            AppEvent::LoadEventProperties(payload) => {
                let payload = *payload;
                info!("LoadEventProperties. title: {}", payload.title);
                let metadata = BrowserMetadata {
                    title: payload.title,
                    dump_prefix: payload.dump_prefix,
                };
                let root = event_to_browser_object(&payload.event)?;
                self.body_pane = BodyPane::StaticPropertyBrowser(
                    StaticPropertyBrowserManager::new(metadata, root)?,
                );
                self.refresh_polling_demand();
            }
            AppEvent::ResourceManagerHistory(history) => {
                self.history.add_resource_entry(history);
            }
            AppEvent::PropertyManagerHistory(history) => {
                self.history.add_property_entry(history);
            }
            AppEvent::PerfResult { generation } => {
                if generation == self.perf_generation {
                    self.pending_redraw = true;
                }
            }
        }
        Ok(())
    }

    fn build_status_lines(&self) -> Vec<Line<'_>> {
        let mut res = Vec::<Line>::with_capacity(3);

        // Get about information from the service content
        let about = &self.client.service_content().about;

        // 1. vSphere full product name
        res.push(Line::from(vec![
            "vSphere: ".yellow(),
            about.full_name.clone().gray(),
        ]));

        // 2. vSphere system UUID
        if let Some(ref uuid) = about.instance_uuid {
            res.push(Line::from(vec![
                "vSphere UUID: ".yellow(),
                uuid.clone().gray(),
            ]));
        } else {
            res.push(Line::from(vec!["vSphere UUID: ".yellow(), "N/A".gray()]));
        }

        // 3. Used API version and wire format (JSON vs SOAP)
        let wire = match self.client.transport() {
            Transport::Json => "JSON",
            Transport::Soap => "SOAP",
        };
        res.push(Line::from(vec![
            "API Version: ".yellow(),
            self.client.api_release().gray(),
            " (".gray(),
            wire.gray(),
            ")".gray(),
        ]));

        res
    }

    fn draw(&mut self, frame: &mut Frame) {
        let vertical = Layout::vertical([Constraint::Length(5), Constraint::Fill(1)]);
        let [top_area, body_area] = vertical.areas(frame.area());

        self.render_header(frame, top_area);

        self.body_pane.render(frame, body_area);

        // Draw search popup if active
        if self.search_state.is_active() {
            self.search_state.render(frame);
        }
        if self.resource_selection_state.is_active() {
            self.resource_selection_state.render(frame);
        }
        if self.vm_action_ui.is_active() {
            self.vm_action_ui.render(frame);
        }
        if self.vm_summary_ui.is_active() {
            self.vm_summary_ui.render(frame);
        }
        if let Some(ref msg) = self.error_popup {
            vm_action_ui::render_error_popup(frame, msg);
        }
    }

    fn render_header(&mut self, frame: &mut Frame, top_area: Rect) {
        let horizontal = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(16),
            Constraint::Length(16),
            Constraint::Length(21),
        ]);
        let [status_area, expand_area, help_area, logo_area] = horizontal.areas(top_area);

        // Render statuses
        let status_lines: Vec<Line> = self.build_status_lines();
        let status_paragraph = ratatui::widgets::Paragraph::new(status_lines)
            .style(ratatui::style::Style::default().fg(ratatui::style::Color::Green));
        frame.render_widget(status_paragraph, status_area);

        let (expand_hints, help_hints) = self.body_pane.get_hints();
        // Render expand hints
        let expand_lines = hints::decorate_hints(expand_hints);
        let expand_paragraph = ratatui::widgets::Paragraph::new(expand_lines)
            .style(ratatui::style::Style::default().fg(ratatui::style::Color::Cyan));
        frame.render_widget(expand_paragraph, expand_area);

        // Render help hints
        let help_lines = hints::decorate_hints(help_hints);
        let help_paragraph = ratatui::widgets::Paragraph::new(help_lines)
            .style(ratatui::style::Style::default().fg(ratatui::style::Color::Yellow));
        frame.render_widget(help_paragraph, help_area);

        // Render ASCII art logo
        let logo_paragraph = ratatui::widgets::Paragraph::new(ASCII_ART)
            .style(ratatui::style::Style::default().fg(ratatui::style::Color::Cyan))
            .alignment(ratatui::layout::Alignment::Left);
        frame.render_widget(logo_paragraph, logo_area);
    }

    async fn handle_terminal_event(&mut self, event: &CrosstermEvent) -> anyhow::Result<()> {
        if let CrosstermEvent::Key(key) = event
            && key.kind == KeyEventKind::Press
        {
            if matches!(key.code, KeyCode::Char('c') if key.modifiers == crossterm::event::KeyModifiers::CONTROL)
            {
                self.events.send(AppEvent::Quit);
                return Ok(());
            }

            if self.error_popup.is_some() {
                if vm_action_ui::error_popup_handle_key(key) {
                    self.error_popup = None;
                    self.pending_redraw = true;
                }
                return Ok(());
            }

            if self.vm_summary_ui.is_active() {
                match self.vm_summary_ui.handle_key(key) {
                    VmSummaryKeyOutcome::Ignored => {}
                    VmSummaryKeyOutcome::Consumed => {
                        self.pending_redraw = true;
                        return Ok(());
                    }
                    VmSummaryKeyOutcome::Close => {
                        self.pending_redraw = true;
                        self.refresh_polling_demand();
                        return Ok(());
                    }
                }
            }

            if self.vm_action_ui.is_active() {
                match self.vm_action_ui.handle_key(key) {
                    VmActionKeyOutcome::Ignored => {}
                    VmActionKeyOutcome::Consumed | VmActionKeyOutcome::Close => {
                        self.pending_redraw = true;
                        return Ok(());
                    }
                    VmActionKeyOutcome::Execute { vm, action } => {
                        let op_id = self.next_op_id();
                        self.vm_action_pending_execute = Some(op_id);
                        let req = OperationRequest::ExecuteInventoryOperation {
                            op_id,
                            op: InventoryOperation::Vm { vm, action },
                        };
                        if self.submit_op(req).await.is_err() {
                            self.vm_action_pending_execute = None;
                            self.error_popup = Some(
                                "Could not queue VM power action (ops worker unavailable).".into(),
                            );
                        }
                        self.pending_redraw = true;
                        return Ok(());
                    }
                }
            }

            if self.search_state.is_active() && self.search_state.handle_key(key, &mut self.events)
            {
                return Ok(());
            }

            if self.resource_selection_state.is_active()
                && self
                    .resource_selection_state
                    .handle_key(key, &mut self.events)
            {
                return Ok(());
            }

            let key_outcome = self.body_pane.handle_key(key, &mut self.events).await?;
            if key_outcome.new_perf_view {
                self.perf_generation = self.perf_generation.saturating_add(1);
            }
            if key_outcome.handled {
                self.refresh_polling_demand();
                return Ok(());
            }

            match key.code {
                KeyCode::Char('q') => self.events.send(AppEvent::Quit),
                KeyCode::Char('r') => self.events.send(AppEvent::OpenResourceSelection),
                KeyCode::Backspace => self.back().await?,
                _ => {}
            }
        }
        Ok(())
    }

    async fn back(&mut self) -> anyhow::Result<()> {
        if let Some(entry) = self.history.pop() {
            match entry {
                History::Resource(record) => match self.body_pane {
                    BodyPane::ResourceBrowser(ref mut resource_mgr) => {
                        resource_mgr.load_history_record(record).await?;
                        self.perf_generation = self.perf_generation.saturating_add(1);
                    }
                    BodyPane::PropertyBrowser(_) | BodyPane::StaticPropertyBrowser(_) => {
                        self.perf_generation = self.perf_generation.saturating_add(1);
                        self.body_pane = BodyPane::ResourceBrowser(Box::new(
                            ResourceManager::from_history_record(
                                record,
                                self.client.clone(),
                                self.cache_mgr.clone(),
                            )
                            .await?,
                        ));
                    }
                },
                History::Property(record) => match record {
                    PropertyHistoryRecord::Managed { obj, state } => match &mut self.body_pane {
                        BodyPane::ResourceBrowser(_) | BodyPane::StaticPropertyBrowser(_) => {
                            self.body_pane = BodyPane::PropertyBrowser(
                                PropertyBrowserManager::from_history_record(
                                    PropertyHistoryRecord::Managed { obj, state },
                                    self.cache_mgr.clone(),
                                )
                                .await?,
                            );
                        }
                        BodyPane::PropertyBrowser(property_mgr) => {
                            property_mgr
                                .load_history_record(PropertyHistoryRecord::Managed { obj, state })
                                .await?;
                        }
                    },
                    PropertyHistoryRecord::Static {
                        metadata,
                        root,
                        state,
                    } => match &mut self.body_pane {
                        BodyPane::ResourceBrowser(_) | BodyPane::PropertyBrowser(_) => {
                            self.body_pane = BodyPane::StaticPropertyBrowser(
                                StaticPropertyBrowserManager::from_history(metadata, root, state)?,
                            );
                        }
                        BodyPane::StaticPropertyBrowser(static_mgr) => {
                            static_mgr.load_history_record(metadata, root, state)?;
                        }
                    },
                },
            }
            self.refresh_polling_demand();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::PerfDemandState;
    use crate::perf_worker::PerfRequest;
    use crate::resource_browser::perf::new_perf_snapshot_share;
    use vim_rs::types::enums::MoTypesEnum;
    use vim_rs::types::structs::ManagedObjectReference;

    fn vm(id: &str) -> ManagedObjectReference {
        ManagedObjectReference {
            r#type: MoTypesEnum::VirtualMachine,
            value: id.into(),
        }
    }

    #[test]
    fn perf_demand_matches_only_when_generation_entities_and_snapshot_match() {
        let shared_snapshot = new_perf_snapshot_share();
        let same = PerfDemandState::active(&PerfRequest {
            generation: 7,
            entities: vec![vm("vm-1"), vm("vm-2")],
            snapshot: shared_snapshot.clone(),
        });
        let same_again = PerfDemandState::active(&PerfRequest {
            generation: 7,
            entities: vec![vm("vm-1"), vm("vm-2")],
            snapshot: shared_snapshot,
        });
        let different_entities = PerfDemandState::active(&PerfRequest {
            generation: 7,
            entities: vec![vm("vm-1")],
            snapshot: new_perf_snapshot_share(),
        });

        assert!(same.matches(&same_again));
        assert!(!same.matches(&different_entities));
    }

    #[test]
    fn perf_demand_distinguishes_pause_state_and_generation() {
        let paused = PerfDemandState::paused(3);
        let same_paused = PerfDemandState::paused(3);
        let different_generation = PerfDemandState::paused(4);
        let active = PerfDemandState::active(&PerfRequest {
            generation: 3,
            entities: vec![vm("vm-1")],
            snapshot: new_perf_snapshot_share(),
        });

        assert!(paused.matches(&same_paused));
        assert!(!paused.matches(&different_generation));
        assert!(!paused.matches(&active));
    }
}

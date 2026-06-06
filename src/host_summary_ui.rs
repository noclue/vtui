//! Host summary modal: loading, scrollable content, scrollbar.

use crate::host_summary::{
    HostDiskRow, HostGraphicsRow, HostMemoryTierRow, HostPnicRow, HostSummary, HostVmRow, LOG_TARGET,
};
use crate::operation_types::OperationId;
use crate::resource_browser::formatting::{
    ID_COLUMN_WIDTH, STATUS, STATUS_COLUMN_WIDTH, format_compact_bitrate_bps,
    format_compact_mem_bytes, format_compact_mhz, status_color,
};
use crate::vm_summary::format::format_popup_cpu_mhz;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use log::debug;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::prelude::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, Padding, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState,
};
use vim_rs::types::enums::{
    HostSystemConnectionStateEnum, HostSystemPowerStateEnum, VirtualMachinePowerStateEnum,
};

const HEADER_RIGHT_COL: usize = 39;
const TABLE_COL_GAP: usize = 1;

/// Width for VM **Used** / **CPU** / **Mem** columns (matches compact formatters).
const VM_METRIC_COL_W: usize = 4;
/// VM **Name** column never exceeds this; **OS** receives the rest of the middle region.
const VM_NAME_MAX_W: usize = 35;

const LABEL_COLOR: Color = Color::Gray;
const VALUE_COLOR: Color = Color::Yellow;
const BORDER_COLOR: Color = Color::Yellow;
const BACKGROUND_COLOR: Color = Color::Rgb(32, 32, 32);

const POWER_ON: &str = "● ";
const POWER_OFF: &str = "○ ";
const SUSPENDED: &str = "◐ ";

#[derive(Debug, Default)]
pub struct HostSummaryUi {
    layer: HostSummaryLayer,
    pending_request: Option<OperationId>,
}

#[derive(Debug, Default)]
#[allow(clippy::large_enum_variant)]
enum HostSummaryLayer {
    #[default]
    Closed,
    Loading {
        _request_id: OperationId,
    },
    Ready {
        summary: HostSummary,
        scroll: u16,
        text: Text<'static>,
        content_width: u16,
        viewport_height: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostSummaryKeyOutcome {
    Ignored,
    Consumed,
    Close,
}

impl HostSummaryUi {
    pub fn is_active(&self) -> bool {
        !matches!(self.layer, HostSummaryLayer::Closed)
    }

    pub fn start_loading(&mut self, request_id: OperationId) {
        debug!(
            target: LOG_TARGET,
            "host summary ui: loading state request_id={request_id}"
        );
        self.pending_request = Some(request_id);
        self.layer = HostSummaryLayer::Loading {
            _request_id: request_id,
        };
    }

    pub fn close(&mut self) {
        if !matches!(self.layer, HostSummaryLayer::Closed) {
            debug!(target: LOG_TARGET, "host summary ui: close");
        }
        self.layer = HostSummaryLayer::Closed;
        self.pending_request = None;
    }

    pub fn pending_matches(&self, request_id: OperationId) -> bool {
        self.pending_request == Some(request_id)
    }

    pub fn apply_success(&mut self, request_id: OperationId, summary: HostSummary) {
        if self.pending_request != Some(request_id) {
            debug!(
                target: LOG_TARGET,
                "host summary ui: apply_success ignored (stale request_id={request_id} name={})",
                summary.host_name
            );
            return;
        }
        self.pending_request = None;
        debug!(
            target: LOG_TARGET,
            "host summary ui: showing summary request_id={request_id} name={} nics={} disks={} vms={}",
            summary.host_name,
            summary.nics.len(),
            summary.disks.len(),
            summary.vms.len()
        );
        self.layer = HostSummaryLayer::Ready {
            summary,
            scroll: 0,
            text: Text::default(),
            content_width: 0,
            viewport_height: 0,
        };
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> HostSummaryKeyOutcome {
        match &mut self.layer {
            HostSummaryLayer::Closed => HostSummaryKeyOutcome::Ignored,
            HostSummaryLayer::Loading { .. } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.close();
                    HostSummaryKeyOutcome::Close
                }
                _ => HostSummaryKeyOutcome::Consumed,
            },
            HostSummaryLayer::Ready {
                scroll,
                text,
                viewport_height,
                ..
            } => {
                let n_lines = text.lines.len();
                let max_scroll = max_line_scroll_offset(n_lines, *viewport_height);
                let page = 10u16;
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                        self.close();
                        HostSummaryKeyOutcome::Close
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        *scroll = scroll.saturating_sub(1);
                        HostSummaryKeyOutcome::Consumed
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *scroll = (*scroll + 1).min(max_scroll);
                        HostSummaryKeyOutcome::Consumed
                    }
                    KeyCode::PageUp | KeyCode::Char('b')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        *scroll = scroll.saturating_sub(page);
                        HostSummaryKeyOutcome::Consumed
                    }
                    KeyCode::PageDown | KeyCode::Char('f')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        *scroll = (*scroll + page).min(max_scroll);
                        HostSummaryKeyOutcome::Consumed
                    }
                    KeyCode::PageUp => {
                        *scroll = scroll.saturating_sub(page);
                        HostSummaryKeyOutcome::Consumed
                    }
                    KeyCode::PageDown => {
                        *scroll = (*scroll + page).min(max_scroll);
                        HostSummaryKeyOutcome::Consumed
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        *scroll = 0;
                        HostSummaryKeyOutcome::Consumed
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        *scroll = max_scroll;
                        HostSummaryKeyOutcome::Consumed
                    }
                    _ => HostSummaryKeyOutcome::Consumed,
                }
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        match &mut self.layer {
            HostSummaryLayer::Closed => {}
            HostSummaryLayer::Loading { .. } => {
                let area = summary_popup_rect(frame.area());
                let block = Block::default()
                    .title(" Host summary ")
                    .style(Style::default().bg(BACKGROUND_COLOR))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(BORDER_COLOR))
                    .title_bottom(Line::from("Esc / q close"));
                let p = Paragraph::new("\n  Loading host summary…")
                    .alignment(Alignment::Center)
                    .style(Style::default().bg(BACKGROUND_COLOR))
                    .block(block);
                frame.render_widget(Clear, area);
                frame.render_widget(p, area);
            }
            HostSummaryLayer::Ready {
                scroll,
                text,
                summary,
                content_width,
                viewport_height,
            } => {
                let area = summary_popup_rect(frame.area());
                frame.render_widget(Clear, area);

                let title = format!(" {} ", popup_title(summary));
                let footer =
                    "Esc/q close  ↑/↓ scroll  PgUp/PgDn page  g/G top/bottom  Ctrl-b/f page";
                let block = Block::default()
                    .title(title)
                    .style(Style::default().bg(BACKGROUND_COLOR))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(BORDER_COLOR))
                    .title_bottom(Line::from(footer))
                    .padding(Padding::right(1));

                let inner = block.inner(area);
                *viewport_height = inner.height;
                let inner_w = inner.width;
                if *content_width != inner_w {
                    let lines = build_summary_lines(summary, inner_w as usize);
                    let line_count = lines.len();
                    debug!(
                        target: LOG_TARGET,
                        "host summary ui: layout rebuild content_width={inner_w} lines={line_count}"
                    );
                    *text = Text::from(lines);
                    *content_width = inner_w;
                }
                let max_scroll = max_line_scroll_offset(text.lines.len(), inner.height);
                *scroll = (*scroll).min(max_scroll);

                let paragraph = Paragraph::new(text.clone())
                    .scroll((*scroll, 0))
                    .style(Style::default().bg(BACKGROUND_COLOR))
                    .block(block);

                frame.render_widget(paragraph, area);

                let raw_lines = text.lines.len();
                let vh = inner.height as usize;
                let scrollbar_content_len = scrollbar_content_length(raw_lines, vh);
                let sb_pos = (*scroll as usize).min(scrollbar_content_len.saturating_sub(1));
                let mut sb_state = ScrollbarState::new(scrollbar_content_len)
                    .position(sb_pos)
                    .viewport_content_length(vh);
                let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None)
                    .track_style(Style::default().bg(Color::DarkGray).fg(Color::DarkGray))
                    .thumb_style(Style::default().bg(Color::Gray).fg(Color::Gray));
                let sb_area = Rect {
                    x: inner.x + inner.width,
                    y: inner.y,
                    width: 1,
                    height: inner.height,
                };
                frame.render_stateful_widget(sb, sb_area, &mut sb_state);
            }
        }
    }
}

fn popup_title(s: &HostSummary) -> String {
    let path = s.inventory_path.trim();
    if path.is_empty() {
        format!("Host summary — {}", s.host_name)
    } else {
        format!("Host summary — {path}")
    }
}

fn scrollbar_content_length(n_lines: usize, viewport_h: usize) -> usize {
    if n_lines == 0 || viewport_h == 0 {
        return 1;
    }
    if n_lines <= viewport_h {
        1
    } else {
        n_lines - viewport_h + 1
    }
}

fn max_line_scroll_offset(n_lines: usize, viewport_h: u16) -> u16 {
    if n_lines == 0 || viewport_h == 0 {
        return 0;
    }
    let vh = viewport_h as usize;
    if n_lines <= vh {
        0
    } else {
        u16::try_from(n_lines - vh).unwrap_or(u16::MAX)
    }
}

fn summary_popup_rect(r: Rect) -> Rect {
    const SIDE_MARGIN: u16 = 2;
    let avail_w = r.width.saturating_sub(SIDE_MARGIN * 2);
    let avail_h = r.height.saturating_sub(SIDE_MARGIN * 2);
    let w = avail_w.max(20);
    let h = (avail_h * 80 / 100).max(8).min(avail_h);
    Rect {
        x: r.x + SIDE_MARGIN,
        y: r.y + (r.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}

fn header_column_widths(total: usize) -> (usize, usize) {
    if total > HEADER_RIGHT_COL {
        (total - HEADER_RIGHT_COL, HEADER_RIGHT_COL)
    } else if total > 1 {
        let left = total / 2;
        (left, total - left)
    } else {
        (0, total.max(1))
    }
}

fn lbl(s: &'static str) -> Span<'static> {
    Span::styled(s, Style::default().fg(LABEL_COLOR))
}

fn val_span(s: String) -> Span<'static> {
    Span::styled(s, Style::default().fg(VALUE_COLOR))
}

fn table_hdr(s: impl Into<String>) -> Span<'static> {
    Span::styled(
        s.into(),
        Style::default()
            .fg(LABEL_COLOR)
            .add_modifier(Modifier::BOLD),
    )
}

fn table_val(s: String) -> Span<'static> {
    Span::styled(s, Style::default().fg(VALUE_COLOR))
}

fn spans_display_len(spans: &[Span]) -> usize {
    spans.iter().map(|sp| sp.content.chars().count()).sum()
}

fn clip_spans_to_width(spans: Vec<Span<'static>>, max: usize) -> Vec<Span<'static>> {
    if spans_display_len(&spans) <= max {
        return spans;
    }
    let mut out = Vec::new();
    let mut used = 0usize;
    for sp in spans {
        let len = sp.content.chars().count();
        if used + len <= max {
            used += len;
            out.push(sp);
        } else {
            let rem = max.saturating_sub(used);
            if rem == 0 {
                break;
            }
            let take = rem.saturating_sub(1).max(1);
            let ch: String = sp.content.chars().take(take).collect();
            let clipped = if rem > 1 && sp.content.chars().count() > take {
                format!("{ch}…")
            } else {
                "…".to_string()
            };
            out.push(Span::styled(clipped, sp.style));
            break;
        }
    }
    out
}

fn pad_spans_to_width(mut spans: Vec<Span<'static>>, width: usize) -> Vec<Span<'static>> {
    let n = spans_display_len(&spans);
    if n < width {
        spans.push(Span::raw(" ".repeat(width - n)));
    }
    spans
}

fn header_row(
    left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    left_w: usize,
    right_w: usize,
) -> Line<'static> {
    let l = pad_spans_to_width(clip_spans_to_width(left, left_w), left_w);
    let r = pad_spans_to_width(clip_spans_to_width(right, right_w), right_w);
    let mut v = l;
    v.extend(r);
    Line::from(v)
}

fn join_table_4(c0: &str, c1: &str, c2: &str, c3: &str) -> String {
    let g = " ".repeat(TABLE_COL_GAP);
    format!("{c0}{g}{c1}{g}{c2}{g}{c3}")
}

fn join_table_5(c0: &str, c1: &str, c2: &str, c3: &str, c4: &str) -> String {
    let g = " ".repeat(TABLE_COL_GAP);
    format!("{c0}{g}{c1}{g}{c2}{g}{c3}{g}{c4}")
}

fn join_table_8(a: [&str; 8]) -> String {
    let g = " ".repeat(TABLE_COL_GAP);
    format!(
        "{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
        a[0], g, a[1], g, a[2], g, a[3], g, a[4], g, a[5], g, a[6], g, a[7]
    )
}

fn vm_row_gap() -> Span<'static> {
    Span::raw(" ".repeat(TABLE_COL_GAP))
}

/// Left-aligned cell of exactly `width` Unicode scalar characters (padding or ellipsis).
/// For `width == 0`, returns an empty string.
fn fit_cell(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let n = s.chars().count();
    if n <= width {
        let mut out = String::with_capacity(width);
        out.push_str(s);
        while out.chars().count() < width {
            out.push(' ');
        }
        out
    } else if width == 1 {
        "…".to_string()
    } else {
        let take = width - 1;
        let t: String = s.chars().take(take).collect();
        let mut out = t;
        out.push('…');
        debug_assert_eq!(out.chars().count(), width);
        out
    }
}

fn format_pnic_speed_dup(mbps: Option<i32>, duplex: Option<bool>, width: usize) -> String {
    let rate = mbps.filter(|&m| m > 0).map(|m| {
        let bps = (m as u64).saturating_mul(1_000_000);
        format_compact_bitrate_bps(bps)
    });
    let d = match duplex {
        Some(true) => 'F',
        Some(false) => 'H',
        None => '-',
    };
    let inner = match rate {
        Some(r) => format!("{r} {d}"),
        None => format!("   - {d}"),
    };
    fit_cell(&inner, width)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let t: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{t}…")
}

fn format_host_power_uptime(s: &HostSummary) -> String {
    match s.power_state {
        HostSystemPowerStateEnum::PoweredOn => {
            if let Some(sec) = s.uptime_seconds.filter(|v| *v > 0) {
                let d = sec / 86400;
                let h = (sec % 86400) / 3600;
                format!("Up ({}d {}h)", d, h)
            } else {
                "Powered on".to_string()
            }
        }
        HostSystemPowerStateEnum::PoweredOff => "Powered off".to_string(),
        HostSystemPowerStateEnum::StandBy => "Standby".to_string(),
        HostSystemPowerStateEnum::Unknown => "Unknown".to_string(),
        _ => format!("{:?}", s.power_state),
    }
}

fn format_connection(cs: &HostSystemConnectionStateEnum) -> &'static str {
    match cs {
        HostSystemConnectionStateEnum::Connected => "Connected",
        HostSystemConnectionStateEnum::NotResponding => "Not responding",
        HostSystemConnectionStateEnum::Disconnected => "Disconnected",
        _ => "Other",
    }
}

fn format_vm_power(ps: &VirtualMachinePowerStateEnum) -> &'static str {
    match ps {
        VirtualMachinePowerStateEnum::PoweredOn => POWER_ON,
        VirtualMachinePowerStateEnum::PoweredOff => POWER_OFF,
        VirtualMachinePowerStateEnum::Suspended => SUSPENDED,
        _ => "? ",
    }
}

fn build_summary_lines(s: &HostSummary, total_width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let (left_w, right_w) = header_column_widths(total_width);

    let status_style = status_color(&s.overall_status);
    let power_uptime = format_host_power_uptime(s);

    let cpu_str = s
        .cpu_usage_mhz
        .map(format_popup_cpu_mhz)
        .unwrap_or_else(|| "-".to_string());

    let mem_str = s
        .memory_usage_mb
        .map(|mb| format_compact_mem_bytes((mb as i128) * 1024 * 1024))
        .unwrap_or_else(|| "-".to_string());

    let hw_mem = s
        .hw_memory_size_bytes
        .map(|b| format_compact_mem_bytes(b as i128))
        .unwrap_or_else(|| "-".to_string());

    let vendor_model = match (s.hw_vendor.as_deref(), s.hw_model.as_deref()) {
        (Some(v), Some(m)) if !v.is_empty() || !m.is_empty() => {
            format!("{} {}", v.trim(), m.trim()).trim().to_string()
        }
        (Some(v), _) if !v.is_empty() => v.trim().to_string(),
        (_, Some(m)) if !m.is_empty() => m.trim().to_string(),
        _ => "-".to_string(),
    };

    let cpu_model = s.hw_cpu_model.clone().unwrap_or_else(|| "-".to_string());

    let pkgs = s
        .hw_num_cpu_pkgs
        .map(|n| n.to_string())
        .unwrap_or_else(|| "-".to_string());
    let cores = s
        .hw_num_cpu_cores
        .map(|n| n.to_string())
        .unwrap_or_else(|| "-".to_string());
    let threads = s
        .hw_num_cpu_threads
        .map(|n| n.to_string())
        .unwrap_or_else(|| "-".to_string());
    let cpu_hw_mhz = s
        .hw_cpu_mhz
        .map(format_popup_cpu_mhz)
        .unwrap_or_else(|| "-".to_string());

    lines.push(header_row(
        vec![
            lbl("Host: "),
            val_span(format!("{} ({})", s.host_name, s.host_id)),
        ],
        vec![
            lbl("Connection: "),
            val_span(format_connection(&s.connection_state).to_string()),
        ],
        left_w,
        right_w,
    ));

    lines.push(header_row(
        vec![
            lbl("Status/Power: "),
            Span::styled(STATUS, status_style),
            Span::raw(" "),
            val_span(power_uptime),
        ],
        vec![lbl("CPU usage: "), val_span(cpu_str)],
        left_w,
        right_w,
    ));

    lines.push(header_row(
        vec![lbl("Memory usage: "), val_span(mem_str)],
        vec![lbl("HW memory: "), val_span(hw_mem)],
        left_w,
        right_w,
    ));

    lines.push(header_row(
        vec![lbl("Vendor / model: "), val_span(vendor_model)],
        vec![lbl("CPU model: "), val_span(cpu_model)],
        left_w,
        right_w,
    ));

    lines.push(header_row(
        vec![
            lbl("CPU pkgs/cores/thr: "),
            val_span(format!("{pkgs} / {cores} / {threads}")),
        ],
        vec![lbl("CPU rated: "), val_span(cpu_hw_mhz)],
        left_w,
        right_w,
    ));

    if !s.memory_tiers.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Memory tiering",
            Style::default()
                .fg(LABEL_COLOR)
                .add_modifier(Modifier::BOLD),
        )));
        for l in format_memory_tier_table(&s.memory_tiers, total_width) {
            lines.push(l);
        }
    }

    if !s.graphics.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Graphics",
            Style::default()
                .fg(LABEL_COLOR)
                .add_modifier(Modifier::BOLD),
        )));
        for l in format_graphics_table(&s.graphics, total_width) {
            lines.push(l);
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Physical NICs",
        Style::default()
            .fg(LABEL_COLOR)
            .add_modifier(Modifier::BOLD),
    )));
    for l in format_pnic_table(&s.nics, total_width) {
        lines.push(l);
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Disks (SCSI LUNs)",
        Style::default()
            .fg(LABEL_COLOR)
            .add_modifier(Modifier::BOLD),
    )));
    for l in format_disk_table(&s.disks, total_width) {
        lines.push(l);
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Virtual machines",
        Style::default()
            .fg(LABEL_COLOR)
            .add_modifier(Modifier::BOLD),
    )));
    if s.total_vm_count > s.vms.len() {
        let banner = format!(
            "Showing {} of {} resident VMs",
            s.vms.len(),
            s.total_vm_count
        );
        lines.push(Line::from(Span::styled(
            banner,
            Style::default().fg(LABEL_COLOR),
        )));
    }
    for l in format_vm_table(&s.vms, total_width) {
        lines.push(l);
    }

    lines
}

fn format_memory_tier_table(rows: &[HostMemoryTierRow], total_width: usize) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return vec![Line::from("(no tiers)")];
    }
    let name_w = (total_width / 4).clamp(8, 24);
    let type_w = (total_width / 5).clamp(6, 16);
    let size_w = 10.min(total_width / 6);
    let flags_w = total_width.saturating_sub(name_w + type_w + size_w + 3 * TABLE_COL_GAP);
    let hdr = join_table_4(
        &fit_cell("Name", name_w),
        &fit_cell("Type", type_w),
        &fit_cell("Size", size_w),
        &fit_cell("Flags", flags_w.max(4)),
    );
    let mut out = vec![Line::from(vec![table_hdr(hdr)])];
    for r in rows {
        let sz = format_compact_mem_bytes(r.size_bytes as i128);
        let flags = if r.flags.is_empty() {
            "-".to_string()
        } else {
            r.flags.join(", ")
        };
        let row = join_table_4(
            &fit_cell(&truncate(&r.name, name_w), name_w),
            &fit_cell(&truncate(&r.tier_type, type_w), type_w),
            &fit_cell(&sz, size_w),
            &fit_cell(&truncate(&flags, flags_w.max(4)), flags_w.max(4)),
        );
        out.push(Line::from(vec![table_val(row)]));
    }
    out
}

fn format_graphics_table(rows: &[HostGraphicsRow], total_width: usize) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return vec![Line::from("(no graphics)")];
    }
    const TYPE_CAP: usize = 10;
    const VRAM_CAP: usize = 8;
    const VM_CAP: usize = 8;
    let gaps = 4 * TABLE_COL_GAP;
    let fixed_suffix = TYPE_CAP + VRAM_CAP + VM_CAP + gaps;
    let flex = total_width.saturating_sub(fixed_suffix);
    let dev_w = (flex * 58 / 100).max(12);
    let vend_w = flex.saturating_sub(dev_w).max(8);
    let typ_w = TYPE_CAP;
    let vram_w = VRAM_CAP;
    let vm_w = VM_CAP;
    let hdr = join_table_5(
        &fit_cell("Device", dev_w),
        &fit_cell("Vendor", vend_w),
        &fit_cell("Type", typ_w),
        &fit_cell("VRAM", vram_w),
        &fit_cell("VMs", vm_w),
    );
    let mut out = vec![Line::from(vec![table_hdr(hdr)])];
    for r in rows {
        let vram_b = r.memory_size_kb.saturating_mul(1024);
        let vram = format_compact_mem_bytes(vram_b as i128);
        let row = join_table_5(
            &fit_cell(&truncate(&r.device_name, dev_w), dev_w),
            &fit_cell(&truncate(&r.vendor_name, vend_w), vend_w),
            &fit_cell(&truncate(&r.graphics_type, typ_w), typ_w),
            &fit_cell(&vram, vram_w),
            &fit_cell(&r.attached_vm_count.to_string(), vm_w),
        );
        out.push(Line::from(vec![table_val(row)]));
    }
    out
}

fn pnic_col_widths(total: usize) -> (usize, usize, usize, usize, usize, usize) {
    let g = 5 * TABLE_COL_GAP;
    let budget = total.saturating_sub(g);
    let spd = 11usize;
    let wol = 3usize;
    let rest = budget.saturating_sub(spd + wol);
    let dev = (rest * 30) / 100;
    let drv = (rest * 24) / 100;
    let mac = (rest * 28) / 100;
    let pci = rest.saturating_sub(dev + drv + mac);
    (dev.max(6), drv.max(4), mac.max(14), spd, pci.max(8), wol)
}

fn format_pnic_table(rows: &[HostPnicRow], total_width: usize) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return vec![Line::from("(no NICs)")];
    }
    let (w_dev, w_drv, w_mac, w_spd, w_pci, w_wol) = pnic_col_widths(total_width.max(72));
    let hdr = join_table_6_hdr(w_dev, w_drv, w_mac, w_spd, w_pci, w_wol);
    let mut out = vec![Line::from(vec![table_hdr(hdr)])];
    for r in rows {
        let spd_dup = format_pnic_speed_dup(r.link_speed_mbps, r.duplex, w_spd);
        let row = join_table_6_row(
            &fit_cell(&truncate(&r.device, w_dev), w_dev),
            &fit_cell(&truncate(r.driver.as_deref().unwrap_or("-"), w_drv), w_drv),
            &fit_cell(&truncate(&r.mac, w_mac), w_mac),
            &spd_dup,
            &fit_cell(&truncate(&r.pci, w_pci), w_pci),
            &fit_cell(if r.wake_on_lan_supported { "Y" } else { "N" }, w_wol),
        );
        out.push(Line::from(vec![table_val(row)]));
    }
    out
}

fn join_table_6_hdr(d: usize, drv: usize, m: usize, sp: usize, p: usize, w: usize) -> String {
    let g = " ".repeat(TABLE_COL_GAP);
    format!(
        "{}{}{}{}{}{}{}{}{}{}{}",
        fit_cell("Device", d),
        g,
        fit_cell("Driver", drv),
        g,
        fit_cell("MAC", m),
        g,
        fit_cell("Speed/Dup", sp),
        g,
        fit_cell("PCI", p),
        g,
        fit_cell("WOL", w)
    )
}

fn join_table_6_row(a: &str, b: &str, c: &str, sp: &str, e: &str, f: &str) -> String {
    let g = " ".repeat(TABLE_COL_GAP);
    format!("{a}{g}{b}{g}{c}{g}{sp}{g}{e}{g}{f}")
}

fn disk_col_widths(total: usize) -> (usize, usize, usize, usize, usize, usize) {
    let gaps = 5 * TABLE_COL_GAP;
    let budget = total.saturating_sub(gaps);
    let cap = 8usize;
    let ssd = 4usize;
    let loc = 4usize;
    let vend = 6usize;
    let model = 16usize;
    let tail = vend + model + cap + ssd + loc;
    let dev = budget.saturating_sub(tail).max(8);
    (dev, vend, model, cap, ssd, loc)
}

fn format_disk_table(rows: &[HostDiskRow], total_width: usize) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return vec![Line::from("(no disks)")];
    }
    let (w_d, w_v, w_m, w_cap, w_ssd, w_loc) = disk_col_widths(total_width.max(80));
    let hdr = {
        let g = " ".repeat(TABLE_COL_GAP);
        [
            fit_cell("Device", w_d),
            fit_cell("Vendor", w_v),
            fit_cell("Model", w_m),
            fit_cell("Capacity", w_cap),
            fit_cell("SSD", w_ssd),
            fit_cell("Loc", w_loc),
        ]
        .join(&g)
    };
    let mut out = vec![Line::from(vec![table_hdr(hdr)])];
    for r in rows {
        let cap = r
            .capacity_bytes
            .map(|b| format_compact_mem_bytes(b as i128))
            .unwrap_or_else(|| "-".to_string());
        let ssd = r.ssd.map(|b| if b { "yes" } else { "no" }).unwrap_or("-");
        let loc = r.local.map(|b| if b { "yes" } else { "no" }).unwrap_or("-");
        let g = " ".repeat(TABLE_COL_GAP);
        let row = [
            fit_cell(&truncate(&r.device_name, w_d), w_d),
            fit_cell(&truncate(r.vendor.as_deref().unwrap_or("-"), w_v), w_v),
            fit_cell(&truncate(r.model.as_deref().unwrap_or("-"), w_m), w_m),
            fit_cell(&cap, w_cap),
            fit_cell(ssd, w_ssd),
            fit_cell(loc, w_loc),
        ]
        .join(&g);
        out.push(Line::from(vec![table_val(row)]));
    }
    out
}

fn vm_col_widths(total: usize) -> [usize; 8] {
    let gaps = 7 * TABLE_COL_GAP;
    let id = ID_COLUMN_WIDTH as usize;
    let st = STATUS_COLUMN_WIDTH as usize;
    let pw = STATUS_COLUMN_WIDTH as usize;
    let metrics = VM_METRIC_COL_W * 3;
    let middle = total.saturating_sub(gaps + id + st + pw + metrics);
    let name_w = middle.min(VM_NAME_MAX_W);
    let os_w = middle.saturating_sub(name_w);
    [
        id,
        st,
        pw,
        name_w,
        os_w,
        VM_METRIC_COL_W,
        VM_METRIC_COL_W,
        VM_METRIC_COL_W,
    ]
}

fn format_vm_table(rows: &[HostVmRow], total_width: usize) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return vec![Line::from("(no VMs on host)")];
    }
    let w = vm_col_widths(total_width.max(72));
    let hdr = join_table_8([
        &fit_cell("ID", w[0]),
        &fit_cell("S", w[1]),
        &fit_cell("P", w[2]),
        &fit_cell("Name", w[3]),
        &fit_cell("OS", w[4]),
        &fit_cell("Used", w[5]),
        &fit_cell("CPU", w[6]),
        &fit_cell("Mem", w[7]),
    ]);
    let mut out = vec![Line::from(vec![table_hdr(hdr)])];
    for r in rows {
        let status_style = status_color(&r.overall_status);
        let pw = format_vm_power(&r.power_state);
        let used = r
            .storage_used_bytes
            .map(|b| format_compact_mem_bytes(b as i128))
            .unwrap_or_else(|| "-".to_string());
        let cpu = r
            .cpu_usage_mhz
            .map(|mhz| format_compact_mhz(mhz as i64))
            .unwrap_or_else(|| "-".to_string());
        let mem = r
            .memory_usage_mb
            .map(|mb| format_compact_mem_bytes((mb as i128) * 1024 * 1024))
            .unwrap_or_else(|| "-".to_string());
        let name_s = Span::styled(
            fit_cell(&truncate(&r.vm_name, w[3]), w[3]),
            Style::default().fg(VALUE_COLOR),
        );
        let line = Line::from(vec![
            table_val(fit_cell(&r.vm_id, w[0])),
            vm_row_gap(),
            Span::styled(fit_cell(STATUS, w[1]), status_style),
            vm_row_gap(),
            Span::styled(fit_cell(pw, w[2]), Style::default().fg(VALUE_COLOR)),
            vm_row_gap(),
            name_s,
            vm_row_gap(),
            table_val(fit_cell(
                &truncate(r.guest_os.as_deref().unwrap_or("-"), w[4]),
                w[4],
            )),
            vm_row_gap(),
            table_val(fit_cell(&used, w[5])),
            vm_row_gap(),
            table_val(fit_cell(&cpu, w[6])),
            vm_row_gap(),
            table_val(fit_cell(&mem, w[7])),
        ]);
        out.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_summary::{
        HostDiskRow, HostGraphicsRow, HostMemoryTierRow, HostPnicRow, HostSummary, HostVmRow,
    };
    use insta::assert_snapshot;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use vim_rs::types::enums::{
        HostSystemConnectionStateEnum, HostSystemPowerStateEnum, ManagedEntityStatusEnum,
        VirtualMachinePowerStateEnum,
    };

    fn minimal_summary() -> HostSummary {
        HostSummary {
            host_id: "host-9".into(),
            host_name: "esxi-lab".into(),
            inventory_path: "/dc/host/cluster/esxi-lab".into(),
            overall_status: ManagedEntityStatusEnum::Green,
            connection_state: HostSystemConnectionStateEnum::Connected,
            power_state: HostSystemPowerStateEnum::PoweredOn,
            uptime_seconds: Some(3600),
            cpu_usage_mhz: Some(1200),
            memory_usage_mb: Some(8192),
            hw_vendor: Some("VendorCo".into()),
            hw_model: Some("Gen12".into()),
            hw_cpu_model: Some("Xeon Gold".into()),
            hw_cpu_mhz: Some(2400),
            hw_num_cpu_pkgs: Some(2),
            hw_num_cpu_cores: Some(24),
            hw_num_cpu_threads: Some(48),
            hw_memory_size_bytes: Some(128 * 1024 * 1024 * 1024),
            nics: vec![],
            disks: vec![],
            memory_tiers: vec![],
            graphics: vec![],
            vms: vec![],
            total_vm_count: 0,
        }
    }

    fn render_snapshot(ui: &mut HostSummaryUi, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| ui.render(f)).unwrap();
        format!("{}", term.backend())
    }

    #[test]
    fn fit_cell_scalar_width_matches_requested() {
        let samples = ["", "a", "hello", "☆★", "ジェイソン"];
        for width in 1..=40 {
            for s in samples {
                let out = fit_cell(s, width);
                assert_eq!(
                    out.chars().count(),
                    width,
                    "fit_cell({s:?}, {width}) -> {out:?}"
                );
            }
        }
        assert!(fit_cell("xy", 0).is_empty());
    }

    #[test]
    fn keys_close_loading() {
        let mut ui = HostSummaryUi::default();
        ui.start_loading(7);
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let k = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(ui.handle_key(&k), HostSummaryKeyOutcome::Close);
        assert!(!ui.is_active());
    }

    #[test]
    fn keys_scroll_ready() {
        let mut ui = HostSummaryUi::default();
        ui.start_loading(1);
        ui.apply_success(1, minimal_summary());
        let mut term = Terminal::new(TestBackend::new(80, 28)).unwrap();
        term.draw(|f| ui.render(f)).unwrap();
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let k = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(ui.handle_key(&k), HostSummaryKeyOutcome::Consumed);
    }

    #[test]
    fn stale_success_ignored() {
        let mut ui = HostSummaryUi::default();
        ui.start_loading(1);
        ui.apply_success(2, minimal_summary());
        assert!(ui.pending_matches(1));
        assert!(ui.is_active());
    }

    #[test]
    fn snapshot_loading() {
        let mut ui = HostSummaryUi::default();
        ui.start_loading(3);
        assert_snapshot!(render_snapshot(&mut ui, 80, 18));
    }

    #[test]
    fn snapshot_ready_minimal() {
        let mut ui = HostSummaryUi::default();
        ui.start_loading(1);
        ui.apply_success(1, minimal_summary());
        assert_snapshot!(render_snapshot(&mut ui, 80, 22));
    }

    #[test]
    fn snapshot_rich_hardware() {
        let mut s = minimal_summary();
        s.nics.push(HostPnicRow {
            device: "vmnic0".into(),
            driver: Some("bnxtnet".into()),
            driver_version: None,
            firmware_version: None,
            mac: "00:11:22:33:44:55".into(),
            link_speed_mbps: Some(10000),
            duplex: Some(true),
            pci: "0000:04:00.0".into(),
            wake_on_lan_supported: true,
        });
        s.disks.push(HostDiskRow {
            device_name: "naa.aaa".into(),
            vendor: Some("ATA".into()),
            model: Some("SSD500".into()),
            capacity_bytes: Some(500_000_000_000),
            ssd: Some(true),
            local: Some(true),
        });
        s.memory_tiers.push(HostMemoryTierRow {
            name: "Tier0".into(),
            tier_type: "dram".into(),
            size_bytes: 64_i64 * 1024 * 1024 * 1024,
            flags: vec!["fast".into()],
        });
        s.graphics.push(HostGraphicsRow {
            device_name: "gpu0".into(),
            vendor_name: "NVIDIA".into(),
            pci_id: "0000:86:00.0".into(),
            graphics_type: "sharedPassthru".into(),
            memory_size_kb: 16 * 1024,
            vgpu_mode: None,
            attached_vm_count: 0,
        });
        let mut ui = HostSummaryUi::default();
        ui.start_loading(1);
        ui.apply_success(1, s);
        assert_snapshot!(render_snapshot(&mut ui, 100, 28));
    }

    #[test]
    fn snapshot_vm_cap_banner() {
        let mut s = minimal_summary();
        s.total_vm_count = 400;
        s.vms.push(HostVmRow {
            vm_id: "vm-1".into(),
            vm_name: "a".into(),
            overall_status: ManagedEntityStatusEnum::Green,
            power_state: VirtualMachinePowerStateEnum::PoweredOn,
            guest_os: Some("Linux".into()),
            storage_used_bytes: Some(10),
            cpu_usage_mhz: Some(100),
            memory_usage_mb: Some(512),
        });
        let mut ui = HostSummaryUi::default();
        ui.start_loading(1);
        ui.apply_success(1, s);
        assert_snapshot!(render_snapshot(&mut ui, 90, 24));
    }
}

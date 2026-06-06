//! VM summary modal: loading, scrollable content, scrollbar.

use crate::operation_types::OperationId;
use crate::resource_browser::formatting::{STATUS, format_compact_mem_bytes, status_color};
use crate::vm_summary::format::format_popup_cpu_mhz;
use crate::vm_summary::{VmDiskRow, VmNetworkRow, VmSummary};
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
use vim_rs::types::enums::VirtualMachinePowerStateEnum;

/// Right column width (matches `Constraint::Length(39)` style split with `Fill(1)` on the left).
const HEADER_RIGHT_COL: usize = 39;

/// Spaces inserted between table columns so clipped text does not touch the next column.
const TABLE_COL_GAP: usize = 1;

const LABEL_COLOR: Color = Color::Gray;
const VALUE_COLOR: Color = Color::Yellow;
const BORDER_COLOR: Color = Color::Yellow;
const BACKGROUND_COLOR: Color = Color::Rgb(32, 32, 32);

#[derive(Debug, Default)]
pub struct VmSummaryUi {
    layer: VmSummaryLayer,
    pending_request: Option<OperationId>,
}

#[derive(Debug, Default)]
#[allow(clippy::large_enum_variant)]
enum VmSummaryLayer {
    #[default]
    Closed,
    Loading {
        _request_id: OperationId,
    },
    Ready {
        summary: VmSummary,
        scroll: u16,
        text: Text<'static>,
        /// `inner.width` of the popup content; when it changes, body text is rebuilt.
        content_width: u16,
        /// Visible text rows inside the block (for scroll limits and scrollbar thumb).
        viewport_height: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmSummaryKeyOutcome {
    Ignored,
    Consumed,
    Close,
}

impl VmSummaryUi {
    pub fn is_active(&self) -> bool {
        !matches!(self.layer, VmSummaryLayer::Closed)
    }

    pub fn start_loading(&mut self, request_id: OperationId) {
        debug!(
            target: "vm_summary",
            "vm summary ui: loading state request_id={request_id}"
        );
        self.pending_request = Some(request_id);
        self.layer = VmSummaryLayer::Loading {
            _request_id: request_id,
        };
    }

    pub fn close(&mut self) {
        if !matches!(self.layer, VmSummaryLayer::Closed) {
            debug!(target: "vm_summary", "vm summary ui: close");
        }
        self.layer = VmSummaryLayer::Closed;
        self.pending_request = None;
    }

    pub fn pending_matches(&self, request_id: OperationId) -> bool {
        self.pending_request == Some(request_id)
    }

    pub fn apply_success(&mut self, request_id: OperationId, summary: VmSummary) {
        if self.pending_request != Some(request_id) {
            debug!(
                target: "vm_summary",
                "vm summary ui: apply_success ignored (stale request_id={request_id} name={})",
                summary.vm_name
            );
            return;
        }
        self.pending_request = None;
        debug!(
            target: "vm_summary",
            "vm summary ui: showing summary request_id={request_id} name={} nics={} disks={}",
            summary.vm_name,
            summary.networking.len(),
            summary.disks.len()
        );
        self.layer = VmSummaryLayer::Ready {
            summary,
            scroll: 0,
            text: Text::default(),
            content_width: 0,
            viewport_height: 0,
        };
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> VmSummaryKeyOutcome {
        match &mut self.layer {
            VmSummaryLayer::Closed => VmSummaryKeyOutcome::Ignored,
            VmSummaryLayer::Loading { .. } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.close();
                    VmSummaryKeyOutcome::Close
                }
                _ => VmSummaryKeyOutcome::Consumed,
            },
            VmSummaryLayer::Ready {
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
                        VmSummaryKeyOutcome::Close
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        *scroll = scroll.saturating_sub(1);
                        VmSummaryKeyOutcome::Consumed
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *scroll = (*scroll + 1).min(max_scroll);
                        VmSummaryKeyOutcome::Consumed
                    }
                    KeyCode::PageUp | KeyCode::Char('b')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        *scroll = scroll.saturating_sub(page);
                        VmSummaryKeyOutcome::Consumed
                    }
                    KeyCode::PageDown | KeyCode::Char('f')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        *scroll = (*scroll + page).min(max_scroll);
                        VmSummaryKeyOutcome::Consumed
                    }
                    KeyCode::PageUp => {
                        *scroll = scroll.saturating_sub(page);
                        VmSummaryKeyOutcome::Consumed
                    }
                    KeyCode::PageDown => {
                        *scroll = (*scroll + page).min(max_scroll);
                        VmSummaryKeyOutcome::Consumed
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        *scroll = 0;
                        VmSummaryKeyOutcome::Consumed
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        *scroll = max_scroll;
                        VmSummaryKeyOutcome::Consumed
                    }
                    _ => VmSummaryKeyOutcome::Consumed,
                }
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        match &mut self.layer {
            VmSummaryLayer::Closed => {}
            VmSummaryLayer::Loading { .. } => {
                let area = summary_popup_rect(frame.area());
                let block = Block::default()
                    .title(" VM summary ")
                    .style(Style::default().bg(BACKGROUND_COLOR))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(BORDER_COLOR))
                    .title_bottom(Line::from("Esc / q close"));
                let p = Paragraph::new("\n  Loading VM summary…")
                    .alignment(Alignment::Center)
                    .style(Style::default().bg(BACKGROUND_COLOR))
                    .block(block);
                frame.render_widget(Clear, area);
                frame.render_widget(p, area);
            }
            VmSummaryLayer::Ready {
                scroll,
                text,
                summary,
                content_width,
                viewport_height,
            } => {
                let area = summary_popup_rect(frame.area());
                frame.render_widget(Clear, area);

                let title = format!(" VM summary — {} ", summary.vm_name);
                let footer =
                    "Esc/q close  ↑/↓ scroll  PgUp/PgDn page  g/G top/bottom  Ctrl-b/f page";
                let block = Block::default()
                    .title(title)
                    .style(Style::default().bg(BACKGROUND_COLOR))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(BORDER_COLOR))
                    .title_bottom(Line::from(footer))
                    // Reserve the right column for the scrollbar so row styles do not paint under it.
                    .padding(Padding::right(1));

                let inner = block.inner(area);
                *viewport_height = inner.height;
                let inner_w = inner.width;
                if *content_width != inner_w {
                    let lines = build_summary_lines(summary, inner_w as usize);
                    let line_count = lines.len();
                    debug!(
                        target: "vm_summary",
                        "vm summary ui: layout rebuild content_width={inner_w} lines={line_count}"
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
                // Ratatui thumb math uses `content_length - 1` as the max *position*; positions are
                // scroll offsets 0..=(n_lines - viewport). Total positions = n - viewport + 1.
                let scrollbar_content_len = scrollbar_content_length(raw_lines, vh);
                let sb_pos = (*scroll as usize).min(scrollbar_content_len.saturating_sub(1));
                let mut sb_state = ScrollbarState::new(scrollbar_content_len)
                    .position(sb_pos)
                    .viewport_content_length(vh);
                let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None)
                    // Lighter track than the dialog `DarkGray` fill so the gutter reads as its own strip.
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

/// Number of discrete line scroll offsets for [`ScrollbarState::content_length`] (ratatui expects
/// `max_position = content_length - 1` to match the last scroll step where the thumb reaches the end).
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

/// Largest line scroll offset so the bottom of the text can align with the bottom of the viewport
/// (not `n_lines - 1`, which leaves most of the window blank).
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
    // Horizontal margin: leave this many terminal columns free on each side of the dialog.
    const SIDE_MARGIN: u16 = 2;
    let max_w = r.width.saturating_sub(SIDE_MARGIN * 2).max(1);
    let max_h = r.height.saturating_sub(SIDE_MARGIN * 2).max(1);
    let w = max_w;
    let h = (max_h * 80 / 100).max(8.min(max_h)).min(max_h);
    Rect {
        x: r.x + SIDE_MARGIN,
        y: r.y + (r.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}

/// Split total width like `Layout::horizontal([Fill(1), Length(39)])`: fixed right column when it fits.
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

/// Split a comma-separated string (as returned by vSphere for multiple IPs) into display lines.
fn split_comma_phrases(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn build_summary_lines(s: &VmSummary, total_width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    let (left_w, right_w) = header_column_widths(total_width);

    let status_style = status_color(&s.overall_status);
    let power_uptime = format_power_uptime(s);

    let cpu_str = s
        .cpu_usage_mhz
        .map(format_popup_cpu_mhz)
        .unwrap_or_else(|| "-".to_string());

    let mem_str = match (s.host_memory_usage_mb, s.memory_size_mb) {
        (Some(u), Some(p)) => format!(
            "{} of {}",
            format_compact_mem_bytes((u as i128) * 1024 * 1024),
            format_compact_mem_bytes((p as i128) * 1024 * 1024)
        ),
        _ => "-".to_string(),
    };

    let ip_lines = match s.primary_ip.as_deref() {
        None => vec!["-".to_string()],
        Some(p) => {
            let v = split_comma_phrases(p);
            if v.is_empty() {
                vec!["-".to_string()]
            } else {
                v
            }
        }
    };

    let vm_left = vec![
        lbl("VM: "),
        val_span(format!("{} ({})", s.vm_name, s.vm_id)),
    ];
    lines.push(header_row(
        vm_left,
        vec![lbl("IP: "), val_span(ip_lines[0].clone())],
        left_w,
        right_w,
    ));
    // Continuation lines: pad with spaces matching "IP: " so values align with the first row.
    for ip in ip_lines.iter().skip(1) {
        lines.push(header_row(
            vec![],
            vec![lbl("    "), val_span(ip.clone())],
            left_w,
            right_w,
        ));
    }

    let power_left = vec![
        lbl("Status/Power: "),
        Span::styled(STATUS, status_style),
        Span::raw(" "),
        val_span(power_uptime),
    ];
    let vcpu_str = s
        .vcpu_count
        .map(|v| v.to_string())
        .unwrap_or_else(|| "-".to_string());
    lines.push(header_row(
        power_left,
        vec![lbl("vCPUs: "), val_span(vcpu_str)],
        left_w,
        right_w,
    ));

    lines.push(header_row(
        vec![
            lbl("OS: "),
            val_span(s.guest_os.clone().unwrap_or_else(|| "-".to_string())),
        ],
        vec![lbl("CPU: "), val_span(cpu_str)],
        left_w,
        right_w,
    ));

    lines.push(header_row(
        vec![
            lbl("Tools: "),
            val_span(s.tools_line.clone().unwrap_or_else(|| "-".to_string())),
        ],
        vec![lbl("Memory: "), val_span(mem_str)],
        left_w,
        right_w,
    ));

    let host_str = s
        .host
        .as_ref()
        .map(|h| format!("{} ({})", h.host_name, h.host_id))
        .unwrap_or_else(|| "-".to_string());
    let disk_str = s
        .disk_used_bytes
        .map(|b| format_compact_mem_bytes(b as i128))
        .unwrap_or_else(|| "-".to_string());
    lines.push(header_row(
        vec![lbl("Host: "), val_span(host_str)],
        vec![lbl("Disk: "), val_span(disk_str)],
        left_w,
        right_w,
    ));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Networking",
        Style::default()
            .fg(LABEL_COLOR)
            .add_modifier(Modifier::BOLD),
    )));
    for l in format_network_table(&s.networking, total_width) {
        lines.push(l);
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Disks",
        Style::default()
            .fg(LABEL_COLOR)
            .add_modifier(Modifier::BOLD),
    )));
    for l in format_disk_table(&s.disks, total_width) {
        lines.push(l);
    }

    lines
}

fn format_power_uptime(s: &VmSummary) -> String {
    match s.power_state {
        VirtualMachinePowerStateEnum::PoweredOn => {
            if let Some(sec) = s.uptime_seconds.filter(|v| *v > 0) {
                let d = sec / 86400;
                let h = (sec % 86400) / 3600;
                format!("Running ({}d {}h)", d, h)
            } else {
                "Running".to_string()
            }
        }
        VirtualMachinePowerStateEnum::PoweredOff => "Powered Off".to_string(),
        VirtualMachinePowerStateEnum::Suspended => "Suspended".to_string(),
        _ => format!("{:?}", s.power_state),
    }
}

fn network_col_widths(total: usize) -> (usize, usize, usize, usize) {
    const NIC: usize = 22;
    const NET: usize = 31;
    const MAC: usize = 19;
    let gaps = 3 * TABLE_COL_GAP;
    let budget = total.saturating_sub(gaps);
    let used = NIC + NET + MAC;
    if budget <= used {
        let q = (budget / 4).max(2);
        let ip = budget.saturating_sub(q * 3).max(1);
        return (q, q, q, ip);
    }
    let ip = budget.saturating_sub(used).max(4);
    (NIC, NET, MAC, ip)
}

fn join_table_4(c0: &str, c1: &str, c2: &str, c3: &str) -> String {
    let g = " ".repeat(TABLE_COL_GAP);
    format!("{c0}{g}{c1}{g}{c2}{g}{c3}")
}

fn join_table_5(c0: &str, c1: &str, c2: &str, c3: &str, c4: &str) -> String {
    let g = " ".repeat(TABLE_COL_GAP);
    format!("{c0}{g}{c1}{g}{c2}{g}{c3}{g}{c4}")
}

fn fit_cell(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n <= width {
        format!("{:<w$}", s, w = width)
    } else if width <= 1 {
        "…".to_string()
    } else {
        let t: String = s.chars().take(width.saturating_sub(1)).collect();
        let clipped = format!("{t}…");
        format!("{:<w$}", clipped, w = width)
    }
}

fn format_network_table(rows: &[VmNetworkRow], total_width: usize) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return vec![Line::from("(no NICs)")];
    }
    let (w_nic, w_net, w_mac, w_ip) = network_col_widths(total_width.max(72));
    let hdr = join_table_4(
        &fit_cell("NIC", w_nic),
        &fit_cell("Network", w_net),
        &fit_cell("MAC", w_mac),
        &fit_cell("IPs", w_ip),
    );
    let mut out = vec![Line::from(vec![table_hdr(hdr)])];
    for r in rows {
        let mut ip_parts: Vec<String> = Vec::new();
        if r.ips.is_empty() {
            ip_parts.push("-".to_string());
        } else {
            for ip in &r.ips {
                ip_parts.extend(split_comma_phrases(ip));
            }
        }
        if ip_parts.is_empty() {
            ip_parts.push("-".to_string());
        }

        let nic = fit_cell(&truncate(&r.nic_label, w_nic), w_nic);
        let net = fit_cell(&truncate(&r.network, w_net), w_net);
        let mac = fit_cell(&truncate(&r.mac, w_mac), w_mac);
        let row0 = join_table_4(&nic, &net, &mac, &fit_cell(&ip_parts[0], w_ip));
        out.push(Line::from(vec![table_val(row0)]));
        for ip in ip_parts.iter().skip(1) {
            let cont = join_table_4(
                &fit_cell("", w_nic),
                &fit_cell("", w_net),
                &fit_cell("", w_mac),
                &fit_cell(ip.as_str(), w_ip),
            );
            out.push(Line::from(vec![table_val(cont)]));
        }
    }
    out
}

fn disk_col_widths(total: usize) -> (usize, usize, usize, usize, usize) {
    // Capacity column (compact mem format); narrowed vs older layout (-4 chars vs 10).
    const CAP: usize = 6;
    const THIN: usize = 5;
    // Minimum mode width so `Independent` / `Dependent` are not clipped (+5 vs old default).
    const MODE_MIN: usize = 13;
    // VMDK column narrowed vs proportional split (-15 chars).
    const VMDK_TRIM: usize = 15;
    let gaps = 4 * TABLE_COL_GAP;
    let budget = total.saturating_sub(gaps);
    if budget < CAP + THIN + MODE_MIN + 16 {
        let r = budget.saturating_sub(CAP + THIN);
        let a = (r.saturating_sub(MODE_MIN)) / 2;
        let m = r.saturating_sub(a + a).max(MODE_MIN);
        return (a.max(4), a.max(4), CAP, THIN, m.max(2));
    }
    let flex = budget.saturating_sub(CAP + THIN + MODE_MIN);
    let mut vmdk = (flex * 56) / 100;
    vmdk = vmdk.saturating_sub(VMDK_TRIM).max(12);
    let ds = flex.saturating_sub(vmdk).max(8);
    let mode = budget.saturating_sub(vmdk + ds + CAP + THIN);
    (vmdk, ds, CAP, THIN, mode.max(MODE_MIN))
}

fn thin_cell(t: Option<bool>) -> &'static str {
    match t {
        Some(true) => "thin ",
        Some(false) => "thick",
        None => "  -  ",
    }
}

fn format_disk_table(rows: &[VmDiskRow], total_width: usize) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return vec![Line::from("(no disks)")];
    }
    let (w_v, w_ds, w_cap, w_thin, w_mode) = disk_col_widths(total_width.max(60));
    let hdr = join_table_5(
        &fit_cell("VMDK", w_v),
        &fit_cell("Datastore", w_ds),
        &fit_cell("Cap", w_cap),
        &fit_cell("Thin", w_thin),
        &fit_cell("Mode", w_mode),
    );
    let mut out = vec![Line::from(vec![table_hdr(hdr)])];
    for r in rows {
        let cap = format_capacity(r.capacity_bytes);
        let thin_s = thin_cell(r.thin);
        let row = join_table_5(
            &fit_cell(&truncate(&r.vmdk_file, w_v), w_v),
            &fit_cell(&truncate(&r.datastore, w_ds), w_ds),
            &fit_cell(&cap, w_cap),
            &fit_cell(thin_s, w_thin),
            &fit_cell(&r.mode, w_mode),
        );
        out.push(Line::from(vec![table_val(row)]));
    }
    out
}

fn format_capacity(bytes: u64) -> String {
    format_compact_mem_bytes(bytes as i128)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let t: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{t}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn summary_popup_rect_fits_narrow_terminal() {
        let frame = Rect::new(0, 0, 20, 35);
        let popup = summary_popup_rect(frame);
        assert_eq!(popup.width, 16);
        assert_eq!(popup.x, 2);
        assert!(popup.x + popup.width <= frame.x + frame.width);
        assert!(popup.y + popup.height <= frame.y + frame.height);
    }

    #[test]
    fn render_loading_fits_narrow_terminal() {
        let mut ui = VmSummaryUi::default();
        ui.start_loading(1);
        let mut term = Terminal::new(TestBackend::new(20, 35)).unwrap();
        term.draw(|f| ui.render(f)).unwrap();
    }
}

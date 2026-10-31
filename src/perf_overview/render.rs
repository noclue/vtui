//! Draw the performance overview into the body rect.

use std::time::SystemTime;

use chrono::{DateTime, Utc};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::symbols::Marker;
use ratatui::text::Line;
use ratatui::widgets::{
    Axis, Block, BorderType, Chart, Dataset, GraphType, LegendPosition, Paragraph, Sparkline,
};

use crate::perf_overview::layout::{Domain, SlotKind, perf_overview_layout};
use crate::perf_overview::store::{PerfOverviewStore, SamplePoint, SeriesId, sum_pair};
use crate::perf_overview::ui::PerfOverview;
use crate::resource_browser::formatting::{
    bucket_series, format_binary_bytes, format_cpu_mhz, format_percent, format_shared_binary_pair,
    sparkline_line,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ChartRun {
    pub name: &'static str,
    pub color: Color,
    pub points: Vec<(f64, f64)>,
}

pub fn legend_enabled(panel_width: u16) -> bool {
    panel_width >= 50
}

/// CPU stays 0–100 until a sample exceeds 100 (multicore). Then it uses the same headroom as the other domains.
pub fn cpu_scale_max(peak: f64) -> f64 {
    if peak <= 100.0 { 100.0 } else { peak * 1.1 }
}

pub fn y_bounds(domain: Domain, peak: f64) -> (f64, f64) {
    if domain == Domain::Cpu {
        (0.0, cpu_scale_max(peak))
    } else if peak <= 0.0 {
        (0.0, 1.0)
    } else {
        (0.0, peak * 1.1)
    }
}

/// Realtime overview window. Ninety samples, twenty seconds apart.
pub const OVERVIEW_WINDOW_SECS: f64 = 30.0 * 60.0;

pub fn overview_x_axis() -> ([f64; 2], Vec<String>) {
    ([-OVERVIEW_WINDOW_SECS, 0.0], x_labels(OVERVIEW_WINDOW_SECS))
}

pub fn relative_duration_label(seconds: f64) -> String {
    let secs = seconds.abs().round() as i64;
    if secs <= 0 {
        "0".to_string()
    } else if secs % 3600 == 0 {
        format!("{}h", secs / 3600)
    } else if secs % 60 == 0 {
        format!("{}m", secs / 60)
    } else {
        format!("{secs}s")
    }
}

/// Split a series on gaps. Each run is one line dataset.
pub fn contiguous_runs(
    xs: &[f64],
    ys: &[Option<f64>],
    name: &'static str,
    color: Color,
) -> Vec<ChartRun> {
    let mut runs = Vec::new();
    let mut current = Vec::new();
    for (x, y) in xs.iter().zip(ys.iter()) {
        match y {
            Some(value) => current.push((*x, *value)),
            None if !current.is_empty() => runs.push(ChartRun {
                name,
                color,
                points: std::mem::take(&mut current),
            }),
            None => {}
        }
    }
    if !current.is_empty() {
        runs.push(ChartRun {
            name,
            color,
            points: current,
        });
    }
    if runs.len() > 1 {
        for run in runs.iter_mut().skip(1) {
            run.name = "";
        }
    }
    runs
}

pub fn chart_runs_for_domain(
    domain: Domain,
    store: &PerfOverviewStore,
    newest: Option<SystemTime>,
    inner_width: usize,
) -> Vec<ChartRun> {
    if !store.domain_present(domain) {
        return Vec::new();
    }
    match domain {
        Domain::Cpu => runs_one(
            store.series(SeriesId::CpuUsage),
            newest,
            inner_width,
            "CPU",
            Color::Cyan,
        ),
        Domain::Memory => runs_one(
            store.series(SeriesId::MemConsumed),
            newest,
            inner_width,
            "Memory",
            Color::Green,
        ),
        Domain::Network => runs_pair(
            store.series(SeriesId::NetRx),
            store.series(SeriesId::NetTx),
            newest,
            inner_width,
            "RX",
            Color::Blue,
            "TX",
            Color::Magenta,
        ),
        Domain::Disk => runs_pair(
            store.series(SeriesId::DiskRead),
            store.series(SeriesId::DiskWrite),
            newest,
            inner_width,
            "Read",
            Color::Yellow,
            "Write",
            Color::Red,
        ),
    }
}

fn runs_one(
    points: Option<&[SamplePoint]>,
    newest: Option<SystemTime>,
    inner_width: usize,
    name: &'static str,
    color: Color,
) -> Vec<ChartRun> {
    let Some(points) = points else {
        return Vec::new();
    };
    let (xs, ys) = bucket_timed(points, newest, inner_width, false);
    contiguous_runs(&xs, &ys, name, color)
}

#[allow(clippy::too_many_arguments)]
fn runs_pair(
    left: Option<&[SamplePoint]>,
    right: Option<&[SamplePoint]>,
    newest: Option<SystemTime>,
    inner_width: usize,
    left_name: &'static str,
    left_color: Color,
    right_name: &'static str,
    right_color: Color,
) -> Vec<ChartRun> {
    let mut runs = Vec::new();
    extend_side(
        &mut runs,
        left,
        right,
        newest,
        inner_width,
        left_name,
        left_color,
    );
    extend_side(
        &mut runs,
        right,
        left,
        newest,
        inner_width,
        right_name,
        right_color,
    );
    runs
}

/// Plot `side`. When it is missing and the other side exists, plot zeros on the other side's timestamps.
fn extend_side(
    runs: &mut Vec<ChartRun>,
    side: Option<&[SamplePoint]>,
    other: Option<&[SamplePoint]>,
    newest: Option<SystemTime>,
    inner_width: usize,
    name: &'static str,
    color: Color,
) {
    let (points, zero_fill) = match (side, other) {
        (Some(points), _) => (points, false),
        (None, Some(points)) => (points, true),
        (None, None) => return,
    };
    let (xs, ys) = bucket_timed(points, newest, inner_width, zero_fill);
    runs.extend(contiguous_runs(&xs, &ys, name, color));
}

fn bucket_timed(
    points: &[SamplePoint],
    newest: Option<SystemTime>,
    width: usize,
    zero_fill: bool,
) -> (Vec<f64>, Vec<Option<f64>>) {
    let values: Vec<Option<f64>> = points
        .iter()
        .map(|point| {
            if zero_fill {
                point.value.map(|_| 0.0)
            } else {
                point.value
            }
        })
        .collect();
    let bucketed = bucket_series(&values, width.max(1));
    let n = points.len();
    let width = bucketed.len();
    let newest = newest.or_else(|| points.last().map(|point| point.ts));
    let mut xs = Vec::with_capacity(width);
    let mut ys = Vec::with_capacity(width);
    for (col, value) in bucketed.into_iter().enumerate() {
        let index = if n == 0 || width == 0 {
            0
        } else if width >= n {
            (col * n) / width
        } else {
            (((col + 1) * n) / width).saturating_sub(1)
        };
        let ts = points
            .get(index)
            .map(|point| point.ts)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        xs.push(seconds_before(newest, ts));
        ys.push(value);
    }
    (xs, ys)
}

fn seconds_before(newest: Option<SystemTime>, ts: SystemTime) -> f64 {
    let Some(newest) = newest else {
        return 0.0;
    };
    match newest.duration_since(ts) {
        Ok(elapsed) => -(elapsed.as_secs_f64()),
        Err(err) => err.duration().as_secs_f64(),
    }
}

pub fn x_labels(span_secs: f64) -> Vec<String> {
    let span = span_secs.abs();
    vec![
        relative_duration_label(span),
        relative_duration_label(span / 2.0),
        "0".to_string(),
    ]
}

pub fn draw(frame: &mut Frame, area: Rect, overview: &PerfOverview) {
    let layout = perf_overview_layout(area.width, area.height, overview.focus);
    let store = overview.store.read().ok();
    let store = store.as_deref();
    if let Some(title) = layout.title {
        let line = title_line(store);
        frame.render_widget(Paragraph::new(line), place(area, title));
    }
    if let Some(combined) = layout.combined {
        let line = clip_ellipsis(&combined_line(store), combined.width as usize);
        frame.render_widget(Paragraph::new(line), place(area, combined));
    }
    for slot in &layout.slots {
        let rect = place(area, slot.rect);
        let badge = badge_text(slot.domain, store);
        match slot.kind {
            SlotKind::Badge => {
                frame.render_widget(Paragraph::new(badge).style(domain_style(slot.domain)), rect);
            }
            SlotKind::Sparkline | SlotKind::BadgeWithSparkline => {
                draw_spark(frame, rect, slot.kind, slot.domain, &badge, store);
            }
            SlotKind::Chart => draw_chart(frame, rect, slot.domain, &badge, store),
        }
    }
}

fn draw_spark(
    frame: &mut Frame,
    rect: Rect,
    kind: SlotKind,
    domain: Domain,
    badge: &str,
    store: Option<&PerfOverviewStore>,
) {
    if kind == SlotKind::BadgeWithSparkline || rect.height == 1 {
        let badge_h = 1.min(rect.height);
        frame.render_widget(
            Paragraph::new(badge.to_string()).style(domain_style(domain)),
            Rect {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: badge_h,
            },
        );
        if kind == SlotKind::BadgeWithSparkline && rect.height > 1 {
            let spark = Rect {
                x: rect.x,
                y: rect.y + 1,
                width: rect.width,
                height: rect.height - 1,
            };
            paint_spark(frame, spark, domain, store);
        }
        return;
    }
    paint_spark(frame, rect, domain, store);
}

fn paint_spark(frame: &mut Frame, rect: Rect, domain: Domain, store: Option<&PerfOverviewStore>) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let values = spark_values(domain, store);
    let bucketed = bucket_series(&values, rect.width as usize);
    if rect.height == 1 {
        frame.render_widget(
            Paragraph::new(sparkline_line(&bucketed)).style(domain_style(domain)),
            rect,
        );
        return;
    }
    let (data, max) = spark_u64(&bucketed, domain);
    let widget = Sparkline::default()
        .data(data)
        .max(max)
        .absent_value_symbol(" ")
        .style(domain_style(domain));
    frame.render_widget(widget, rect);
}

fn draw_chart(
    frame: &mut Frame,
    rect: Rect,
    domain: Domain,
    badge: &str,
    store: Option<&PerfOverviewStore>,
) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(domain_style(domain))
        .title(Line::from(badge.to_string()).style(domain_style(domain)));
    let Some(store) = store else {
        frame.render_widget(block, rect);
        return;
    };
    let inner_width = rect.width.saturating_sub(2).max(1) as usize;
    let runs = chart_runs_for_domain(domain, store, store.sample_time(), inner_width);
    let peak = runs
        .iter()
        .flat_map(|run| run.points.iter().map(|(_, y)| *y))
        .fold(0.0_f64, f64::max);
    let (y_min, y_max) = y_bounds(domain, peak);
    let (x_bounds, labels) = overview_x_axis();
    // A line is drawn between points. One returned sample, or one sample between gaps, needs a
    // neighbor or the stroke is skipped.
    let stretched: Vec<Option<Vec<(f64, f64)>>> = runs
        .iter()
        .map(|run| stretch_lone_sample(&run.points))
        .collect();
    let datasets: Vec<Dataset> = runs
        .iter()
        .zip(stretched.iter())
        .map(|(run, extra)| {
            let data = extra.as_deref().unwrap_or(run.points.as_slice());
            let mut dataset = Dataset::default()
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(run.color))
                .data(data);
            if !run.name.is_empty() {
                dataset = dataset.name(run.name);
            }
            dataset
        })
        .collect();
    let mut chart = Chart::new(datasets)
        .block(block)
        .x_axis(
            Axis::default()
                .bounds(x_bounds)
                .labels(labels.iter().map(String::as_str))
                // Right keeps the first label from reserving a margin left of the plot.
                .labels_alignment(Alignment::Right)
                .style(Style::default().fg(Color::DarkGray)),
        )
        .y_axis(
            Axis::default()
                .bounds([y_min, y_max])
                .style(Style::default().fg(Color::DarkGray)),
        );
    if !legend_enabled(rect.width) {
        chart = chart.legend_position(None);
    } else {
        chart = chart.legend_position(Some(LegendPosition::TopLeft));
    }
    frame.render_widget(chart, rect);
    // Ratatui right-aligns the first label onto the left border and centers the middle one.
    // Place each label so its first column sits on that sample instead.
    paint_x_labels(frame, rect, &labels);
}

/// One sample cannot form a line. Extend it one realtime interval toward the past.
pub fn stretch_lone_sample(points: &[(f64, f64)]) -> Option<Vec<(f64, f64)>> {
    let [(x, y)] = *points else {
        return None;
    };
    let earlier = (x - 20.0).max(-OVERVIEW_WINDOW_SECS);
    (earlier < x).then(|| vec![(earlier, y), (x, y)])
}

fn paint_x_labels(frame: &mut Frame, rect: Rect, labels: &[String]) {
    if labels.len() < 2 || rect.width < 4 || rect.height < 4 {
        return;
    }
    let y = rect.y + rect.height - 2;
    let left = rect.x + 1;
    let inner_width = rect.width.saturating_sub(2);
    let style = Style::default().fg(Color::DarkGray);
    let buf = frame.buffer_mut();
    for x in left..left + inner_width {
        buf[(x, y)].set_symbol(" ").set_style(style);
    }
    let last = (labels.len() - 1) as u16;
    let span = inner_width.saturating_sub(1);
    for (index, label) in labels.iter().enumerate() {
        let mut x = left + (index as u16 * span) / last;
        let width = label.chars().count() as u16;
        if x + width > left + inner_width {
            x = (left + inner_width).saturating_sub(width);
        }
        buf.set_string(x, y, label, style);
    }
    let border = domain_border_style(buf, rect);
    buf[(rect.x, y)].set_symbol("│").set_style(border);
    buf[(rect.x + rect.width - 1, y)]
        .set_symbol("│")
        .set_style(border);
}

fn domain_border_style(buf: &ratatui::buffer::Buffer, rect: Rect) -> Style {
    buf[(rect.x, rect.y.saturating_add(1))].style()
}

fn spark_values(domain: Domain, store: Option<&PerfOverviewStore>) -> Vec<Option<f64>> {
    let Some(store) = store else {
        return Vec::new();
    };
    match domain {
        Domain::Cpu => values_of(store.series(SeriesId::CpuUsage)),
        Domain::Memory => values_of(store.series(SeriesId::MemConsumed)),
        Domain::Network => sum_series(store.series(SeriesId::NetRx), store.series(SeriesId::NetTx)),
        Domain::Disk => sum_series(
            store.series(SeriesId::DiskRead),
            store.series(SeriesId::DiskWrite),
        ),
    }
}

fn values_of(points: Option<&[SamplePoint]>) -> Vec<Option<f64>> {
    points
        .unwrap_or(&[])
        .iter()
        .map(|point| point.value)
        .collect()
}

fn sum_series(left: Option<&[SamplePoint]>, right: Option<&[SamplePoint]>) -> Vec<Option<f64>> {
    match (left, right) {
        (None, None) => Vec::new(),
        (Some(points), None) | (None, Some(points)) => {
            points.iter().map(|point| point.value).collect()
        }
        (Some(left), Some(right)) => {
            let len = left.len().max(right.len());
            (0..len)
                .map(|index| {
                    sum_pair(
                        left.get(index).and_then(|point| point.value),
                        right.get(index).and_then(|point| point.value),
                    )
                })
                .collect()
        }
    }
}

fn spark_u64(values: &[Option<f64>], domain: Domain) -> (Vec<Option<u64>>, u64) {
    let data = values
        .iter()
        .map(|value| value.map(|number| number.max(0.0).round() as u64))
        .collect();
    let peak = values.iter().copied().flatten().fold(0.0_f64, f64::max);
    let max = if domain == Domain::Cpu {
        cpu_scale_max(peak).round() as u64
    } else {
        peak.max(1.0).round() as u64
    };
    (data, max.max(1))
}

pub fn title_line(store: Option<&PerfOverviewStore>) -> String {
    let (kind, name) = store
        .map(|store| (store.kind_label, store.name.as_str()))
        .unwrap_or(("VM", ""));
    let sample = match store {
        Some(store) if store.error().is_some() => format!("ERR {}", store.error().unwrap_or("")),
        Some(store) => match store.sample_time() {
            Some(time) => format!("sample {}", format_sample_time(time)),
            None => "sample —".to_string(),
        },
        None => "sample —".to_string(),
    };
    format!("{kind} {name} · realtime 20s · 30m · {sample}")
}

fn format_sample_time(time: SystemTime) -> String {
    DateTime::<Utc>::from(time).format("%H:%M:%SZ").to_string()
}

pub fn badge_text(domain: Domain, store: Option<&PerfOverviewStore>) -> String {
    let Some(store) = store else {
        return "No samples".to_string();
    };
    if !store.domain_present(domain) {
        return "No samples".to_string();
    }
    match domain {
        Domain::Cpu => {
            let pct = store.latest(SeriesId::CpuUsage).map(format_percent);
            let mhz = store.latest(SeriesId::CpuMhz).map(format_cpu_mhz);
            match (pct, mhz) {
                (Some(pct), Some(mhz)) => format!("CPU {pct} {mhz}"),
                (Some(pct), None) => format!("CPU {pct}"),
                (None, Some(mhz)) => format!("CPU {mhz}"),
                (None, None) => "CPU —".to_string(),
            }
        }
        Domain::Memory => {
            let bytes = store
                .latest(SeriesId::MemConsumed)
                .map(|value| format_binary_bytes(value.round() as i128));
            let pct = store.latest(SeriesId::MemUsage).map(format_percent);
            match (bytes, pct) {
                (Some(bytes), Some(pct)) => format!("MEM {bytes} {pct}"),
                (Some(bytes), None) => format!("MEM {bytes}"),
                (None, Some(pct)) => format!("MEM {pct}"),
                (None, None) => "MEM —".to_string(),
            }
        }
        Domain::Network => format!(
            "NET {}",
            format_shared_binary_pair(
                "↓",
                "↑",
                store.latest(SeriesId::NetRx).unwrap_or(0.0),
                store.latest(SeriesId::NetTx).unwrap_or(0.0),
            )
        ),
        Domain::Disk => format!(
            "DISK {}",
            format_shared_binary_pair(
                "R",
                "W",
                store.latest(SeriesId::DiskRead).unwrap_or(0.0),
                store.latest(SeriesId::DiskWrite).unwrap_or(0.0),
            )
        ),
    }
}

fn combined_line(store: Option<&PerfOverviewStore>) -> String {
    Domain::ALL
        .into_iter()
        .map(|domain| badge_text(domain, store))
        .collect::<Vec<_>>()
        .join("  ")
}

fn clip_ellipsis(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if count <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let mut out: String = text.chars().take(width - 1).collect();
    out.push('…');
    out
}

fn domain_style(domain: Domain) -> Style {
    let color = match domain {
        Domain::Cpu => Color::Cyan,
        Domain::Memory => Color::Green,
        Domain::Network => Color::Blue,
        Domain::Disk => Color::Yellow,
    };
    Style::default().fg(color)
}

fn place(area: Rect, local: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(local.x),
        y: area.y.saturating_add(local.y),
        width: local.width,
        height: local.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perf_overview::store::{
        PerfOverviewStore, SamplePoint, SeriesId, StoreUpdate, new_overview_store,
    };
    use insta::assert_snapshot;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::{Constraint, Layout};
    use std::collections::BTreeMap;
    use std::time::Duration;

    fn ts(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn series(start: u64, values: &[Option<f64>]) -> Vec<SamplePoint> {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| SamplePoint {
                ts: ts(start + index as u64 * 20),
                value: *value,
            })
            .collect()
    }

    fn demo_store() -> PerfOverviewStore {
        let mut samples = BTreeMap::new();
        let cpu: Vec<Option<f64>> = (0..12).map(|i| Some(10.0 + (i as f64) * 2.0)).collect();
        samples.insert(SeriesId::CpuUsage, series(1_700_000_000, &cpu));
        samples.insert(SeriesId::CpuMhz, series(1_700_000_000, &[Some(480.0); 12]));
        let mem: Vec<Option<f64>> = (0..12)
            .map(|i| Some(1.4 * 1024.0 * 1024.0 * 1024.0 + i as f64))
            .collect();
        samples.insert(SeriesId::MemConsumed, series(1_700_000_000, &mem));
        samples.insert(SeriesId::MemUsage, series(1_700_000_000, &[Some(35.0); 12]));
        let rx: Vec<Option<f64>> = (0..12)
            .map(|i| {
                if i == 4 {
                    None
                } else {
                    Some((2.1 + i as f64 * 0.1) * 1024.0 * 1024.0)
                }
            })
            .collect();
        samples.insert(SeriesId::NetRx, series(1_700_000_000, &rx));
        samples.insert(
            SeriesId::NetTx,
            series(1_700_000_000, &[Some(0.4 * 1024.0 * 1024.0); 12]),
        );
        samples.insert(
            SeriesId::DiskRead,
            series(1_700_000_000, &[Some(1.2 * 1024.0 * 1024.0); 12]),
        );
        samples.insert(
            SeriesId::DiskWrite,
            series(1_700_000_000, &[Some(0.3 * 1024.0 * 1024.0); 12]),
        );
        let mut store = PerfOverviewStore::empty("web-1".into(), "VM");
        store.apply(StoreUpdate::Replace, samples);
        store
    }

    fn render_terminal(width: u16, height: u16, header_on: bool) -> String {
        let store = new_overview_store("web-1".into(), "VM");
        {
            let mut guard = store.write().unwrap();
            *guard = demo_store();
        }
        let overview = PerfOverview::new(
            vim_rs::types::structs::ManagedObjectReference {
                r#type: vim_rs::types::enums::MoTypesEnum::VirtualMachine,
                value: "vm-1".into(),
            },
            store,
        );
        let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
        term.draw(|frame| {
            let header = if header_on { 5 } else { 0 };
            let [top, body] = Layout::vertical([Constraint::Length(header), Constraint::Fill(1)])
                .areas(frame.area());
            if header_on {
                frame.render_widget(Paragraph::new("header"), top);
            }
            overview.render(frame, body);
        })
        .unwrap();
        format!("{}", term.backend())
    }

    #[test]
    fn snapshots_cover_header_on_and_chart_sizes() {
        assert_snapshot!("overview_32x12_header", render_terminal(32, 12, true));
        assert_snapshot!("overview_80x24_header", render_terminal(80, 24, true));
        assert_snapshot!("overview_135x42_header", render_terminal(135, 42, true));
        assert_snapshot!("overview_40x15_no_header", render_terminal(40, 15, false));
    }

    #[test]
    fn title_uses_sample_time_or_err_or_dash() {
        let mut store = demo_store();
        let line = title_line(Some(&store));
        assert!(line.contains("VM web-1"));
        assert!(line.contains("realtime 20s"));
        assert!(line.contains("30m"));
        assert!(line.contains("sample "));
        assert!(!line.contains("sample —"));

        store.apply(
            StoreUpdate::Keep {
                message: "timeout".into(),
            },
            BTreeMap::new(),
        );
        let err = title_line(Some(&store));
        assert!(err.contains("ERR timeout"));

        store.apply(StoreUpdate::Clear, BTreeMap::new());
        assert!(title_line(Some(&store)).contains("sample —"));
    }

    #[test]
    fn short_body_has_no_title_row_in_the_layout_draw() {
        let line = title_line(None);
        assert!(line.contains("sample —"));
        let clipped = clip_ellipsis("CPU 12%  MEM lots of text", 8);
        assert!(clipped.ends_with('…'));
        assert_eq!(clipped.chars().count(), 8);
    }

    #[test]
    fn network_and_disk_are_two_series_and_gaps_split_lines() {
        let store = demo_store();
        let net = chart_runs_for_domain(Domain::Network, &store, store.sample_time(), 12);
        let names: Vec<_> = net
            .iter()
            .map(|run| run.name)
            .filter(|name| !name.is_empty())
            .collect();
        assert!(names.contains(&"RX"));
        assert!(names.contains(&"TX"));
        assert!(net.len() >= 3, "the RX gap should split that series");

        let disk = chart_runs_for_domain(Domain::Disk, &store, store.sample_time(), 12);
        let disk_names: Vec<_> = disk.iter().map(|run| run.name).collect();
        assert!(disk_names.contains(&"Read"));
        assert!(disk_names.contains(&"Write"));

        let split = contiguous_runs(
            &[0.0, 1.0, 2.0],
            &[Some(1.0), None, Some(3.0)],
            "RX",
            Color::Blue,
        );
        assert_eq!(split.len(), 2);
        assert_eq!(split[0].points, vec![(0.0, 1.0)]);
        assert_eq!(split[1].points, vec![(2.0, 3.0)]);
    }

    #[test]
    fn legend_and_y_bounds_follow_the_contract() {
        assert!(!legend_enabled(49));
        assert!(legend_enabled(50));
        assert_eq!(y_bounds(Domain::Cpu, 12.0), (0.0, 100.0));
        assert_eq!(y_bounds(Domain::Cpu, 100.0), (0.0, 100.0));
        let (cpu_min, cpu_max) = y_bounds(Domain::Cpu, 118.0);
        assert_eq!(cpu_min, 0.0);
        assert!((cpu_max - 129.8).abs() < 0.001);
        let (min, max) = y_bounds(Domain::Memory, 50.0);
        assert_eq!(min, 0.0);
        assert!((max - 55.0).abs() < 0.001);
        assert_eq!(y_bounds(Domain::Disk, 0.0), (0.0, 1.0));
        let (bounds, labels) = overview_x_axis();
        assert_eq!(bounds, [-OVERVIEW_WINDOW_SECS, 0.0]);
        assert_eq!(
            labels,
            vec!["30m".to_string(), "15m".to_string(), "0".to_string()]
        );
        assert_ne!(x_labels(1780.0), labels);
    }

    #[test]
    fn a_lone_sample_becomes_a_line_segment() {
        assert_eq!(
            stretch_lone_sample(&[(0.0, 12.0)]).as_deref(),
            Some([(-20.0, 12.0), (0.0, 12.0)].as_slice())
        );
        assert!(stretch_lone_sample(&[(0.0, 1.0), (-20.0, 2.0)]).is_none());
    }

    #[test]
    fn missing_domain_says_no_samples_and_badges_match_examples() {
        let store = demo_store();
        assert_eq!(badge_text(Domain::Cpu, Some(&store)), "CPU 32% 480MHz");
        assert!(badge_text(Domain::Memory, Some(&store)).starts_with("MEM "));
        assert!(badge_text(Domain::Memory, Some(&store)).contains('%'));
        assert!(badge_text(Domain::Network, Some(&store)).starts_with("NET ↓"));
        assert!(badge_text(Domain::Disk, Some(&store)).contains("MiB/s"));

        let mut bare = demo_store();
        bare.apply(StoreUpdate::Clear, BTreeMap::new());
        assert_eq!(badge_text(Domain::Disk, Some(&bare)), "No samples");
    }
}

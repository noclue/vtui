use ratatui::style::{Color, Style};
use ratatui::widgets::Cell;
use vim_rs::types::enums::ManagedEntityStatusEnum;

pub(crate) const ID_COLUMN_WIDTH: u16 = 18;
pub(crate) const STATUS_COLUMN_WIDTH: u16 = 2;

pub(crate) const STATUS: &str = "● ";

pub(crate) fn status_color(status: &ManagedEntityStatusEnum) -> Style {
    match status {
        ManagedEntityStatusEnum::Green => Style::new().fg(Color::Green),
        ManagedEntityStatusEnum::Yellow => Style::new().fg(Color::Yellow),
        ManagedEntityStatusEnum::Red => Style::new().fg(Color::Red),
        ManagedEntityStatusEnum::Gray => Style::new().fg(Color::Gray),
        _ => Style::default(),
    }
}

pub fn format_compact_memory_size(mb: i64) -> String {
    let bytes = mb * 1024 * 1024;
    format_compact_metric(bytes as f64)
}

/// Compact human-readable metric: exactly **4 characters** — up to 3 numeric chars + scale letter
/// `P|T|G|M|K`, or a trailing space when value &lt; 1000.
///
/// For scaled value &lt; 10 (after choosing `K`/`M`/…): one digit, `.`, one digit, then unit (e.g. `1.5M`).
/// For ≥ 10: up to three digits + unit (e.g. ` 42G`, `999M`).
pub fn format_compact_metric(value: f64) -> String {
    if !value.is_finite() || value < 0.0 {
        return "   -".to_string();
    }
    if value == 0.0 {
        return "   0".to_string();
    }

    const SCALES: [(f64, char); 5] = [(1e15, 'P'), (1e12, 'T'), (1e9, 'G'), (1e6, 'M'), (1e3, 'K')];

    let mut div = 1.0_f64;
    let mut unit = ' ';
    for (d, u) in SCALES {
        if value >= d {
            div = d;
            unit = u;
            break;
        }
    }

    let s = value / div;
    if unit == ' ' {
        let n = s.min(999.0).round() as i64;
        format!("{:>3} ", n)
    } else if s < 10.0 {
        format!("{:>3.1}{}", s, unit)
    } else {
        format!("{:>3.0}{}", s.min(999.0), unit)
    }
}

/// Sparkline (▁–▇) from raw `query_perf` integer samples. Values are vSphere **hundredths of a
/// percent** (0 = 0%, 10000 = 100%). Uses **absolute** 0–10000 scaling so the bars reflect real
/// utilization against total capacity.
///
/// `None` slots render as ▁ (empty).
pub fn sparkline_from_perf_samples(slots: &[Option<i64>]) -> String {
    const BARS: [char; 7] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇'];
    const MAX_VAL: i64 = 10_000;
    let mut out = String::with_capacity(slots.len());
    for opt in slots {
        let idx = match opt {
            None => 0usize,
            Some(v) => {
                let clamped = v.clamp(&0, &MAX_VAL);
                // Map 0..10000 → 0..6 with rounding
                let t = (clamped * 6 + MAX_VAL / 2) / MAX_VAL;
                (t as usize).min(6)
            }
        };
        out.push(BARS[idx]);
    }
    out
}

pub fn format_byte_size(bytes: i64) -> Cell<'static> {
    let bytes_f64 = bytes as f64;

    if bytes_f64 < 0.0 {
        return Cell::from("N/A");
    }

    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;
    const PIB: f64 = TIB * 1024.0;

    let (size, unit) = if bytes_f64 >= PIB {
        (bytes_f64 / PIB, "PiB")
    } else if bytes_f64 >= TIB {
        (bytes_f64 / TIB, "TiB")
    } else if bytes_f64 >= GIB {
        (bytes_f64 / GIB, "GiB")
    } else if bytes_f64 >= MIB {
        (bytes_f64 / MIB, "MiB")
    } else if bytes_f64 >= KIB {
        (bytes_f64 / KIB, "KiB")
    } else {
        (bytes_f64, "B")
    };

    Cell::from(format!("{:.2} {}", size, unit))
}

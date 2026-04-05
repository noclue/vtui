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

/// Numeric part is **3 characters** (right-aligned) + unit: values **≥ 10** use a whole number
/// (`10G`); **< 10** use `d.d` (`3.4G`). Avoids `10.0G`.
fn format_compact_scaled(s: f64, unit: char) -> String {
    if !s.is_finite() || s < 0.0 {
        return format!("   {}", unit);
    }
    if s == 0.0 {
        return format!("  0{}", unit);
    }
    let capped = s.min(999.0);
    if capped >= 10.0 {
        let whole = capped.round().clamp(10.0, 999.0) as i64;
        return format!("{:>3}{}", whole, unit);
    }
    let x = (capped * 10.0).round() as i64;
    if x >= 100 {
        format!("{:>3}{}", (x / 10).min(999), unit)
    } else {
        let int_part = x / 10;
        let frac = x % 10;
        format!("{:>3}{}", format!("{}.{}", int_part, frac), unit)
    }
}

/// `cpu.usagemhz` values are already **MHz**. A generic decimal compact formatter would show `428 `
/// (no unit); this renders **`428M`** (MHz) or **`2.5G`** / **`10G`** (GHz) in four characters.
pub fn format_compact_mhz(mhz: i64) -> String {
    if mhz < 0 {
        return "   -".to_string();
    }
    if mhz == 0 {
        return "   0".to_string();
    }
    if mhz < 1000 {
        return format!("{:>3}M", mhz.min(999));
    }
    let g = mhz as f64 / 1000.0;
    format_compact_scaled(g, 'G')
}

/// Memory size from **bytes** using **1024-based** steps (`K`/`M`/`G`/…) so VM RAM matches GiB-style
/// expectations (e.g. ~1.4 GiB → `1.4G`). Decimal 10³ scaling is wrong for this column.
pub fn format_compact_mem_bytes(bytes: i128) -> String {
    if bytes < 0 {
        return "   -".to_string();
    }
    if bytes == 0 {
        return "   0".to_string();
    }
    let v = bytes as f64;
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;
    const PIB: f64 = TIB * 1024.0;

    let (div, unit) = if v >= PIB {
        (PIB, 'P')
    } else if v >= TIB {
        (TIB, 'T')
    } else if v >= GIB {
        (GIB, 'G')
    } else if v >= MIB {
        (MIB, 'M')
    } else if v >= KIB {
        (KIB, 'K')
    } else {
        let n = (v.min(999.0)).round() as i64;
        return format!("{:>3} ", n);
    };

    let s = v / div;
    format_compact_scaled(s, unit)
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

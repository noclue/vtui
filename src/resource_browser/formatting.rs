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
/// Suitable after converting e.g. Mb/s → bps (`mbps * 1_000_000`).
pub fn format_compact_bitrate_bps(bps: u64) -> String {
    if bps == 0 {
        return "   0".to_string();
    }
    const K: u64 = 1_000;
    const M: u64 = K * 1_000;
    const G: u64 = M * 1_000;
    const T: u64 = G * 1_000;
    const P: u64 = T * 1_000;

    let (div, unit) = if bps >= P {
        (P as f64, 'P')
    } else if bps >= T {
        (T as f64, 'T')
    } else if bps >= G {
        (G as f64, 'G')
    } else if bps >= M {
        (M as f64, 'M')
    } else if bps >= K {
        (K as f64, 'K')
    } else {
        let n = (bps.min(999)) as i64;
        return format!("{:>3} ", n);
    };
    let s = bps as f64 / div;
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

const BINARY_UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];

fn binary_unit(abs: f64) -> (f64, &'static str) {
    const KIB: f64 = 1024.0;
    let mut divisor = KIB;
    let mut index = 0usize;
    while abs >= divisor * 1024.0 && index < BINARY_UNITS.len() - 1 {
        divisor *= 1024.0;
        index += 1;
    }
    (divisor, BINARY_UNITS[index])
}

fn format_magnitude(value: f64) -> String {
    let value = value.abs();
    if !value.is_finite() {
        return "0".to_string();
    }
    if value >= 100.0 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.1}")
    }
}

/// Binary size for overview badges. Smallest unit is KiB. No space before the unit.
pub fn format_binary_bytes(bytes: i128) -> String {
    let (divisor, unit) = binary_unit((bytes as f64).abs());
    format!("{}{unit}", format_magnitude(bytes as f64 / divisor))
}

/// Binary rate for overview badges. Smallest unit is KiB/s.
///
/// Pair badges use [`format_shared_binary_pair`], so the binary target has no caller.
#[allow(dead_code)]
pub fn format_binary_rate(bytes_per_sec: f64) -> String {
    let (divisor, unit) = binary_unit(bytes_per_sec.abs());
    format!("{}{unit}/s", format_magnitude(bytes_per_sec / divisor))
}

/// Two rates, one unit, chosen from the larger absolute value. No space before the unit.
pub fn format_shared_binary_pair(
    left_mark: &str,
    right_mark: &str,
    left: f64,
    right: f64,
) -> String {
    let scale = left.abs().max(right.abs());
    let (divisor, unit) = binary_unit(scale);
    format!(
        "{left_mark}{} {right_mark}{}{unit}/s",
        format_magnitude(left / divisor),
        format_magnitude(right / divisor)
    )
}

/// Percent for a badge. Whole numbers drop the decimal (`12%`).
pub fn format_percent(percent: f64) -> String {
    if !percent.is_finite() {
        return "0%".to_string();
    }
    if percent >= 100.0 || (percent - percent.round()).abs() < 0.05 {
        format!("{}%", percent.round() as i64)
    } else {
        format!("{percent:.1}%")
    }
}

/// CPU frequency badge. Integer MHz below 10000, otherwise GHz with the decimal rule.
pub fn format_cpu_mhz(mhz: f64) -> String {
    if !mhz.is_finite() {
        return "0MHz".to_string();
    }
    if mhz >= 10_000.0 {
        let ghz = mhz / 1000.0;
        if ghz >= 100.0 {
            format!("{}GHz", ghz.round() as i64)
        } else {
            format!("{ghz:.1}GHz")
        }
    } else {
        format!("{}MHz", mhz.round() as i64)
    }
}

/// Bucket `values` into `width` columns.
///
/// `width >= n` repeats samples with `floor(col * n / width)`. `width < n` is the mean of the
/// `Some` values in the half-open index range, or `None` when that range has no values.
pub fn bucket_series(values: &[Option<f64>], width: usize) -> Vec<Option<f64>> {
    let n = values.len();
    if width == 0 || n == 0 {
        return Vec::new();
    }
    if width >= n {
        (0..width).map(|col| values[(col * n) / width]).collect()
    } else {
        (0..width)
            .map(|col| {
                let start = (col * n) / width;
                let end = ((col + 1) * n) / width;
                let present: Vec<f64> = values[start..end].iter().copied().flatten().collect();
                if present.is_empty() {
                    None
                } else {
                    Some(present.iter().sum::<f64>() / present.len() as f64)
                }
            })
            .collect()
    }
}

const SPARK_BARS: [char; 7] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇'];

/// One-row sparkline. A gap is a space. Zero is the lowest bar.
pub fn sparkline_line(values: &[Option<f64>]) -> String {
    let max = values
        .iter()
        .copied()
        .flatten()
        .fold(0.0_f64, f64::max)
        .max(1.0);
    values
        .iter()
        .map(|value| match value {
            None => ' ',
            Some(value) if *value <= 0.0 => SPARK_BARS[0],
            Some(value) => {
                let scaled = ((value / max) * 6.0).round() as usize;
                SPARK_BARS[scaled.min(6)]
            }
        })
        .collect()
}

#[cfg(test)]
mod compact_format_tests {
    use super::{format_compact_bitrate_bps, format_compact_mem_bytes, format_compact_mhz};

    fn assert_four(label: &str, s: &str) {
        assert_eq!(
            s.chars().count(),
            4,
            "{label}: expected 4 chars, got {:?} len {}",
            s,
            s.chars().count()
        );
    }

    #[test]
    fn compact_columns_are_four_chars() {
        assert_four("mhz", &format_compact_mhz(428));
        assert_four("mhz_zero", &format_compact_mhz(0));
        assert_four("mem", &format_compact_mem_bytes(1024));
        assert_four("bps", &format_compact_bitrate_bps(1_000_000_000));
        assert_four("bps_small", &format_compact_bitrate_bps(500));
        assert_four("bps_100m", &format_compact_bitrate_bps(100 * 1_000_000));
    }
}

#[cfg(test)]
mod overview_format_tests {
    use super::{
        bucket_series, format_binary_bytes, format_binary_rate, format_cpu_mhz, format_percent,
        format_shared_binary_pair, sparkline_line,
    };

    #[test]
    fn binary_sizes_use_1024_and_the_decimal_rule() {
        assert_eq!(format_binary_bytes(0), "0.0KiB");
        assert_eq!(format_binary_bytes(512), "0.5KiB");
        assert_eq!(format_binary_bytes(1024), "1.0KiB");
        assert_eq!(format_binary_bytes(1_572_864), "1.5MiB");
        let mib = 1024i128 * 1024;
        assert_eq!(format_binary_bytes(100 * mib), "100MiB");
        assert_eq!(format_binary_bytes((99.9 * mib as f64) as i128), "99.9MiB");
        assert_eq!(format_binary_bytes(1023 * 1024), "1023KiB");
    }

    #[test]
    fn binary_rates_share_the_same_rules() {
        let mib = 1024.0 * 1024.0;
        assert_eq!(format_binary_rate(2.1 * mib), "2.1MiB/s");
        assert_eq!(format_binary_rate(1.4 * mib), "1.4MiB/s");
        assert_eq!(format_binary_rate(100.0 * mib), "100MiB/s");
    }

    #[test]
    fn rate_pairs_share_the_unit_of_the_larger_value() {
        let mib = 1024.0 * 1024.0;
        assert_eq!(
            format_shared_binary_pair("↓", "↑", 2.1 * mib, 0.4 * mib),
            "↓2.1 ↑0.4MiB/s"
        );
        assert_eq!(
            format_shared_binary_pair("R", "W", 1.2 * mib, 0.3 * mib),
            "R1.2 W0.3MiB/s"
        );
        assert_eq!(
            format_shared_binary_pair("↓", "↑", 1.5 * mib, 512.0 * 1024.0),
            "↓1.5 ↑0.5MiB/s"
        );
    }

    #[test]
    fn cpu_frequency_and_percent_badges() {
        assert_eq!(format_cpu_mhz(480.0), "480MHz");
        assert_eq!(format_cpu_mhz(9_999.0), "9999MHz");
        assert_eq!(format_cpu_mhz(10_000.0), "10.0GHz");
        assert_eq!(format_cpu_mhz(100_000.0), "100GHz");
        assert_eq!(format_percent(12.0), "12%");
        assert_eq!(format_percent(35.4), "35.4%");
        assert_eq!(format_percent(100.0), "100%");
    }

    #[test]
    fn bucket_series_repeats_when_wider_and_averages_when_narrower() {
        let values = [Some(1.0), None, Some(3.0), Some(5.0)];
        let wide = bucket_series(&values, 8);
        assert_eq!(wide.len(), 8);
        assert_eq!(wide[0], Some(1.0));
        assert_eq!(wide[1], Some(1.0));
        assert!(wide.iter().any(|value| value.is_none()));

        let narrow = bucket_series(&values, 2);
        assert_eq!(narrow[0], Some(1.0));
        assert_eq!(narrow[1], Some(4.0));

        let gaps = bucket_series(&[None, None, Some(2.0), Some(4.0)], 2);
        assert_eq!(gaps[0], None);
        assert_eq!(gaps[1], Some(3.0));
    }

    #[test]
    fn sparkline_line_breaks_on_gaps_and_keeps_zero_low() {
        let line = sparkline_line(&[Some(0.0), None, Some(1.0), Some(2.0)]);
        assert_eq!(line.chars().next(), Some('▁'));
        assert_eq!(line.chars().nth(1), Some(' '));
        assert_ne!(line.chars().nth(1), Some('▁'));
        assert!(line.chars().nth(3).unwrap() > line.chars().nth(2).unwrap());
    }
}

//! Body-coordinate layout for the performance overview.
//!
//! First match wins. Rects include borders. Content height is body height minus one when the
//! title row is shown (`height >= 8`).

use ratatui::layout::Rect;

/// The four overview domains, in canonical order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Domain {
    Cpu,
    Memory,
    Network,
    Disk,
}

impl Domain {
    pub const ALL: [Domain; 4] = [Domain::Cpu, Domain::Memory, Domain::Network, Domain::Disk];

    /// Focused domain first, then the rest in canonical order.
    pub fn focus_order(self) -> [Domain; 4] {
        let mut order = [self; 4];
        let mut index = 1;
        for domain in Self::ALL {
            if domain != self {
                order[index] = domain;
                index += 1;
            }
        }
        order
    }
}

/// How a slot is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotKind {
    /// Border plus border-title badge. Plot fills the rest.
    Chart,
    /// One text line.
    Badge,
    /// Rail band: row 0 is the badge, the rows below are a sparkline.
    BadgeWithSparkline,
    /// Template 8 bars only. The four badges are separate slots.
    Sparkline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub domain: Domain,
    pub rect: Rect,
    pub kind: SlotKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerfLayout {
    /// `1..=8`, or `9` for 9a and 9b.
    pub template: u8,
    pub title: Option<Rect>,
    pub slots: Vec<Slot>,
    /// Template 9b only.
    pub combined: Option<Rect>,
}

/// `width` and `height` are the body rect.
pub fn perf_overview_layout(width: u16, height: u16, focus: Domain) -> PerfLayout {
    let title = (height >= 8).then_some(Rect {
        x: 0,
        y: 0,
        width,
        height: 1,
    });
    let origin = u16::from(title.is_some());
    let content_h = height.saturating_sub(origin);
    let order = focus.focus_order();

    if width >= 72 && content_h >= 16 {
        return charts_grid(1, width, origin, content_h, 2, 2, &order, title);
    }
    if width >= 144 && content_h >= 8 {
        return charts_grid(2, width, origin, content_h, 4, 1, &order, title);
    }
    if width >= 36 && content_h >= 32 {
        return charts_grid(3, width, origin, content_h, 1, 4, &order, title);
    }
    if width >= 72 && content_h >= 9 {
        return charts_then_badges(4, width, origin, content_h, 2, 1, &order, title);
    }
    if width >= 36 && content_h >= 18 {
        return charts_then_badges(5, width, origin, content_h, 1, 2, &order, title);
    }
    if width >= 58 && content_h >= 8 {
        return chart_and_rail(width, origin, content_h, &order, title);
    }
    if width >= 36 && content_h >= 11 {
        return charts_then_badges(7, width, origin, content_h, 1, 3, &order, title);
    }
    if width >= 21 && content_h >= 4 {
        return template_eight(width, origin, content_h, focus, title);
    }
    if width < 21 && content_h >= 4 {
        return template_nine_a(width, origin, title);
    }
    template_nine_b(width, origin, height, title)
}

#[allow(clippy::too_many_arguments)]
fn charts_grid(
    template: u8,
    width: u16,
    origin: u16,
    content_h: u16,
    cols: u16,
    rows: u16,
    order: &[Domain; 4],
    title: Option<Rect>,
) -> PerfLayout {
    let col_w = split(width, cols);
    let row_h = split(content_h, rows);
    let mut slots = Vec::with_capacity(4);
    let mut index = 0;
    let mut y = origin;
    for height in row_h {
        let mut x = 0;
        for col_width in &col_w {
            slots.push(Slot {
                domain: order[index],
                rect: Rect {
                    x,
                    y,
                    width: *col_width,
                    height,
                },
                kind: SlotKind::Chart,
            });
            x += *col_width;
            index += 1;
        }
        y += height;
    }
    PerfLayout {
        template,
        title,
        slots,
        combined: None,
    }
}

/// `chart_cols` × one chart row, then `badge_rows` of badges.
/// Template 4 is two charts side by side and one badge row of two.
/// Template 5 is two stacked charts and two badge rows.
/// Template 7 is one chart and three badge rows.
#[allow(clippy::too_many_arguments)]
fn charts_then_badges(
    template: u8,
    width: u16,
    origin: u16,
    content_h: u16,
    chart_cols: u16,
    badge_rows: u16,
    order: &[Domain; 4],
    title: Option<Rect>,
) -> PerfLayout {
    let chart_h = content_h.saturating_sub(badge_rows);
    let mut slots = Vec::with_capacity(4);
    let mut index = 0usize;

    if template == 5 {
        let heights = split(chart_h, 2);
        let mut y = origin;
        for height in heights {
            slots.push(Slot {
                domain: order[index],
                rect: Rect {
                    x: 0,
                    y,
                    width,
                    height,
                },
                kind: SlotKind::Chart,
            });
            y += height;
            index += 1;
        }
        for row in 0..badge_rows {
            slots.push(Slot {
                domain: order[index],
                rect: Rect {
                    x: 0,
                    y: origin + chart_h + row,
                    width,
                    height: 1,
                },
                kind: SlotKind::Badge,
            });
            index += 1;
        }
    } else if chart_cols == 2 {
        let widths = split(width, 2);
        let mut x = 0;
        for col_w in widths {
            slots.push(Slot {
                domain: order[index],
                rect: Rect {
                    x,
                    y: origin,
                    width: col_w,
                    height: chart_h,
                },
                kind: SlotKind::Chart,
            });
            x += col_w;
            index += 1;
        }
        let badge_w = split(width, 2);
        let mut x = 0;
        for col_w in badge_w {
            slots.push(Slot {
                domain: order[index],
                rect: Rect {
                    x,
                    y: origin + chart_h,
                    width: col_w,
                    height: 1,
                },
                kind: SlotKind::Badge,
            });
            x += col_w;
            index += 1;
        }
    } else {
        slots.push(Slot {
            domain: order[0],
            rect: Rect {
                x: 0,
                y: origin,
                width,
                height: chart_h,
            },
            kind: SlotKind::Chart,
        });
        for row in 0..badge_rows {
            slots.push(Slot {
                domain: order[row as usize + 1],
                rect: Rect {
                    x: 0,
                    y: origin + chart_h + row,
                    width,
                    height: 1,
                },
                kind: SlotKind::Badge,
            });
        }
    }

    PerfLayout {
        template,
        title,
        slots,
        combined: None,
    }
}

fn chart_and_rail(
    width: u16,
    origin: u16,
    content_h: u16,
    order: &[Domain; 4],
    title: Option<Rect>,
) -> PerfLayout {
    let rail_w = 22;
    let chart_w = width.saturating_sub(rail_w);
    let mut slots = vec![Slot {
        domain: order[0],
        rect: Rect {
            x: 0,
            y: origin,
            width: chart_w,
            height: content_h,
        },
        kind: SlotKind::Chart,
    }];
    let bands = split(content_h, 3);
    let mut y = origin;
    for (band, domain) in bands.into_iter().zip(order.iter().skip(1)) {
        slots.push(Slot {
            domain: *domain,
            rect: Rect {
                x: chart_w,
                y,
                width: rail_w,
                height: band,
            },
            kind: SlotKind::BadgeWithSparkline,
        });
        y += band;
    }
    PerfLayout {
        template: 6,
        title,
        slots,
        combined: None,
    }
}

fn template_eight(
    width: u16,
    origin: u16,
    content_h: u16,
    focus: Domain,
    title: Option<Rect>,
) -> PerfLayout {
    let mut slots = Vec::with_capacity(5);
    for (row, domain) in Domain::ALL.into_iter().enumerate() {
        slots.push(Slot {
            domain,
            rect: Rect {
                x: 0,
                y: origin + row as u16,
                width,
                height: 1,
            },
            kind: SlotKind::Badge,
        });
    }
    let spark_h = content_h.saturating_sub(4);
    if spark_h >= 1 {
        slots.push(Slot {
            domain: focus,
            rect: Rect {
                x: 0,
                y: origin + 4,
                width,
                height: spark_h,
            },
            kind: SlotKind::Sparkline,
        });
    }
    PerfLayout {
        template: 8,
        title,
        slots,
        combined: None,
    }
}

fn template_nine_a(width: u16, origin: u16, title: Option<Rect>) -> PerfLayout {
    let slots = Domain::ALL
        .into_iter()
        .enumerate()
        .map(|(row, domain)| Slot {
            domain,
            rect: Rect {
                x: 0,
                y: origin + row as u16,
                width,
                height: 1,
            },
            kind: SlotKind::Badge,
        })
        .collect();
    PerfLayout {
        template: 9,
        title,
        slots,
        combined: None,
    }
}

fn template_nine_b(width: u16, origin: u16, height: u16, title: Option<Rect>) -> PerfLayout {
    let combined = (height > origin).then_some(Rect {
        x: 0,
        y: origin,
        width,
        height: 1,
    });
    PerfLayout {
        template: 9,
        title,
        slots: Vec::new(),
        combined,
    }
}

/// Split `total` into `n` parts. Earlier parts receive the remainder so none are short by more than one.
fn split(total: u16, n: u16) -> Vec<u16> {
    let n = n.max(1);
    let base = total / n;
    let extra = total % n;
    (0..n)
        .map(|index| base + u16::from(index < extra))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content_height(height: u16) -> u16 {
        if height >= 8 { height - 1 } else { height }
    }

    fn layout_for_terminal(cols: u16, rows: u16, header_on: bool, focus: Domain) -> PerfLayout {
        let body_h = if header_on {
            rows.saturating_sub(5)
        } else {
            rows
        };
        perf_overview_layout(cols, body_h, focus)
    }

    #[test]
    fn checked_terminal_sizes_match_the_contract() {
        let cases = [
            (32u16, 12u16, true, 8u8, false),
            (80, 24, true, 1, true),
            (135, 42, true, 1, true),
            (72, 16, true, 4, true),
            (58, 17, true, 6, true),
            (40, 15, true, 8, true),
            (40, 15, false, 7, true),
            (40, 45, true, 3, true),
        ];
        for (cols, rows, header_on, template, titled) in cases {
            let layout = layout_for_terminal(cols, rows, header_on, Domain::Cpu);
            assert_eq!(
                layout.template, template,
                "{cols}x{rows} header_on={header_on}"
            );
            assert_eq!(layout.title.is_some(), titled, "{cols}x{rows}");
        }
    }

    #[test]
    fn narrow_body_is_four_clipped_badges_and_short_body_is_one_line() {
        let nine_a = perf_overview_layout(10, 6, Domain::Cpu);
        assert_eq!(nine_a.template, 9);
        assert!(nine_a.combined.is_none());
        assert_eq!(nine_a.slots.len(), 4);
        assert!(nine_a.slots.iter().all(|slot| slot.kind == SlotKind::Badge));

        let nine_b = perf_overview_layout(40, 3, Domain::Memory);
        assert_eq!(nine_b.template, 9);
        assert!(nine_b.slots.is_empty());
        assert!(nine_b.combined.is_some());
        assert!(nine_b.title.is_none());
    }

    #[test]
    fn template_eight_sparkline_is_the_focused_domain() {
        let layout = perf_overview_layout(32, 7, Domain::Disk);
        let spark: Vec<_> = layout
            .slots
            .iter()
            .filter(|slot| slot.kind == SlotKind::Sparkline)
            .collect();
        assert_eq!(spark.len(), 1);
        assert_eq!(spark[0].domain, Domain::Disk);
        assert_eq!(spark[0].rect.height, 3);
        assert_eq!(
            layout
                .slots
                .iter()
                .filter(|slot| slot.kind == SlotKind::Badge)
                .count(),
            4
        );
    }

    #[test]
    fn invariant_holds_for_the_size_matrix_and_every_focus() {
        for width in 1u16..=200 {
            for height in 1u16..=60 {
                for focus in Domain::ALL {
                    let layout = perf_overview_layout(width, height, focus);
                    assert_rects(&layout, width, height, focus);
                }
            }
        }
    }

    fn assert_rects(layout: &PerfLayout, width: u16, height: u16, focus: Domain) {
        let mut rects = Vec::new();
        if let Some(title) = layout.title {
            assert_eq!(title.y, 0);
            assert_eq!(title.height, 1);
            assert_inside(title, width, height);
            rects.push(title);
        }
        for slot in &layout.slots {
            assert_inside(slot.rect, width, height);
            if slot.kind == SlotKind::Chart {
                assert!(slot.rect.width >= 36, "chart width {}", slot.rect.width);
                assert!(slot.rect.height >= 8, "chart height {}", slot.rect.height);
            }
            rects.push(slot.rect);
        }
        if let Some(combined) = layout.combined {
            assert_inside(combined, width, height);
            rects.push(combined);
        }
        for (left_index, left) in rects.iter().enumerate() {
            for right in rects.iter().skip(left_index + 1) {
                assert!(!overlaps(*left, *right), "{left:?} overlaps {right:?}");
            }
        }

        let content_h = content_height(height);
        if content_h >= 4 {
            assert!(layout.combined.is_none());
            let mut domains: Vec<_> = layout
                .slots
                .iter()
                .filter(|slot| slot.kind != SlotKind::Sparkline)
                .map(|slot| slot.domain)
                .collect();
            domains.sort_by_key(|domain| format!("{domain:?}"));
            let mut expected = Domain::ALL.to_vec();
            expected.sort_by_key(|domain| format!("{domain:?}"));
            assert_eq!(domains, expected);
            if layout.template == 8 {
                let sparks: Vec<_> = layout
                    .slots
                    .iter()
                    .filter(|slot| slot.kind == SlotKind::Sparkline)
                    .collect();
                if content_h > 4 {
                    assert_eq!(sparks.len(), 1);
                    assert_eq!(sparks[0].domain, focus);
                } else {
                    assert!(sparks.is_empty());
                }
            }
        } else {
            assert!(layout.slots.is_empty());
            assert!(layout.combined.is_some());
        }
    }

    fn assert_inside(rect: Rect, width: u16, height: u16) {
        let right = u32::from(rect.x) + u32::from(rect.width);
        let bottom = u32::from(rect.y) + u32::from(rect.height);
        assert!(right <= u32::from(width), "{rect:?} leaves width {width}");
        assert!(
            bottom <= u32::from(height),
            "{rect:?} leaves height {height}"
        );
    }

    fn overlaps(left: Rect, right: Rect) -> bool {
        if left.width == 0 || left.height == 0 || right.width == 0 || right.height == 0 {
            return false;
        }
        left.x < right.x.saturating_add(right.width)
            && right.x < left.x.saturating_add(left.width)
            && left.y < right.y.saturating_add(right.height)
            && right.y < left.y.saturating_add(left.height)
    }
}

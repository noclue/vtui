use crate::resource_browser::tabular_data::ColumnLayout;
use ratatui::layout::Constraint;

/// VM browser ID column (6 narrower than the global default).
pub const VM_ID_COLUMN_WIDTH: u16 = 12;
pub const VM_NAME_MIN_WIDTH: u16 = 20;
pub const VM_STATUS_WIDTH: u16 = 2;
pub const VM_POWER_WIDTH: u16 = 2;
pub const VM_OS_WIDTH: u16 = 15;
pub const VM_USED_SPACE_WIDTH: u16 = 12;
pub const VM_CPU_WIDTH: u16 = 10;
pub const VM_MEMORY_WIDTH: u16 = 11;

pub const TABLE_HIGHLIGHT_PREFIX_WIDTH: u16 = 2;
pub const TABLE_COLUMN_SPACING: u16 = 1;

/// Logical VM table column index (matches `VmData::inventory_row` cell order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum VmColumn {
    Id = 0,
    Status = 1,
    Power = 2,
    Name = 3,
    Os = 4,
    UsedSpace = 5,
    Cpu = 6,
    Memory = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VmLayoutTier {
    NameOnly = 0,
    WithStatusPower = 1,
    WithId = 2,
    WithOs = 3,
    WithUsedSpace = 4,
    WithMemory = 5,
    Full = 6,
}

const ALL_TIERS: [VmLayoutTier; 7] = [
    VmLayoutTier::NameOnly,
    VmLayoutTier::WithStatusPower,
    VmLayoutTier::WithId,
    VmLayoutTier::WithOs,
    VmLayoutTier::WithUsedSpace,
    VmLayoutTier::WithMemory,
    VmLayoutTier::Full,
];

pub fn vm_column_width(col: VmColumn) -> u16 {
    match col {
        VmColumn::Id => VM_ID_COLUMN_WIDTH,
        VmColumn::Status => VM_STATUS_WIDTH,
        VmColumn::Power => VM_POWER_WIDTH,
        VmColumn::Name => VM_NAME_MIN_WIDTH,
        VmColumn::Os => VM_OS_WIDTH,
        VmColumn::UsedSpace => VM_USED_SPACE_WIDTH,
        VmColumn::Cpu => VM_CPU_WIDTH,
        VmColumn::Memory => VM_MEMORY_WIDTH,
    }
}

pub fn tier_visible_indices(tier: VmLayoutTier) -> Vec<usize> {
    match tier {
        VmLayoutTier::NameOnly => vec![3],
        VmLayoutTier::WithStatusPower => vec![1, 2, 3],
        VmLayoutTier::WithId => vec![0, 1, 2, 3],
        VmLayoutTier::WithOs => vec![0, 1, 2, 3, 4],
        VmLayoutTier::WithUsedSpace => vec![0, 1, 2, 3, 4, 5],
        VmLayoutTier::WithMemory => vec![0, 1, 2, 3, 4, 5, 7],
        VmLayoutTier::Full => vec![0, 1, 2, 3, 4, 5, 6, 7],
    }
}

pub fn tier_fits(columns_budget: u16, tier: VmLayoutTier) -> bool {
    let indices = tier_visible_indices(tier);
    let width_sum: u16 = indices
        .iter()
        .map(|&i| vm_column_width(VmColumn::from_index(i)))
        .sum();
    let gaps = (indices.len() as u16).saturating_sub(1) * TABLE_COLUMN_SPACING;
    width_sum.saturating_add(gaps) <= columns_budget
}

pub fn highest_tier(columns_budget: u16) -> VmLayoutTier {
    ALL_TIERS
        .iter()
        .rev()
        .copied()
        .find(|&tier| tier_fits(columns_budget, tier))
        .unwrap_or(VmLayoutTier::NameOnly)
}

fn constraint_for_column(col: VmColumn) -> Constraint {
    match col {
        VmColumn::Name => Constraint::Fill(1),
        VmColumn::Os => Constraint::Length(VM_OS_WIDTH),
        VmColumn::UsedSpace => Constraint::Length(VM_USED_SPACE_WIDTH),
        VmColumn::Cpu => Constraint::Length(VM_CPU_WIDTH),
        VmColumn::Memory => Constraint::Length(VM_MEMORY_WIDTH),
        VmColumn::Id => Constraint::Length(VM_ID_COLUMN_WIDTH),
        VmColumn::Status => Constraint::Length(VM_STATUS_WIDTH),
        VmColumn::Power => Constraint::Length(VM_POWER_WIDTH),
    }
}

pub fn vm_column_layout(columns_budget: u16) -> ColumnLayout {
    let tier = highest_tier(columns_budget);
    let visible_indices = tier_visible_indices(tier);
    let constraints = visible_indices
        .iter()
        .map(|&i| {
            if i == 3 && tier == VmLayoutTier::NameOnly && columns_budget < VM_NAME_MIN_WIDTH {
                Constraint::Length(columns_budget.max(1))
            } else {
                constraint_for_column(VmColumn::from_index(i))
            }
        })
        .collect();
    ColumnLayout {
        visible_indices,
        constraints,
    }
}

impl VmColumn {
    fn from_index(i: usize) -> Self {
        match i {
            0 => VmColumn::Id,
            1 => VmColumn::Status,
            2 => VmColumn::Power,
            3 => VmColumn::Name,
            4 => VmColumn::Os,
            5 => VmColumn::UsedSpace,
            6 => VmColumn::Cpu,
            7 => VmColumn::Memory,
            _ => panic!("invalid VM column index {i}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_browser::formatting::{ID_COLUMN_WIDTH, STATUS_COLUMN_WIDTH};

    fn assert_layout_parity(layout: &ColumnLayout) {
        assert_eq!(
            layout.visible_indices.len(),
            layout.constraints.len(),
            "visible_indices and constraints length mismatch"
        );
    }

    #[test]
    fn vm_id_width_is_six_narrower_than_global_default() {
        assert_eq!(VM_ID_COLUMN_WIDTH, 12);
        assert_eq!(ID_COLUMN_WIDTH, 18);
        assert_eq!(ID_COLUMN_WIDTH - VM_ID_COLUMN_WIDTH, 6);
    }

    #[test]
    fn tier_0_1_boundaries() {
        assert_eq!(highest_tier(19), VmLayoutTier::NameOnly);
        assert_eq!(highest_tier(20), VmLayoutTier::NameOnly);
        assert_eq!(highest_tier(25), VmLayoutTier::NameOnly);
        assert_eq!(highest_tier(26), VmLayoutTier::WithStatusPower);
        assert_eq!(highest_tier(38), VmLayoutTier::WithStatusPower);
        assert_eq!(highest_tier(39), VmLayoutTier::WithId);
    }

    #[test]
    fn tier_1_shows_status_power_before_id() {
        let layout = vm_column_layout(30);
        assert_eq!(layout.visible_indices, vec![1, 2, 3]);
        assert!(!layout.visible_indices.contains(&0));
    }

    #[test]
    fn sub_20_still_name_only() {
        let layout = vm_column_layout(15);
        assert_eq!(layout.visible_indices, vec![3]);
        assert_layout_parity(&layout);
    }

    #[test]
    fn tier_2_through_6_boundaries() {
        assert_eq!(highest_tier(54), VmLayoutTier::WithId);
        assert_eq!(highest_tier(55), VmLayoutTier::WithOs);
        assert_eq!(highest_tier(67), VmLayoutTier::WithOs);
        assert_eq!(highest_tier(68), VmLayoutTier::WithUsedSpace);
        assert_eq!(highest_tier(79), VmLayoutTier::WithUsedSpace);
        assert_eq!(highest_tier(80), VmLayoutTier::WithMemory);
        assert_eq!(highest_tier(90), VmLayoutTier::WithMemory);
        assert_eq!(highest_tier(91), VmLayoutTier::Full);
    }

    #[test]
    fn monotonic_reveal_order() {
        let mut prev_len = 0usize;
        for budget in [20, 26, 39, 55, 68, 80, 91] {
            let layout = vm_column_layout(budget);
            assert!(layout.visible_indices.len() >= prev_len);
            assert!(layout.visible_indices.windows(2).all(|w| w[0] < w[1]));
            prev_len = layout.visible_indices.len();
        }
    }

    #[test]
    fn status_and_power_always_paired() {
        for budget in 1..=120u16 {
            let layout = vm_column_layout(budget);
            let has_s = layout.visible_indices.contains(&1);
            let has_p = layout.visible_indices.contains(&2);
            assert_eq!(has_s, has_p, "budget {budget}");
        }
    }

    #[test]
    fn tier_6_includes_cpu_and_memory() {
        let layout = vm_column_layout(120);
        assert_eq!(layout.visible_indices.len(), 8);
        assert_eq!(layout.visible_indices[6], 6);
        assert_eq!(layout.visible_indices[7], 7);
        assert!(matches!(layout.constraints[3], Constraint::Fill(1)));
        assert_layout_parity(&layout);
    }

    #[test]
    fn column_layout_length_parity_all_budgets() {
        for budget in 1..=150u16 {
            assert_layout_parity(&vm_column_layout(budget));
        }
    }

    #[test]
    fn status_width_matches_formatting_constant() {
        assert_eq!(VM_STATUS_WIDTH, STATUS_COLUMN_WIDTH);
    }
}

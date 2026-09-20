//! Fresh per-player equipment, distinct from the published active-pilot
//! snapshot. Authored pickups mutate the path-selected player's record.

/// The source keeps the consumable count in a packed byte ($6C04), its type
/// in a separate byte ($6C05), and weapon level in another ($6C06). The use
/// service at $07:DC9C tests the count, dispatches by type, then decrements
/// the count at $07:DD5E. Do not confuse type with the packed high nibble.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SelectedEquipment {
    pub packed_consumables: u8,
    pub consumable_type: u8,
    pub weapon_level: u8,
}

impl SelectedEquipment {
    /// $7F:B0E5 / $06:9112. Return whether the PREVIOUS count was full and
    /// the type was already selected. Type is replaced even when full.
    /// Reaching the cap on this call does not take the already-full branch.
    pub(crate) fn collect_consumables(&mut self, amount: u8, kind: u8) -> bool {
        const COUNT_MASK: u8 = 0x0F;
        const COUNT_CAP: u8 = 9;
        let same_type = self.consumable_type == kind;
        self.consumable_type = kind;
        let count = self.packed_consumables & COUNT_MASK;
        if count >= COUNT_CAP {
            return same_type;
        }
        // Addition wraps at byte width BEFORE comparison with the cap.
        let updated = count.wrapping_add(amount).min(COUNT_CAP);
        self.packed_consumables = (self.packed_consumables & !COUNT_MASK) | updated;
        false
    }

    /// $7F:BA75. Above-cap source values are retained, not normalized.
    pub(crate) fn upgrade_weapon(&mut self) {
        const WEAPON_LEVEL_CAP: u8 = 3;
        if self.weapon_level < WEAPON_LEVEL_CAP {
            self.weapon_level += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_exhausts_packed_counts_and_wrapping_amounts() {
        for packed in 0..=u8::MAX {
            for amount in 0..=u8::MAX {
                for same_type in [false, true] {
                    let mut state = SelectedEquipment {
                        packed_consumables: packed,
                        consumable_type: if same_type { 173 } else { 201 },
                        weapon_level: !amount,
                    };
                    let old_count = packed % 16;
                    let sum = (u16::from(old_count) + u16::from(amount)) % 256;
                    let expected_count = if old_count >= 9 { old_count } else if sum >= 9 { 9 } else { sum as u8 };
                    assert_eq!(state.collect_consumables(amount, 173), old_count >= 9 && same_type);
                    assert_eq!(state, SelectedEquipment {
                        packed_consumables: packed / 16 * 16 + expected_count,
                        consumable_type: 173,
                        weapon_level: !amount,
                    });
                }
            }
        }
    }

    #[test]
    fn type_comparison_retains_full_bytes_and_precedes_unconditional_assignment() {
        for previous in 0..=u8::MAX {
            for kind in 0..=u8::MAX {
                let mut state = SelectedEquipment {
                    packed_consumables: 0xC9,
                    consumable_type: previous,
                    weapon_level: 219,
                };
                assert_eq!(state.collect_consumables(255, kind), previous == kind);
                assert_eq!(state, SelectedEquipment { packed_consumables: 0xC9, consumable_type: kind, weapon_level: 219 });
                assert!(state.collect_consumables(0, kind));
            }
        }
    }

    #[test]
    fn weapon_upgrade_exhausts_levels_without_normalizing_above_cap() {
        for level in 0..=u8::MAX {
            let mut state = SelectedEquipment {
                packed_consumables: level,
                consumable_type: !level,
                weapon_level: level,
            };
            for visit in 1..=4 {
                state.upgrade_weapon();
                assert_eq!(state, SelectedEquipment {
                    packed_consumables: level,
                    consumable_type: !level,
                    weapon_level: if level >= 3 { level } else { (level + visit).min(3) },
                });
            }
        }
    }
}

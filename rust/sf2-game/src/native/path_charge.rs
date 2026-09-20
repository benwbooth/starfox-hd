//! Charge-orb callback (`$07:F54E`), using fresh selected-player observations.

use super::{Object, ObjectId};

const LINKED_CHARGE_SPRITE_SIZE: u8 = u8::MAX;
const PHASE_HIGH_MASK: u16 = 0xFF00;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedChargeInput {
    pub linked_mode: bool,
    /// Complete auxiliary level byte: the source does not mask its flags.
    pub level: u8,
}

/// Refresh the relative reference, not the child-list attachment or world
/// pose. The authored distance and threshold remain independently retained.
pub fn refresh_attachment(actor: &mut Object, selected: ObjectId, input: SelectedChargeInput) {
    if input.linked_mode {
        actor.extension.texture_scroll_x = LINKED_CHARGE_SPRITE_SIZE;
    }
    actor.extension.parent = Some(selected);
    actor.extension.relative_position.z = actor.extension.path_state.script_value as i16;
    actor.extension.relative_position.y = 0;
    actor.extension.path_state.motion_phase =
        (actor.extension.path_state.motion_phase & PHASE_HIGH_MASK) | u16::from(input.level);
}

#[cfg(test)]
mod tests {
    use super::super::{Behavior, ObjectKind, ObjectStore, ShapeId, Vector3};
    use super::*;

    #[test]
    fn charge_attachment_preserves_all_other_fields_and_word_widths() {
        let mut objects = ObjectStore::new();
        let mut original = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
        let selected = objects.allocate(original.clone()).unwrap();
        original.extension.relative_position = Vector3 {
            x: -197,
            y: 523,
            z: 98,
        };
        original.extension.texture_scroll_x = 73;
        original.base.position = Vector3 {
            x: 903,
            y: -1703,
            z: i16::MIN,
        };
        original.extension.path_state.motion_phase = 0xA539;
        for distance in 0..=u16::MAX {
            for linked_mode in [false, true] {
                // Across all distances, both level and retained threshold
                // independently cover every possible byte, including flags.
                let level = !(distance as u8);
                original.extension.path_state.script_value = distance;
                original.extension.path_state.motion_phase = distance;
                let mut actual = original.clone();
                let mut expected = original.clone();
                expected.extension.parent = Some(selected);
                expected.extension.relative_position.y = 0;
                expected.extension.relative_position.z = distance as i16;
                expected.extension.path_state.motion_phase = (distance & 0xFF00) | u16::from(level);
                if linked_mode {
                    expected.extension.texture_scroll_x = 255;
                }
                refresh_attachment(
                    &mut actual,
                    selected,
                    SelectedChargeInput { linked_mode, level },
                );
                assert_eq!(actual, expected);
            }
        }
    }
}

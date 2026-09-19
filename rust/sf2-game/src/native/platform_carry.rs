//! Selected-player platform correction (`$7F:9EC9`, `$7F:BAF7..BC50`).

use super::attachments::attachment_matrix;
use super::{Angle, Object, ObjectId, Rotation, Vector3};
use sf_core::snes_trig::matrix_rotate_q15;

const HORIZONTAL_SCALE: i16 = 16;
const FINE_ANGLE_BITS: u32 = 8;
const LOW_BYTE: u16 = 0x00FF;

/// Retained carrier state. These source words also serve other motion
/// contexts: clearing continuity or saving yaw changes only the low byte.
/// Keep the whole words so decoded field access can preserve those aliases.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlatformCarryState {
    pub continuity: u16,
    pub saved_yaw: u16,
    pub saved_position: Vector3,
}

/// Resolve this from the world's selection AFTER callbacks, not from the
/// actor's original target. Origin and fine yaw are player auxiliary state,
/// distinct from the player's published object transform.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CarriedPlayer {
    pub enabled: bool,
    pub carrier: Option<ObjectId>,
    pub origin: Vector3,
    pub fine_yaw: u16,
}

/// Apply movement's carry gate and retain a new snapshot. Missing selection
/// is an ineligible player; a disabled actor gate leaves all history intact.
pub fn after_callbacks(actor: &mut Object, owner: ObjectId, selected: Option<&mut CarriedPlayer>) {
    if !actor.extension.path_state.motion.carry_selected_player {
        return;
    }
    let state = &mut actor.extension.path_state.platform_carry;
    let Some(player) = selected.filter(|player| player.enabled && player.carrier == Some(owner))
    else {
        state.continuity &= !LOW_BYTE;
        return;
    };
    if state.continuity & LOW_BYTE != 0 {
        correct(player, *state, actor.base.position, actor.base.yaw);
    }
    state.saved_position = actor.base.position;
    state.continuity = 1; // Unlike the ineligible branch, this writes the whole word.
    state.saved_yaw = (state.saved_yaw & !LOW_BYTE) | u16::from(actor.base.yaw.units());
}

fn correct(
    player: &mut CarriedPlayer,
    previous: PlatformCarryState,
    position: Vector3,
    yaw: Angle,
) {
    let mut x = player
        .origin
        .x
        .wrapping_sub(previous.saved_position.x)
        .wrapping_mul(HORIZONTAL_SCALE);
    let y = player.origin.y.wrapping_sub(previous.saved_position.y);
    let mut z = player
        .origin
        .z
        .wrapping_sub(previous.saved_position.z)
        .wrapping_mul(HORIZONTAL_SCALE);
    let yaw_delta = yaw.units().wrapping_sub(previous.saved_yaw as u8);
    if yaw_delta != 0 {
        player.fine_yaw = player
            .fine_yaw
            .wrapping_add(u16::from(yaw_delta) << FINE_ANGLE_BITS);
        let matrix = attachment_matrix(Rotation {
            yaw: Angle::from_units(yaw_delta),
            ..Rotation::default()
        });
        (x, _, z) = matrix_rotate_q15(matrix, x, y, z);
    }
    player.origin = Vector3 {
        x: position.x.wrapping_add(x / HORIZONTAL_SCALE),
        // The source reads the original delta, not the transform's Y output.
        y: position.y.wrapping_add(y),
        z: position.z.wrapping_add(z / HORIZONTAL_SCALE),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, ObjectKind, ObjectStore, ShapeId};

    fn fixture() -> (Object, ObjectId, CarriedPlayer) {
        let mut actor = Object::new(
            ObjectKind::Scenery,
            ShapeId::from_catalog_index(0),
            Behavior::FollowPath,
        );
        actor.extension.path_state.motion.carry_selected_player = true;
        let owner = ObjectStore::new().allocate(actor.clone()).unwrap();
        (
            actor,
            owner,
            CarriedPlayer {
                enabled: true,
                carrier: Some(owner),
                ..CarriedPlayer::default()
            },
        )
    }

    #[test]
    fn first_contact_snapshots_without_moving_then_follows_translation() {
        let (mut actor, owner, mut player) = fixture();
        player.origin = Vector3 {
            x: 30,
            y: -50,
            z: 70,
        };
        actor.base.position = Vector3 {
            x: 10,
            y: 20,
            z: 30,
        };
        let initial = player;
        after_callbacks(&mut actor, owner, Some(&mut player));
        assert_eq!(player, initial);
        actor.base.position = Vector3 {
            x: 15,
            y: 40,
            z: 23,
        };
        after_callbacks(&mut actor, owner, Some(&mut player));
        assert_eq!(
            player.origin,
            Vector3 {
                x: 35,
                y: -30,
                z: 63
            }
        );
        assert_eq!(
            actor.extension.path_state.platform_carry.saved_position,
            actor.base.position
        );
    }

    #[test]
    fn disabled_actor_preserves_history_but_ineligible_player_clears_only_low_byte() {
        let (mut actor, owner, mut player) = fixture();
        actor.extension.path_state.platform_carry.continuity = 0xAABB;
        actor.extension.path_state.motion.carry_selected_player = false;
        after_callbacks(&mut actor, owner, None);
        assert_eq!(actor.extension.path_state.platform_carry.continuity, 0xAABB);
        actor.extension.path_state.motion.carry_selected_player = true;
        player.enabled = false;
        after_callbacks(&mut actor, owner, Some(&mut player));
        assert_eq!(actor.extension.path_state.platform_carry.continuity, 0xAA00);
        player.enabled = true;
        player.carrier = None;
        actor.extension.path_state.platform_carry.continuity |= 1;
        after_callbacks(&mut actor, owner, Some(&mut player));
        assert_eq!(actor.extension.path_state.platform_carry.continuity, 0xAA00);
    }

    #[test]
    fn snapshot_clears_continuity_high_byte_but_preserves_saved_yaw_high_byte() {
        let (mut actor, owner, mut player) = fixture();
        actor.extension.path_state.platform_carry.continuity = 0xAA00;
        actor.extension.path_state.platform_carry.saved_yaw = 0xBBDD;
        actor.base.yaw = Angle::from_units(7);
        after_callbacks(&mut actor, owner, Some(&mut player));
        let state = actor.extension.path_state.platform_carry;
        assert_eq!(state.continuity, 1);
        assert_eq!(state.saved_yaw, 0xBB07);
    }

    #[test]
    fn wrapped_scaling_applies_even_without_rotation() {
        let (mut actor, owner, mut player) = fixture();
        actor.extension.path_state.platform_carry.continuity = 1;
        player.origin = Vector3 {
            x: 3000,
            y: i16::MIN,
            z: -3000,
        };
        after_callbacks(&mut actor, owner, Some(&mut player));
        assert_eq!(
            player.origin,
            Vector3 {
                x: -1096,
                y: i16::MIN,
                z: 1096
            }
        );
    }

    #[test]
    fn quarter_turn_updates_fine_yaw_and_retains_unrotated_vertical_delta() {
        let (mut actor, owner, mut player) = fixture();
        actor.extension.path_state.platform_carry.continuity = 1;
        actor.base.yaw = Angle::from_units(64);
        player.fine_yaw = 60_000;
        player.origin = Vector3 {
            x: 100,
            y: 30_000,
            z: -100,
        };
        after_callbacks(&mut actor, owner, Some(&mut player));
        assert_eq!(player.fine_yaw, 10_848);
        assert_eq!(
            player.origin,
            Vector3 {
                x: 99,
                y: 30_000,
                z: 99
            }
        );
    }
}

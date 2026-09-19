//! Source facing commands (`$7F:872C..883F`, `$7F:8A72..8B61`).

use super::{Angle, ObjectId, ObjectStore, Rotation, Vector3};
use sf_core::aim_angle::{sf2_pitch_to_target, sf2_xz_angle_distance, sf2_yaw_to_target};

const SELECTED_CHASE_DIVISOR: i8 = 4;
const LINKED_CHASE_DIVISOR: i8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacingCommand {
    SelectedImmediate,
    SelectedSmooth,
    SelectedYaw,
    FixedPlayerImmediate,
    LinkedSmooth,
    LinkedImmediate,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FacingTargets {
    pub selected: Option<ObjectId>,
    /// The live primary selection pointer is not necessarily its fixed slot.
    pub primary: Option<ObjectId>,
    pub fixed_players: [Option<ObjectId>; 2],
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SteeringState {
    /// Shared facing result (149D): linked commands count axes that were
    /// already equal before this command. Selected full-facing clears it;
    /// the selected-yaw-only command leaves it untouched.
    pub unchanged_axes: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteeringError {
    MissingActor(ObjectId),
    MissingSelected,
    MissingFixedPlayer,
    MissingRelativeParent(ObjectId),
    MissingLinked(ObjectId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadiusCenter {
    Selected,
    Linked,
    LocalOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadiusCommand {
    pub center: RadiusCenter,
    /// Positive contracts, negative expands. Literal bytes are sign-extended
    /// when decoded; word-valued variants retain all sixteen bits.
    pub amount: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YawOrbitTarget {
    Object(ObjectId),
    LocalOrigin,
}

/// Horizontal orbit uses the full geometry matrix (`$7F:ACC6..AD63`), not
/// the distinct byte-table rotate_16xz helper. A zero angle still passes
/// through matrix multiplication and retains its fixed-point truncation.
pub fn yaw_orbit_position(position: Vector3, center: Vector3, angle: Angle) -> Vector3 {
    let matrix = super::attachments::attachment_matrix(Rotation {
        yaw: angle,
        ..Rotation::default()
    });
    let (x, _, z) = sf_core::snes_trig::matrix_rotate_q15(
        matrix,
        position.x.wrapping_sub(center.x),
        0,
        position.z.wrapping_sub(center.z),
    );
    Vector3 {
        x: center.x.wrapping_add(x),
        y: position.y,
        z: center.z.wrapping_add(z),
    }
}

pub fn orbit_yaw(
    objects: &mut ObjectStore,
    owner: ObjectId,
    target: YawOrbitTarget,
    angle: Angle,
) -> Result<(), SteeringError> {
    let actor = objects
        .get(owner)
        .ok_or(SteeringError::MissingActor(owner))?;
    let local = target == YawOrbitTarget::LocalOrigin;
    let position = if local {
        actor.extension.relative_position
    } else {
        actor.base.position
    };
    let center = match target {
        YawOrbitTarget::LocalOrigin => Vector3::default(),
        YawOrbitTarget::Object(target) => {
            objects
                .get(target)
                .ok_or(SteeringError::MissingActor(target))?
                .base
                .position
        }
    };
    let result = yaw_orbit_position(position, center, angle);
    let actor = objects.get_mut(owner).expect("validated yaw orbit actor");
    if local {
        actor.extension.relative_position = result;
    } else {
        actor.base.position = result;
    }
    Ok(())
}

/// Radial scaling (`$7F:ADAF`, `$7F:ADC7`, `$7F:AE7C`), not pitch rotation.
/// Local scaling changes retained offsets only, leaving world publication to
/// the attachment service. No variant changes rotation or generated velocity.
pub fn contract_radius(
    objects: &mut ObjectStore,
    owner: ObjectId,
    command: RadiusCommand,
    selected: Option<ObjectId>,
) -> Result<(), SteeringError> {
    let actor = objects
        .get(owner)
        .ok_or(SteeringError::MissingActor(owner))?;
    let local = command.center == RadiusCenter::LocalOrigin;
    let target = match command.center {
        RadiusCenter::Selected => Some(selected.ok_or(SteeringError::MissingSelected)?),
        RadiusCenter::Linked => Some(
            actor
                .base
                .attachment
                .ok_or(SteeringError::MissingLinked(owner))?,
        ),
        RadiusCenter::LocalOrigin => None,
    };
    let position = if local {
        actor.extension.relative_position
    } else {
        actor.base.position
    };
    let center = match target {
        Some(target) => {
            objects
                .get(target)
                .ok_or(SteeringError::MissingActor(target))?
                .base
                .position
        }
        None => Vector3::default(),
    };
    let result = super::path_math::change_radius(position, center, command.amount);
    let actor = objects
        .get_mut(owner)
        .expect("validated radial movement actor");
    if local {
        actor.extension.relative_position = result;
    } else {
        actor.base.position = result;
    }
    Ok(())
}

/// Shortest signed byte displacement, with the source minimum nonzero
/// numerator before signed division. This is not floating-point easing.
fn chase(current: Angle, target: u8, divisor: i8) -> Angle {
    let delta = target.wrapping_sub(current.units()) as i8;
    if delta == 0 {
        return current;
    }
    let step = if delta < 0 {
        delta.min(-divisor)
    } else {
        delta.max(divisor)
    } / divisor;
    current.wrapping_add(step)
}

pub fn face(
    objects: &mut ObjectStore,
    owner: ObjectId,
    command: FacingCommand,
    targets: FacingTargets,
    state: &mut SteeringState,
) -> Result<(), SteeringError> {
    use FacingCommand::*;
    let actor = objects
        .get(owner)
        .ok_or(SteeringError::MissingActor(owner))?;
    let linked = matches!(command, LinkedSmooth | LinkedImmediate);
    let target = if linked {
        // The source returns before clearing the shared result when absent.
        let Some(target) = actor.base.attachment else {
            return Ok(());
        };
        target
    } else if command == FixedPlayerImmediate {
        let index = usize::from(targets.selected != targets.primary);
        targets.fixed_players[index].ok_or(SteeringError::MissingFixedPlayer)?
    } else {
        targets.selected.ok_or(SteeringError::MissingSelected)?
    };
    let target = objects
        .get(target)
        .ok_or(SteeringError::MissingActor(target))?;
    let dx = target.base.position.x.wrapping_sub(actor.base.position.x);
    let dy = target.base.position.y.wrapping_sub(actor.base.position.y);
    let dz = target.base.position.z.wrapping_sub(actor.base.position.z);
    let target_yaw = sf2_yaw_to_target(dx, dz);
    let yaw_only = command == SelectedYaw;
    let divisor = match command {
        SelectedSmooth | SelectedYaw => Some(SELECTED_CHASE_DIVISOR),
        LinkedSmooth => Some(LINKED_CHASE_DIVISOR),
        _ => None,
    };
    let yaw = divisor.map_or(Angle::from_units(target_yaw), |divisor| {
        chase(actor.base.yaw, target_yaw, divisor)
    });
    let pitch = if yaw_only {
        actor.base.pitch
    } else {
        let target_pitch = sf2_pitch_to_target(dy, sf2_xz_angle_distance(dx, dz));
        state.unchanged_axes = if linked {
            u8::from(actor.base.pitch.units() == target_pitch)
                + u8::from(actor.base.yaw.units() == target_yaw)
        } else {
            0
        };
        divisor.map_or(Angle::from_units(target_pitch), |divisor| {
            chase(actor.base.pitch, target_pitch, divisor)
        })
    };
    // Only selected/fixed-facing variants call 883F, and only the relative
    // flag (not the separate attached-coordinate flag) gates this update.
    let relative_yaw = if !linked && actor.extension.path_state.motion.relative_coordinates {
        let parent = actor
            .extension
            .parent
            .ok_or(SteeringError::MissingRelativeParent(owner))?;
        let parent_yaw = if parent == owner {
            yaw
        } else {
            objects
                .get(parent)
                .ok_or(SteeringError::MissingActor(parent))?
                .base
                .yaw
        };
        Some(Angle::from_units(
            yaw.units().wrapping_sub(parent_yaw.units()),
        ))
    } else {
        None
    };
    let actor = objects.get_mut(owner).expect("validated facing actor");
    actor.base.pitch = pitch;
    actor.base.yaw = yaw;
    if let Some(relative_yaw) = relative_yaw {
        actor.extension.relative_rotation.yaw = relative_yaw;
    }
    // No immediate velocity regeneration and no local pitch/roll update.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, Object, ObjectKind, ShapeId, Vector3};

    fn fixture() -> (ObjectStore, ObjectId, ObjectId, FacingTargets) {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Enemy,
                ShapeId::EMPTY,
                Behavior::FollowPath,
            ))
            .unwrap();
        let target = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        objects.get_mut(target).unwrap().base.position.x = 100;
        (
            objects,
            owner,
            target,
            FacingTargets {
                selected: Some(target),
                primary: Some(target),
                fixed_players: [Some(target), Some(owner)],
            },
        )
    }

    #[test]
    fn contraction_updates_all_three_axes_and_local_form_does_not_publish_world_pose() {
        let (mut objects, owner, _, targets) = fixture();
        let position = Vector3 {
            x: 1100,
            y: 1000,
            z: -1000,
        };
        let target = objects
            .get(targets.selected.unwrap())
            .unwrap()
            .base
            .position;
        objects.get_mut(owner).unwrap().base.position = position;
        contract_radius(
            &mut objects,
            owner,
            RadiusCommand {
                center: RadiusCenter::Selected,
                amount: 127,
            },
            targets.selected,
        )
        .unwrap();
        let world = objects.get(owner).unwrap().base.position;
        assert_eq!(
            world,
            super::super::path_math::change_radius(position, target, 127)
        );
        assert!(world.x < position.x && world.y < position.y && world.z > position.z);
        objects.get_mut(owner).unwrap().extension.relative_position = position;
        contract_radius(
            &mut objects,
            owner,
            RadiusCommand {
                center: RadiusCenter::LocalOrigin,
                amount: -127,
            },
            None,
        )
        .unwrap();
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.position, world);
        assert_eq!(
            actor.extension.relative_position,
            super::super::path_math::change_radius(position, Vector3::default(), -127)
        );
    }

    #[test]
    fn horizontal_orbits_use_matrix_precision_and_preserve_altitude() {
        let center = Vector3 {
            x: 500,
            y: 20_000,
            z: -200,
        };
        let position = Vector3 {
            x: 1500,
            y: -1000,
            z: -200,
        };
        assert_eq!(
            yaw_orbit_position(position, center, Angle::from_units(64)),
            Vector3 {
                x: 500,
                y: -1000,
                z: 799
            }
        );
        assert_eq!(
            yaw_orbit_position(position, center, Angle::ZERO),
            Vector3 {
                x: 1499,
                y: -1000,
                z: -200
            }
        );
        let (mut objects, owner, target, _) = fixture();
        objects.get_mut(target).unwrap().base.position = center;
        let actor = objects.get_mut(owner).unwrap();
        actor.base.position = position;
        actor.extension.relative_position = position;
        orbit_yaw(
            &mut objects,
            owner,
            YawOrbitTarget::LocalOrigin,
            Angle::from_units(64),
        )
        .unwrap();
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.position, position);
        assert_eq!(
            actor.extension.relative_position,
            yaw_orbit_position(position, Vector3::default(), Angle::from_units(64))
        );
        orbit_yaw(
            &mut objects,
            owner,
            YawOrbitTarget::Object(target),
            Angle::from_units(64),
        )
        .unwrap();
        assert_eq!(
            objects.get(owner).unwrap().base.position,
            Vector3 {
                x: 500,
                y: -1000,
                z: 799
            }
        );
    }

    #[test]
    fn linked_contraction_uses_base_attachment_and_missing_link_is_not_a_silent_noop() {
        let (mut objects, owner, target, _) = fixture();
        let command = RadiusCommand {
            center: RadiusCenter::Linked,
            amount: 127,
        };
        assert_eq!(
            contract_radius(&mut objects, owner, command, Some(target)),
            Err(SteeringError::MissingLinked(owner))
        );
        objects.get_mut(owner).unwrap().base.attachment = Some(target);
        objects.get_mut(owner).unwrap().extension.parent = Some(owner);
        contract_radius(&mut objects, owner, command, Some(owner)).unwrap();
        assert_eq!(
            objects.get(owner).unwrap().base.position,
            super::super::path_math::change_radius(
                Vector3::default(),
                objects.get(target).unwrap().base.position,
                127
            )
        );
    }

    #[test]
    fn byte_chase_covers_all_displacements_and_wraps_with_minimum_step() {
        for divisor in [4, 8] {
            for current in 0..=255u8 {
                for target in 0..=255u8 {
                    let difference = target.wrapping_sub(current) as i8;
                    let expected = if difference == 0 {
                        0
                    } else {
                        difference.signum()
                            * ((i16::from(difference).abs() / i16::from(divisor)).max(1) as i8)
                    };
                    assert_eq!(
                        chase(Angle::from_units(current), target, divisor).units(),
                        current.wrapping_add(expected as u8)
                    );
                }
            }
        }
    }

    #[test]
    fn selected_smoothing_and_linked_smoothing_have_different_divisors() {
        for (command, expected) in [
            (FacingCommand::SelectedImmediate, 192),
            (FacingCommand::SelectedSmooth, 240),
            (FacingCommand::LinkedSmooth, 248),
        ] {
            let (mut objects, owner, target, targets) = fixture();
            objects.get_mut(owner).unwrap().base.attachment = Some(target);
            let actor = objects.get_mut(owner).unwrap();
            actor.base.roll = Angle::from_units(20);
            actor.base.velocity = Vector3 { x: 1, y: 2, z: 3 };
            face(
                &mut objects,
                owner,
                command,
                targets,
                &mut SteeringState::default(),
            )
            .unwrap();
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.base.yaw.units(), expected);
            assert_eq!(actor.base.roll.units(), 20);
            assert_eq!(actor.base.velocity, Vector3 { x: 1, y: 2, z: 3 });
        }
    }

    #[test]
    fn yaw_only_preserves_pitch_and_shared_result_but_refreshes_relative_yaw() {
        let (mut objects, owner, target, targets) = fixture();
        objects.get_mut(target).unwrap().base.yaw = Angle::from_units(16);
        let actor = objects.get_mut(owner).unwrap();
        actor.base.pitch = Angle::from_units(30);
        actor.extension.parent = Some(target);
        actor.extension.path_state.motion.relative_coordinates = true;
        actor.extension.relative_rotation.pitch = Angle::from_units(70);
        let mut state = SteeringState { unchanged_axes: 19 };
        face(
            &mut objects,
            owner,
            FacingCommand::SelectedYaw,
            targets,
            &mut state,
        )
        .unwrap();
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.pitch.units(), 30);
        assert_eq!(actor.base.yaw.units(), 240);
        assert_eq!(actor.extension.relative_rotation.yaw.units(), 224);
        assert_eq!(actor.extension.relative_rotation.pitch.units(), 70);
        assert_eq!(state.unchanged_axes, 19);
    }

    #[test]
    fn linked_commands_skip_missing_parent_and_count_only_preexisting_alignment() {
        let (mut objects, owner, target, targets) = fixture();
        let mut state = SteeringState { unchanged_axes: 19 };
        face(
            &mut objects,
            owner,
            FacingCommand::LinkedImmediate,
            targets,
            &mut state,
        )
        .unwrap();
        assert_eq!(state.unchanged_axes, 19);
        assert_eq!(objects.get(owner).unwrap().base.yaw, Angle::ZERO);
        objects.get_mut(owner).unwrap().base.attachment = Some(target);
        face(
            &mut objects,
            owner,
            FacingCommand::LinkedImmediate,
            targets,
            &mut state,
        )
        .unwrap();
        assert_eq!(state.unchanged_axes, 1);
        face(
            &mut objects,
            owner,
            FacingCommand::LinkedSmooth,
            targets,
            &mut state,
        )
        .unwrap();
        assert_eq!(state.unchanged_axes, 2);
    }

    #[test]
    fn fixed_player_uses_selection_identity_not_selected_objects_position() {
        let (mut objects, owner, target, mut targets) = fixture();
        objects.get_mut(target).unwrap().base.position.z = 100;
        targets.fixed_players = [Some(owner), Some(target)];
        face(
            &mut objects,
            owner,
            FacingCommand::FixedPlayerImmediate,
            targets,
            &mut SteeringState::default(),
        )
        .unwrap();
        assert_eq!(
            objects.get(owner).unwrap().base.yaw.units(),
            sf2_yaw_to_target(0, 0)
        );
        targets.primary = Some(owner);
        face(
            &mut objects,
            owner,
            FacingCommand::FixedPlayerImmediate,
            targets,
            &mut SteeringState::default(),
        )
        .unwrap();
        assert_eq!(
            objects.get(owner).unwrap().base.yaw.units(),
            sf2_yaw_to_target(100, 100)
        );
    }

    #[test]
    fn local_yaw_update_uses_new_self_heading_and_is_not_gated_by_attachment_flag() {
        let (mut objects, owner, target, targets) = fixture();
        let actor = objects.get_mut(owner).unwrap();
        actor.extension.parent = Some(owner);
        actor.extension.path_state.motion.relative_coordinates = true;
        actor.extension.relative_rotation.yaw = Angle::from_units(7);
        face(
            &mut objects,
            owner,
            FacingCommand::SelectedImmediate,
            targets,
            &mut SteeringState::default(),
        )
        .unwrap();
        assert_eq!(
            objects.get(owner).unwrap().extension.relative_rotation.yaw,
            Angle::ZERO
        );
        let actor = objects.get_mut(owner).unwrap();
        actor.base.attachment = Some(target);
        actor.extension.path_state.motion.relative_coordinates = false;
        actor.extension.path_state.motion.attached_coordinates = true;
        actor.extension.relative_rotation.yaw = Angle::from_units(7);
        face(
            &mut objects,
            owner,
            FacingCommand::SelectedImmediate,
            targets,
            &mut SteeringState::default(),
        )
        .unwrap();
        assert_eq!(
            objects
                .get(owner)
                .unwrap()
                .extension
                .relative_rotation
                .yaw
                .units(),
            7
        );
    }
}

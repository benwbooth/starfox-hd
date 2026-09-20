//! Source path movement primitives (`$7F:9DDE`, `$7F:855F`, `$7F:306E`).
//!
//! Ordinary position integration (`$7F:2C24`) precedes path callbacks;
//! attached-relative integration (`$7F:9E9F`) follows them. These operations
//! are separate so callers cannot accidentally collapse those two phases.

use super::{Angle, Object, ObjectId, ObjectStore, Vector3};

const BANK_TURN_DIVISOR: i8 = 4;
const ORDINARY_VELOCITY_SCALE: i16 = 1;
const ENLARGED_VELOCITY_SCALE: i16 = 4;
const AUXILIARY_MODE_CLASS_MASK: u8 = 0xF0;
const VELOCITY_INHERITANCE_CLASS: u8 = 0x10;

/// Published by the player service at `$07:EA15`. Paths consume the retained
/// snapshot, not the player's potentially newer position, mode or velocity.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PublishedPlayerMotion {
    pub position: Vector3,
    pub delta: Vector3,
}

impl PublishedPlayerMotion {
    pub fn capture(player: &Object, displacement: Vector3, auxiliary_mode: u8) -> Self {
        Self {
            position: player.base.position,
            delta: if auxiliary_mode & AUXILIARY_MODE_CLASS_MASK == VELOCITY_INHERITANCE_CLASS {
                player.base.velocity
            } else {
                displacement
            },
        }
    }
}

/// Add primary-player horizontal motion (`$06:A885..A8D1`). The player's
/// auxiliary mode selects its velocity or retained displacement. This is a
/// one-time velocity addition, not ordinary per-step displacement following.
pub fn inherit_horizontal_motion(
    object: &mut Object,
    player_velocity: Vector3,
    player_displacement: Vector3,
    auxiliary_mode: u8,
) {
    let inherited = if auxiliary_mode & AUXILIARY_MODE_CLASS_MASK == VELOCITY_INHERITANCE_CLASS {
        player_velocity
    } else {
        player_displacement
    };
    object.base.velocity.x = object.base.velocity.x.wrapping_add(inherited.x);
    object.base.velocity.z = object.base.velocity.z.wrapping_add(inherited.z);
}

/// Independent source motion gates. Relative and attached flags can coexist;
/// neither should be inferred from the presence of a parent pointer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MotionSettings {
    /// Source 23 bit 10 enables first-child-chain pose publication.
    pub refresh_child_chain: bool,
    /// Source 22 bit 01 suppresses that publication independently.
    pub suppress_child_refresh: bool,
    /// Source 21 bit 20 enables carrying the currently selected player.
    pub carry_selected_player: bool,
    /// Source 21 bit 08, tested by the displacement service at 9F16.
    pub follow_player_displacement: bool,
    /// Source 21 bit 10; this is NOT the displacement-follow flag.
    pub generate_velocity_each_step: bool,
    /// Source 21 bit 40 applies the bank-derived yaw step.
    pub bank_turn: bool,
    /// Source 23 bit 04: integration uses retained attachment coordinates.
    pub attached_coordinates: bool,
    /// Source 25 bit 04: integration uses retained relative coordinates.
    pub relative_coordinates: bool,
    /// Source 26 bit 80 multiplies all generated velocity words by four.
    pub quadruple_velocity: bool,
}

impl MotionSettings {
    fn uses_relative_coordinates(self) -> bool {
        self.attached_coordinates || self.relative_coordinates
    }
}

/// Live displacement supplied by the world service. Only the horizontal
/// suppression flag belongs to the selected player's auxiliary state.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerDisplacement {
    pub world_delta: Vector3,
    pub suppress_horizontal: bool,
}

/// Full direction selection (`$7F:855F`). A self-relative reference uses
/// ordinary heading even when integration still uses relative coordinates.
pub fn generate_velocity(object: &mut Object, owner: ObjectId) {
    let settings = object.extension.path_state.motion;
    let (pitch, yaw) =
        if settings.uses_relative_coordinates() && object.extension.parent != Some(owner) {
            (
                object.extension.relative_rotation.pitch,
                object.extension.relative_rotation.yaw,
            )
        } else {
            (object.base.pitch, object.base.yaw)
        };
    let scale = if settings.quadruple_velocity {
        ENLARGED_VELOCITY_SCALE
    } else {
        ORDINARY_VELOCITY_SCALE
    };
    object.base.velocity = direction_velocity(pitch, yaw, object.base.speed, scale);
}

/// SETVEL (`$7F:854A`) defers regeneration only when the per-step gate is on.
pub fn set_speed(object: &mut Object, owner: ObjectId, speed: u8) {
    object.base.speed = speed;
    if !object
        .extension
        .path_state
        .motion
        .generate_velocity_each_step
    {
        generate_velocity(object, owner);
    }
}

/// Movement through the callback boundary (`$7F:9DE8..9E70`). Acceleration
/// regenerates before bank turning unless per-step regeneration is enabled;
/// the latter happens after bank turning and selected-player displacement.
pub fn before_callbacks(object: &mut Object, owner: ObjectId, player: PlayerDisplacement) {
    let settings = object.extension.path_state.motion;
    if object.base.acceleration != 0 {
        accelerate(
            &mut object.base.speed,
            object.base.target_speed,
            &mut object.base.acceleration,
        );
        if !settings.generate_velocity_each_step {
            generate_velocity(object, owner);
        }
    }
    if settings.bank_turn {
        object.base.yaw = bank_turn(object.base.yaw, object.base.roll);
    }
    if settings.follow_player_displacement {
        follow_selected_displacement(
            &mut object.base.position,
            player.world_delta,
            player.suppress_horizontal,
        );
    }
    if settings.generate_velocity_each_step {
        generate_velocity(object, owner);
    }
    if !settings.uses_relative_coordinates() {
        integrate(&mut object.base.position, object.base.velocity);
    }
}

/// `$7F:9E8B..9EC7`: call AFTER callbacks and any required child refresh.
/// Resample both the integration flags and velocity; callbacks can change
/// either, including enabling a second integration in the same invocation.
pub fn integrate_relative_after_callbacks(object: &mut Object) {
    if object
        .extension
        .path_state
        .motion
        .uses_relative_coordinates()
    {
        integrate(
            &mut object.extension.relative_position,
            object.base.velocity,
        );
    }
}

/// Complete movement after the callback batch has finished. Selection must
/// be resolved at this boundary because a callback can switch players.
pub fn after_callbacks(
    objects: &mut ObjectStore,
    owner: ObjectId,
    selected: Option<&mut super::platform_carry::CarriedPlayer>,
) -> Result<(), super::attachments::AttachmentError> {
    super::attachments::refresh_after_callbacks(objects, owner)?;
    let actor = objects.get_mut(owner).expect("validated movement actor");
    integrate_relative_after_callbacks(actor);
    super::platform_carry::after_callbacks(actor, owner, selected);
    clear_exit_latches(actor);
    Ok(())
}

/// Common footer (`$7F:9EFD`), also reached by END without movement.
pub fn clear_exit_latches(actor: &mut Object) {
    actor.base.contacts.new_contact_latched = false;
    actor.extension.path_state.clear_on_path_exit_latch = false;
    actor.base.contacts.hit_by_primary = false;
    actor.base.contacts.hit_by_secondary = false;
}

/// Accelerate using the source's signed *byte subtraction* tests. Testing
/// widened integers or saturating the intermediate result is not equivalent
/// when the authored speed or acceleration crosses a byte boundary.
///
/// Reaching the target also disables acceleration. A zero acceleration does
/// nothing, even when the current speed differs from the target.
pub fn accelerate(speed: &mut u8, target: u8, acceleration: &mut u8) {
    if *acceleration == 0 {
        return;
    }
    let below = (speed.wrapping_sub(target) as i8) < 0;
    let next = if below {
        speed.wrapping_add(*acceleration)
    } else {
        speed.wrapping_sub(*acceleration)
    };
    let still_below = (next.wrapping_sub(target) as i8) < 0;
    if below == still_below {
        *speed = next;
    } else {
        *speed = target;
        *acceleration = 0;
    }
}

/// Two signed halves rounded toward zero (`$7F:9E29..9E3C`).
pub fn bank_turn(yaw: Angle, roll: Angle) -> Angle {
    yaw.wrapping_add((roll.units() as i8) / BANK_TURN_DIVISOR)
}

/// Byte-by-byte signed products, retaining the source's intermediate
/// truncation and doubled-byte overflow. Pitch is positive; yaw is negated.
pub fn direction_velocity(pitch: Angle, yaw: Angle, speed: u8, scale: i16) -> Vector3 {
    use sf_core::snes_trig::{mulslog_mac8, COSTAB, SINTAB};
    let yaw = usize::from(yaw.units().wrapping_neg());
    let pitch = usize::from(pitch.units());
    let speed = speed as i8;
    Vector3 {
        x: i16::from(mulslog_mac8(
            mulslog_mac8(speed, SINTAB[yaw]),
            COSTAB[pitch],
        ))
        .wrapping_mul(scale),
        y: i16::from(mulslog_mac8(speed, SINTAB[pitch])).wrapping_mul(scale),
        z: i16::from(mulslog_mac8(
            mulslog_mac8(speed, COSTAB[yaw]),
            COSTAB[pitch],
        ))
        .wrapping_mul(scale),
    }
}

pub fn integrate(position: &mut Vector3, velocity: Vector3) {
    position.x = position.x.wrapping_add(velocity.x);
    position.y = position.y.wrapping_add(velocity.y);
    position.z = position.z.wrapping_add(velocity.z);
}

/// Selected-player world displacement (`$7F:9F16`). Source auxiliary flag
/// bit 2 suppresses horizontal compensation only; depth still advances.
/// Vertical player displacement is never copied by this operation.
pub fn follow_selected_displacement(
    position: &mut Vector3,
    selected_displacement: Vector3,
    suppress_horizontal: bool,
) {
    if !suppress_horizontal {
        position.x = position.x.wrapping_add(selected_displacement.x);
    }
    position.z = position.z.wrapping_add(selected_displacement.z);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, ObjectKind, ObjectStore, Rotation, ShapeId};

    fn setup() -> (Object, ObjectId) {
        let object = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
        let owner = ObjectStore::new().allocate(object.clone()).unwrap();
        (object, owner)
    }

    #[test]
    fn published_motion_captures_all_axes_and_only_mode_class_one_uses_velocity() {
        let (mut player, _) = setup();
        for mode in 0..=u8::MAX {
            player.base.position = Vector3 {
                x: i16::MIN,
                y: 18,
                z: i16::MAX,
            };
            player.base.velocity = Vector3 {
                x: -1,
                y: 273,
                z: -309,
            };
            let displacement = Vector3 {
                x: 719,
                y: -887,
                z: i16::MIN,
            };
            let before = player.clone();
            let published = PublishedPlayerMotion::capture(&player, displacement, mode);
            assert_eq!(published.position, before.base.position);
            assert_eq!(
                published.delta,
                if (16..32).contains(&mode) {
                    before.base.velocity
                } else {
                    displacement
                }
            );
            assert_eq!(player, before);
            player.base.position = Vector3::default();
            player.base.velocity = Vector3::default();
            assert_eq!(published.position, before.base.position);
            assert_ne!(published.delta, player.base.velocity);
        }
    }

    #[test]
    fn regeneration_order_distinguishes_acceleration_and_per_step_modes() {
        for each_step in [false, true] {
            let (mut object, owner) = setup();
            object.base.speed = 12;
            object.base.target_speed = 12;
            object.base.acceleration = 1;
            object.base.roll = Angle::from_units(64);
            object.extension.path_state.motion.bank_turn = true;
            object
                .extension
                .path_state
                .motion
                .generate_velocity_each_step = each_step;
            before_callbacks(&mut object, owner, PlayerDisplacement::default());
            assert_eq!(
                (object.base.speed, object.base.acceleration, object.base.yaw),
                (12, 0, Angle::from_units(16))
            );
            let expected = direction_velocity(
                Angle::ZERO,
                if each_step {
                    Angle::from_units(16)
                } else {
                    Angle::ZERO
                },
                12,
                1,
            );
            assert_eq!(object.base.velocity, expected);
            assert_eq!(object.base.position, expected);
        }
    }

    #[test]
    fn absent_acceleration_preserves_velocity_unless_per_step_generation_is_on() {
        for each_step in [false, true] {
            let (mut object, owner) = setup();
            object.base.speed = 40;
            object.base.velocity = Vector3 { x: 7, y: -8, z: 9 };
            object
                .extension
                .path_state
                .motion
                .generate_velocity_each_step = each_step;
            before_callbacks(&mut object, owner, PlayerDisplacement::default());
            let expected = if each_step {
                direction_velocity(Angle::ZERO, Angle::ZERO, 40, 1)
            } else {
                Vector3 { x: 7, y: -8, z: 9 }
            };
            assert_eq!(object.base.position, expected);
        }
    }

    #[test]
    fn relative_velocity_selects_local_angles_unless_reference_is_self() {
        for attached in [false, true] {
            for relative in [false, true] {
                for self_reference in [false, true] {
                    for enlarged in [false, true] {
                        let (mut object, owner) = setup();
                        object.base.speed = 63;
                        object.base.pitch = Angle::from_units(12);
                        object.base.yaw = Angle::from_units(25);
                        object.extension.relative_rotation = Rotation {
                            pitch: Angle::from_units(128),
                            yaw: Angle::from_units(64),
                            roll: Angle::from_units(5),
                        };
                        object.extension.parent = self_reference.then_some(owner);
                        object.extension.path_state.motion = MotionSettings {
                            attached_coordinates: attached,
                            relative_coordinates: relative,
                            quadruple_velocity: enlarged,
                            ..MotionSettings::default()
                        };
                        generate_velocity(&mut object, owner);
                        let (pitch, yaw) = if (attached || relative) && !self_reference {
                            (Angle::from_units(128), Angle::from_units(64))
                        } else {
                            (object.base.pitch, object.base.yaw)
                        };
                        assert_eq!(
                            object.base.velocity,
                            direction_velocity(pitch, yaw, 63, if enlarged { 4 } else { 1 })
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn relative_integration_reads_post_callback_flags_and_velocity() {
        for relative_before in [false, true] {
            for relative_after in [false, true] {
                let (mut object, owner) = setup();
                let before = Vector3 { x: 3, y: 4, z: 5 };
                let after = Vector3 {
                    x: -6,
                    y: -7,
                    z: -8,
                };
                object.base.velocity = before;
                object.extension.path_state.motion.relative_coordinates = relative_before;
                before_callbacks(&mut object, owner, PlayerDisplacement::default());
                assert_eq!(
                    object.base.position,
                    if relative_before {
                        Vector3::default()
                    } else {
                        before
                    }
                );
                assert_eq!(object.extension.relative_position, Vector3::default());
                object.base.velocity = after;
                object.extension.path_state.motion.relative_coordinates = relative_after;
                integrate_relative_after_callbacks(&mut object);
                assert_eq!(
                    object.extension.relative_position,
                    if relative_after {
                        after
                    } else {
                        Vector3::default()
                    }
                );
            }
        }
    }

    #[test]
    fn displacement_is_independent_of_velocity_regeneration_and_relative_coordinates() {
        let (mut object, owner) = setup();
        object.extension.path_state.motion.relative_coordinates = true;
        object
            .extension
            .path_state
            .motion
            .follow_player_displacement = true;
        object.base.velocity = Vector3 { x: 1, y: 2, z: 3 };
        before_callbacks(
            &mut object,
            owner,
            PlayerDisplacement {
                world_delta: Vector3 {
                    x: 40,
                    y: 50,
                    z: 60,
                },
                suppress_horizontal: true,
            },
        );
        assert_eq!(object.base.position, Vector3 { x: 0, y: 0, z: 60 });
        integrate_relative_after_callbacks(&mut object);
        assert_eq!(
            object.extension.relative_position,
            Vector3 { x: 1, y: 2, z: 3 }
        );
    }

    #[test]
    fn speed_write_defers_velocity_only_in_per_step_mode() {
        for each_step in [false, true] {
            let (mut object, owner) = setup();
            let retained = Vector3 { x: 1, y: 2, z: 3 };
            object.base.velocity = retained;
            object
                .extension
                .path_state
                .motion
                .generate_velocity_each_step = each_step;
            set_speed(&mut object, owner, 40);
            assert_eq!(object.base.speed, 40);
            assert_eq!(
                object.base.velocity,
                if each_step {
                    retained
                } else {
                    direction_velocity(Angle::ZERO, Angle::ZERO, 40, 1)
                }
            );
        }
    }

    #[test]
    fn acceleration_stops_on_crossing_but_not_on_an_exact_landing_from_above() {
        let mut speed = 10;
        let mut amount = 3;
        accelerate(&mut speed, 12, &mut amount);
        assert_eq!((speed, amount), (12, 0));
        speed = 15;
        amount = 3;
        accelerate(&mut speed, 12, &mut amount);
        assert_eq!((speed, amount), (12, 3));
        accelerate(&mut speed, 12, &mut amount);
        assert_eq!((speed, amount), (12, 0));
    }

    #[test]
    fn acceleration_uses_wrapped_byte_differences_and_intermediates() {
        for (initial, target, amount, expected) in [
            (0, 255, 1, (255, 1)),
            (255, 0, 1, (0, 0)),
            (0, 128, 1, (1, 1)),
            (127, 128, 255, (126, 255)),
            (64, 64, 0, (64, 0)),
        ] {
            let mut speed = initial;
            let mut acceleration = amount;
            accelerate(&mut speed, target, &mut acceleration);
            assert_eq!((speed, acceleration), expected);
        }
    }

    #[test]
    fn bank_turn_rounds_negative_roll_toward_zero_and_wraps_yaw() {
        for roll in i8::MIN..=i8::MAX {
            let turned = bank_turn(Angle::from_units(255), Angle::from_units(roll as u8));
            assert_eq!(turned.units(), 255_u8.wrapping_add((roll / 4) as u8));
        }
    }

    #[test]
    fn velocity_keeps_two_truncations_and_signed_byte_speed_overflow() {
        assert_eq!(
            direction_velocity(Angle::ZERO, Angle::ZERO, 63, 4),
            Vector3 { x: 0, y: 0, z: 244 }
        );
        assert_eq!(
            direction_velocity(Angle::ZERO, Angle::ZERO, 128, 4),
            Vector3::default()
        );
        assert_eq!(
            direction_velocity(Angle::ZERO, Angle::from_units(64), 63, 4),
            Vector3 {
                x: -244,
                y: 0,
                z: 0
            }
        );
    }

    #[test]
    fn position_additions_wrap_and_selected_displacement_never_changes_altitude() {
        let mut position = Vector3 {
            x: i16::MAX,
            y: 10,
            z: i16::MIN,
        };
        integrate(
            &mut position,
            Vector3 {
                x: 1,
                y: -20,
                z: -1,
            },
        );
        assert_eq!(
            position,
            Vector3 {
                x: i16::MIN,
                y: -10,
                z: i16::MAX
            }
        );
        let displacement = Vector3 {
            x: 13,
            y: 25,
            z: 10,
        };
        follow_selected_displacement(&mut position, displacement, true);
        assert_eq!(
            position,
            Vector3 {
                x: i16::MIN,
                y: -10,
                z: -32759
            }
        );
        follow_selected_displacement(&mut position, displacement, false);
        assert_eq!(
            position,
            Vector3 {
                x: -32755,
                y: -10,
                z: -32749
            }
        );
    }
    #[test]
    fn post_callback_services_publish_children_integrate_locals_carry_and_clear_latches() {
        let (mut parent, _) = setup();
        parent.base.position.x = 200;
        parent.base.velocity.x = 20;
        parent.extension.relative_position.x = 40;
        parent.extension.path_state.motion.refresh_child_chain = true;
        parent.extension.path_state.motion.relative_coordinates = true;
        parent.extension.path_state.motion.carry_selected_player = true;
        parent.extension.path_state.motion_delta.x = 1;
        parent.extension.path_state.clear_on_path_exit_latch = true;
        parent.base.contacts.new_contact_latched = true;
        parent.base.contacts.hit_by_primary = true;
        parent.base.contacts.hit_by_secondary = true;
        parent.base.contacts.pending_hit = true;
        let mut objects = ObjectStore::new();
        let owner = objects.allocate(parent).unwrap();
        let (mut child, _) = setup();
        child.base.attachment = Some(owner);
        child.extension.relative_position.x = 100;
        let child = objects.allocate(child).unwrap();
        objects.get_mut(owner).unwrap().base.first_child = Some(child);
        let mut selected = super::super::platform_carry::CarriedPlayer {
            enabled: true,
            carrier: Some(owner),
            ..Default::default()
        };
        after_callbacks(&mut objects, owner, Some(&mut selected)).unwrap();
        let actor = objects.get(owner).unwrap();
        assert_eq!(objects.get(child).unwrap().base.position.x, 299);
        assert_eq!(
            objects.get(child).unwrap().extension.relative_position.x,
            100
        );
        assert_eq!(actor.extension.relative_position.x, 60);
        assert_eq!(actor.base.position.x, 200);
        assert_eq!(selected.origin.x, 200);
        assert!(!actor.base.contacts.new_contact_latched);
        assert!(!actor.base.contacts.hit_by_primary);
        assert!(!actor.base.contacts.hit_by_secondary);
        assert!(!actor.extension.path_state.clear_on_path_exit_latch);
        assert!(actor.base.contacts.pending_hit);
    }

    #[test]
    fn suppressed_child_refresh_does_not_suppress_relative_integration() {
        let (mut object, owner) = setup();
        object.extension.path_state.motion.refresh_child_chain = true;
        object.extension.path_state.motion.suppress_child_refresh = true;
        object.extension.path_state.motion.relative_coordinates = true;
        object.base.first_child = Some(owner); // Would diagnose a cycle if followed.
        object.base.velocity.x = 7;
        let mut objects = ObjectStore::new();
        assert_eq!(objects.allocate(object), Some(owner));
        after_callbacks(&mut objects, owner, None).unwrap();
        assert_eq!(objects.get(owner).unwrap().extension.relative_position.x, 7);
    }
}

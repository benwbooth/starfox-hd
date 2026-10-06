//! Linked protection-effect maintenance (`$07:F58D..F5EB`). The effect reads
//! its attachment owner, independently of the path-selected player.

use super::collision_surface::SurfaceMode;
use super::path_fields::{ByteField, ByteOperand};
use super::{Object, ObjectId, ObjectStore};

const DEFLECTION_COUNT_MASK: u8 = 0x1F;
const EXPIRY_CLEAR_LATCH: u8 = 0x20;
const PROJECTILE_DEFLECTION: u8 = 0x40;
const MINIMUM_CONTROL: u8 = 1;
const ABOVE_MINIMUM_MASK: u8 = !MINIMUM_CONTROL;
const SPIN_PITCH: i8 = 8;
const SPIN_ROLL: i8 = 6;
const PHASE_HIGH_MASK: u16 = 0xFF00;
const SURFACE_MODE_MASK: u8 = 0x07;
const COUNTDOWN_PERIOD_MASK: u8 = 0x07;
pub(crate) const SPECIAL_PLAYER_CONFIGURATION: u8 = 9;

/// Authored player-protection control (auxiliary 6C02). All bits survive
/// ordinary observation; the source's minimum override replaces the full
/// control byte, not just its five-bit deflection count.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DeflectionProtection(u8);

impl DeflectionProtection {
    pub const fn from_control(control: u8) -> Self {
        Self(control)
    }
    pub const fn control(self) -> u8 {
        self.0
    }
    pub const fn remaining(self) -> u8 {
        self.0 & DEFLECTION_COUNT_MASK
    }
    /// Bit cleared when the effect observes zero protection. Its producer
    /// is not established here; it is not proof that a visual object exists.
    pub const fn expiry_latch(self) -> bool {
        self.0 & EXPIRY_CLEAR_LATCH != 0
    }
    pub const fn projectile_deflection(self) -> bool {
        self.0 & PROJECTILE_DEFLECTION != 0
    }

    /// `$06:9F0D..9F20`, called only after the surrounding player-update
    /// gates admit this service. The shared clock, not effect age, controls
    /// each eighth-update decrement; zero never borrows into upper flags.
    pub fn advance_countdown(&mut self, strategy_clock: u8) {
        if self.remaining() != 0 && strategy_clock & COUNTDOWN_PERIOD_MASK == 0 {
            self.0 = self.0.wrapping_sub(1);
        }
    }

    fn refresh_effect(&mut self, force_minimum: bool) -> u8 {
        if force_minimum && self.0 & ABOVE_MINIMUM_MASK != 0 {
            self.0 = MINIMUM_CONTROL;
        }
        let remaining = self.remaining();
        if remaining == 0 {
            self.0 &= !EXPIRY_CLEAR_LATCH;
        }
        remaining
    }
}

/// Shared linked-effect spawn activity (1DDF), not a per-effect timer.
/// Source linked-effect producers store one; paths retain the complete byte
/// when copying it into an authored variable and consume it by storing zero.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LinkedEffectActivity {
    pub recent_spawn: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityCommand {
    CopyTo(ByteField),
    Assign(ByteOperand),
}

impl LinkedEffectActivity {
    pub fn apply(&mut self, actor: &mut Object, command: ActivityCommand) {
        match command {
            ActivityCommand::CopyTo(field) => field.write(actor, self.recent_spawn),
            ActivityCommand::Assign(value) => self.recent_spawn = value.read(actor),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProtectionRules {
    // Each gate is independently optional: source branches deliberately
    // skip later observations, so absence must not become a guessed false.
    /// Shared character/mode 1DE2 equals nine, as in player-contact rules.
    pub special_character: Option<bool>,
    /// Shared action gate 1D72 is nonzero.
    pub blocked: Option<bool>,
    /// Shared refresh override 1E0D bit 01.
    pub minimum_override: Option<bool>,
    /// Shared D7F4 low byte is nonzero, the player-contact enable gate.
    pub contacts_enabled: Option<bool>,
}

pub struct LinkedProtection<'a> {
    pub owner: ObjectId,
    pub state: &'a mut DeflectionProtection,
}

pub struct PathProtection<'a> {
    pub rules: ProtectionRules,
    pub linked: Option<LinkedProtection<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionError {
    MissingActor(ObjectId),
    MissingPlayerConfiguration,
    MissingSurfaceMode,
    MissingActionGate,
    MissingMinimumOverride,
    MissingContactEnable,
    MissingAttachment,
    MissingLinkedProtection,
    WrongLinkedOwner {
        expected: ObjectId,
        supplied: ObjectId,
    },
}

/// Returns true for the inline routine's ordinary return, false for its
/// flicker-animation continuation (remaining count zero or one). No clock
/// advances here: surrounding player control owns protection decay.
pub fn update_effect(
    objects: &mut ObjectStore,
    owner: ObjectId,
    input: &mut PathProtection<'_>,
    surface_mode: Option<SurfaceMode>,
) -> Result<bool, ProtectionError> {
    let actor = objects
        .get_mut(owner)
        .ok_or(ProtectionError::MissingActor(owner))?;
    // The source rotates before any gate or linked-player read. Preserve
    // this completed prefix when a later observation is unavailable.
    actor.extension.relative_rotation.pitch = actor
        .extension
        .relative_rotation
        .pitch
        .wrapping_add(SPIN_PITCH);
    actor.extension.relative_rotation.roll = actor
        .extension
        .relative_rotation
        .roll
        .wrapping_add(SPIN_ROLL);
    // The character-mode branch skips the surface-mode read altogether.
    let gated_scene = input
        .rules
        .special_character
        .ok_or(ProtectionError::MissingPlayerConfiguration)?
        || surface_mode
            .ok_or(ProtectionError::MissingSurfaceMode)?
            .flags
            & SURFACE_MODE_MASK
            == 0;
    let suppressed = gated_scene
        && input
            .rules
            .blocked
            .ok_or(ProtectionError::MissingActionGate)?;
    let attachment = actor.base.attachment;
    let remaining = if suppressed {
        // This early exit never resolves or modifies the attached player.
        0
    } else {
        let linked_owner = attachment.ok_or(ProtectionError::MissingAttachment)?;
        objects
            .get(linked_owner)
            .ok_or(ProtectionError::MissingActor(linked_owner))?;
        let linked = input
            .linked
            .as_mut()
            .ok_or(ProtectionError::MissingLinkedProtection)?;
        if linked.owner != linked_owner {
            return Err(ProtectionError::WrongLinkedOwner {
                expected: linked_owner,
                supplied: linked.owner,
            });
        }
        let minimum_override = input
            .rules
            .minimum_override
            .ok_or(ProtectionError::MissingMinimumOverride)?;
        // Override takes the refresh branch without reading the contact gate.
        let refresh = minimum_override
            || !input
                .rules
                .contacts_enabled
                .ok_or(ProtectionError::MissingContactEnable)?;
        linked.state.refresh_effect(refresh)
    };
    let actor = objects.get_mut(owner).expect("validated protection effect");
    actor.extension.path_state.motion_phase =
        (actor.extension.path_state.motion_phase & PHASE_HIGH_MASK) | u16::from(remaining);
    Ok(remaining & ABOVE_MINIMUM_MASK != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Angle, Behavior, ObjectKind, ShapeId, Vector3};

    fn setup() -> (ObjectStore, ObjectId, ObjectId) {
        let mut objects = ObjectStore::new();
        let player = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let mut effect = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
        effect.base.attachment = Some(player);
        effect.base.position = Vector3 {
            x: -123,
            y: 456,
            z: -789,
        };
        effect.base.velocity = Vector3 { x: 7, y: -8, z: 9 };
        effect.extension.path_state.motion_phase = 0xD3A5;
        effect.extension.relative_rotation.yaw = Angle::from_units(103);
        let owner = objects.allocate(effect).unwrap();
        (objects, owner, player)
    }

    #[test]
    fn every_protection_byte_and_gate_preserves_unrelated_state_and_wrapped_spin() {
        let (template, owner, player) = setup();
        for control in 0..=u8::MAX {
            let protection = DeflectionProtection::from_control(control);
            assert_eq!(protection.remaining(), control & 31);
            assert_eq!(protection.expiry_latch(), control & 32 != 0);
            assert_eq!(protection.projectile_deflection(), control & 64 != 0);
            for mode in [0, 1, 2, 7, 8, 255] {
                for flags in 0..16 {
                    let rules = ProtectionRules {
                        special_character: Some(flags & 1 != 0),
                        blocked: Some(flags & 2 != 0),
                        minimum_override: Some(flags & 4 != 0),
                        contacts_enabled: Some(flags & 8 != 0),
                    };
                    let suppressed = (flags & 1 != 0 || mode & 7 == 0) && flags & 2 != 0;
                    let mut expected_control = control;
                    let remaining = if suppressed {
                        0
                    } else {
                        if (flags & 4 != 0 || flags & 8 == 0) && control >= 2 {
                            expected_control = 1;
                        }
                        let count = expected_control & 31;
                        if count == 0 {
                            expected_control &= 223;
                        }
                        count
                    };
                    let mut objects = template.clone();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.extension.relative_rotation.pitch = Angle::from_units(control);
                    actor.extension.relative_rotation.roll = Angle::from_units(control ^ 0xA5);
                    let mut expected = objects.clone();
                    let actor = expected.get_mut(owner).unwrap();
                    actor.extension.relative_rotation.pitch =
                        Angle::from_units(((u16::from(control) + 8) % 256) as u8);
                    actor.extension.relative_rotation.roll =
                        Angle::from_units(((u16::from(control ^ 0xA5) + 6) % 256) as u8);
                    actor.extension.path_state.motion_phase = 0xD300 + u16::from(remaining);
                    let mut actual = protection;
                    assert_eq!(
                        update_effect(
                            &mut objects,
                            owner,
                            &mut PathProtection {
                                rules,
                                linked: Some(LinkedProtection {
                                    owner: player,
                                    state: &mut actual
                                })
                            },
                            Some(SurfaceMode { flags: mode })
                        ),
                        Ok(remaining >= 2)
                    );
                    assert_eq!(objects, expected);
                    assert_eq!(actual.control(), expected_control);
                }
            }
        }
    }

    #[test]
    fn countdown_uses_global_clock_phase_without_borrowing_into_flags() {
        for control in 0..=u8::MAX {
            for clock in 0..=u8::MAX {
                let mut protection = DeflectionProtection::from_control(control);
                let expected = if control % 32 != 0 && clock % 8 == 0 {
                    control - 1
                } else {
                    control
                };
                protection.advance_countdown(clock);
                assert_eq!(protection.control(), expected);
                assert_eq!(protection.control() & 0xE0, control & 0xE0);
            }
        }
    }

    #[test]
    fn suppressed_scene_short_circuits_attachment_and_character_mode_skips_surface_input() {
        for special_character in [false, true] {
            let (mut objects, owner, _) = setup();
            objects.get_mut(owner).unwrap().base.attachment = None;
            let mut expected = objects.clone();
            let actor = expected.get_mut(owner).unwrap();
            actor.extension.relative_rotation.pitch = Angle::from_units(8);
            actor.extension.relative_rotation.roll = Angle::from_units(6);
            actor.extension.path_state.motion_phase = 0xD300;
            let mut input = PathProtection {
                rules: ProtectionRules {
                    special_character: Some(special_character),
                    blocked: Some(true),
                    ..Default::default()
                },
                linked: None,
            };
            assert_eq!(
                update_effect(
                    &mut objects,
                    owner,
                    &mut input,
                    (!special_character).then_some(SurfaceMode { flags: 8 })
                ),
                Ok(false)
            );
            assert_eq!(objects, expected);
        }
    }

    #[test]
    fn missing_or_wrong_linked_inputs_preserve_only_the_completed_spin_prefix() {
        for failure in 0..5 {
            let (mut objects, owner, player) = setup();
            let mut protection = DeflectionProtection::from_control(255);
            if failure == 1 {
                objects.get_mut(owner).unwrap().base.attachment = None;
            }
            if failure == 4 {
                objects.remove(player);
                objects.get_mut(owner).unwrap().base.attachment = Some(player);
            }
            let mut expected = objects.clone();
            let actor = expected.get_mut(owner).unwrap();
            actor.extension.relative_rotation.pitch = Angle::from_units(8);
            actor.extension.relative_rotation.roll = Angle::from_units(6);
            let mut input = PathProtection {
                rules: ProtectionRules {
                    special_character: Some(false),
                    ..Default::default()
                },
                linked: if failure == 2 {
                    None
                } else {
                    Some(LinkedProtection {
                        owner: if failure == 3 { owner } else { player },
                        state: &mut protection,
                    })
                },
            };
            assert_eq!(
                update_effect(
                    &mut objects,
                    owner,
                    &mut input,
                    (failure != 0).then_some(SurfaceMode { flags: 1 })
                ),
                Err(match failure {
                    0 => ProtectionError::MissingSurfaceMode,
                    1 => ProtectionError::MissingAttachment,
                    2 => ProtectionError::MissingLinkedProtection,
                    3 => ProtectionError::WrongLinkedOwner {
                        expected: player,
                        supplied: owner
                    },
                    _ => ProtectionError::MissingActor(player),
                })
            );
            assert_eq!(objects, expected);
            assert_eq!(protection.control(), 255);
        }
    }

    #[test]
    fn missing_gates_follow_source_read_order_and_keep_the_spin_prefix() {
        for failure in 0..6 {
            let (mut objects, owner, player) = setup();
            let mut protection = DeflectionProtection::from_control(255);
            let mut expected = objects.clone();
            let actor = expected.get_mut(owner).unwrap();
            actor.extension.relative_rotation.pitch = Angle::from_units(8);
            actor.extension.relative_rotation.roll = Angle::from_units(6);
            if failure == 3 {
                objects.get_mut(owner).unwrap().base.attachment = None;
                expected.get_mut(owner).unwrap().base.attachment = None;
            }
            let mut input = PathProtection {
                rules: ProtectionRules {
                    special_character: (failure != 0).then_some(failure == 2),
                    blocked: None,
                    minimum_override: (failure == 5).then_some(false),
                    contacts_enabled: None,
                },
                linked: Some(LinkedProtection {
                    owner: player,
                    state: &mut protection,
                }),
            };
            assert_eq!(
                update_effect(
                    &mut objects,
                    owner,
                    &mut input,
                    (failure >= 3).then_some(SurfaceMode { flags: 1 })
                ),
                Err(match failure {
                    0 => ProtectionError::MissingPlayerConfiguration,
                    1 => ProtectionError::MissingSurfaceMode,
                    2 => ProtectionError::MissingActionGate,
                    3 => ProtectionError::MissingAttachment,
                    4 => ProtectionError::MissingMinimumOverride,
                    _ => ProtectionError::MissingContactEnable,
                })
            );
            assert_eq!(objects, expected);
            assert_eq!(protection.control(), 255);
        }
    }

    #[test]
    fn minimum_override_skips_contact_gate_and_surface_mode_skips_action_gate() {
        let (mut objects, owner, player) = setup();
        let mut protection = DeflectionProtection::from_control(255);
        let mut input = PathProtection {
            rules: ProtectionRules {
                special_character: Some(false),
                minimum_override: Some(true),
                ..Default::default()
            },
            linked: Some(LinkedProtection {
                owner: player,
                state: &mut protection,
            }),
        };
        assert_eq!(
            update_effect(
                &mut objects,
                owner,
                &mut input,
                Some(SurfaceMode { flags: 1 })
            ),
            Ok(false)
        );
        assert_eq!(protection.control(), 1);
    }

    #[test]
    fn shared_activity_keeps_full_byte_and_touches_only_its_named_destination() {
        let (objects, owner, _) = setup();
        let initial = objects.get(owner).unwrap();
        for value in 0..=u8::MAX {
            let mut actor = initial.clone();
            let mut expected = initial.clone();
            let mut activity = LinkedEffectActivity {
                recent_spawn: value,
            };
            expected.extension.texture_scroll_x = value;
            activity.apply(
                &mut actor,
                ActivityCommand::CopyTo(ByteField::TextureScrollX),
            );
            assert_eq!(actor, expected);
            assert_eq!(activity.recent_spawn, value);
            activity.apply(&mut actor, ActivityCommand::Assign(ByteOperand::Literal(0)));
            assert_eq!(activity.recent_spawn, 0);
            activity.apply(
                &mut actor,
                ActivityCommand::Assign(ByteOperand::Actor(ByteField::TextureScrollX)),
            );
            assert_eq!(activity.recent_spawn, value);
            assert_eq!(actor, expected);
        }
    }
}

//! Player hit-state leaves (`$06:91AC..9235`, `$06:9824..989F`,
//! `$06:AA6F..AB2C`). Timers count strategy visits, not presentation frames.
//! The player callback owns contact selection, reflection and sound dispatch.

use super::collision_pass::ActorContacts;
use super::hit_response::health_after_damage;
use super::{Angle, ObjectFlags, Vector3};

const COUNT_MASK: u8 = 0x3F;
const TAG_MASK: u8 = 0xC0;
const HIT_BLINK_MASK: u8 = 0x03;
const IMPACT_FEEDBACK_MASK: u8 = 0x70;
const DEFLECTION_FEEDBACK_MASK: u8 = 0x08;
const SOUND_RANDOM_MASK: u8 = 0x07;
const BANK_IMPULSE: i8 = 30;
const TURN_AWAY: i8 = 64;
const HEAVY_SOUND_THRESHOLD: u8 = 4;
// Recovery count, feedback duration, and initial camera recoil respectively.
const LIGHT_IMPACT_PROFILE: (u8, u8, i16) = (4, 2, 96);
const HEAVY_IMPACT_PROFILE: (u8, u8, i16) = (10, 8, 128);
const DEFLECTION_FEEDBACK_DURATION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Impact {
    Light,
    Heavy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactSound {
    LightImpact,
    HeavyImpact,
    Deflection,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerHitControl {
    /// Low six bits count visits; upper tags survive countdown. Contact
    /// gating and light-impact rejection test the WHOLE value for zero.
    pub recovery: u8,
    pub secondary_protection: u8,
    pub hold_secondary_protection: bool,
    pub feedback_duration: u8,
    /// Requests to the player feedback owner, not collision-box flags.
    pub feedback_flags: u8,
    pub camera_pitch_recoil: i16,
    pub bank_impulse: i8,
    pub impact_latched: bool,
    pub impact_variant: bool,
    pub reserve_shield: u8,
    pub deflection_sound_cooldown: u8,
}

impl PlayerHitControl {
    /// Collision-disable follows recovery only. Contact suppression also
    /// follows pause and secondary protection. The clock controls marking,
    /// NOT whether recovery decrements; unmarked phases do not clear a mark.
    pub fn advance_contact_filters(
        &mut self,
        contacts: &mut ActorContacts,
        flags: &mut ObjectFlags,
        hit_marked: &mut bool,
        strategy_clock: u8,
        paused: bool,
    ) {
        let suppress_recovery = if self.recovery & COUNT_MASK == 0 {
            false
        } else {
            if strategy_clock & HIT_BLINK_MASK != 0 {
                *hit_marked = true;
            }
            self.recovery = self.recovery.wrapping_sub(1) | (self.recovery & TAG_MASK);
            self.recovery != 0
        };
        flags.collision_disabled = suppress_recovery;
        contacts.suppress_contacts_next_epoch = suppress_recovery;
        if paused {
            contacts.suppress_contacts_next_epoch = true;
        } else if self.secondary_protection & COUNT_MASK != 0 {
            let remaining = (self.secondary_protection & COUNT_MASK) - 1;
            self.secondary_protection = (self.secondary_protection & TAG_MASK) | remaining;
            contacts.suppress_contacts_next_epoch = true;
        } else if self.hold_secondary_protection {
            // Reload replaces upper tags as well.
            self.secondary_protection = COUNT_MASK;
            contacts.suppress_contacts_next_epoch = true;
        }
    }

    /// Light impacts during recovery have NO effects. Heavy impacts restart
    /// recovery and replace its tags; neither overwrites nonzero recoil.
    pub fn impact(&mut self, impact: Impact, strategy_clock: u8) -> bool {
        let (recovery, feedback, recoil) = match impact {
            Impact::Light if self.recovery != 0 => return false,
            Impact::Light => LIGHT_IMPACT_PROFILE,
            Impact::Heavy => HEAVY_IMPACT_PROFILE,
        };
        self.recovery = recovery;
        self.feedback_duration = feedback;
        self.feedback_flags |= IMPACT_FEEDBACK_MASK;
        if self.camera_pitch_recoil == 0 {
            self.camera_pitch_recoil = recoil;
        }
        self.impact_latched = true;
        self.impact_variant = false;
        self.bank_impulse = if strategy_clock & 1 == 0 {
            BANK_IMPULSE
        } else {
            -BANK_IMPULSE
        };
        true
    }

    pub fn request_deflection_feedback(&mut self) {
        self.feedback_duration = DEFLECTION_FEEDBACK_DURATION;
        self.feedback_flags |= DEFLECTION_FEEDBACK_MASK;
    }

    /// The caller queues the side-specific cue BEFORE consuming the shared
    /// random byte. Skipped cues consume no random state. Zero is valid.
    pub fn set_deflection_sound_cooldown(&mut self, random_byte: u8) {
        self.deflection_sound_cooldown = random_byte & SOUND_RANDOM_MASK;
    }

    pub fn deflection_sound_due(&self) -> bool {
        self.deflection_sound_cooldown == 0
    }

    /// Nonempty reserve absorbs the whole contact, even if incoming damage
    /// exceeds it. Signed byte-result clamping is retained; no damage spills.
    pub fn absorb_with_reserve(&mut self, damage: &mut u8) {
        if self.reserve_shield != 0 {
            self.reserve_shield = health_after_damage(self.reserve_shield, *damage);
            *damage = 0;
        }
    }
}

pub fn impact_sound(damage: u8) -> ContactSound {
    if (damage.wrapping_sub(HEAVY_SOUND_THRESHOLD) as i8) >= 0 {
        ContactSound::HeavyImpact
    } else {
        ContactSound::LightImpact
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContactTurn {
    pub target_yaw: Angle,
    pub yaw_impulse: i8,
    pub bank_target: i8,
}

/// Invoke after player-mode and other-class suppression gates. Bearing is
/// the negated HIGH BYTE of the fine angle, not high byte of its negation.
pub fn turn_from_contact(position: Vector3, yaw: Angle, other: Vector3) -> ContactTurn {
    let target = sf_core::aim_angle::sf2_yaw_to_target(
        other.x.wrapping_sub(position.x),
        other.z.wrapping_sub(position.z),
    );
    let impulse = if (yaw.units().wrapping_sub(target) as i8) < 0 {
        -TURN_AWAY
    } else {
        TURN_AWAY
    };
    ContactTurn {
        target_yaw: Angle::from_units(target),
        yaw_impulse: impulse,
        bank_target: impulse / 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_counts_every_visit_but_blink_only_sets_marks() {
        for clock in 0..=u8::MAX {
            for initial in 0..=u8::MAX {
                for marked in [false, true] {
                    let mut state = PlayerHitControl {
                        recovery: initial,
                        ..PlayerHitControl::default()
                    };
                    let mut contacts = ActorContacts::default();
                    let mut flags = ObjectFlags::default();
                    let mut hit_marked = marked;
                    state.advance_contact_filters(
                        &mut contacts,
                        &mut flags,
                        &mut hit_marked,
                        clock,
                        false,
                    );
                    let has_count = initial & COUNT_MASK != 0;
                    let expected = if has_count { initial - 1 } else { initial };
                    assert_eq!(state.recovery, expected);
                    assert_eq!(flags.collision_disabled, has_count && expected != 0);
                    assert_eq!(
                        contacts.suppress_contacts_next_epoch,
                        flags.collision_disabled
                    );
                    assert_eq!(hit_marked, marked || (has_count && clock & 3 != 0));
                }
            }
        }
    }

    #[test]
    fn secondary_protection_and_pause_do_not_disable_collision_queue() {
        for initial in 0..=u8::MAX {
            for paused in [false, true] {
                for hold in [false, true] {
                    let mut state = PlayerHitControl {
                        secondary_protection: initial,
                        hold_secondary_protection: hold,
                        ..PlayerHitControl::default()
                    };
                    let mut contacts = ActorContacts::default();
                    let mut flags = ObjectFlags {
                        collision_disabled: true,
                        ..ObjectFlags::default()
                    };
                    state.advance_contact_filters(&mut contacts, &mut flags, &mut false, 0, paused);
                    let has_count = initial & COUNT_MASK != 0;
                    assert_eq!(
                        state.secondary_protection,
                        if paused {
                            initial
                        } else if has_count {
                            initial - 1
                        } else if hold {
                            63
                        } else {
                            initial
                        }
                    );
                    assert_eq!(
                        contacts.suppress_contacts_next_epoch,
                        paused || has_count || hold
                    );
                    assert!(!flags.collision_disabled);
                }
            }
        }
    }

    #[test]
    fn tagged_zero_differs_between_last_count_and_next_visit() {
        let mut state = PlayerHitControl {
            recovery: 0x81,
            ..PlayerHitControl::default()
        };
        let mut contacts = ActorContacts::default();
        let mut flags = ObjectFlags::default();
        state.advance_contact_filters(&mut contacts, &mut flags, &mut false, 0, false);
        assert_eq!(state.recovery, 0x80);
        assert!(flags.collision_disabled);
        state.advance_contact_filters(&mut contacts, &mut flags, &mut false, 1, false);
        assert_eq!(state.recovery, 0x80);
        assert!(!flags.collision_disabled);
        let before = state;
        assert!(!state.impact(Impact::Light, 0));
        assert_eq!(state, before);
        assert!(state.impact(Impact::Heavy, 0));
        assert_eq!(state.recovery, 10);
    }

    #[test]
    fn impact_preserves_existing_recoil_and_uses_shared_clock_parity() {
        for impact in [Impact::Light, Impact::Heavy] {
            for clock in [0, 1, 254, 255] {
                for recoil in [0, -37, 79] {
                    let mut state = PlayerHitControl {
                        camera_pitch_recoil: recoil,
                        feedback_flags: 0x89,
                        impact_variant: true,
                        ..PlayerHitControl::default()
                    };
                    assert!(state.impact(impact, clock));
                    let (recovery, feedback, new_recoil) = match impact {
                        Impact::Light => (4, 2, 96),
                        Impact::Heavy => (10, 8, 128),
                    };
                    assert_eq!(
                        (state.recovery, state.feedback_duration),
                        (recovery, feedback)
                    );
                    assert_eq!(
                        state.camera_pitch_recoil,
                        if recoil == 0 { new_recoil } else { recoil }
                    );
                    assert_eq!(state.feedback_flags, 0xF9);
                    assert_eq!(state.bank_impulse, if clock & 1 == 0 { 30 } else { -30 });
                    assert!(state.impact_latched && !state.impact_variant);
                }
            }
        }
    }

    #[test]
    fn reserve_absorbs_without_spill_and_retains_signed_byte_edges() {
        for (initial, incoming, reserve, remaining_damage) in [
            (0, 7, 0, 7),
            (2, 7, 0, 0),
            (7, 7, 0, 0),
            (10, 3, 7, 0),
            (255, 0, 0, 0),
            (1, 255, 2, 0),
        ] {
            let mut state = PlayerHitControl {
                reserve_shield: initial,
                ..PlayerHitControl::default()
            };
            let mut damage = incoming;
            state.absorb_with_reserve(&mut damage);
            assert_eq!((state.reserve_shield, damage), (reserve, remaining_damage));
        }
    }

    #[test]
    fn sound_threshold_and_deflection_keep_byte_boundaries() {
        for damage in 0..=u8::MAX {
            assert_eq!(
                impact_sound(damage),
                if (4..132).contains(&damage) {
                    ContactSound::HeavyImpact
                } else {
                    ContactSound::LightImpact
                }
            );
            let mut state = PlayerHitControl {
                feedback_flags: 0x40,
                ..PlayerHitControl::default()
            };
            state.request_deflection_feedback();
            assert_eq!((state.feedback_duration, state.feedback_flags), (1, 0x48));
            state.set_deflection_sound_cooldown(damage);
            assert_eq!(state.deflection_sound_cooldown, damage % 8);
            assert_eq!(state.deflection_sound_due(), damage % 8 == 0);
        }
    }

    #[test]
    fn turning_uses_signed_wrapped_yaw_not_unsigned_comparison() {
        let target = Vector3 {
            x: 10,
            y: 500,
            z: 0,
        };
        for yaw in 0..=u8::MAX {
            let turn = turn_from_contact(Vector3::default(), Angle::from_units(yaw), target);
            assert_eq!(turn.target_yaw.units(), 192);
            assert_eq!(
                turn.yaw_impulse,
                if yaw.wrapping_sub(192) < 128 { 64 } else { -64 }
            );
            assert_eq!(turn.bank_target, turn.yaw_impulse / 2);
        }
    }
}

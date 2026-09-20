//! Path-owned primary-player target controls (`$07:B746..B8C9`) and the
//! linked-player following helpers (`$07:F5EC..F6CF`). The target record is
//! shared player state, not another copy of the invoking actor's extension.

use super::{Angle, ObjectId, Vector3};

const OBJECT_ORIGIN_MODE: u16 = 2;
const TARGET_LIMIT: u16 = 255;
const TARGET_AXIS_MODE: u8 = 3;
const TARGET_CONTROL: u8 = 31;
const CONFIGURED_RATES: [u8; 3] = [4, 8, 8];
const DOUBLED_LOW_BYTE_RATES: [u8; 3] = [1, 2, 2];
const ALTERNATE_AXIS_RATES: [u8; 3] = [6, 6, 3];
const CONFIGURED_LIMITS: [u8; 3] = [31; 3];
const LOCKED_RATES: [u8; 3] = [3, 3, 4];
const LOCKED_LIMITS: [u8; 3] = [16, 16, 31];
const LINKED_TARGET_RANGE: i16 = 8;
const LINKED_FORWARD_OFFSET: i8 = 80;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerTargetControl {
    /// Source control bit 80 blocks target/rate/limit configuration.
    pub configuration_locked: bool,
    /// Source control bit 40 selects separately authored target offsets.
    pub offset_enabled: bool,
    pub mode: u16,
    pub owner: Option<ObjectId>,
    pub origin: Vector3,
    pub range: i16,
    pub positive_range: u16,
    pub limit: u16,
    pub axis_mode: u8,
    pub control: u8,
    pub axis_rates: [u8; 3],
    pub axis_limits: [u8; 3],
}

pub struct PrimaryControl<'a> {
    pub target: &'a mut PlayerTargetControl,
    /// Fresh primary-player auxiliary link flag 80, independent of the
    /// target configuration lock and the path's selected-player context.
    pub linked_mode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerControlCommand {
    Configure(i16),
    ConfigureDoubledLowByte(i16),
    ConfigureAlternateAxes(i16),
    LockForLinkedMode,
    FollowPrimaryPosition,
    RefreshOwnedOrigin,
}

impl PlayerTargetControl {
    pub fn refresh_owned_origin(&mut self, owner: ObjectId, position: Vector3) {
        if self.owner == Some(owner) {
            self.origin = position;
        }
    }

    /// B79F clears the offset flag before checking the lock. B833 is called
    /// even when B79F/B89B/B861 skip their writes, so an existing matching
    /// owner still refreshes its origin while configuration is locked.
    pub fn configure(&mut self, owner: ObjectId, position: Vector3, range: i16) {
        self.configure_with_rates(owner, position, range, CONFIGURED_RATES);
    }

    /// $07:B6EF doubles ONLY the low byte; no carry reaches the high byte.
    pub fn configure_doubled_low_byte(&mut self, owner: ObjectId, position: Vector3, range: i16) {
        let range = ((range as u16 & 0xFF00) | u16::from((range as u8).wrapping_mul(2))) as i16;
        self.configure_with_rates(owner, position, range, DOUBLED_LOW_BYTE_RATES);
    }

    pub fn configure_alternate_axes(&mut self, owner: ObjectId, position: Vector3, range: i16) {
        self.configure_with_rates(owner, position, range, ALTERNATE_AXIS_RATES);
    }

    fn configure_with_rates(
        &mut self,
        owner: ObjectId,
        position: Vector3,
        range: i16,
        rates: [u8; 3],
    ) {
        self.offset_enabled = false;
        if !self.configuration_locked {
            self.mode = OBJECT_ORIGIN_MODE;
            self.origin = position;
            self.limit = TARGET_LIMIT;
            self.range = range;
            self.positive_range = if range < 0 { 1 } else { range as u16 };
            self.owner = Some(owner);
            self.axis_mode = TARGET_AXIS_MODE;
            self.control = TARGET_CONTROL;
            self.axis_rates = rates;
            self.axis_limits = CONFIGURED_LIMITS;
        }
        self.refresh_owned_origin(owner, position);
    }

    pub fn lock_for_linked_mode(&mut self, linked_mode: bool) {
        self.configuration_locked = true;
        self.axis_limits = LOCKED_LIMITS;
        self.axis_rates = LOCKED_RATES;
        if linked_mode {
            self.range = LINKED_TARGET_RANGE;
            self.positive_range = LINKED_TARGET_RANGE as u16;
        }
    }
}

/// The source rotates an 80-unit signed-byte offset through pitch then yaw,
/// including zero angles (no identity shortcut), truncating between stages.
/// In ordinary mode it simply copies all three primary world coordinates.
pub fn primary_position(position: Vector3, pitch: Angle, yaw: Angle, linked_mode: bool) -> Vector3 {
    if !linked_mode {
        return position;
    }
    let (y, depth) = sf_core::snes_trig::rotate_8yz(pitch.units(), 0, LINKED_FORWARD_OFFSET);
    let (x, z) = sf_core::snes_trig::rotate_8xz(yaw.units(), 0, depth as i8);
    Vector3 {
        x: position.x.wrapping_add(x),
        y: position.y.wrapping_add(y),
        z: position.z.wrapping_add(z),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, Object, ObjectKind, ObjectStore, ShapeId};

    fn identities() -> (ObjectId, ObjectId) {
        let mut objects = ObjectStore::new();
        let first = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let second = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        (first, second)
    }

    fn retained(owner: Option<ObjectId>) -> PlayerTargetControl {
        PlayerTargetControl {
            configuration_locked: false,
            offset_enabled: true,
            mode: 17,
            owner,
            origin: Vector3 {
                x: 10,
                y: -20,
                z: 30,
            },
            range: -111,
            positive_range: 222,
            limit: 333,
            axis_mode: 91,
            control: 127,
            axis_rates: [21, 22, 23],
            axis_limits: [51, 52, 53],
        }
    }

    #[test]
    fn configuration_preserves_all_word_ranges_and_locked_ownership_refresh() {
        let (owner, other) = identities();
        let position = Vector3 {
            x: i16::MIN,
            y: i16::MAX,
            z: -1,
        };
        for range in i16::MIN..=i16::MAX {
            for locked in [false, true] {
                for prior_owner in [None, Some(owner), Some(other)] {
                    let mut actual = retained(prior_owner);
                    actual.configuration_locked = locked;
                    let mut expected = actual;
                    expected.offset_enabled = false;
                    if !locked {
                        expected.mode = 2;
                        expected.owner = Some(owner);
                        expected.range = range;
                        expected.positive_range = if range < 0 { 1 } else { range as u16 };
                        expected.limit = 255;
                        expected.axis_mode = 3;
                        expected.control = 31;
                        expected.axis_rates = [4, 8, 8];
                        expected.axis_limits = [31, 31, 31];
                    }
                    if !locked || prior_owner == Some(owner) {
                        expected.origin = position;
                    }
                    actual.configure(owner, position, range);
                    assert_eq!(actual, expected);
                }
            }
        }
    }

    #[test]
    fn configuration_variants_keep_low_byte_doubling_separate_from_word_arithmetic() {
        let (owner, other) = identities();
        let position = Vector3 {
            x: 91,
            y: -72,
            z: 53,
        };
        for encoded in 0..=u16::MAX {
            for doubled in [false, true] {
                let bytes = encoded.to_le_bytes();
                let expected_range = if doubled {
                    i16::from_le_bytes([((u16::from(bytes[0]) * 2) % 256) as u8, bytes[1]])
                } else {
                    encoded as i16
                };
                for locked in [false, true] {
                    for prior_owner in [None, Some(owner), Some(other)] {
                        let mut actual = retained(prior_owner);
                        actual.configuration_locked = locked;
                        let mut expected = actual;
                        expected.configure(owner, position, expected_range);
                        if !locked {
                            expected.axis_rates = if doubled { [1, 2, 2] } else { [6, 6, 3] };
                        }
                        if doubled {
                            actual.configure_doubled_low_byte(owner, position, encoded as i16);
                        } else {
                            actual.configure_alternate_axes(owner, position, encoded as i16);
                        }
                        assert_eq!(actual, expected);
                    }
                }
            }
        }
    }

    #[test]
    fn locking_and_origin_refresh_have_separate_flags_and_ownership_rules() {
        let (owner, other) = identities();
        for locked in [false, true] {
            for offset in [false, true] {
                for linked in [false, true] {
                    let mut actual = retained(Some(owner));
                    actual.configuration_locked = locked;
                    actual.offset_enabled = offset;
                    let mut expected = actual;
                    expected.configuration_locked = true;
                    expected.axis_rates = [3, 3, 4];
                    expected.axis_limits = [16, 16, 31];
                    if linked {
                        expected.range = 8;
                        expected.positive_range = 8;
                    }
                    actual.lock_for_linked_mode(linked);
                    assert_eq!(actual, expected);
                    let position = Vector3 {
                        x: -123,
                        y: 456,
                        z: -789,
                    };
                    actual.refresh_owned_origin(other, position);
                    assert_eq!(actual, expected);
                    actual.refresh_owned_origin(owner, position);
                    expected.origin = position;
                    assert_eq!(actual, expected);
                }
            }
        }
    }

    #[test]
    fn linked_origin_uses_signed_byte_rotations_for_every_pitch_and_yaw() {
        let position = Vector3 {
            x: i16::MAX,
            y: i16::MIN,
            z: -1,
        };
        // Signed multiply-high after doubling the magnitude; source byte
        // products truncate magnitude before applying their sign.
        let product = |a: i8, b: i8| -> i16 {
            let magnitude = (i32::from(a).abs() * 2 * i32::from(b).abs()) >> 8;
            (if (a < 0) != (b < 0) {
                -magnitude
            } else {
                magnitude
            }) as i16
        };
        for pitch in 0..=u8::MAX {
            let y = product(80, sf_core::snes_trig::SINTAB[usize::from(pitch)]);
            let depth = product(80, sf_core::snes_trig::COSTAB[usize::from(pitch)]) as i8;
            for yaw in 0..=u8::MAX {
                let reverse = usize::from(yaw.wrapping_neg());
                let x = product(depth, sf_core::snes_trig::SINTAB[reverse]);
                let z = product(depth, sf_core::snes_trig::COSTAB[reverse]);
                let expected = Vector3 {
                    x: position.x.wrapping_add(x),
                    y: position.y.wrapping_add(y),
                    z: position.z.wrapping_add(z),
                };
                assert_eq!(
                    primary_position(
                        position,
                        Angle::from_units(pitch),
                        Angle::from_units(yaw),
                        true
                    ),
                    expected
                );
                assert_eq!(
                    primary_position(
                        position,
                        Angle::from_units(pitch),
                        Angle::from_units(yaw),
                        false
                    ),
                    position
                );
            }
        }
    }
}

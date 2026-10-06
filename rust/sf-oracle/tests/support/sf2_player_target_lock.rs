//! Unmodified display retention block, including the original sound queue.
//! The bootstrap supplies the already-selected player allocation; execution
//! stops before the following graphics-access/drawing work. No ROM patch,
//! generated expected fixture, or replacement source subroutine is used.

use super::*;
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_program::SelectedAuxiliaryState;
use sf2_game::path_scene_state::EncounterObjectiveCounts;
use sf2_game::path_sound::AuthoredCue;
use sf2_game::path_target::{PublishedHomingTarget, TargetingUpgradeState};
use sf2_game::player_target_lock::{self, TargetLock, TargetLockError, TargetReticle};
use sf2_game::scene_path_world::PlayerPathRecords;
use sf2_game::view_transition::ViewTransitionMode;
use sf2_game::SoundEvent;

const STORAGE: u16 = 0x0200;
const BASE: u32 = STORAGE as u32;
const ENTRY: u32 = 0x07A50A;
const STOP: u32 = 0x07A66C;
const TARGET: u16 = 0x0800; // zero low byte must still count as non-null

#[derive(Debug, Clone, Copy)]
struct Case {
    upgrade: u8,
    candidate: bool,
    remaining: u16,
    mode: u16,
    activity: u8,
    control: u8,
    previous: u8,
    forced: u8,
    published: u8,
    clock: u8,
    grace: u8,
    style: u8,
    screen: [u8; 2],
    reticle: [u8; 2],
    queue: u8,
}

impl Default for Case {
    fn default() -> Self {
        Self {
            upgrade: 0x80,
            candidate: true,
            remaining: 0xAB01,
            mode: 0xFFFD,
            activity: 0xFD,
            control: 0xD7,
            previous: 1,
            forced: 0,
            published: 2,
            clock: 0,
            grace: 173,
            style: 231,
            screen: [104, 104],
            reticle: [128, 128],
            queue: 0,
        }
    }
}

struct Check {
    source: Source,
    objects: ObjectStore,
    world: ScenePathWorld,
    primary: ObjectId,
    other: ObjectId,
    target: ObjectId,
    expected: Vec<u8>,
}

impl Check {
    fn new() -> Self {
        let mut source = Source::new(&rom(), 0xA7);
        source.writes = Some(Vec::new());
        let mut objects = ObjectStore::new();
        let primary = actor(&mut objects);
        let other = actor(&mut objects);
        let target = actor(&mut objects);
        let mut world = ScenePathWorld::new(RandomState::default());
        world.primary_player = Some(primary);
        world.secondary_player = Some(other);
        world.player_display_subject = Some(other);
        world
            .bind_player(&objects, primary, PlayerPathRecords::default())
            .unwrap();
        world
            .bind_player(&objects, other, PlayerPathRecords::default())
            .unwrap();
        let expected = (0..0x10000)
            .map(|address| source.bus.read8(WRAM + address))
            .collect();
        Self {
            source,
            objects,
            world,
            primary,
            other,
            target,
            expected,
        }
    }

    fn object(&self, selector: u8) -> Option<ObjectId> {
        match selector {
            0 => None,
            1 => Some(self.target),
            2 => Some(self.other),
            _ => unreachable!(),
        }
    }

    fn pointer(&self, object: Option<ObjectId>) -> u16 {
        object.map_or(0, |id| {
            if id == self.target {
                TARGET
            } else {
                assert_eq!(id, self.other);
                OTHER
            }
        })
    }

    fn byte(&mut self, address: u32, value: u8) {
        self.source.bus.write8(WRAM + address, value);
        self.expected[address as usize] = value;
    }

    fn word(&mut self, address: u32, value: u16) {
        for (index, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.byte(address + index as u32, byte);
        }
    }

    fn seed(&mut self, case: Case) {
        let selection = TargetSelection {
            display_status: !case.style,
            control_flags: case.control,
            candidate: case.candidate.then_some(self.target),
            forced_owner: self.object(case.forced),
            distance: 49171,
            auxiliary_distance: 17393,
            pitch: 33701,
            yaw: 61397,
            position: Vector3 {
                x: -901,
                y: 17239,
                z: -7713,
            },
            screen: case.screen,
            clipped_yaw: 173,
        };
        let target = self.target;
        source_target(&mut self.source, BASE, selection, |id| {
            if id == target {
                TARGET
            } else {
                OTHER
            }
        });
        for address in BASE + 0x6A65..BASE + 0x6C3D {
            self.expected[address as usize] = self.source.bus.read8(WRAM + address);
        }
        let lock = TargetLock {
            previous_candidate: self.object(case.previous),
            acquisition_clock: case.clock,
            grace_remaining: case.grace,
            marker_style: case.style,
        };
        let auxiliary = SelectedAuxiliaryState {
            mode: 0xC7,
            action_flags: case.activity,
            stored_world_position: Vector3::default(),
            stored_rotation: Default::default(),
        };
        let records = self.world.player_mut(&self.objects, self.primary).unwrap();
        records.target_selection = Some(selection);
        records.target_lock = Some(lock);
        records.auxiliary = Some(auxiliary);
        self.world.targeting_upgrade = Some(TargetingUpgradeState {
            pilot_flags: case.upgrade,
        });
        self.world.objective_counts = Some(EncounterObjectiveCounts {
            remaining_word: case.remaining,
            ..Default::default()
        });
        self.world.contacts_enabled = Some(case.remaining as u8 == 0); // opposite stale observation
        self.world.view_transition_mode = Some(ViewTransitionMode { flags: case.mode });
        self.world.published_homing_target = Some(PublishedHomingTarget {
            object: self.object(case.published),
        });
        self.world.target_reticle = TargetReticle {
            horizontal: Some(case.reticle[0]),
            vertical: Some(case.reticle[1]),
        };
        self.world.audio = Default::default();
        for (address, value) in [
            (0x1DDD, case.upgrade),
            (BASE + 0x6B77, case.activity),
            (BASE + 0x6BC6, case.clock),
            (BASE + 0x6BC7, case.grace),
            (BASE + 0x6BB7, case.style),
            (0x1E30, case.reticle[0]),
            (0x1E31, case.reticle[1]),
        ] {
            self.byte(address, value);
        }
        self.word(0xD7F4, case.remaining);
        self.word(0x1B84, case.mode);
        self.word(BASE + 0x6BC8, self.pointer(lock.previous_candidate));
        self.word(0x1D90, self.pointer(self.object(case.published)));
        self.word(0x1D16, u16::from(case.queue));
        for slot in 0..16 {
            self.word(0x1CF6 + slot * 2, 0xA731 + slot as u16 * 113);
        }
    }

    fn compare(&mut self, case: Case) {
        let records = self.world.player(&self.objects, self.primary).unwrap();
        let selection = records.target_selection.unwrap();
        let lock = records.target_lock.unwrap();
        let mut expected = self.expected.clone();
        let mut word = |address: u32, value: u16| {
            expected[address as usize..address as usize + 2].copy_from_slice(&value.to_le_bytes());
        };
        word(BASE + 0x6BC8, self.pointer(lock.previous_candidate));
        word(BASE + 0x6BCA, self.pointer(selection.forced_owner));
        word(
            0x1D90,
            self.pointer(self.world.published_homing_target.unwrap().object),
        );
        expected[(BASE + 0x6BC2) as usize] = selection.control_flags;
        expected[(BASE + 0x6BC6) as usize] = lock.acquisition_clock;
        expected[(BASE + 0x6BC7) as usize] = lock.grace_remaining;
        expected[(BASE + 0x6BB7) as usize] = lock.marker_style;
        let events: Vec<_> = self
            .world
            .audio
            .take_events()
            .into_iter()
            .flatten()
            .collect();
        assert!(events.len() <= 1);
        if !events.is_empty() {
            assert_eq!(
                events,
                [SoundEvent::Authored(AuthoredCue::new(
                    59,
                    0,
                    PlayerTarget::Primary
                ))]
            );
            let slot = 0x1CF6 + usize::from(case.queue);
            expected[slot..slot + 2].copy_from_slice(&59u16.to_le_bytes());
            expected[0x1D16] = case.queue.wrapping_add(2) & 31;
        }
        // Check the whole player allocation, shared state and queue, not only
        // selected outputs. The write log also rejects changes elsewhere.
        for range in [
            BASE + 0x6A65..BASE + 0x6C3D,
            0x1CF6..0x1D18,
            0x1D90..0x1D92,
            0x1DDD..0x1DDE,
            0xD7F4..0xD7F6,
            0x1B84..0x1B86,
            0x1E30..0x1E32,
        ] {
            for address in range {
                assert_eq!(
                    self.source.bus.read8(WRAM + address),
                    expected[address as usize],
                    "{case:?}: {address:04X}"
                );
            }
        }
        for &(address, _) in self.source.writes.as_ref().unwrap() {
            if address < 0x033F {
                continue;
            } // bootstrap stack and scratch only
            assert_eq!(address >> 16, 0x7E, "unexpected source write {address:06X}");
            let offset = address - WRAM;
            let permitted = [
                0x1D90..0x1D92,
                BASE + 0x6BC8..BASE + 0x6BCC,
                BASE + 0x6BC6..BASE + 0x6BC8,
                BASE + 0x6BC2..BASE + 0x6BC3,
                BASE + 0x6BB7..BASE + 0x6BB8,
                0x1CF6..0x1D16,
                0x1D16..0x1D17,
            ];
            assert!(
                permitted.iter().any(|range| range.contains(&offset)),
                "unexpected source write {address:06X}"
            );
            assert_eq!(
                self.source.bus.read8(address),
                expected[offset as usize],
                "{case:?}: {address:06X}"
            );
        }
        assert_eq!(
            *self.world.player(&self.objects, self.other).unwrap(),
            PlayerPathRecords::default()
        );
    }

    fn run(&mut self, case: Case) {
        self.seed(case);
        let before = self.objects.clone();
        self.source
            .run_with_y(ENTRY, Some(STOP), 0, OWNER, true, Some(STORAGE));
        player_target_lock::update(&self.objects, &mut self.world).unwrap();
        self.compare(case);
        assert_eq!(self.objects, before);
    }
}

#[test]
fn target_retention_matches_original_all_control_bytes_and_cancellation_gate_combinations() {
    let mut check = Check::new();
    for control in 0..=u8::MAX {
        for gates in 0..64 {
            check.run(Case {
                control,
                upgrade: if gates & 1 != 0 {
                    control | 0x80
                } else {
                    control & 0x7F
                },
                candidate: gates & 2 != 0,
                remaining: u16::from_be_bytes([
                    !control,
                    if gates & 4 != 0 { control.max(1) } else { 0 },
                ]),
                mode: (u16::from(control) << 8) | if gates & 8 != 0 { 2 } else { 0 },
                activity: (control & 0xFE) | u8::from(gates & 16 != 0),
                previous: if gates & 32 != 0 { 1 } else { 2 },
                forced: (control % 3),
                clock: control.wrapping_mul(131),
                grace: !control,
                style: control.rotate_left(3),
                queue: (control & 15) * 2,
                ..Case::default()
            });
        }
    }
}

#[test]
fn target_retention_matches_original_every_horizontal_and_vertical_byte_pair() {
    let mut check = Check::new();
    for axis in 0..2 {
        for reticle in 0..=u8::MAX {
            for target in 0..=u8::MAX {
                let mut case = Case::default();
                case.reticle[axis] = reticle;
                case.screen[axis] = target;
                case.forced = 2;
                case.grace = 10;
                check.run(case);
            }
        }
    }
}

#[test]
fn target_retention_matches_original_every_timer_style_and_queue_slot() {
    let mut check = Check::new();
    for byte in 0..=u8::MAX {
        for forced in 0..3 {
            for outside in [false, true] {
                check.run(Case {
                    forced,
                    clock: byte,
                    grace: byte,
                    style: byte,
                    queue: (byte & 15) * 2,
                    reticle: if outside { [0, 128] } else { [128, 128] },
                    ..Case::default()
                });
                // Acquisition clears all 16 low-nibble marker variants, only
                // when there is no retained owner, at every queue position.
                check.run(Case {
                    forced,
                    clock: 0,
                    grace: byte,
                    style: byte,
                    queue: (byte & 15) * 2,
                    reticle: if outside { [128, 0] } else { [128, 128] },
                    ..Case::default()
                });
            }
        }
    }
}

#[test]
fn target_retention_missing_reticle_stops_at_exact_original_mutation_prefix() {
    for vertical in [false, true] {
        let mut check = Check::new();
        let case = Case::default();
        check.seed(case);
        let (boundary, error) = if vertical {
            check.world.target_reticle.vertical = None;
            (0x07A5A3, TargetLockError::MissingVerticalReticle)
        } else {
            check.world.target_reticle.horizontal = None;
            (0x07A58C, TargetLockError::MissingHorizontalReticle)
        };
        check
            .source
            .run_with_y(ENTRY, Some(boundary), 0, OWNER, true, Some(STORAGE));
        assert_eq!(
            player_target_lock::update(&check.objects, &mut check.world),
            Err(error)
        );
        check.compare(case);
    }
}

#[test]
fn target_retention_acquisition_preserves_original_sound_publication_write_order() {
    let mut check = Check::new();
    let case = Case {
        queue: 30,
        ..Case::default()
    };
    check.run(case);
    let writes: Vec<_> = check
        .source
        .writes
        .as_ref()
        .unwrap()
        .iter()
        .copied()
        .filter(|(address, _)| *address >= WRAM)
        .map(|(address, value)| (address - WRAM, value))
        .collect();
    let mut expected = Vec::new();
    let word = |out: &mut Vec<_>, address, value: u16| {
        out.push((address, value as u8));
        out.push((address + 1, (value >> 8) as u8));
    };
    word(&mut expected, BASE + 0x6BC8, 0);
    word(&mut expected, 0x1D90, 0);
    word(&mut expected, BASE + 0x6BC8, TARGET);
    expected.push((BASE + 0x6BB7, case.style & 0xF0));
    word(&mut expected, 0x1CF6 + u32::from(case.queue), 59);
    expected.push((0x1D16, 0));
    word(&mut expected, BASE + 0x6BCA, TARGET);
    word(&mut expected, BASE + 0x6BC8, TARGET);
    word(&mut expected, 0x1D90, TARGET);
    expected.push((BASE + 0x6BC7, 10));
    expected.push((BASE + 0x6BC2, case.control & !0x18));
    assert_eq!(writes, expected);
}

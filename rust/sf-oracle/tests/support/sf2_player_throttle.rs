//! Unpatched control, effect installation, pool allocation/formatting,
//! numbered attachment, pilot lookup and sound queue execution.

use super::{rom, Source, WRAM};
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_program::SelectedAuxiliaryState;
use sf2_game::player_charge::PlayerCharge;
use sf2_game::player_throttle::{self, PlayerThrottle, ThrottleError};
use sf2_game::player_visit::PlayerVisitControl;
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use sf2_game::{
    Angle, Behavior, Button, Buttons, InputState, Object, ObjectId, ObjectKind,
    ObjectSpawnDefaults, ObjectStore, RandomState, ShapeId, SoundEvent, Vector3,
};

const OWNER: u16 = 0x03BD;
const SLOT: u32 = 64;

#[derive(Clone, Copy, Debug)]
enum Operation {
    Advance,
    Boost,
    Brake,
    Cancel,
    Deactivate,
}

struct Native {
    objects: ObjectStore,
    world: ScenePathWorld,
    owner: ObjectId,
    expected_pool_fault: bool,
}

fn address(id: Option<ObjectId>) -> u16 {
    id.map_or(0, |id| OWNER + id.index() as u16 * 0x3F)
}

impl Native {
    fn new(source: &mut Source, count: usize) -> Self {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        for _ in 1..count {
            objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::Unassigned,
                ))
                .unwrap();
        }
        if count > 1 {
            let pressured = objects.active_ids()[count - 2];
            objects
                .get_mut(pressured)
                .unwrap()
                .base
                .flags
                .reclaim_on_pool_pressure = true;
        }
        for (id, actor) in objects.active_objects() {
            let base = u32::from(address(Some(id)));
            for offset in 0..0x3F {
                source.bus.write8(WRAM + base + offset, 0);
                source.bus.write8(WRAM + 0x1CC1 + base + offset, 0);
            }
            source.bus.write16(base, address(actor.base.next));
            source.bus.write16(base + 2, address(actor.base.previous));
            source.bus.write16(base + 4, 0xBC9C);
            source.bus.write8(base + 0x2D, actor.base.hit_points);
            source.bus.write8(
                base + 0x20,
                if actor.base.flags.reclaim_on_pool_pressure {
                    0x10
                } else {
                    0
                },
            );
        }
        source
            .bus
            .write16(0x12A8, address(objects.active_ids().first().copied()));
        source.bus.write16(
            0x12AA,
            if count == 60 {
                0
            } else {
                OWNER + count as u16 * 0x3F
            },
        );
        for index in count..60 {
            let base = u32::from(OWNER) + index as u32 * 0x3F;
            for offset in 0..0x3F {
                source.bus.write8(WRAM + base + offset, 0xA5);
                source.bus.write8(WRAM + 0x1CC1 + base + offset, 0x5A);
            }
            source
                .bus
                .write16(base, if index == 59 { 0 } else { base as u16 + 0x3F });
        }
        source.bus.write16(u32::from(OWNER) + 0x2B, SLOT as u16);
        source.bus.write16(0x1B84, 0);
        source.bus.write8(0x190E, 42);
        source.bus.write16(0x1651, OWNER);
        for offset in 0..472 {
            source.bus.write8(WRAM + 0x6A61 + SLOT + offset, 0);
        }
        let actor = objects.get_mut(owner).unwrap();
        actor.base.position = Vector3 {
            x: -319,
            y: 173,
            z: 813,
        };
        actor.base.pitch = Angle::from_units(71);
        actor.base.yaw = Angle::from_units(183);
        actor.base.roll = Angle::from_units(237);
        for (offset, value) in [(12, -319_i16), (14, 173), (16, 813)] {
            source.bus.write16(u32::from(OWNER) + offset, value as u16);
        }
        for (offset, value) in [(0x12, 71), (0x14, 183), (0x16, 237)] {
            source.bus.write8(u32::from(OWNER) + offset, value);
        }
        let mut world = ScenePathWorld::new(RandomState::default());
        world.spawn_defaults = Some(ObjectSpawnDefaults {
            group: 42,
            run_when_paused: false,
        });
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    throttle: Some(PlayerThrottle::default()),
                    charge: Some(PlayerCharge::default()),
                    visit: Some(PlayerVisitControl::default()),
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode: 0,
                        action_flags: 0,
                        stored_world_position: Default::default(),
                        stored_rotation: Default::default(),
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        Self {
            objects,
            world,
            owner,
            expected_pool_fault: false,
        }
    }

    fn record(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }

    fn seed(&mut self, source: &mut Source, seed: u16, pilot: u8, linked: u8, primary: bool) {
        let low = seed as u8;
        let high = (seed >> 8) as u8;
        let state = PlayerThrottle {
            brake_preference: low,
            effect_flags: high,
            effect_level: low.rotate_left(3),
            entry_marker: high.rotate_left(3),
        };
        self.record().throttle = Some(state);
        self.record().auxiliary.as_mut().unwrap().action_flags = low;
        self.record().charge.as_mut().unwrap().linked_mode = linked & 0x80 != 0;
        self.record()
            .charge
            .as_mut()
            .unwrap()
            .linked_muzzle_disabled = linked & 0x40 != 0;
        self.record().visit.as_mut().unwrap().pilot_code = pilot;
        self.world.primary_player = primary.then_some(self.owner);
        source.bus.write16(0x12C3, if primary { OWNER } else { 0 });
        for (offset, value) in [
            (0x6B78, state.brake_preference),
            (0x6B79, state.effect_flags),
            (0x6B7A, state.effect_level),
            (0x6B7F, state.entry_marker),
            (0x6B77, low),
            (0x6B63, linked),
            (0x6BFF, pilot),
        ] {
            source.bus.write8(WRAM + offset + SLOT, value);
        }
    }

    fn visit(
        &mut self,
        source: &mut Source,
        operation: Operation,
        activity: u16,
        held: u16,
        pressed: u16,
        clock: u16,
    ) {
        source.bus.write16(WRAM + 0xD7F4, activity);
        source.bus.write16(0x1938, held);
        source.bus.write16(0x1936, pressed);
        source.bus.write16(0xC4, clock);
        source.bus.write16(0x1D16, 0);
        self.world.contacts_enabled = Some(activity as u8 != 0);
        self.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(held),
            pressed: Buttons::from_bits(pressed),
        });
        self.world.strategy_clock = clock;
        let before = *self.world.player(&self.objects, self.owner).unwrap();
        let preserved: Vec<_> = (0..472)
            .map(|offset| source.bus.read8(WRAM + 0x6A61 + SLOT + offset))
            .collect();
        // A full strategy pool enters the non-returning fatal display.
        // Compare its real entry boundary, never patch the source allocator
        // or pretend its unreachable carry-clear continuation executes.
        let stop = self.expected_pool_fault.then_some(0x008032);
        let result = match operation {
            Operation::Advance => {
                source.run(0x06F010, stop.or(Some(0x06F05C)), 0, OWNER, true);
                player_throttle::advance(&mut self.objects, &mut self.world, self.owner)
            }
            Operation::Boost => {
                source.run(0x06A47B, stop, 0, OWNER, true);
                player_throttle::boost(&mut self.objects, &mut self.world, self.owner)
            }
            Operation::Brake => {
                source.run(0x06A4C5, stop, 0, OWNER, true);
                player_throttle::brake(&mut self.objects, &mut self.world, self.owner)
            }
            Operation::Cancel => {
                source.run(0x06A51D, stop, 0, OWNER, true);
                player_throttle::cancel(&self.objects, &mut self.world, self.owner)
            }
            Operation::Deactivate => {
                source.run(0x06A55B, stop, 0, OWNER, true);
                player_throttle::deactivate_effect(&self.objects, &mut self.world, self.owner)
            }
        };
        assert_eq!(
            result,
            if self.expected_pool_fault {
                Err(ThrottleError::ObjectPoolExhausted)
            } else {
                Ok(())
            }
        );
        let control = self.record().throttle.unwrap();
        for (offset, value) in [
            (0x6B78, control.brake_preference),
            (0x6B79, control.effect_flags),
            (0x6B7A, control.effect_level),
            (0x6B7F, control.entry_marker),
            (0x6B77, self.record().auxiliary.unwrap().action_flags),
        ] {
            assert_eq!(
                source.bus.read8(WRAM + offset + SLOT),
                value,
                "{operation:?} field {offset:X}, input {held:04X}/{pressed:04X}, clock {clock}"
            );
        }
        for (offset, value) in preserved.into_iter().enumerate() {
            let field = 0x6A61 + offset as u32;
            if ![0x6B77, 0x6B78, 0x6B79, 0x6B7A, 0x6B7F].contains(&field) {
                assert_eq!(
                    source.bus.read8(WRAM + field + SLOT),
                    value,
                    "preserve {field:X}"
                );
            }
        }
        let mut expected = before;
        expected.throttle = Some(control);
        expected.auxiliary.as_mut().unwrap().action_flags =
            self.record().auxiliary.unwrap().action_flags;
        assert_eq!(self.record(), &expected);
        let events: Vec<_> = self
            .world
            .audio
            .take_events()
            .into_iter()
            .flatten()
            .map(|event| {
                let SoundEvent::Authored(cue) = event else {
                    panic!("unexpected non-authored sound")
                };
                u16::from(cue.id)
                    | u16::from(cue.parameter()) << 8
                    | if cue.target == PlayerTarget::Secondary {
                        0x8000
                    } else {
                        0
                    }
            })
            .collect();
        assert_eq!(source.bus.read16(0x1D16) as usize, events.len() * 2);
        for (index, event) in events.iter().enumerate() {
            assert_eq!(source.bus.read16(0x1CF6 + index as u32 * 2), *event);
        }
        self.compare_pool(source);
    }

    fn compare_pool(&self, source: &Source) {
        assert_eq!(
            source.bus.read16(0x12A8),
            address(self.objects.active_ids().first().copied())
        );
        let mut chain = Vec::new();
        let mut current = source.bus.read16(0x12A8);
        while current != 0 {
            chain.push(current);
            assert!(chain.len() <= 60);
            current = source.bus.read16(u32::from(current));
        }
        assert_eq!(
            chain,
            self.objects
                .active_ids()
                .iter()
                .map(|id| address(Some(*id)))
                .collect::<Vec<_>>()
        );
        let mut free_count = 0;
        let mut free = source.bus.read16(0x12AA);
        while free != 0 {
            free_count += 1;
            assert!(free_count <= 60);
            assert!(!chain.contains(&free));
            free = source.bus.read16(u32::from(free));
        }
        assert_eq!(free_count, 60 - self.objects.len());
        for (id, actor) in self.objects.active_objects() {
            let base = u32::from(address(Some(id)));
            assert_eq!(source.bus.read16(base), address(actor.base.next));
            assert_eq!(source.bus.read16(base + 2), address(actor.base.previous));
            assert_eq!(source.bus.read16(base + 6), address(actor.base.attachment));
            assert_eq!(
                source.bus.read8(base + 0x25) & 8 != 0,
                actor.base.flags.remove_after_tick
            );
            assert_eq!(
                source.bus.read16(base + 0x29),
                address(actor.base.first_child.or(actor.base.next_sibling))
            );
            if actor.base.child_number == 0 {
                continue;
            }
            assert_eq!(source.bus.read8(base + 0x13), actor.base.child_number);
            assert_eq!(
                source.bus.read16(base + 4),
                0xBC9C + actor.base.shape.catalog_index() as u16 * 28
            );
            assert_eq!(source.bus.read16(base + 0x19), 0x7E1E);
            assert_eq!(source.bus.read8(base + 0x1B), 0x7F);
            assert_eq!(
                source.bus.read16(base + 0x2B),
                if actor.base.child_number == 12 {
                    0xF32C
                } else {
                    0xF36F
                }
            );
            assert_eq!(source.bus.read16(base + 12) as i16, actor.base.position.x);
            assert_eq!(source.bus.read16(base + 14) as i16, actor.base.position.y);
            assert_eq!(source.bus.read16(base + 16) as i16, actor.base.position.z);
            assert_eq!(source.bus.read8(base + 0x12), actor.base.pitch.units());
            assert_eq!(source.bus.read8(base + 0x14), actor.base.yaw.units());
            assert_eq!(source.bus.read8(base + 0x16), actor.base.roll.units());
            assert_eq!(
                source.bus.read16(WRAM + 0x1CCF + base) as i16,
                actor.extension.relative_position.x
            );
            assert_eq!(
                source.bus.read16(WRAM + 0x1CD1 + base) as i16,
                actor.extension.relative_position.y
            );
            assert_eq!(
                source.bus.read16(WRAM + 0x1CD3 + base) as i16,
                actor.extension.relative_position.z
            );
            assert_eq!(source.bus.read8(base + 0x2D), actor.base.hit_points);
            assert_eq!(source.bus.read8(base + 0x2E), actor.base.attack_power);
            assert_eq!(
                source.bus.read8(WRAM + 0x1CF0 + base),
                actor.extension.spawn_group
            );
            assert_eq!(
                source.bus.read8(base + 0x21) & 1 != 0,
                actor.base.flags.collision_disabled
            );
            assert_eq!(
                source.bus.read8(base + 0x22) & 4 != 0,
                actor.base.flags.general_search_eligible
            );
            assert_eq!(
                source.bus.read8(base + 0x23) & 4 != 0,
                actor.extension.path_state.motion.attached_coordinates
            );
            assert_eq!(
                source.bus.read8(base + 0x25) & 1 != 0,
                actor.base.flags.remove_with_parent
            );
            assert_eq!(
                source.bus.read8(base + 0x26) & 8 != 0,
                actor.base.contacts.run_when_paused
            );
        }
    }
}

#[test]
fn all_throttle_state_bytes_and_transition_entries_match_original() {
    let mut source = Source::new(&rom(), 0);
    let mut native = Native::new(&mut source, 1);
    for operation in [
        Operation::Boost,
        Operation::Brake,
        Operation::Cancel,
        Operation::Deactivate,
    ] {
        for seed in 0..=u16::MAX {
            native.seed(&mut source, seed, seed as u8, 0x80, seed & 1 == 0);
            native.visit(&mut source, operation, 1, 0, 0, seed >> 8);
        }
    }
}

#[test]
fn throttle_activity_preference_and_opposing_inputs_match_original() {
    let mut source = Source::new(&rom(), 0);
    let mut native = Native::new(&mut source, 1);
    let choices = [
        0,
        Button::A as u16,
        Button::Y as u16,
        Button::A as u16 | Button::Y as u16,
    ];
    for preference in 0..=255_u16 {
        for held in choices {
            for pressed in choices {
                for activity in [0, 1, 0x100, 0x101] {
                    native.seed(
                        &mut source,
                        preference | preference.wrapping_mul(0x100),
                        preference as u8,
                        0x80,
                        preference & 1 == 0,
                    );
                    native.visit(
                        &mut source,
                        Operation::Advance,
                        activity,
                        held,
                        pressed,
                        preference,
                    );
                }
            }
        }
    }
}

#[test]
fn all_pilots_linked_gates_partial_and_full_pools_match_original_real_installers() {
    let mut source = Source::new(&rom(), 0);
    for pilot in 0..=255 {
        for count in [1, 2, 58, 59, 60] {
            for linked in [0, 0x40, 0x80, 0xC0] {
                for operation in [Operation::Boost, Operation::Brake] {
                    let mut native = Native::new(&mut source, count);
                    native.expected_pool_fault = linked != 0x80
                        && match operation {
                            Operation::Boost => count == 60,
                            Operation::Brake => count >= 59,
                            _ => unreachable!(),
                        };
                    native.seed(
                        &mut source,
                        u16::from(pilot).wrapping_mul(257),
                        pilot,
                        linked,
                        pilot & 1 == 0,
                    );
                    native.visit(&mut source, operation, 1, 0, 0, u16::from(pilot));
                    if native.expected_pool_fault {
                        continue;
                    }
                    // Same retained pool and children on the next visit: no
                    // refresh, duplicate child, cue or lost sibling links.
                    native.visit(
                        &mut source,
                        operation,
                        1,
                        0,
                        0,
                        u16::from(pilot).wrapping_add(1),
                    );
                }
            }
        }
    }
}

#[test]
fn continuous_throttle_changes_keep_independent_state_children_and_cues() {
    let mut source = Source::new(&rom(), 0);
    let mut native = Native::new(&mut source, 2);
    native.seed(&mut source, 0, 4, 0, true);
    let choices = [
        0,
        Button::A as u16,
        Button::Y as u16,
        Button::A as u16 | Button::Y as u16,
    ];
    let mut previous = 0;
    for clock in 0..2048_u16 {
        let held = choices[usize::from(clock / 7 % 4)];
        native.visit(
            &mut source,
            Operation::Advance,
            if clock % 17 == 0 { 0x100 } else { 1 },
            held,
            held & !previous,
            clock,
        );
        previous = held;
    }
}

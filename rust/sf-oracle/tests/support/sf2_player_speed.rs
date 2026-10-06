//! Complete unmodified $06:F05D..F1FB, including original pilot lookup,
//! byte speed easing and the widened signed-thrust calculation.
use super::{actor, rom, Source, OWNER, WRAM};
use sf2_game::path_program::ActionGate;
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_speed::{self, PlayerSpeed, SpeedContext};
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use sf2_game::{Buttons, InputState, ObjectId, ObjectStore, RandomState};

const SLOT: u32 = 64;

#[derive(Debug, Clone, Copy, Default)]
struct Case {
    pilot: u8,
    gate: u8,
    ignore: bool,
    actions: u8,
    mode: u8,
    config: u8,
    held: u16,
    clock: u16,
    target: i8,
    alternate: i8,
}

struct Native {
    objects: ObjectStore,
    world: ScenePathWorld,
    owner: ObjectId,
    _runtime: PathRuntime,
    context: SpeedContext,
}
impl Native {
    fn new(source: &mut Source) -> Self {
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects);
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut runtime = PathRuntime::default();
        player_storage::initialize(
            &mut objects,
            &mut world,
            &mut runtime,
            owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 0,
                score: Default::default(),
            },
        )
        .unwrap();
        source.bus.write16(u32::from(OWNER) + 0x2B, SLOT as u16);
        source.writes = Some(Vec::new());
        Self {
            objects,
            world,
            owner,
            _runtime: runtime,
            context: SpeedContext::default(),
        }
    }
    fn record(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn seed(&mut self, source: &mut Source, speed: u8, thrust: i8, linked: u8) {
        self.objects.get_mut(self.owner).unwrap().base.speed = speed;
        self.record().speed = Some(PlayerSpeed { thrust });
        let charge = self.record().charge.as_mut().unwrap();
        charge.linked_mode = linked & 0x80 != 0;
        charge.linked_muzzle_disabled = linked & 0x40 != 0;
        source.bus.write8(u32::from(OWNER) + 0x18, speed);
        source.bus.write8(WRAM + SLOT + 0x6B62, thrust as u8);
        source.bus.write8(WRAM + SLOT + 0x6B63, linked);
    }
    fn inputs(&mut self, source: &mut Source, case: Case) {
        self.record().visit.as_mut().unwrap().pilot_code = case.pilot;
        self.record().contact.as_mut().unwrap().ignores_contacts = case.ignore;
        self.record().auxiliary.as_mut().unwrap().action_flags = case.actions;
        self.record().auxiliary.as_mut().unwrap().mode = case.mode;
        self.world.action_gate = Some(ActionGate { code: case.gate });
        self.world.scene.player_configuration = Some(case.config);
        self.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(case.held),
            pressed: Buttons::from_bits(0xFFFF),
        });
        self.world.strategy_clock = case.clock;
        self.context = SpeedContext {
            thrust_target: Some(case.target),
            alternate_thrust_target: Some(case.alternate),
        };
        for (offset, value) in [
            (0x6BFF, case.pilot),
            (0x6A72, if case.ignore { 0xFF } else { 0xEF }),
            (0x6B77, case.actions),
            (0x6AA0, case.mode),
        ] {
            source.bus.write8(WRAM + SLOT + offset, value);
        }
        for (offset, value) in [
            (0x1D72, case.gate),
            (0x1DE2, case.config),
            (0x1DBA, case.target as u8),
            (0x1DB9, case.alternate as u8),
        ] {
            source.bus.write8(offset, value);
        }
        source.bus.write16(0x1938, case.held);
        source.bus.write16(0x1936, 0xFFFF);
        source.bus.write16(0xC4, case.clock);
    }
    fn step(&mut self, source: &mut Source) {
        let mut expected = *self.record();
        let mut expected_objects = self.objects.clone();
        let linked = source.bus.read8(WRAM + SLOT + 0x6B63);
        source.run(0x06F05D, Some(0x06F1FB), 0, OWNER, true);
        player_speed::advance(&mut self.objects, &mut self.world, self.owner, self.context)
            .unwrap();
        expected.speed.as_mut().unwrap().thrust = source.bus.read8(WRAM + SLOT + 0x6B62) as i8;
        expected_objects.get_mut(self.owner).unwrap().base.speed =
            source.bus.read8(u32::from(OWNER) + 0x18);
        assert_eq!(*self.record(), expected);
        assert_eq!(self.objects, expected_objects);
        assert_eq!(source.bus.read8(WRAM + SLOT + 0x6B63), linked);
        for &(address, _) in source.writes.as_ref().unwrap() {
            if (WRAM + SLOT + 0x6A61..WRAM + SLOT + 0x6A61 + 472).contains(&address) {
                assert_eq!(address, WRAM + SLOT + 0x6B62, "unexpected player write");
            }
        }
    }
}

#[test]
fn speed_all_signed_thrust_pairs_and_byte_speed_boundaries_match_original() {
    let mut source = Source::new(&rom(), 0x5A);
    let mut native = Native::new(&mut source);
    for current in i8::MIN..=i8::MAX {
        for target in i8::MIN..=i8::MAX {
            native.inputs(
                &mut source,
                Case {
                    mode: 1,
                    actions: 4,
                    target,
                    ..Default::default()
                },
            );
            native.seed(&mut source, current as u8, current, target as u8);
            native.step(&mut source);
            native.inputs(
                &mut source,
                Case {
                    mode: 0,
                    actions: 4,
                    alternate: target,
                    ..Default::default()
                },
            );
            native.seed(&mut source, current as u8, current, target as u8);
            native.step(&mut source);
        }
    }
}

#[test]
fn speed_all_pilots_flags_modes_and_boost_configuration_match_original() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = Native::new(&mut source);
    for pilot in 0..=u8::MAX {
        for actions in [0, 4, 0x20, 0x24, 0x40, 0x44, 0x60, 0xFF] {
            for mode in [0, 1, 2, 0x11, 0x21, 0xF1, 0xFE, 0xFF] {
                for config in [0, 9] {
                    native.inputs(
                        &mut source,
                        Case {
                            pilot,
                            actions,
                            mode,
                            config,
                            target: -128,
                            alternate: 127,
                            ..Default::default()
                        },
                    );
                    native.seed(&mut source, pilot.wrapping_mul(13), actions as i8, mode);
                    native.step(&mut source);
                }
            }
        }
    }
    for byte in 0..=u8::MAX {
        for flavor in 0..16_u8 {
            native.inputs(
                &mut source,
                Case {
                    pilot: flavor % 6,
                    actions: byte,
                    mode: flavor,
                    config: byte,
                    target: byte as i8,
                    alternate: !byte as i8,
                    held: u16::from(byte) * 257,
                    clock: u16::from(flavor),
                    ..Default::default()
                },
            );
            native.seed(&mut source, byte.wrapping_mul(11), (!byte) as i8, flavor);
            native.step(&mut source);
        }
    }
}

#[test]
fn speed_all_processed_input_words_and_directional_cadences_match_original() {
    let mut source = Source::new(&rom(), 0x31);
    let mut native = Native::new(&mut source);
    for held in 0..=u16::MAX {
        for clock in [0, 1] {
            native.inputs(
                &mut source,
                Case {
                    held,
                    clock,
                    pilot: ((held >> 5) % 6) as u8,
                    mode: (held >> 8) as u8,
                    target: held as i8,
                    alternate: (held >> 8) as i8,
                    ..Default::default()
                },
            );
            native.seed(&mut source, held as u8, (held >> 8) as i8, held as u8);
            native.step(&mut source);
        }
    }
}

#[test]
fn speed_forced_gates_and_independently_retained_sequences_match_original() {
    let mut source = Source::new(&rom(), 0x52);
    let mut native = Native::new(&mut source);
    for gate in 0..=u8::MAX {
        for ignore in [false, true] {
            for current in [0, 1, 38, 127, 128, 129, 254, 255] {
                native.inputs(
                    &mut source,
                    Case {
                        gate,
                        ignore,
                        actions: 0xFF,
                        config: 9,
                        ..Default::default()
                    },
                );
                native.seed(&mut source, current, current as i8, gate);
                native.step(&mut source);
            }
        }
    }
    native.seed(&mut source, 239, -127, 0xC5);
    for visit in 0..4096_u16 {
        native.inputs(
            &mut source,
            Case {
                pilot: ((visit / 11) % 8) as u8,
                gate: u8::from(visit % 97 < 7),
                ignore: visit % 43 < 5,
                actions: [0, 4, 0x20, 0x40, 0x60][usize::from(visit / 13) % 5],
                mode: (visit / 17) as u8,
                config: if visit % 23 < 5 { 9 } else { 0 },
                held: [0, 0x100, 0x200, 0x300, 0x30][usize::from(visit / 7) % 5],
                clock: visit,
                target: (visit % 8) as i8,
                alternate: (visit / 5) as i8,
            },
        );
        native.step(&mut source);
    }
}

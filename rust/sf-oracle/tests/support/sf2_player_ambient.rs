//! Full unmodified ambient routine, including both waveforms and its real
//! scripted-view and mode-family gates. No source code is patched.

use super::{actor, rom, Source, OWNER, WRAM};
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_ambient::{self, PlayerAmbient};
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use sf2_game::view_transition::ViewTransitionMode;
use sf2_game::{ObjectId, ObjectStore, RandomState};

const SLOT: u32 = 64;

struct Native {
    objects: ObjectStore,
    world: ScenePathWorld,
    _runtime: PathRuntime,
    owner: ObjectId,
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
            _runtime: runtime,
            owner,
        }
    }
    fn record(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn seed(&mut self, source: &mut Source, bank_phase: u8, offset_phase: u8, offset: i16) {
        self.record().ambient = Some(PlayerAmbient {
            bank_phase,
            offset_phase,
            retained_offset: offset,
        });
        self.record().pose.as_mut().unwrap().ambient_bank = -99;
        source.bus.write8(WRAM + SLOT + 0x6AD6, bank_phase);
        source.bus.write8(WRAM + SLOT + 0x6ADB, offset_phase);
        source.bus.write16(WRAM + SLOT + 0x6AE2, offset as u16);
        source.bus.write8(WRAM + SLOT + 0x6AD5, (-99_i8) as u8);
    }
    fn mode(&mut self, source: &mut Source, mode: u8, flags: u16) {
        self.record().auxiliary.as_mut().unwrap().mode = mode;
        self.world.view_transition_mode = Some(ViewTransitionMode { flags });
        source.bus.write8(WRAM + SLOT + 0x6AA0, mode);
        source.bus.write16(0x1B84, flags);
    }
    fn step(&mut self, source: &mut Source) {
        let mut expected = *self.record();
        let objects = self.objects.clone();
        source.run(0x06F2F7, Some(0x06F365), 0, OWNER, true);
        player_ambient::advance(&self.objects, &mut self.world, self.owner).unwrap();
        let ambient = expected.ambient.as_mut().unwrap();
        ambient.bank_phase = source.bus.read8(WRAM + SLOT + 0x6AD6);
        ambient.offset_phase = source.bus.read8(WRAM + SLOT + 0x6ADB);
        ambient.retained_offset = source.bus.read16(WRAM + SLOT + 0x6AE2) as i16;
        expected.pose.as_mut().unwrap().ambient_bank = source.bus.read8(WRAM + SLOT + 0x6AD5) as i8;
        assert_eq!(*self.record(), expected);
        assert_eq!(self.objects, objects);
        for &(address, _) in source.writes.as_ref().unwrap() {
            if (WRAM + SLOT + 0x6A61..WRAM + SLOT + 0x6A61 + 472).contains(&address) {
                assert!(
                    [0x6AD5, 0x6AD6, 0x6ADB, 0x6AE2, 0x6AE3].contains(&(address - WRAM - SLOT)),
                    "unexpected player write {address:X}"
                );
            }
        }
    }
}

#[test]
fn ambient_all_phase_pairs_and_all_retained_words_match_original_wrapping() {
    let mut source = Source::new(&rom(), 0x5A);
    let mut native = Native::new(&mut source);
    native.mode(&mut source, 0x10, 0);
    for seed in 0..=u16::MAX {
        native.seed(
            &mut source,
            seed as u8,
            (seed >> 8) as u8,
            seed.rotate_left(5) as i16,
        );
        native.step(&mut source);
        native.seed(&mut source, seed as u8, (seed as u8) & 31, seed as i16);
        native.step(&mut source);
    }
    for offset in [i16::MIN, i16::MIN + 1, -2, -1, 0, 1, i16::MAX - 1, i16::MAX] {
        for phase in 0..=u8::MAX {
            native.seed(&mut source, phase, phase, offset);
            native.step(&mut source);
        }
    }
}

#[test]
fn ambient_all_mode_families_and_view_gates_match_original_without_touching_other_fields() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = Native::new(&mut source);
    for mode in 0..=u8::MAX {
        for flags in [0, 2, 0xFFFD, 0xFFFF] {
            native.mode(&mut source, mode, flags);
            for phase in [0, 1, 28, 29, 30, 31, 254, 255] {
                native.seed(
                    &mut source,
                    phase,
                    phase,
                    if phase & 1 != 0 { i16::MAX } else { i16::MIN },
                );
                native.step(&mut source);
            }
        }
    }
}

#[test]
fn ambient_continuous_visits_preserve_phase_across_scripted_and_special_modes() {
    let mut source = Source::new(&rom(), 0x31);
    let mut native = Native::new(&mut source);
    native.seed(&mut source, 0, 0, -4);
    for visit in 0..2048 {
        native.mode(
            &mut source,
            if visit % 41 < 13 { 0x3F } else { 0x10 },
            if visit % 31 < 7 { 2 } else { 0 },
        );
        native.step(&mut source);
    }
}

#[test]
fn ambient_bank_is_consumed_by_original_and_native_pose_composition_without_feedback() {
    let mut source = Source::new(&rom(), 0);
    let mut native = Native::new(&mut source);
    native.seed(&mut source, 0, 0, 0);
    native.world.player_pitch_target = Some(0);
    native.world.player_yaw_increment = Some(0);
    for visit in 0..2048 {
        native.mode(&mut source, 0x10, if visit % 41 < 9 { 2 } else { 0 });
        native.step(&mut source);
        source.run(0x06ECB0, Some(0x06EE09), 0, OWNER, true);
        sf2_game::player_pose::compose(
            &mut native.objects,
            &mut native.world,
            &mut native._runtime.resources,
            native.owner,
        )
        .unwrap();
        let actor = native.objects.get(native.owner).unwrap();
        assert_eq!(
            actor.base.pitch.units(),
            source.bus.read8(u32::from(OWNER) + 0x12)
        );
        assert_eq!(
            actor.base.yaw.units(),
            source.bus.read8(u32::from(OWNER) + 0x14)
        );
        assert_eq!(
            actor.base.roll.units(),
            source.bus.read8(u32::from(OWNER) + 0x16)
        );
    }
}

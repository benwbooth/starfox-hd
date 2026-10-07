//! Whole original plane preparation, including carry, inherited caller offset,
//! clipping publication and retained rejection behavior.

use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::player_surface_prepare;
use sf2_game::scene_path_world::PlayerPathRecords;
use sf2_game::ObjectId;

struct Fixture {
    native: Native,
    support: ObjectId,
}
impl Fixture {
    fn new(source: &mut Source) -> Self {
        let mut native = Native::new(source, 2, 0, 0, 0);
        player_storage::initialize(
            &mut native.objects,
            &mut native.world,
            &mut PathRuntime::default(),
            native.owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 19,
                score: Default::default(),
            },
        )
        .unwrap();
        let support = native
            .objects
            .active_ids()
            .iter()
            .copied()
            .find(|&id| id != native.owner)
            .unwrap();
        native.world.player_carry_mode = Some(0);
        native.world.environment_plane_height = Some(0);
        native.world.surface_clipping_plane_height = Some(0x1234);
        native
            .world
            .player_mut(&native.objects, native.owner)
            .unwrap()
            .surface
            .as_mut()
            .unwrap()
            .plane_height = -981;
        Self { native, support }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.native
            .world
            .player_mut(&self.native.objects, self.native.owner)
            .unwrap()
    }
    fn seed_retained(&mut self, source: &mut Source) {
        let surface = self.records().surface.unwrap();
        source
            .bus
            .write16(WRAM + SLOT + 0x6A7D, surface.plane_height as u16);
        source.bus.write8(WRAM + SLOT + 0x6A82, surface.material);
        source.bus.write16(
            0x7024E0,
            self.native.world.surface_clipping_plane_height.unwrap() as u16,
        );
    }
    fn step(&mut self, source: &mut Source, inherited_offset: i16) -> Option<i16> {
        let contact = self
            .native
            .objects
            .get(self.native.owner)
            .unwrap()
            .extension
            .surface_contact;
        source.bus.write16(
            WRAM + u32::from(OWNER) + 0x1CE8,
            address(contact.supporting_object),
        );
        source
            .bus
            .write8(WRAM + u32::from(OWNER) + 0x1CEB, contact.flags);
        let support = self.native.objects.get(self.support).unwrap();
        let base = WRAM + u32::from(address(Some(self.support)));
        source
            .bus
            .write16(base + 0x1CE4, support.extension.path_state.script_value);
        source
            .bus
            .write16(base + 14, support.base.position.y as u16);
        source.bus.write16(
            WRAM + SLOT + 0x6AF7,
            self.records().motion.unwrap().surface_height as u16,
        );
        source.bus.write16(
            0x1E0F,
            self.native.world.environment_plane_height.unwrap() as u16,
        );
        source
            .bus
            .write8(0x1E13, self.native.world.player_carry_mode.unwrap());
        source.bus.write16(8, inherited_offset as u16);
        source.bus.write8(0x5E, 1);
        for (offset, value) in [
            (0x6A7C, 0x91),
            (0x6A7F, 0x72),
            (0x6A81, 0xB3),
            (0x6A83, 0x54),
        ] {
            source.bus.write8(WRAM + SLOT + offset, value);
        }
        let mut before = *self.records();
        source.run(0x07E5C8, None, 0, OWNER, true);
        let actual = player_surface_prepare::prepare(
            &self.native.objects,
            &mut self.native.world,
            self.native.owner,
        )
        .unwrap();
        assert_eq!(actual.is_some(), source.last_carry);
        assert_eq!(
            actual.unwrap_or(inherited_offset),
            source.bus.read16(8) as i16
        );
        assert_eq!(
            self.native.world.surface_clipping_plane_height,
            Some(source.bus.read16(0x7024E0) as i16)
        );
        let surface = before.surface.as_mut().unwrap();
        surface.plane_height = source.bus.read16(WRAM + SLOT + 0x6A7D) as i16;
        surface.material = source.bus.read8(WRAM + SLOT + 0x6A82);
        assert_eq!(*self.records(), before);
        assert_eq!(source.bus.read8(0x5E), 1);
        for (offset, value) in [
            (0x6A7C, 0x91),
            (0x6A7F, 0x72),
            (0x6A81, 0xB3),
            (0x6A83, 0x54),
        ] {
            assert_eq!(source.bus.read8(WRAM + SLOT + offset), value);
        }
        actual
    }
    fn contact(&mut self, present: bool, flags: u8) {
        let contact = &mut self
            .native
            .objects
            .get_mut(self.native.owner)
            .unwrap()
            .extension
            .surface_contact;
        contact.supporting_object = present.then_some(self.support);
        contact.flags = flags;
    }
}

#[test]
fn surface_preparation_matches_original_every_material_and_support_gate() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source);
    for flags in 0..=u8::MAX {
        for present in [false, true] {
            for height in [i16::MIN, -1, 0, 1, 255, 256, i16::MAX] {
                f.contact(present, flags);
                f.native.world.environment_plane_height = Some(height);
                f.native.world.player_carry_mode = Some(flags ^ 0xA5);
                f.records().motion.as_mut().unwrap().surface_height = height.wrapping_neg();
                let support = f.native.objects.get_mut(f.support).unwrap();
                support.extension.path_state.script_value = u16::from(flags).wrapping_mul(193);
                support.base.position.y = height;
                f.seed_retained(&mut source);
                f.step(&mut source, 0x2345);
            }
        }
    }
}

#[test]
fn surface_preparation_matches_original_all_wrapped_height_words() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source);
    let mut admitted = 0;
    let mut rejected = 0;
    for value in 0..=u16::MAX {
        for branch in 0..4 {
            f.contact(
                branch != 0,
                match branch {
                    2 => 1,
                    3 => 4,
                    _ => 0,
                },
            );
            f.native.world.environment_plane_height = Some(value as i16);
            f.native.world.player_carry_mode = Some((value >> 8) as u8);
            f.records().motion.as_mut().unwrap().surface_height = value.wrapping_mul(197) as i16;
            let support = f.native.objects.get_mut(f.support).unwrap();
            support.extension.path_state.script_value = value.rotate_left(7);
            support.base.position.y = value as i16;
            f.seed_retained(&mut source);
            if f.step(&mut source, value.rotate_right(3) as i16).is_some() {
                admitted += 1;
            } else {
                rejected += 1;
            }
        }
    }
    assert!(admitted > 100_000 && rejected > 30_000);
}

#[test]
fn surface_preparation_matches_original_independently_retained_clipping_and_plane_transitions() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source);
    f.seed_retained(&mut source);
    let mut retained_rejections = 0;
    let mut publications = 0;
    for tick in 0_u16..8192 {
        f.contact(tick & 3 != 0, [0, 1, 4, 3, 6, 255][usize::from(tick % 6)]);
        f.native.world.player_carry_mode = Some((tick / 19) as u8);
        f.native.world.environment_plane_height = Some(if tick & 1 == 0 {
            0
        } else {
            tick.wrapping_mul(37) as i16
        });
        f.records().motion.as_mut().unwrap().surface_height = tick.rotate_left(9) as i16;
        let support = f.native.objects.get_mut(f.support).unwrap();
        support.base.position.y = tick.wrapping_mul(157) as i16;
        support.extension.path_state.script_value = tick.rotate_right(3);
        let old_clip = f.native.world.surface_clipping_plane_height;
        let actual = f.step(&mut source, tick.rotate_left(5) as i16);
        if actual.is_some() {
            publications += 1;
        } else {
            assert_eq!(f.native.world.surface_clipping_plane_height, old_clip);
            retained_rejections += 1;
        }
    }
    assert!(publications > 1000 && retained_rejections > 4000);
}

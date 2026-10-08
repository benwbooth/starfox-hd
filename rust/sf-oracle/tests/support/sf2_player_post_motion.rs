//! Continuous original post-mode prefix and its effects/attachment suffix.
//! Original and native state advance independently across repeated visits.

use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{
    attachment_poses as poses, attachment_tests, damage_effects_tests,
    frame_effects_tests as effects, rom, Source, WRAM,
};
use sf2_game::path_runtime::PathRuntime;
use sf2_game::path_scene_state::EncounterCoordination;
use sf2_game::path_sound::MusicControlRequest;
use sf2_game::player_palette::PlayerPaletteControl;
use sf2_game::player_post_motion;
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::scene_path_world::PlayerPathRecords;
use sf2_game::Vector3;

struct Fixture {
    native: Native,
    runtime: PathRuntime,
    readiness: u8,
}

impl Fixture {
    fn new(source: &mut Source, value: u16, variant: u8) -> Self {
        let mut native = effects::fixture(source, 4, value);
        attachment_tests::prepare(
            &mut native,
            variant.wrapping_mul(17),
            variant & 2 != 0,
            value,
        );
        let mut records = *native.world.player(&native.objects, native.owner).unwrap();
        let mut runtime = PathRuntime::default();
        player_storage::replace(
            &mut native.objects,
            &mut native.world,
            &mut runtime,
            native.owner,
            PlayerStorageInputs {
                pilot_code: 2,
                reserve_shield: 40,
                score: Default::default(),
            },
        )
        .unwrap();
        player_storage::get_mut(&native.objects, &mut runtime.resources, native.owner)
            .unwrap()
            .fine_yaw = value.rotate_left(7);
        records.mission = Some(sf2_game::player_mission::PlayerMissionControl {
            flags: value.rotate_left(3) as u8,
        });
        records.palette_effects = Some(PlayerPaletteControl::from_control(value as u8));
        records
            .contact
            .as_mut()
            .unwrap()
            .hit
            .deflection_sound_cooldown = value as u8;
        records.contact.as_mut().unwrap().hit.camera_pitch_recoil =
            if variant & 2 == 0 { 0 } else { value as i16 };
        records.contact.as_mut().unwrap().hit.feedback_flags = value.rotate_left(3) as u8;
        records.contact.as_mut().unwrap().hit.feedback_duration = value as u8;
        records.motion.as_mut().unwrap().contact_flags = (value >> 8) as u8;
        records.motion.as_mut().unwrap().previous_position = Vector3 {
            x: -391,
            y: 7931,
            z: -129,
        };
        records.auxiliary.as_mut().unwrap().action_flags = value.rotate_left(5) as u8;
        records.charge.as_mut().unwrap().speed_impulse = value.rotate_left(7) as i16;
        records.charge.as_mut().unwrap().speed_impulse_ticks = 137;
        native
            .world
            .bind_player(&native.objects, native.owner, records)
            .unwrap();
        native.world.scene.player_configuration = Some(9);
        native.world.scene.encounter_location = Some(if variant & 1 == 0 { 11 } else { 8 });
        native.world.coordination = Some(EncounterCoordination {
            progress: [0, 253, 254, 255][variant as usize % 4],
            ..Default::default()
        });
        native.world.contacts_enabled = Some(variant & 1 != 0);
        native.world.interception_active = Some(variant & 2 != 0);
        let readiness = if variant & 4 == 0 {
            0
        } else {
            (value as u8).max(1)
        };
        native.world.interception_music_ready = Some(readiness != 0);
        let ids = native.objects.active_ids().to_vec();
        for &id in &ids[1..3] {
            native.objects.get_mut(id).unwrap().base.child_number = 22;
        }
        native
            .objects
            .get_mut(ids[1])
            .unwrap()
            .base
            .flags
            .remove_after_tick = true;
        native.world.primary_player = Some(if variant & 8 == 0 {
            native.owner
        } else {
            ids[3]
        });
        Self {
            native,
            runtime,
            readiness,
        }
    }

    fn seed(&mut self, source: &mut Source) {
        let n = &mut self.native;
        effects::seed(source, n, 79);
        attachment_tests::seed(source, n);
        for (id, actor) in n.objects.active_objects() {
            let base = u32::from(address(Some(id)));
            source.bus.write8(base + 0x13, actor.base.child_number);
            source.bus.write8(
                base + 0x25,
                (source.bus.read8(base + 0x25) & !8)
                    | if actor.base.flags.remove_after_tick {
                        8
                    } else {
                        0
                    },
            );
        }
        let r = n.world.player(&n.objects, n.owner).unwrap();
        for (offset, value) in [
            (0x6BE7, r.contact.unwrap().hit.deflection_sound_cooldown),
            (0x6BE6, r.motion.unwrap().contact_flags),
            (0x6BE9, r.palette_effects.unwrap().bits() | 0x1F),
            (0x6A71, r.mission.unwrap().flags),
            (0x6B58, r.charge.unwrap().speed_impulse_ticks),
        ] {
            source.bus.write8(WRAM + SLOT + offset, value);
        }
        source
            .bus
            .write16(WRAM + SLOT + 0x6B56, r.charge.unwrap().speed_impulse as u16);
        source.bus.write16(
            WRAM + SLOT + 0x6ABB,
            player_storage::get(&n.objects, &self.runtime.resources, n.owner)
                .unwrap()
                .fine_yaw,
        );
        for (offset, value) in [
            (0x6AC7, r.motion.unwrap().previous_position.x),
            (0x6AC9, r.motion.unwrap().previous_position.y),
            (0x6ACB, r.motion.unwrap().previous_position.z),
        ] {
            source.bus.write16(WRAM + SLOT + offset, value as u16);
        }
        source
            .bus
            .write8(0x1DE2, n.world.scene.player_configuration.unwrap());
        source
            .bus
            .write16(0x1BB5, n.world.scene.encounter_location.unwrap());
        source
            .bus
            .write8(WRAM + 0xD787, n.world.coordination.unwrap().progress);
        assert_eq!(
            source.bus.read8(WRAM + 0xD787),
            n.world.coordination.unwrap().progress
        );
        source.bus.write16(
            0x1B8A,
            0xFFDF | u16::from(n.world.interception_active.unwrap()) * 0x20,
        );
        source.bus.write8(0x1DDE, self.readiness);
        source.bus.write16(0x12C3, address(n.world.primary_player));
        n.world
            .audio
            .request_music_control(MusicControlRequest::EncounterExit);
        source.bus.write8(0x1CDA, 2);
        source.bus.write8(0x1CD9, 0);
    }

    fn expected_prefix(&self, source: &Source, mut before: PlayerPathRecords) -> PlayerPathRecords {
        let byte = |offset| source.bus.read8(WRAM + SLOT + offset);
        let word = |offset| source.bus.read16(WRAM + SLOT + offset);
        let hit = &mut before.contact.as_mut().unwrap().hit;
        hit.deflection_sound_cooldown = byte(0x6BE7);
        hit.feedback_duration = byte(0x6C11);
        hit.feedback_flags = byte(0x6C12);
        hit.camera_pitch_recoil = word(0x6B3B) as i16;
        before.palette_effects = Some(PlayerPaletteControl::from_control(byte(0x6BE9)));
        assert_eq!(byte(0x6BE9) & 0x1F, 0x1F);
        before.mission.as_mut().unwrap().flags = byte(0x6A71);
        before.motion.as_mut().unwrap().previous_position = Vector3 {
            x: word(0x6AC7) as i16,
            y: word(0x6AC9) as i16,
            z: word(0x6ACB) as i16,
        };
        before.motion.as_mut().unwrap().contact_flags = byte(0x6BE6);
        before.charge.as_mut().unwrap().speed_impulse = word(0x6B56) as i16;
        assert_eq!(byte(0x6B58), before.charge.unwrap().speed_impulse_ticks);
        assert_eq!(
            source.bus.read8(0x1CDA),
            match self.native.world.audio.pending_music_control().unwrap() {
                MusicControlRequest::InterceptionReady => 1,
                MusicControlRequest::EncounterExit => 2,
                other => panic!("unexpected music control {other:?}"),
            }
        );
        assert_eq!(source.bus.read8(0x1CD9), 0);
        assert_eq!(source.bus.read8(0x1DDE), self.readiness);
        before
    }

    fn step(&mut self, source: &mut Source, complete: bool) {
        // Each implementation consumes its own event queue at the end of the
        // prior visit; preserve all gameplay state and every existing child.
        source.bus.write16(0x1D16, 0);
        for slot in 0..16 {
            source.bus.write16(0x1CF6 + slot * 2, 0xACE0 + slot as u16);
        }
        let n = &mut self.native;
        let before = *n.world.player(&n.objects, n.owner).unwrap();
        source.bus.write8(0, 79);
        source.run(
            0x069DBA,
            Some(if complete { 0x069FAD } else { 0x069EE8 }),
            0,
            OWNER,
            true,
        );
        if complete {
            player_post_motion::advance(
                &mut n.objects,
                &mut n.world,
                &self.runtime.resources,
                n.owner,
                Some(79),
            )
            .unwrap();
        } else {
            player_post_motion::prepare(
                &mut n.objects,
                &mut n.world,
                &self.runtime.resources,
                n.owner,
            )
            .unwrap();
        }
        let before = self.expected_prefix(source, before);
        let n = &mut self.native;
        poses::compare(&source.bus, &n.objects);
        if complete {
            effects::compare(source, n, before);
        } else {
            assert_eq!(*n.world.player(&n.objects, n.owner).unwrap(), before);
            damage_effects_tests::compare_cues(source, n);
            n.compare_pool(source);
        }
    }
}

#[test]
fn original_post_motion_all_cooldown_contact_bytes_modes_and_independent_repeated_visits() {
    let mut source = Source::new(&rom(), 0);
    for value in 0..=u16::MAX {
        for variant in [value as u8 & 15, (value as u8 & 15) ^ 15] {
            let mut f = Fixture::new(&mut source, value, variant);
            f.native
                .world
                .player_mut(&f.native.objects, f.native.owner)
                .unwrap()
                .auxiliary
                .as_mut()
                .unwrap()
                .mode = value as u8;
            f.seed(&mut source);
            for _ in 0..3 {
                f.step(&mut source, false);
            }
        }
    }
}

#[test]
fn original_post_motion_progress_configuration_location_words_music_gates_and_latches() {
    let mut source = Source::new(&rom(), 0);
    for value in 0..=u16::MAX {
        let mut f = Fixture::new(&mut source, value, value as u8 & 15);
        f.native.world.scene.player_configuration = Some(value as u8);
        f.native.world.coordination.as_mut().unwrap().progress = (value >> 8) as u8;
        f.seed(&mut source);
        f.step(&mut source, false);
    }
    for location in 0..=u16::MAX {
        let mut f = Fixture::new(&mut source, location, 7);
        f.native.world.scene.encounter_location = Some(location);
        f.native.world.coordination.as_mut().unwrap().progress =
            if location & 1 == 0 { 254 } else { 255 };
        f.seed(&mut source);
        f.step(&mut source, false);
    }
    for flags in 0..=255u16 {
        for gates in 0..16 {
            for location in [8, 11] {
                let mut f = Fixture::new(&mut source, flags, gates);
                f.native.world.scene.encounter_location = Some(location);
                f.native.world.coordination.as_mut().unwrap().progress =
                    if location == 11 { 255 } else { 254 };
                f.native
                    .world
                    .player_mut(&f.native.objects, f.native.owner)
                    .unwrap()
                    .mission
                    .as_mut()
                    .unwrap()
                    .flags = flags as u8;
                f.seed(&mut source);
                f.step(&mut source, false);
                f.step(&mut source, false);
            }
        }
    }
}

#[test]
fn original_post_motion_continuous_effects_recovery_and_new_attachment_publication() {
    let mut source = Source::new(&rom(), 0);
    source.bus.enable_gsu();
    for value in 0..=255u16 {
        for variant in 0..16 {
            let mut f = Fixture::new(&mut source, value * 257, variant);
            let n = &mut f.native;
            let r = n.world.player_mut(&n.objects, n.owner).unwrap();
            r.auxiliary.as_mut().unwrap().mode = value as u8;
            r.particles.as_mut().unwrap().flags = [0, 0x20, 0x60, 0x80][variant as usize % 4];
            r.particles.as_mut().unwrap().age = if variant & 2 == 0 { 24 } else { 128 };
            r.contact.as_mut().unwrap().hit.reserve_shield = value as u8 % 80;
            n.world.shield_recovery.as_mut().unwrap().amount = if variant & 4 == 0 { 0 } else { 5 };
            n.world.action_gate.as_mut().unwrap().code = variant & 2;
            f.seed(&mut source);
            f.step(&mut source, true);
            f.step(&mut source, true);
        }
    }
}

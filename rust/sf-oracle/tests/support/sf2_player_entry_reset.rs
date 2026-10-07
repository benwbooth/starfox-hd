//! The unmodified entry prefix and reset tail, including shared audio state.
use super::motion_reset_tests::Reader;
use super::{actor, rom, Source, OTHER, OWNER, WRAM};
use sf2_game::path_motion::PublishedPlayerMotion;
use sf2_game::path_runtime::PathRuntime;
use sf2_game::path_scene_state::EncounterHandoff;
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::positional_audio::{LoopListener, LoopSelection, PositionalAudio};
use sf2_game::scene_path_world::ScenePathWorld;
use sf2_game::{Angle, ObjectId, ObjectSpawnDefaults, ObjectStore, ShapeId, SpatialLoop, Vector3};

struct Fixture {
    source: Source,
    objects: ObjectStore,
    world: ScenePathWorld,
    runtime: PathRuntime,
    audio: PositionalAudio,
    owner: ObjectId,
    other: ObjectId,
    slot: u32,
}

impl Fixture {
    fn new(rom: &[u8], seed: u8) -> Self {
        let mut source = Source::new(rom, 0);
        source.run(0x7F1737, None, 0, OWNER, true);
        source.run(0x0DD5B7, None, 0, OWNER, true);
        for (offset, &byte) in rom[0x50000..0x54E00].iter().enumerate() {
            source.bus.write8(0x7F7E00 + offset as u32, byte);
        }
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects);
        let other = actor(&mut objects);
        let mut world = ScenePathWorld::new(Default::default());
        let mut runtime = PathRuntime::default();
        source.run(0x068260, Some(0x0682B7), 0, OWNER, true);
        player_storage::replace(
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
        let slot = u32::from(source.bus.read16(u32::from(OWNER) + 0x2B));
        for index in 0..472 {
            source.bus.write8(
                WRAM + slot + 0x6A61 + index,
                seed.wrapping_add((index as u8).wrapping_mul(73)),
            );
        }
        for field in [0x6A98, 0x6BB8, 0x6BC8, 0x6BCA] {
            source.bus.write16(WRAM + slot + field, OTHER);
        }
        for (field, pointer, bank) in [
            (0x6A9A, 0x8048, 7),
            (0x6A9D, 0x9DF6, 7),
            (0x6C13, 0xBDDA, 0x0D),
        ] {
            source.bus.write16(WRAM + slot + field, pointer);
            source.bus.write8(WRAM + slot + field + 2, bank);
        }
        let reader = Reader {
            source: &source,
            slot,
            other,
        };
        *player_storage::get_mut(&objects, &mut runtime.resources, owner).unwrap() =
            reader.storage();
        *world.player_mut(&objects, owner).unwrap() = reader.records();
        world
            .bind_shots(
                &objects,
                owner,
                sf2_game::path_shots::ActiveShots::from_count(
                    source.bus.read8(WRAM + slot + 0x6C03),
                ),
            )
            .unwrap();
        world.spawn_defaults = Some(ObjectSpawnDefaults {
            group: seed,
            run_when_paused: true,
        });
        world.region_registration_count = Some(seed ^ 0x71);
        world.secondary_region_group = Some(seed ^ 0xA9);
        for (address, value) in [(0x190E, seed), (0x190F, seed ^ 0xA9), (0x1910, seed ^ 0x71)] {
            source.bus.write8(address, value);
        }
        world.render_environment.ambient_control = Some(
            sf2_game::player_surface_render::AmbientParticleControl::from_bits(
                0xCB00 | u16::from(seed),
            ),
        );
        source.bus.write16(0x7001BC, 0xCB00 | u16::from(seed));
        world.scene.active_weapon_level = Some(seed);
        world.active_consumables = Some(sf2_game::player_visit::PublishedConsumables {
            packed_count: seed ^ 0xB3,
            kind: seed ^ 0xE7,
        });
        for (address, value) in [(0x1DD4, seed), (0x1DD2, seed ^ 0xB3), (0x1DD3, seed ^ 0xE7)] {
            source.bus.write8(address, value);
        }
        world.surface_mode = Some(sf2_game::collision_surface::SurfaceMode { flags: seed });
        source.bus.write8(0x1B4D, seed);
        world.handoff = Some(EncounterHandoff {
            player_flags: seed,
            x: -397,
            z: 1957,
            heading_word: 917,
        });
        source.bus.write8(0x1D74, seed);
        world.weapons = Some(Default::default());
        let position = Vector3 {
            x: -397,
            y: 977,
            z: 317,
        };
        let delta = Vector3 {
            x: 31733,
            y: -3973,
            z: -1397,
        };
        world.published_motion = Some(PublishedPlayerMotion { position, delta });
        for (address, value) in [
            (0xD7EC, position.x),
            (0xD7EE, position.y),
            (0xD7F0, position.z),
            (0x1E1C, delta.x),
            (0x1E1E, delta.y),
            (0x1E20, delta.z),
        ] {
            source.bus.write16(WRAM + address, value as u16);
        }
        Self {
            source,
            objects,
            world,
            runtime,
            audio: Default::default(),
            owner,
            other,
            slot,
        }
    }

    fn select(&mut self, z: i16) -> LoopSelection {
        let actor = self.objects.get_mut(self.owner).unwrap();
        actor.base.position.z = z;
        actor.extension.spatial_loop = SpatialLoop::from_authored_control(5);
        self.audio.begin_epoch();
        self.audio
            .observe(
                self.owner,
                actor,
                false,
                Some(LoopListener {
                    position: Vector3::default(),
                    bearing: Angle::ZERO,
                }),
            )
            .unwrap();
        let selection = self.audio.pending().unwrap();
        self.source.bus.write8(0x1CEC, selection.control);
        self.source.bus.write8(0x1CED, 5);
        self.source.bus.write16(0x1CEE, OWNER);
        self.source.bus.write16(0x1CF4, selection.distance);
        selection
    }

    fn compare(&self) {
        let reader = Reader {
            source: &self.source,
            slot: self.slot,
            other: self.other,
        };
        assert_eq!(
            self.world.player(&self.objects, self.owner).unwrap(),
            &reader.records()
        );
        assert_eq!(
            player_storage::get(&self.objects, &self.runtime.resources, self.owner).unwrap(),
            &reader.storage()
        );
        assert_eq!(
            self.world.shots(&self.objects, self.owner).unwrap().count(),
            self.source.bus.read8(WRAM + self.slot + 0x6C03)
        );
        assert_eq!(
            self.objects.get(self.owner).unwrap().base.shape,
            ShapeId::EMPTY
        );
        assert_eq!(self.source.bus.read16(u32::from(OWNER) + 4), 0xBC9C);
        assert_eq!(
            self.objects
                .get(self.owner)
                .unwrap()
                .base
                .flags
                .collision_disabled,
            self.source.bus.read8(u32::from(OWNER) + 0x21) & 1 != 0
        );
        assert_eq!(
            self.world.engine_sound_control.unwrap().bits(),
            self.source.bus.read8(0x1CE5)
        );
        assert_eq!(
            self.world.handoff.unwrap().player_flags,
            self.source.bus.read8(0x1D74)
        );
        let counts = self.world.weapons.unwrap().hostile_counts;
        assert_eq!(counts.collision_disabled, self.source.bus.read8(0x1D69));
        assert_eq!(counts.aligned_half_plane, self.source.bus.read8(0x1D6B));
        let motion = self.world.published_motion.unwrap();
        for (address, value) in [
            (0xD7EC, motion.position.x),
            (0xD7EE, motion.position.y),
            (0xD7F0, motion.position.z),
            (0x1E1C, motion.delta.x),
            (0x1E1E, motion.delta.y),
            (0x1E20, motion.delta.z),
        ] {
            assert_eq!(self.source.bus.read16(WRAM + address), value as u16);
        }
        assert_eq!(
            self.audio.published_control(),
            self.source.bus.read8(0x1CE6)
        );
        if let Some(selection) = self.audio.retained_selection() {
            assert_eq!(
                self.source.bus.read8(0x1CE7),
                selection.sound.sound.authored_control()
            );
            assert_eq!(self.source.bus.read16(0x1CE8), OWNER);
            assert_eq!(self.source.bus.read16(0x1CEA), selection.distance);
        }
        if let Some(selection) = self.audio.pending() {
            assert_eq!(self.source.bus.read8(0x1CEC), selection.control);
            assert_eq!(
                self.source.bus.read8(0x1CED),
                selection.sound.sound.authored_control()
            );
            assert_eq!(self.source.bus.read16(0x1CEE), OWNER);
            assert_eq!(self.source.bus.read16(0x1CF4), selection.distance);
        }
    }
}

#[test]
fn entry_tail_matches_original_every_action_clock_and_preserves_silenced_audio_identity() {
    let mut f = Fixture::new(&rom(), 173);
    let selected = f.select(100);
    f.source.run(0x03815A, Some(0x038192), 0, OWNER, true);
    f.audio.publish(false);
    let pending = f.select(3000);
    for counter in 0..=u16::MAX {
        for (field, value) in [
            (0x6C16, counter),
            (0x6C18, !counter),
            (0x6C1A, counter.rotate_left(3)),
        ] {
            f.source.bus.write16(WRAM + f.slot + field, value);
        }
        f.source.bus.write16(WRAM + f.slot + 0x6C13, 0xBF63);
        f.source.bus.write8(WRAM + f.slot + 0x6C15, 0x0D);
        let camera = [0x8048, 0x8089, 0x80B3][counter as usize % 3];
        f.source.bus.write16(WRAM + f.slot + 0x6A9A, camera);
        f.source.bus.write8(WRAM + f.slot + 0x6A9C, 7);
        *f.world.player_mut(&f.objects, f.owner).unwrap() = Reader {
            source: &f.source,
            slot: f.slot,
            other: f.other,
        }
        .records();
        f.source.bus.write8(0x1D74, counter as u8);
        f.world.handoff.as_mut().unwrap().player_flags = counter as u8;
        f.source.bus.write8(u32::from(OWNER) + 0x21, counter as u8);
        f.objects
            .get_mut(f.owner)
            .unwrap()
            .base
            .flags
            .collision_disabled = counter & 1 != 0;
        f.source.bus.write16(0x1D69, counter);
        f.source.bus.write16(0x1D6B, counter.rotate_left(5));
        f.source.bus.write16(0x1D6D, !counter);
        f.world.weapons.as_mut().unwrap().hostile_counts =
            sf2_game::weapon_launch::HostileLaunchCounts {
                collision_disabled: counter as u8,
                aligned_half_plane: counter.rotate_left(5) as u8,
            };
        f.source.run(0x068413, Some(0x06846C), 0, OWNER, true);
        sf2_game::player_entry_reset::finish(&mut f.objects, &mut f.world, &mut f.audio, f.owner)
            .unwrap();
        f.compare();
        assert_eq!(
            f.source.bus.read8(u32::from(OWNER) + 0x21),
            counter as u8 | 1
        );
        assert_eq!(f.audio.retained_selection(), Some(selected));
        assert_eq!(f.audio.pending(), Some(pending));
        assert_eq!(f.audio.published(), None);
        for address in [0x1D69, 0x1D6B, 0x1D6D] {
            assert_eq!(f.source.bus.read16(address), 0);
        }
    }
    f.source.bus.write8(0x1B84, 1);
    f.source.run(0x03815A, Some(0x038192), 0, OWNER, true);
    f.audio.publish(true);
    f.compare();
    assert_eq!(f.audio.published(), None);
    f.source.bus.write8(0x1B84, 0);
    f.source.run(0x03815A, Some(0x038192), 0, OWNER, true);
    f.audio.publish(false);
    f.compare();
    assert_eq!(f.audio.published(), Some(pending));
}

#[test]
fn complete_entry_reset_matches_original_dirty_storage_real_proxy_clear_and_repeated_visits() {
    let rom = rom();
    for seed in 0..=u8::MAX {
        let mut f = Fixture::new(&rom, seed);
        let ids = f.objects.active_ids().to_vec();
        let owner = f.owner;
        let address = |id| if id == owner { OWNER } else { OTHER };
        f.source.bus.write16(0x12A8, address(ids[0]));
        for (index, &id) in ids.iter().enumerate() {
            let base = u32::from(address(id));
            f.source
                .bus
                .write16(base, ids.get(index + 1).map_or(0, |&id| address(id)));
            f.source.bus.write8(base + 0x22, 4);
            f.objects
                .get_mut(id)
                .unwrap()
                .base
                .flags
                .general_search_eligible = true;
        }
        f.source.bus.write16(u32::from(OTHER) + 4, 0xBC9C);
        f.source.bus.write16(u32::from(OTHER) + 0x2B, 0x8FFE);
        f.source.run(0x7FAF00, Some(0x7FCAE8), 0, OTHER, true);
        let proxy = f
            .world
            .proxies
            .capture_actor(
                &mut f.objects,
                f.other,
                sf2_game::PathCursor {
                    path: sf2_game::PathId::from_catalog_index(2),
                    command_index: 0x8FFF,
                },
                &f.runtime.resources,
            )
            .unwrap()
            .unwrap();
        let allocation = f.objects.get(f.owner).unwrap().base.player_storage;
        let resources = f.runtime.resources.clone();
        let selected = f.select(100);
        f.source.run(0x03815A, Some(0x038192), 0, OWNER, true);
        f.audio.publish(false);
        let pending = f.select(3000);
        for _ in 0..2 {
            f.source.run(0x0683F1, Some(0x06846C), 0, OWNER, true);
            sf2_game::player_entry_reset::reset(
                &mut f.objects,
                &mut f.world,
                &mut f.runtime.resources,
                &mut f.audio,
                f.owner,
            )
            .unwrap();
            f.compare();
            assert!(f.world.proxies.get(proxy).is_none());
            assert_eq!(f.source.bus.read16(0x1285), 0);
            assert_eq!(f.objects.get(f.other).unwrap().extension.scene_proxy, None);
            assert_eq!(f.source.bus.read16(WRAM + u32::from(OTHER) + 0x1CE6), 0);
            assert_eq!(
                f.objects.get(f.owner).unwrap().base.player_storage,
                allocation
            );
            assert_eq!(
                f.runtime.resources.available_capacity(),
                resources.available_capacity()
            );
            assert_eq!(
                f.runtime.resources.owner_count(f.owner),
                resources.owner_count(f.owner)
            );
            for id in ids.iter().copied() {
                assert!(f.objects.get(id).unwrap().base.flags.remove_after_tick);
                assert_ne!(f.source.bus.read8(u32::from(address(id)) + 0x25) & 8, 0);
            }
            assert_eq!(
                f.world.region_registration_count,
                Some(f.source.bus.read8(0x1910))
            );
            assert_eq!(
                f.world.spawn_defaults.unwrap().group,
                f.source.bus.read8(0x190E)
            );
            assert_eq!(
                f.world.secondary_region_group,
                Some(f.source.bus.read8(0x190F))
            );
            assert_eq!(
                f.world.occupancy,
                Some(sf2_game::world_occupancy::WorldOccupancy::fully_occupied())
            );
            for offset in 0..2048 {
                assert_eq!(f.source.bus.read8(WRAM + 0xCF36 + offset), 255);
            }
            assert_eq!(f.audio.retained_selection(), Some(selected));
            assert_eq!(f.audio.pending(), Some(pending));
            assert_eq!(
                f.world.render_environment.ambient_control.unwrap().bits(),
                f.source.bus.read16(0x7001BC)
            );
            assert_eq!(
                f.world.surface_mode.unwrap().flags,
                f.source.bus.read8(0x1B4D)
            );
        }
    }
}

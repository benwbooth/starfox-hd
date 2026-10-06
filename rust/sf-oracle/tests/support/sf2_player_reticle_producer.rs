//! Full original reticle mode and host/graphics projection services. No
//! source instruction is replaced; the host launches both original jobs.

use super::*;
use sf2_game::intro_projection::ProjectionViewport;
use sf2_game::path_scene_state::EncounterObjectiveCounts;
use sf2_game::player_action::PlayerServiceFlags;
use sf2_game::player_reticle::{self, MarkerProjection, ReticleDisplay, ReticleError};
use sf2_game::player_target_lock::TargetReticle;
use sf2_game::scene_path_world::PlayerPathRecords;
use sf2_game::view_transition::ViewTransitionMode;

const STORAGE: u16 = 0x0200;
const CAMERA: u16 = 0x033F;

fn records() -> PlayerPathRecords {
    PlayerPathRecords {
        reticle_display: Some(ReticleDisplay::default()),
        contact: Some(Default::default()),
        charge: Some(Default::default()),
        rapid_aim: Some(Default::default()),
        ..Default::default()
    }
}

#[test]
fn reticle_mode_matches_original_all_bytes_and_gate_combinations() {
    let mut source = Source::new(&rom(), 0xA7);
    source.writes = Some(Vec::new());
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    let unrelated = actor(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::default());
    world.primary_player = Some(unrelated); // service belongs to its caller
    world.bind_player(&objects, owner, records()).unwrap();
    world.bind_player(&objects, unrelated, records()).unwrap();
    source.bus.write16(WRAM + u32::from(OWNER) + 0x2B, STORAGE);
    let allocation = WRAM + 0x6A67 + u32::from(STORAGE);
    let initial: Vec<_> = (0..472)
        .map(|offset| source.bus.read8(allocation + offset))
        .collect();
    for byte in 0..=u8::MAX {
        for gates in 0..128u8 {
            let inhibit = (u16::from(byte) << 8 & !0x0100)
                | u16::from(byte)
                | if gates & 1 != 0 { 0x0100 } else { 0 };
            let mode =
                (u16::from(byte) << 8) | u16::from(byte & !2) | if gates & 2 != 0 { 2 } else { 0 };
            let contacts = if gates & 4 != 0 { 0 } else { byte.max(1) };
            let services = (byte & !1) | u8::from(gates & 8 != 0);
            let enabled = (byte & !0x80) | if gates & 16 == 0 { 0x80 } else { 0 };
            let protection = (byte & !0x80) | if gates & 32 != 0 { 0x80 } else { 0 };
            let reflection = (byte & !2) | if gates & 64 != 0 { 2 } else { 0 };
            source.bus.write16(WRAM + 0x1B96, inhibit);
            source.bus.write16(WRAM + 0x1B84, mode);
            source
                .bus
                .write16(WRAM + 0xD7F4, u16::from_le_bytes([contacts, !byte]));
            for (address, value) in [
                (0x1E0D, services),
                (0x1E2F, enabled),
                (0x1AA6, reflection),
                (u32::from(OWNER) + 0x16, byte.rotate_left(3)),
                (u32::from(STORAGE) + 0x6B7D, protection),
                (u32::from(STORAGE) + 0x6B63, byte),
                (u32::from(STORAGE) + 0x6BA2, !byte),
                (u32::from(STORAGE) + 0x6BA3, byte),
                (u32::from(STORAGE) + 0x6BA4, byte ^ 0xB7),
            ] {
                source.bus.write8(WRAM + address, value);
            }
            objects.get_mut(owner).unwrap().base.roll = Angle::from_units(byte.rotate_left(3));
            world.reticle_inhibited = Some(inhibit & 0x0100 != 0);
            world.reticle_enabled = Some(enabled & 0x80 != 0);
            world.view_transition_mode = Some(ViewTransitionMode { flags: mode });
            world.contacts_enabled = Some(contacts == 0); // deliberately stale
            world.objective_counts = Some(EncounterObjectiveCounts {
                remaining_word: u16::from_le_bytes([contacts, !byte]),
                ..Default::default()
            });
            world.player_service_flags = Some(PlayerServiceFlags::from_bits(services));
            world.reflect_all_contacts = Some(reflection & 2 != 0);
            let player = world.player_mut(&objects, owner).unwrap();
            player.reticle_display = Some(ReticleDisplay {
                roll: Angle::from_units(!byte),
                mode_flags: byte,
                display_flags: byte ^ 0xB7,
            });
            player
                .contact
                .as_mut()
                .unwrap()
                .hit
                .hold_secondary_protection = protection & 0x80 != 0;
            player.charge.as_mut().unwrap().linked_mode = byte & 0x80 != 0;
            player.charge.as_mut().unwrap().linked_muzzle_disabled = byte & 0x40 != 0;
            let unchanged = objects.clone();
            source.run(0x07B038, None, 0, OWNER, true);
            player_reticle::prepare(&objects, &mut world, owner).unwrap();
            let display = world
                .player(&objects, owner)
                .unwrap()
                .reticle_display
                .unwrap();
            assert_eq!(objects, unchanged);
            assert_eq!(*world.player(&objects, unrelated).unwrap(), records());
            let mut expected = initial.clone();
            for (field, value) in [
                (0x6B7D, protection),
                (0x6B63, byte),
                (0x6BA2, display.roll.units()),
                (0x6BA3, display.mode_flags),
                (0x6BA4, display.display_flags),
            ] {
                expected[field - 0x6A67] = value;
            }
            for (offset, value) in expected.into_iter().enumerate() {
                assert_eq!(
                    source.bus.read8(allocation + offset as u32),
                    value,
                    "{byte} {gates} field {:04X}",
                    0x6A67 + offset
                );
            }
            for &(address, value) in source.writes.as_ref().unwrap() {
                if address < 0x033F {
                    continue;
                }
                assert!(
                    (WRAM + u32::from(STORAGE) + 0x6BA2..=WRAM + u32::from(STORAGE) + 0x6BA4)
                        .contains(&address),
                    "unexpected write {address:06X}={value:02X}"
                );
            }
        }
    }
}

#[test]
fn reticle_mode_missing_inputs_match_original_mutation_prefixes() {
    let mut source = Source::new(&rom(), 0xA7);
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::default());
    world.bind_player(&objects, owner, records()).unwrap();
    source.bus.write16(WRAM + u32::from(OWNER) + 0x2B, STORAGE);
    for missing_reflection in [false, true] {
        for byte in 0..=u8::MAX {
            let display = ReticleDisplay {
                roll: Angle::from_units(!byte),
                mode_flags: byte,
                display_flags: 0xFF,
            };
            world.player_mut(&objects, owner).unwrap().reticle_display = Some(display);
            world.reticle_inhibited = missing_reflection.then_some(true);
            for (address, value) in [(0x6BA2, !byte), (0x6BA3, byte), (0x6BA4, 0xFF)] {
                source
                    .bus
                    .write8(WRAM + u32::from(STORAGE) + address, value);
            }
            source.bus.write16(WRAM + 0x1B96, 0x0100);
            source.run(
                0x07B038,
                Some(if missing_reflection {
                    0x07B0B7
                } else {
                    0x07B048
                }),
                0,
                OWNER,
                true,
            );
            assert_eq!(
                player_reticle::prepare(&objects, &mut world, owner),
                Err(if missing_reflection {
                    ReticleError::MissingReflectionMode
                } else {
                    ReticleError::MissingInhibition
                })
            );
            let display = world
                .player(&objects, owner)
                .unwrap()
                .reticle_display
                .unwrap();
            assert_eq!(
                [
                    display.roll.units(),
                    display.mode_flags,
                    display.display_flags
                ],
                [0x6BA2, 0x6BA3, 0x6BA4]
                    .map(|address| source.bus.read8(WRAM + u32::from(STORAGE) + address))
            );
        }
    }
}

struct ProjectionCheck {
    source: Source,
    objects: ObjectStore,
    world: ScenePathWorld,
    owner: ObjectId,
    proxy: ObjectId,
    view: ObjectId,
    target: ObjectId,
    retain: bool,
}

impl ProjectionCheck {
    fn new() -> Self {
        let mut source = Source::new(&rom(), 0xA7);
        source.bus.enable_gsu();
        source.writes = Some(Vec::new());
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects);
        let proxy = actor(&mut objects);
        let view = actor(&mut objects);
        let target = actor(&mut objects);
        let mut world = ScenePathWorld::new(RandomState::default());
        world.primary_player = Some(owner);
        world.fixed_players[0] = Some(view);
        world.weapons = Some(sf2_game::weapon_dispatch::WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        world.bind_player(&objects, owner, records()).unwrap();
        Self {
            source,
            objects,
            world,
            owner,
            proxy,
            view,
            target,
            retain: false,
        }
    }

    fn compare(
        &mut self,
        angles: [u8; 3],
        aim: [i16; 3],
        origin: [i16; 3],
        matrix: [[i16; 3]; 3],
        view_angles: FixedViewAngles,
        viewport: ProjectionViewport,
        previous: [u8; 2],
        flags: [u8; 2],
    ) {
        let source = &mut self.source;
        if self.retain {
            use sf2_game::path_program::SelectedAuxiliaryState;
            use sf2_game::path_target::TargetingUpgradeState;
            use sf2_game::player_target_lock::TargetLock;
            source.bus.write16(WRAM + 0x12C3, OWNER);
            source.bus.write8(WRAM + 0x1DDD, 0x80);
            source.bus.write16(WRAM + 0xD7F4, 1);
            source.bus.write16(WRAM + 0x1B84, 0);
            source.bus.write16(WRAM + 0x1D90, OTHER);
            source.bus.write8(WRAM + 0x1D16, 0);
            for slot in 0..16 {
                source.bus.write16(WRAM + 0x1CF6 + slot * 2, 0xA7A7);
            }
            let base = WRAM + u32::from(STORAGE);
            for (address, value) in [
                (0x6B77, 1),
                (0x6BC2, 0xD7),
                (0x6BC6, previous[1]),
                (0x6BC7, previous[0]),
                (0x6BB7, 0xAF),
            ] {
                source.bus.write8(base + address, value);
            }
            for (address, value) in [
                (0x6BB8, 0x0800),
                (0x6BC8, 0x0800),
                (0x6BCA, if previous[0] & 1 == 0 { 0 } else { 0x0800 }),
            ] {
                source.bus.write16(base + address, value);
            }
            self.world.targeting_upgrade = Some(TargetingUpgradeState { pilot_flags: 0x80 });
            self.world.contacts_enabled = Some(true);
            self.world.view_transition_mode = Some(ViewTransitionMode::default());
            self.world.audio = Default::default();
            let player = self.world.player_mut(&self.objects, self.owner).unwrap();
            player.auxiliary = Some(SelectedAuxiliaryState {
                mode: 0,
                action_flags: 1,
                stored_world_position: Default::default(),
                stored_rotation: Default::default(),
            });
            player.target_selection = Some(TargetSelection {
                candidate: Some(self.target),
                forced_owner: (previous[0] & 1 != 0).then_some(self.target),
                control_flags: 0xD7,
                screen: [112, 96],
                ..Default::default()
            });
            player.target_lock = Some(TargetLock {
                previous_candidate: Some(self.target),
                acquisition_clock: previous[1],
                grace_remaining: previous[0],
                marker_style: 0xAF,
            });
            source_target(
                source,
                u32::from(STORAGE),
                player.target_selection.unwrap(),
                |_| 0x0800,
            );
            self.world.published_homing_target =
                Some(sf2_game::path_target::PublishedHomingTarget {
                    object: Some(self.proxy),
                });
        }
        source.bus.write16(WRAM + u32::from(OWNER) + 0x2B, STORAGE);
        source.bus.write16(WRAM + 0x14D6, OTHER);
        source.bus.write8(WRAM + 0x1E2F, flags[0]);
        source
            .bus
            .write8(WRAM + u32::from(STORAGE) + 0x6BA3, flags[1]);
        source.bus.write8(WRAM + 0x1E30, previous[0]);
        source.bus.write8(WRAM + 0x1E31, previous[1]);
        source.bus.write8(WRAM + 0x1913, 1); // already loaned by the outer display
        source.bus.write8(WRAM + 0x5E, 0x18);
        for (index, value) in angles.into_iter().enumerate() {
            source
                .bus
                .write8(WRAM + u32::from(OWNER) + 0x12 + index as u32 * 2, value);
            source
                .bus
                .write8(WRAM + u32::from(OTHER) + 0x12 + index as u32 * 2, !value);
        }
        for (index, value) in aim.into_iter().enumerate() {
            source.bus.write16(
                WRAM + u32::from(STORAGE) + 0x6B9C + index as u32 * 2,
                value as u16,
            );
            source.bus.write16(
                WRAM + u32::from(OTHER) + 0x0C + index as u32 * 2,
                !value as u16,
            );
        }
        for (index, value) in origin.into_iter().enumerate() {
            source.bus.write16(
                WRAM + u32::from(CAMERA) + 0x39 + index as u32 * 2,
                value as u16,
            );
            source.bus.write16(
                WRAM + u32::from(CAMERA) + 0x0C + index as u32 * 2,
                !value as u16,
            );
        }
        for (index, value) in [view_angles.pitch, view_angles.yaw, view_angles.roll]
            .into_iter()
            .enumerate()
        {
            source
                .bus
                .write16(WRAM + u32::from(CAMERA) + 0x12 + index as u32 * 2, value);
            source.bus.write16(WRAM + 0x1595 + index as u32 * 2, !value);
        }
        for (index, value) in matrix.into_iter().flatten().enumerate() {
            source
                .bus
                .write16(0x7000E4 + index as u32 * 2, value as u16);
        }
        for (index, value) in [
            viewport.center[0],
            viewport.center[1],
            viewport.left,
            viewport.right,
            viewport.top,
            viewport.bottom,
        ]
        .into_iter()
        .enumerate()
        {
            source
                .bus
                .write16(0x700034 + index as u32 * 2, value as u16);
        }
        let native = &mut self.objects.get_mut(self.owner).unwrap().base;
        native.pitch = Angle::from_units(angles[0]);
        native.yaw = Angle::from_units(angles[1]);
        native.roll = Angle::from_units(angles[2]);
        let native = &mut self.objects.get_mut(self.proxy).unwrap().base;
        native.pitch = Angle::from_units(!angles[0]);
        native.yaw = Angle::from_units(!angles[1]);
        native.roll = Angle::from_units(!angles[2]);
        native.position = Vector3 {
            x: !aim[0],
            y: !aim[1],
            z: !aim[2],
        };
        let player = self.world.player_mut(&self.objects, self.owner).unwrap();
        player.rapid_aim.as_mut().unwrap().retained_aim = Vector3 {
            x: aim[0],
            y: aim[1],
            z: aim[2],
        };
        player.reticle_display.as_mut().unwrap().mode_flags = flags[1];
        let view = self.objects.get_mut(self.view).unwrap();
        view.base.position = Vector3 {
            x: !origin[0],
            y: !origin[1],
            z: !origin[2],
        };
        view.extension.path_state.platform_carry.saved_position = Vector3 {
            x: origin[0],
            y: origin[1],
            z: origin[2],
        };
        view_angles.write_to(view);
        self.world.reticle_enabled = Some(flags[0] & 0x80 != 0);
        self.world.target_reticle = TargetReticle {
            horizontal: Some(previous[0]),
            vertical: Some(previous[1]),
        };
        self.world.marker_projection = MarkerProjection {
            view_matrix: Some(matrix),
            viewport: Some(viewport),
            published_view_angles: Some(FixedViewAngles {
                pitch: !view_angles.pitch,
                yaw: !view_angles.yaw,
                roll: !view_angles.roll,
            }),
        };
        let mut expected_objects = self.objects.clone();
        let mut expected_proxy: Vec<_> = (0..63)
            .map(|offset| source.bus.read8(WRAM + u32::from(OTHER) + offset))
            .collect();
        let before_kicks = source.bus.gsu_kicks;
        source.run_with_y(
            0x07A418,
            Some(if self.retain { 0x07A66C } else { 0x07A505 }),
            0,
            OWNER,
            true,
            Some(STORAGE),
        );
        if self.retain {
            let mut execution = sf2_game::scene_strategy::SceneExecution::default();
            let mut callbacks = Callbacks;
            let catalog = sf2_game::path_program::PathCatalog::new(Vec::new()).unwrap();
            sf2_game::scene_strategy::SceneActors {
                objects: &mut self.objects,
                world: &mut self.world,
                execution: &mut execution,
                catalog: &catalog,
                callbacks: &mut callbacks,
                statement_budget: 64,
            }
            .position_and_retain_primary_target()
            .unwrap();
            let player = self.world.player(&self.objects, self.owner).unwrap();
            let selection = player.target_selection.unwrap();
            let lock = player.target_lock.unwrap();
            let source_id = |id: Option<ObjectId>| {
                if id == Some(self.target) {
                    0x0800
                } else if id.is_none() {
                    0
                } else {
                    panic!("unexpected target")
                }
            };
            let base = WRAM + u32::from(STORAGE);
            for (field, value) in [
                (0x6BB7, lock.marker_style),
                (0x6BC2, selection.control_flags),
                (0x6BC6, lock.acquisition_clock),
                (0x6BC7, lock.grace_remaining),
            ] {
                assert_eq!(
                    source.bus.read8(base + field),
                    value,
                    "field={field:04X} angles={angles:?} previous={previous:?} flags={flags:?}"
                );
            }
            for (field, value) in [
                (0x6BC8, source_id(lock.previous_candidate)),
                (0x6BCA, source_id(selection.forced_owner)),
            ] {
                assert_eq!(source.bus.read16(base + field), value);
            }
            assert_eq!(
                source.bus.read16(WRAM + 0x1D90),
                source_id(self.world.published_homing_target.unwrap().object)
            );
            assert_source_target(
                source,
                u32::from(STORAGE),
                selection,
                |_| 0x0800,
                "reticle-to-retention",
            );
            let events: Vec<_> = self
                .world
                .audio
                .take_events()
                .into_iter()
                .flatten()
                .collect();
            assert!(
                events.is_empty()
                    || events
                        == vec![sf2_game::SoundEvent::Authored(
                            sf2_game::path_sound::AuthoredCue::new(
                                59,
                                0,
                                sf2_game::path_control::PlayerTarget::Primary
                            )
                        )]
            );
            assert_eq!(source.bus.read8(WRAM + 0x1D16), events.len() as u8 * 2);
            for slot in 0..16 {
                assert_eq!(
                    source.bus.read16(WRAM + 0x1CF6 + slot * 2),
                    if slot == 0 && !events.is_empty() {
                        59
                    } else {
                        0xA7A7
                    }
                );
            }
        } else {
            player_reticle::position(&mut self.objects, &mut self.world, self.owner).unwrap();
        }
        assert_eq!(
            self.world.target_reticle.horizontal,
            Some(source.bus.read8(WRAM + 0x1E30)),
            "{angles:?} {aim:?} {origin:?} {matrix:?} {flags:?}"
        );
        assert_eq!(
            self.world.target_reticle.vertical,
            Some(source.bus.read8(WRAM + 0x1E31)),
            "{angles:?} {aim:?} {origin:?} {matrix:?} {flags:?}"
        );
        let published = self.world.marker_projection.published_view_angles.unwrap();
        assert_eq!(
            [published.pitch, published.yaw, published.roll],
            [0x1595, 0x1597, 0x1599].map(|address| source.bus.read16(WRAM + address))
        );
        let active = flags[0] & 0x80 != 0 && flags[1] & 0x40 != 0;
        assert_eq!(
            source.bus.gsu_kicks - before_kicks,
            if active { 2 } else { 0 }
        );
        if active {
            let proxy = &self.objects.get(self.proxy).unwrap().base;
            for (index, value) in [proxy.pitch.units(), proxy.yaw.units(), proxy.roll.units()]
                .into_iter()
                .enumerate()
            {
                expected_proxy[0x12 + index * 2] = value;
            }
            for (index, value) in [proxy.position.x, proxy.position.y, proxy.position.z]
                .into_iter()
                .enumerate()
            {
                expected_proxy[0x0C + index * 2..0x0E + index * 2]
                    .copy_from_slice(&value.to_le_bytes());
            }
            let expected = &mut expected_objects.get_mut(self.proxy).unwrap().base;
            expected.pitch = Angle::from_units(angles[0]);
            expected.yaw = Angle::from_units(angles[1]);
            expected.roll = Angle::from_units(angles[2]);
            expected.position = Vector3 {
                x: aim[0],
                y: aim[1],
                z: aim[2],
            };
            assert_eq!(source.bus.read8(WRAM + 0x1DAE), 8);
            assert_eq!(source.bus.read16(WRAM + 0x1DB0), CAMERA);
        }
        assert_eq!(self.objects, expected_objects);
        for (offset, expected) in expected_proxy.into_iter().enumerate() {
            assert_eq!(
                source.bus.read8(WRAM + u32::from(OTHER) + offset as u32),
                expected
            );
        }
        for (index, value) in matrix.into_iter().flatten().enumerate() {
            assert_eq!(source.bus.read16(0x7000E4 + index as u32 * 2), value as u16);
        }
        assert_eq!(source.bus.read8(WRAM + 0x1913), 1);
        assert_eq!(source.bus.read8(WRAM + 0x5E), 0x18);
        for &(address, value) in source.writes.as_ref().unwrap() {
            // The graphics-loan wrapper temporarily selects bank zero; its
            // low-memory data still aliases the same canonical work RAM.
            let address = if (0x033F..0x2000).contains(&address) {
                WRAM + address
            } else {
                address
            };
            let allowed = address < 0x033F
                || [
                    0x00301E,
                    0x00301F,
                    0x003030,
                    0x003034,
                    0x00303A,
                    0x004202,
                    0x004203,
                    WRAM + 0x1913,
                    WRAM + 0x1DAE,
                    WRAM + 0x1DB0,
                    WRAM + 0x1DB1,
                    WRAM + 0x1E30,
                    WRAM + 0x1E31,
                ]
                .contains(&address)
                || (WRAM + 0x1595..=WRAM + 0x159A).contains(&address)
                || (WRAM + u32::from(OTHER) + 0x0C..=WRAM + u32::from(OTHER) + 0x11)
                    .contains(&address)
                || [
                    WRAM + u32::from(OTHER) + 0x12,
                    WRAM + u32::from(OTHER) + 0x14,
                    WRAM + u32::from(OTHER) + 0x16,
                ]
                .contains(&address)
                || [0x700068, 0x700069, 0x70002C, 0x70002D, 0x70002E, 0x70002F].contains(&address);
            let allowed = allowed
                || self.retain
                    && ([
                        WRAM + 0x1D90,
                        WRAM + 0x1D91,
                        WRAM + 0x1D16,
                        WRAM + 0x1CF6,
                        WRAM + 0x1CF7,
                    ]
                    .contains(&address)
                        || [
                            0x6BB7, 0x6BC2, 0x6BC6, 0x6BC7, 0x6BC8, 0x6BC9, 0x6BCA, 0x6BCB,
                        ]
                        .into_iter()
                        .any(|field| address == WRAM + u32::from(STORAGE) + field));
            assert!(
                allowed,
                "unexpected persistent write {address:06X}={value:02X}"
            );
        }
    }
}

struct Callbacks;
impl sf2_game::scene_strategy::SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<sf2_game::strategy_schedule::StrategyCompletion, ()> {
        panic!("not an actor visit")
    }
    fn death_override(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<sf2_game::strategy_schedule::StrategyCompletion>, ()> {
        panic!("not a death visit")
    }
    fn resume_map_on_death(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<(), ()> {
        panic!("not a map visit")
    }
}

#[test]
fn complete_projection_and_retention_match_one_uninterrupted_original_display_slice() {
    let mut check = ProjectionCheck::new();
    check.retain = true;
    let viewport = ProjectionViewport {
        center: [112, 96],
        left: 0,
        right: 224,
        top: 0,
        bottom: 192,
    };
    let matrix = [[32767, 0, 0], [0, 32767, 0], [0, 0, 32767]];
    for byte in 0..=u8::MAX {
        for clock in [0, 1, 254, 255] {
            for flags in [[0, 0x40], [0x80, 0], [0x80, 0xC0]] {
                check.compare(
                    [byte, !byte, byte.rotate_left(3)],
                    [i16::from(byte) - 128, 51, 512],
                    [12, -18, 20],
                    matrix,
                    FixedViewAngles {
                        pitch: 0x8371,
                        yaw: 0xFE23,
                        roll: 0x9235,
                    },
                    viewport,
                    [byte, clock],
                    flags,
                );
            }
        }
    }
}

#[test]
fn reticle_producer_runs_original_host_and_both_graphics_jobs() {
    let mut check = ProjectionCheck::new();
    let viewport = ProjectionViewport {
        center: [112, 96],
        left: 0,
        right: 224,
        top: 0,
        bottom: 192,
    };
    let matrix = [[32767, 0, 0], [0, 32767, 0], [0, 0, 32767]];
    check.compare(
        [0; 3],
        [0, 0, 1000],
        [0; 3],
        matrix,
        FixedViewAngles::default(),
        viewport,
        [128; 2],
        [0x80, 0x40],
    );
    for pitch in 0..=u8::MAX {
        for yaw in 0..=u8::MAX {
            check.compare(
                [pitch, yaw, pitch ^ yaw],
                [256, -173, 512],
                [12, -18, 20],
                matrix,
                FixedViewAngles {
                    pitch: 0x8371,
                    yaw: 0xFE23,
                    roll: 0x9235,
                },
                viewport,
                [pitch, yaw],
                [0x80, 0xC0],
            );
        }
    }
    let mut random = 0x713A_46CBu32;
    let mut next = || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        random as u16
    };
    for _ in 0..8192 {
        let angles = [next() as u8, next() as u8, next() as u8];
        let aim = [next() as i16, next() as i16, next() as i16];
        let origin = [next() as i16, next() as i16, next() as i16];
        let matrix = std::array::from_fn(|_| std::array::from_fn(|_| next() as i16));
        let view = FixedViewAngles {
            pitch: next(),
            yaw: next(),
            roll: next(),
        };
        let viewport = ProjectionViewport {
            center: [next() as i16, next() as i16],
            left: next() as i16,
            right: next() as i16,
            top: next() as i16,
            bottom: next() as i16,
        };
        check.compare(
            angles,
            aim,
            origin,
            matrix,
            view,
            viewport,
            [next() as u8, next() as u8],
            [0x80, 0x40],
        );
    }
    for enable in 0..=u8::MAX {
        for mode in [0, 1, 0x40, 0x80, 0xC0, 0xFF] {
            check.compare(
                [enable, !enable, 23],
                [200, -300, 1000],
                [0; 3],
                matrix,
                FixedViewAngles::default(),
                viewport,
                [enable, !enable],
                [enable, mode],
            );
        }
    }
}

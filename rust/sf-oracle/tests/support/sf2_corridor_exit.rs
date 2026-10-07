//! Complete authored corridor controllers and their helpers against unchanged
//! original code. Paths run to their actual movement boundary, not a fabricated
//! trace. Source state is never fed back into native simulation.
use super::surface_particle_tests::{address, Native, OWNER};
use super::{assert_source_target, rom, source_target, Source, WRAM};
use sf2_game::path_commands::{ControlCommand, ControlStep};
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_invocation::InvocationWorld;
use sf2_game::path_program::{PathCatalog, Statement};
use sf2_game::path_runtime::PathRuntime;
use sf2_game::path_scene_state::EncounterHandoff;
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::scene_path_world::PlayerPathRecords;
use sf2_game::weapon_dispatch::WeaponState;
use sf2_game::{authored_paths, Angle, ObjectId, PathCursor, PathId, Vector3};

struct Fixture {
    native: Native,
    runtime: PathRuntime,
    players: [ObjectId; 2],
    slots: [u32; 2],
    owner: ObjectId,
    proxy: ObjectId,
    view: ObjectId,
}
fn cursor(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index,
    }
}
impl Fixture {
    fn new(source: &mut Source) -> Self {
        for (index, &byte) in rom()[0x50000..0x54E00].iter().enumerate() {
            source.bus.write8(0x7F7E00 + index as u32, byte);
        }
        source.run(0x7F1737, None, 0, OWNER, true);
        let mut native = Native::new(source, 5, 0, 0, 0);
        let ids = native.objects.active_ids().to_vec();
        let players = [ids[0], ids[1]];
        let (owner, proxy, view) = (ids[2], ids[3], ids[4]);
        let mut slots = [0; 2];
        let mut runtime = PathRuntime::default();
        for (index, player) in players.into_iter().enumerate() {
            source.run(0x068260, Some(0x0682B7), 0, address(Some(player)), true);
            slots[index] = u32::from(source.bus.read16(u32::from(address(Some(player))) + 0x2B));
            player_storage::initialize(
                &mut native.objects,
                &mut native.world,
                &mut runtime,
                player,
                PlayerStorageInputs {
                    pilot_code: 0,
                    reserve_shield: 0,
                    score: Default::default(),
                },
            )
            .unwrap();
        }
        native.world.primary_player = Some(players[0]);
        native.world.secondary_player = Some(players[1]);
        native.world.fixed_players[0] = Some(view);
        native.world.handoff = Some(EncounterHandoff::default());
        native.world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        source.bus.write16(0x12C3, address(Some(players[0])));
        source.bus.write16(0x12C5, address(Some(players[1])));
        source.bus.write16(0x14D6, address(Some(proxy)));
        Self {
            native,
            runtime,
            players,
            slots,
            owner,
            proxy,
            view,
        }
    }
    fn record(&self, index: usize) -> PlayerPathRecords {
        *self
            .native
            .world
            .player(&self.native.objects, self.players[index])
            .unwrap()
    }
    fn seed_records(&self, source: &mut Source) {
        for (index, slot) in self.slots.into_iter().enumerate() {
            let r = self.record(index);
            let b = r.boundary.unwrap();
            for (offset, value) in [
                (0x6A61, b.reset_control),
                (0x6AA0, r.auxiliary.unwrap().mode),
                (0x6AA1, r.mode_selection.unwrap().requested),
                (0x6B77, r.auxiliary.unwrap().action_flags),
                (0x6AAE, r.steering.unwrap().locked_heading.units()),
                (0x6BE2, r.contact.unwrap().hit.secondary_protection),
            ] {
                source.bus.write8(WRAM + slot + offset, value);
            }
            for (offset, value) in [
                (0x6AAF, b.center.x),
                (0x6AB1, b.center.y),
                (0x6AB3, b.center.z),
                (0x6AB5, b.half_width),
                (0x6AB7, b.half_height),
            ] {
                source.bus.write16(WRAM + slot + offset, value as u16);
            }
            source_target(source, slot, r.target_selection.unwrap(), |id| {
                address(Some(id))
            });
        }
        let handoff = self.native.world.handoff.unwrap();
        source.bus.write8(0x1D74, handoff.player_flags);
        source.bus.write16(0x1D88, handoff.x as u16);
        source.bus.write16(0x1D8C, handoff.z as u16);
        source.bus.write16(0x1D8E, handoff.heading_word);
    }
    fn seed_objects(&self, source: &mut Source) {
        for (id, actor) in self.native.objects.active_objects() {
            let base = u32::from(address(Some(id)));
            for (offset, value) in [
                (12, actor.base.position.x),
                (14, actor.base.position.y),
                (16, actor.base.position.z),
            ] {
                source.bus.write16(base + offset, value as u16);
            }
            for (offset, value) in [
                (0x32, actor.base.velocity.x),
                (0x34, actor.base.velocity.y),
                (0x36, actor.base.velocity.z),
            ] {
                source.bus.write16(base + offset, value as u16);
            }
            source.bus.write8(
                base + 0x21,
                u8::from(actor.extension.path_state.motion.follow_player_displacement) * 8,
            );
            for (offset, value) in [
                (0x12, actor.base.pitch.units()),
                (0x14, actor.base.yaw.units()),
                (0x16, actor.base.roll.units()),
            ] {
                source.bus.write8(base + offset, value);
            }
            source
                .bus
                .write8(base + 0x20, u8::from(actor.base.contacts.hit_marked) * 2);
            source.bus.write8(
                base + 0x24,
                u8::from(
                    actor.extension.path_state.conditions.selected_player
                        == PlayerTarget::Secondary,
                ) * 0x80,
            );
            source.bus.write16(
                WRAM + base + 0x1CE4,
                actor.extension.path_state.script_value,
            );
            source.bus.write16(
                WRAM + base + 0x1CD1,
                actor.extension.relative_position.y as u16,
            );
        }
        let view = self.native.objects.get(self.view).unwrap();
        for (offset, value) in [
            (12, view.base.position.x),
            (14, view.base.position.y),
            (16, view.base.position.z),
        ] {
            source.bus.write16(0x033F + offset, value as u16);
        }
        source
            .bus
            .write16(0x033F + 0x12, u16::from(view.base.pitch.units()));
        source
            .bus
            .write16(0x033F + 0x14, u16::from(view.base.yaw.units()));
    }
    fn run(&mut self, catalog: &PathCatalog, selected: usize) {
        let selection = if selected == 0 {
            PlayerTarget::Primary
        } else {
            PlayerTarget::Secondary
        };
        self.native
            .objects
            .get_mut(self.owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .selected_player = selection;
        let mut world = self
            .native
            .world
            .path_world(&self.native.objects, self.owner, selection)
            .unwrap();
        assert_eq!(
            self.runtime
                .enter_program(
                    catalog,
                    &mut self.native.objects,
                    self.owner,
                    &mut world,
                    100
                )
                .unwrap()
                .step,
            ControlStep::Movement
        );
    }
    fn run_full(&mut self, catalog: &PathCatalog) {
        use sf2_game::path_invocation::{InvocationEntry, PathInvocation};
        let mut invocation = PathInvocation::default();
        invocation.runtime = std::mem::take(&mut self.runtime);
        invocation
            .begin(self.owner, InvocationEntry::Program)
            .unwrap();
        assert_eq!(
            invocation
                .resume(
                    catalog,
                    &mut self.native.objects,
                    &mut self.native.world,
                    100
                )
                .unwrap(),
            self.owner
        );
        assert!(!invocation.is_active());
        self.runtime = invocation.runtime;
    }
    fn compare_records(&self, source: &Source, before: [PlayerPathRecords; 2]) {
        for (index, mut expected) in before.into_iter().enumerate() {
            let slot = self.slots[index];
            let byte = |offset| source.bus.read8(WRAM + slot + offset);
            let word = |offset| source.bus.read16(WRAM + slot + offset) as i16;
            expected.auxiliary.as_mut().unwrap().mode = byte(0x6AA0);
            expected.mode_selection.as_mut().unwrap().requested = byte(0x6AA1);
            expected.auxiliary.as_mut().unwrap().action_flags = byte(0x6B77);
            expected.steering.as_mut().unwrap().locked_heading = Angle::from_units(byte(0x6AAE));
            expected.contact.as_mut().unwrap().hit.secondary_protection = byte(0x6BE2);
            let boundary = expected.boundary.as_mut().unwrap();
            boundary.reset_control = byte(0x6A61);
            boundary.center = Vector3 {
                x: word(0x6AAF),
                y: word(0x6AB1),
                z: word(0x6AB3),
            };
            boundary.half_width = word(0x6AB5);
            boundary.half_height = word(0x6AB7);
            let actual = self.record(index);
            assert_source_target(
                source,
                slot,
                actual.target_selection.unwrap(),
                |id| address(Some(id)),
                "corridor",
            );
            expected.target_selection = actual.target_selection;
            assert_eq!(actual, expected);
        }
        let handoff = self.native.world.handoff.unwrap();
        assert_eq!(handoff.player_flags, source.bus.read8(0x1D74));
        assert_eq!(handoff.x as u16, source.bus.read16(0x1D88));
        assert_eq!(handoff.z as u16, source.bus.read16(0x1D8C));
        assert_eq!(handoff.heading_word, source.bus.read16(0x1D8E));
    }
    fn compare_objects(&self, source: &Source, catalog: &PathCatalog) {
        for (id, actor) in self.native.objects.active_objects() {
            let base = u32::from(address(Some(id)));
            assert_eq!(
                actor.base.position,
                Vector3 {
                    x: source.bus.read16(base + 12) as i16,
                    y: source.bus.read16(base + 14) as i16,
                    z: source.bus.read16(base + 16) as i16,
                },
                "actor {id:?}"
            );
            for (offset, value) in [
                (0x12, actor.base.pitch.units()),
                (0x14, actor.base.yaw.units()),
                (0x16, actor.base.roll.units()),
            ] {
                assert_eq!(value, source.bus.read8(base + offset));
            }
            assert_eq!(
                actor.extension.path_state.script_value,
                source.bus.read16(WRAM + base + 0x1CE4)
            );
            assert_eq!(
                actor.extension.relative_position.y as u16,
                source.bus.read16(WRAM + base + 0x1CD1)
            );
        }
        assert_eq!(
            self.native
                .objects
                .get(self.proxy)
                .unwrap()
                .base
                .contacts
                .hit_marked,
            source
                .bus
                .read8(u32::from(address(Some(self.proxy))) + 0x20)
                & 2
                != 0
        );
        assert_eq!(
            self.native.world.published_camera_roll,
            Some(source.bus.read16(0x1E0B))
        );
        let native_cursor = self
            .native
            .objects
            .get(self.owner)
            .unwrap()
            .base
            .path
            .unwrap();
        let source_cursor = source
            .bus
            .read16(u32::from(address(Some(self.owner))) + 0x2B);
        assert!(
            match (source_cursor, catalog.statement(native_cursor).unwrap()) {
                (0xD1D3, Statement::ConsiderPrimaryTarget { .. }) => true,
                (0xD213, Statement::SetRegionHeading { .. }) => true,
                (0xD23E, Statement::Mutate { .. }) => true,
                _ => false,
            },
            "source cursor {source_cursor:04X}, native {native_cursor:?}"
        );
        assert_eq!(
            self.runtime.resources.available_capacity(),
            source.available()
        );
    }
}

#[test]
fn corridor_reset_matches_every_original_flag_and_control_byte_for_either_selected_player() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source);
    let catalog = PathCatalog::new(vec![vec![
        Statement::ResetSelectedRegion { next: cursor(1) },
        Statement::Control(ControlCommand::Hold),
    ]])
    .unwrap();
    for selected in 0..2 {
        source
            .bus
            .write16(WRAM + 0xCF1F, address(Some(f.players[selected])));
        for flags in 0..=u8::MAX {
            for control in 0..=u8::MAX {
                let record = f
                    .native
                    .world
                    .player_mut(&f.native.objects, f.players[selected])
                    .unwrap();
                record.auxiliary.as_mut().unwrap().action_flags = flags;
                record.boundary.as_mut().unwrap().reset_control = control;
                f.seed_records(&mut source);
                let before = [f.record(0), f.record(1)];
                source.run(0x7FB660, Some(0x7FCAE8), 0, address(Some(f.owner)), true);
                f.native.objects.get_mut(f.owner).unwrap().base.path = Some(cursor(0));
                f.run(&catalog, selected);
                f.compare_records(&source, before);
            }
        }
    }
}

#[test]
fn corridor_entry_matches_every_original_handoff_and_protection_byte() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source);
    let catalog = PathCatalog::new(vec![vec![
        Statement::BeginCorridorEntry { next: cursor(1) },
        Statement::Control(ControlCommand::Hold),
    ]])
    .unwrap();
    for flags in 0..=u8::MAX {
        for protection in 0..=u8::MAX {
            f.native.world.handoff.as_mut().unwrap().player_flags = flags;
            f.native
                .world
                .player_mut(&f.native.objects, f.players[0])
                .unwrap()
                .contact
                .as_mut()
                .unwrap()
                .hit
                .secondary_protection = protection;
            f.seed_records(&mut source);
            let before = [f.record(0), f.record(1)];
            source.run(0x07F95F, None, 0, address(Some(f.owner)), true);
            f.native.objects.get_mut(f.owner).unwrap().base.path = Some(cursor(0));
            f.run(&catalog, 1);
            f.compare_records(&source, before);
        }
    }
}

#[test]
fn both_path_mode_requests_match_all_original_pending_and_current_mode_bytes() {
    use sf2_game::player_mode_selection::ModeRequest;
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source);
    for (entry, request) in [
        (0x7FB081, ModeRequest::FreeFlight),
        (0x7FB04D, ModeRequest::Walker),
    ] {
        let catalog = PathCatalog::new(vec![vec![
            Statement::RequestSelectedMode {
                request,
                next: cursor(1),
            },
            Statement::Control(ControlCommand::Hold),
        ]])
        .unwrap();
        for selected in 0..2 {
            source
                .bus
                .write16(WRAM + 0xCF1F, address(Some(f.players[selected])));
            for current in 0..=u8::MAX {
                for pending in 0..=u8::MAX {
                    let record = f
                        .native
                        .world
                        .player_mut(&f.native.objects, f.players[selected])
                        .unwrap();
                    record.auxiliary.as_mut().unwrap().mode = current;
                    record.mode_selection.as_mut().unwrap().requested = pending;
                    f.seed_records(&mut source);
                    let before = [f.record(0), f.record(1)];
                    source.run(entry, Some(0x7FCAE8), 0, address(Some(f.owner)), true);
                    f.native.objects.get_mut(f.owner).unwrap().base.path = Some(cursor(0));
                    f.run(&catalog, selected);
                    f.compare_records(&source, before);
                }
            }
        }
    }
}

#[test]
fn both_complete_corridor_controllers_match_original_paths_at_every_heading_and_admission_edges() {
    let rom = rom();
    let mut source = Source::new(&rom, 0);
    source.bus.enable_gsu();
    for (index, &byte) in rom[0x50000..0x54E00].iter().enumerate() {
        source.bus.write8(0x7F7E00 + index as u32, byte);
    }
    let mut f = Fixture::new(&mut source);
    let catalog = authored_paths::catalog();
    let positions = [
        (0, 0, -499),
        (0, 0, -199),
        (0, 0, 199),
        (0, 0, 499),
        (0, 0, 1000),
        (0, 0, -1000),
        (100, 0, 0),
        (-100, 0, 0),
        (0, -300, 0),
        (0, 300, 0),
        (0, 301, 0),
        (i16::MIN, -1, i16::MAX),
    ];
    for (root, entry) in [
        (0xD1CB, authored_paths::NARROWING_CORRIDOR_EXIT),
        (0xD207, authored_paths::LEVEL_CORRIDOR_EXIT),
    ] {
        for selected in 0..2 {
            for heading in 0..=u8::MAX {
                for (case, &(x, y, z)) in positions.iter().enumerate() {
                    for active in [false, true] {
                        let actor = f.native.objects.get_mut(f.owner).unwrap();
                        actor.base.path = Some(entry);
                        actor.base.position = Vector3 {
                            x: -222,
                            y: 171,
                            z: 313,
                        };
                        actor.base.pitch = Angle::from_units(31);
                        actor.base.roll = Angle::from_units(97);
                        actor.base.yaw = Angle::from_units(117);
                        actor.extension.path_state.conditions.selected_player = if selected == 0 {
                            PlayerTarget::Primary
                        } else {
                            PlayerTarget::Secondary
                        };
                        f.native
                            .objects
                            .get_mut(f.players[selected])
                            .unwrap()
                            .base
                            .position = Vector3 {
                            x: 300i16.wrapping_add(x),
                            y,
                            z: (-270i16).wrapping_add(z),
                        };
                        f.native
                            .objects
                            .get_mut(f.proxy)
                            .unwrap()
                            .base
                            .contacts
                            .hit_marked = true;
                        for player in f.players {
                            let record = f
                                .native
                                .world
                                .player_mut(&f.native.objects, player)
                                .unwrap();
                            record.auxiliary.as_mut().unwrap().action_flags =
                                0xB3 | if active { 4 } else { 0 };
                            record.auxiliary.as_mut().unwrap().mode = heading;
                            record.mode_selection.as_mut().unwrap().requested = !heading;
                            record.boundary.as_mut().unwrap().reset_control = !heading;
                            record.contact.as_mut().unwrap().hit.secondary_protection = heading;
                            record.target_selection.as_mut().unwrap().control_flags =
                                if case & 1 != 0 { 0x10 } else { 0 };
                            record.target_selection.as_mut().unwrap().distance = 65535;
                        }
                        f.native.world.handoff = Some(EncounterHandoff {
                            player_flags: heading,
                            x: 300,
                            z: -270,
                            heading_word: 0xCB00 | u16::from(heading),
                        });
                        f.native.world.published_camera_roll = Some(0xAD09);
                        source.bus.write16(0x1E0B, 0xAD09);
                        source
                            .bus
                            .write16(u32::from(address(Some(f.owner))) + 0x2B, root);
                        f.seed_records(&mut source);
                        f.seed_objects(&mut source);
                        let before = [f.record(0), f.record(1)];
                        source.run(0x7F7E53, Some(0x7F9DDE), 0, address(Some(f.owner)), true);
                        f.run(&catalog, selected);
                        f.compare_records(&source, before);
                        f.compare_objects(&source, &catalog);
                        assert_eq!(
                            f.runtime.region.heading_offset.units(),
                            source.bus.read8(0x00A7)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn corridor_paths_keep_their_continuations_and_share_live_motion_across_complete_visits() {
    use sf2_game::path_motion::PublishedPlayerMotion;
    let rom = rom();
    let catalog = authored_paths::catalog();
    for (root, entry) in [
        (0xD1CB, authored_paths::NARROWING_CORRIDOR_EXIT),
        (0xD207, authored_paths::LEVEL_CORRIDOR_EXIT),
    ] {
        for initial_heading in [0u8, 32, 64, 96, 128, 160, 192, 224] {
            for selected in 0..2 {
                let mut source = Source::new(&rom, 0);
                source.bus.enable_gsu();
                let mut f = Fixture::new(&mut source);
                let actor = f.native.objects.get_mut(f.owner).unwrap();
                actor.base.path = Some(entry);
                actor.base.velocity = Vector3 { x: 3, y: -1, z: -2 };
                actor.extension.path_state.motion.follow_player_displacement = true;
                actor.extension.path_state.conditions.selected_player = if selected == 0 {
                    PlayerTarget::Primary
                } else {
                    PlayerTarget::Secondary
                };
                f.native.world.handoff = Some(EncounterHandoff {
                    x: 300,
                    z: -270,
                    heading_word: 0xFD00 | u16::from(initial_heading),
                    ..Default::default()
                });
                f.native.world.published_camera_roll = Some(13079);
                source.bus.write16(0x1E0B, 13079);
                source
                    .bus
                    .write16(u32::from(address(Some(f.owner))) + 0x2B, root);
                f.seed_records(&mut source);
                f.seed_objects(&mut source);
                for visit in 0..160u16 {
                    let selected = if visit < 80 { selected } else { 1 - selected };
                    let selection = if selected == 0 {
                        PlayerTarget::Primary
                    } else {
                        PlayerTarget::Secondary
                    };
                    f.native
                        .objects
                        .get_mut(f.owner)
                        .unwrap()
                        .extension
                        .path_state
                        .conditions
                        .selected_player = selection;
                    source.bus.write8(
                        u32::from(address(Some(f.owner))) + 0x24,
                        u8::from(selected == 1) * 0x80,
                    );
                    let position = Vector3 {
                        x: 300 + (visit as i16 % 11 - 5) * 7,
                        y: if visit % 19 == 0 { 400 } else { 0 },
                        z: -270
                            + if visit < 40 {
                                500
                            } else if visit < 120 {
                                100
                            } else {
                                -1500
                            },
                    };
                    f.native
                        .objects
                        .get_mut(f.players[selected])
                        .unwrap()
                        .base
                        .position = position;
                    for (offset, value) in [(12, position.x), (14, position.y), (16, position.z)] {
                        source.bus.write16(
                            u32::from(address(Some(f.players[selected]))) + offset,
                            value as u16,
                        );
                    }
                    let delta = Vector3 {
                        x: visit as i16 % 3 - 1,
                        y: visit as i16 % 5 - 2,
                        z: visit as i16 % 7 - 3,
                    };
                    f.native.world.published_motion =
                        Some(PublishedPlayerMotion { position, delta });
                    for (offset, value) in [(0x1E1C, delta.x), (0x1E1E, delta.y), (0x1E20, delta.z)]
                    {
                        source.bus.write16(offset, value as u16);
                    }
                    // Simulate independent consumers without touching either
                    // controller's cursor, clock, stack or width history.
                    if visit % 13 == 0 {
                        f.native.world.handoff.as_mut().unwrap().player_flags &= !1;
                        source.bus.write8(0x1D74, source.bus.read8(0x1D74) & !1);
                    }
                    let protection = visit as u8;
                    f.native
                        .world
                        .player_mut(&f.native.objects, f.players[0])
                        .unwrap()
                        .contact
                        .as_mut()
                        .unwrap()
                        .hit
                        .secondary_protection = protection;
                    source.bus.write8(WRAM + f.slots[0] + 0x6BE2, protection);
                    let before = [f.record(0), f.record(1)];
                    source.run(0x7F7E53, None, 0, address(Some(f.owner)), true);
                    f.run_full(&catalog);
                    f.compare_records(&source, before);
                    f.compare_objects(&source, &catalog);
                }
            }
        }
    }
}

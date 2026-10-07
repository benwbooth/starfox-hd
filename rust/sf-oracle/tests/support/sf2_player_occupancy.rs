//! Live grid queries and the complete player response against unmodified
//! original code. Each side retains its own state during continuous visits.

use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_occupancy::{self, OccupancyContext, PlayerOccupancy};
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::scene_path_world::PlayerPathRecords;
use sf2_game::weapon_dispatch::WeaponState;
use sf2_game::world_occupancy::{MarkerCoverage, OccupancyChange, WorldOccupancy, WorldRectangle};
use sf2_game::{ObjectId, Vector3};

struct Fixture {
    native: Native,
    runtime: PathRuntime,
    proxy: ObjectId,
    view: ObjectId,
    context: OccupancyContext,
}
impl Fixture {
    fn new(source: &mut Source) -> Self {
        let mut native = Native::new(source, 3, 0, 0, 0);
        let mut runtime = PathRuntime::default();
        player_storage::initialize(
            &mut native.objects,
            &mut native.world,
            &mut runtime,
            native.owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 17,
                score: Default::default(),
            },
        )
        .unwrap();
        let others: Vec<_> = native
            .objects
            .active_ids()
            .iter()
            .copied()
            .filter(|&id| id != native.owner)
            .collect();
        let proxy = others[0];
        let view = others[1];
        native.world.fixed_players[0] = Some(view);
        native.world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        native.world.action_gate = Some(Default::default());
        native.world.player_yaw_increment = Some(0);
        Self {
            native,
            runtime,
            proxy,
            view,
            context: OccupancyContext {
                camera_override_active: Some(false),
                diagonal_tie_bias: Some(0),
            },
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.native
            .world
            .player_mut(&self.native.objects, self.native.owner)
            .unwrap()
    }
    fn base(&self, id: ObjectId) -> u32 {
        WRAM + if id == self.view {
            0x033F
        } else {
            u32::from(address(Some(id)))
        }
    }
    fn plane(&mut self, source: &mut Source, blocked: impl Fn(u16, u16) -> bool) {
        let mut world = WorldOccupancy::default();
        for z in 0..128_u16 {
            for group in 0..16_u16 {
                let mut byte = 0;
                for bit in 0..8 {
                    let x = group * 8 + bit;
                    if blocked(x, z) {
                        byte |= 1 << bit;
                        let marker = MarkerCoverage::from_rectangle(WorldRectangle {
                            x: (x * 512) as i16,
                            z: (z * 512) as i16,
                            width: 1,
                            depth: 1,
                        })
                        .unwrap();
                        world.apply(&marker, OccupancyChange::Mark);
                    }
                }
                source
                    .bus
                    .write8(WRAM + 0xCF36 + u32::from(z * 16 + group), byte);
            }
        }
        self.native.world.occupancy = Some(world);
    }
    fn vary(&mut self, seed: u16) {
        let record = self.records();
        record.motion.as_mut().unwrap().contact_flags = seed as u8;
        record.pose.as_mut().unwrap().turning_lean = seed.rotate_left(5);
        record.pose.as_mut().unwrap().heading_return_bank = seed.rotate_right(3) as i8;
        record.yaw_motion = Some(seed & 0x00FF);
        record.auxiliary.as_mut().unwrap().mode = seed as u8;
        record.contact.as_mut().unwrap().ignores_contacts = seed & 0x100 != 0;
        record.occupancy = Some(PlayerOccupancy {
            current_cell: [seed.rotate_left(3) as i8, seed.rotate_right(3) as i8],
            previous_cell: [seed as i8, !seed as i8],
            displacement: [seed as i16, seed.wrapping_neg() as i16],
        });
        record.boundary.as_mut().unwrap().return_position = Vector3 {
            x: seed as i16,
            y: seed.rotate_right(5) as i16,
            z: !seed as i16,
        };
        self.native
            .objects
            .get_mut(self.proxy)
            .unwrap()
            .base
            .position = Vector3 {
            x: !seed as i16,
            y: seed.rotate_left(9) as i16,
            z: seed as i16,
        };
        self.native
            .objects
            .get_mut(self.view)
            .unwrap()
            .base
            .position = Vector3 {
            x: seed.rotate_left(7) as i16,
            y: seed as i16,
            z: seed.rotate_right(9) as i16,
        };
        player_storage::get_mut(
            &self.native.objects,
            &mut self.runtime.resources,
            self.native.owner,
        )
        .unwrap()
        .fine_yaw = seed;
        self.native.world.player_yaw_increment = Some(seed.rotate_left(3));
    }
    fn seed(&self, source: &mut Source) {
        source.bus.write16(0x14D6, address(Some(self.proxy)));
        source
            .bus
            .write8(0x1D72, self.native.world.action_gate.unwrap().code);
        source
            .bus
            .write16(0x1E38, self.native.world.player_yaw_increment.unwrap());
        source
            .bus
            .write8(0x3F, self.context.diagonal_tie_bias.unwrap());
        let record = self
            .native
            .world
            .player(&self.native.objects, self.native.owner)
            .unwrap();
        let state = record.occupancy.unwrap();
        for (offset, value) in [
            (0x6BE6, record.motion.unwrap().contact_flags),
            (
                0x6BEB,
                if record.occupancy_exempt.unwrap() {
                    0xD5
                } else {
                    0x55
                },
            ),
            (0x6AA0, record.auxiliary.unwrap().mode),
            (
                0x6A72,
                0x81 | u8::from(record.contact.unwrap().ignores_contacts) * 0x10,
            ),
            (0x6ADA, record.pose.unwrap().heading_return_bank as u8),
            (0x6B29, state.current_cell[0] as u8),
            (0x6B2A, state.current_cell[1] as u8),
            (0x6B2C, state.previous_cell[0] as u8),
            (0x6B2D, state.previous_cell[1] as u8),
        ] {
            source.bus.write8(WRAM + SLOT + offset, value);
        }
        for (offset, value) in [
            (
                0x6A9D,
                if self.context.camera_override_active.unwrap() {
                    0x9F68
                } else {
                    0
                },
            ),
            (
                0x6ABB,
                player_storage::get(
                    &self.native.objects,
                    &self.runtime.resources,
                    self.native.owner,
                )
                .unwrap()
                .fine_yaw,
            ),
            (0x6ACD, record.yaw_motion.unwrap()),
            (0x6AD0, record.pose.unwrap().turning_lean),
            (0x6AF9, state.displacement[0] as u16),
            (0x6AFB, state.displacement[1] as u16),
            (0x6BED, record.boundary.unwrap().return_position.x as u16),
            (0x6BEF, record.boundary.unwrap().return_position.y as u16),
            (0x6BF1, record.boundary.unwrap().return_position.z as u16),
        ] {
            source.bus.write16(WRAM + SLOT + offset, value);
        }
        for (id, object) in self.native.objects.active_objects() {
            let base = self.base(id);
            for (offset, value) in [
                (12, object.base.position.x),
                (14, object.base.position.y),
                (16, object.base.position.z),
            ] {
                source.bus.write16(base + offset, value as u16);
            }
            for (offset, value) in [
                (18, object.base.pitch.units()),
                (20, object.base.yaw.units()),
                (22, object.base.roll.units()),
            ] {
                source.bus.write8(base + offset, value);
            }
        }
    }
    fn advance(&mut self, source: &mut Source) {
        let mut expected = *self.records();
        let mut storage = player_storage::get(
            &self.native.objects,
            &self.runtime.resources,
            self.native.owner,
        )
        .unwrap()
        .clone();
        source.run(0x07E685, None, 0, OWNER, true);
        player_occupancy::advance(
            &mut self.native.objects,
            &mut self.native.world,
            &mut self.runtime.resources,
            self.native.owner,
            self.context,
        )
        .unwrap();
        let byte = |offset| source.bus.read8(WRAM + SLOT + offset);
        let word = |offset| source.bus.read16(WRAM + SLOT + offset);
        expected.occupancy = Some(PlayerOccupancy {
            current_cell: [byte(0x6B29) as i8, byte(0x6B2A) as i8],
            previous_cell: [byte(0x6B2C) as i8, byte(0x6B2D) as i8],
            displacement: [word(0x6AF9) as i16, word(0x6AFB) as i16],
        });
        expected.motion.as_mut().unwrap().contact_flags = byte(0x6BE6);
        expected.pose.as_mut().unwrap().turning_lean = word(0x6AD0);
        expected.pose.as_mut().unwrap().heading_return_bank = byte(0x6ADA) as i8;
        expected.boundary.as_mut().unwrap().return_position = Vector3 {
            x: word(0x6BED) as i16,
            y: word(0x6BEF) as i16,
            z: word(0x6BF1) as i16,
        };
        assert_eq!(*self.records(), expected);
        storage.fine_yaw = word(0x6ABB);
        assert_eq!(
            player_storage::get(
                &self.native.objects,
                &self.runtime.resources,
                self.native.owner
            )
            .unwrap(),
            &storage
        );
        assert_eq!(
            self.native.world.player_yaw_increment,
            Some(source.bus.read16(0x1E38))
        );
        for (id, actor) in self.native.objects.active_objects() {
            let base = self.base(id);
            assert_eq!(
                actor.base.position,
                Vector3 {
                    x: source.bus.read16(base + 12) as i16,
                    y: source.bus.read16(base + 14) as i16,
                    z: source.bus.read16(base + 16) as i16
                },
                "actor {id:?}"
            );
            assert_eq!(
                [
                    actor.base.pitch.units(),
                    actor.base.yaw.units(),
                    actor.base.roll.units()
                ],
                [
                    source.bus.read8(base + 18),
                    source.bus.read8(base + 20),
                    source.bus.read8(base + 22)
                ]
            );
        }
    }
}

#[test]
fn all_mode_contact_flag_pairs_and_lazy_gates_match_original() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source);
    f.plane(&mut source, |x, z| x == 1 && z == 0);
    for mode in 0..=255_u16 {
        for flags in 0..=255_u16 {
            f.vary(mode << 8 | flags);
            f.records().auxiliary.as_mut().unwrap().mode = mode as u8;
            f.records().occupancy.as_mut().unwrap().current_cell = [0, 0];
            f.native
                .objects
                .get_mut(f.native.owner)
                .unwrap()
                .base
                .position = Vector3 {
                x: 777,
                y: -59,
                z: 127,
            };
            f.seed(&mut source);
            f.advance(&mut source);
        }
    }
    for flags in 0..=255_u16 {
        for gate in 0..8 {
            f.vary(flags);
            f.records().occupancy_exempt = Some(gate & 1 != 0);
            f.context.camera_override_active = Some(gate & 2 != 0);
            f.native.world.action_gate.as_mut().unwrap().code =
                if gate & 4 != 0 { flags as u8 } else { 0 };
            f.seed(&mut source);
            f.advance(&mut source);
        }
    }
}

#[test]
fn full_word_positions_cell_history_and_neighbor_topologies_match_original() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source);
    f.plane(&mut source, |x, z| {
        (x.wrapping_mul(17) ^ z.wrapping_mul(13)) & 3 != 0
    });
    for value in 0..=u16::MAX {
        f.vary(value);
        f.context.diagonal_tie_bias = Some((value >> 8) as u8);
        f.native
            .objects
            .get_mut(f.native.owner)
            .unwrap()
            .base
            .position = Vector3 {
            x: value as i16,
            y: !value as i16,
            z: value.rotate_right(7) as i16,
        };
        f.seed(&mut source);
        f.advance(&mut source);
    }
}

#[test]
fn every_diagonal_bias_across_signed_cells_with_both_neighbors_open_matches_original() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source);
    f.plane(&mut source, |x, z| x & 1 == 1 && z & 1 == 1);
    let mut x_chosen = 0;
    let mut z_chosen = 0;
    for bias in 0..=255_u16 {
        for coord in 0..=255_u16 {
            f.vary(bias << 8 | coord);
            f.context.diagonal_tie_bias = Some(bias as u8);
            let z = coord
                .wrapping_mul(2)
                .wrapping_add(1)
                .wrapping_mul(512)
                .wrapping_add(127) as i16;
            let current_z = ((z >> 9) as i8).wrapping_sub(1);
            f.records().occupancy.as_mut().unwrap().current_cell = [0, current_z];
            f.native
                .objects
                .get_mut(f.native.owner)
                .unwrap()
                .base
                .position = Vector3 { x: 777, y: -53, z };
            f.seed(&mut source);
            f.advance(&mut source);
            if f.records().occupancy.unwrap().current_cell[0] == 1 {
                x_chosen += 1;
            } else {
                z_chosen += 1;
            }
        }
    }
    assert!(x_chosen > 1000 && z_chosen > 1000);
}

#[test]
fn all_fine_yaw_words_and_retained_lean_drive_original_heading_response() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source);
    f.plane(&mut source, |_, _| true);
    for yaw in 0..=u16::MAX {
        f.vary(yaw);
        f.records().auxiliary.as_mut().unwrap().mode = 0x10;
        f.records().yaw_motion = Some(if yaw & 0x1000 != 0 { yaw } else { yaw & 255 });
        f.records().occupancy.as_mut().unwrap().current_cell =
            [(yaw as i8).wrapping_mul(7), (yaw >> 8) as i8];
        f.native
            .objects
            .get_mut(f.native.owner)
            .unwrap()
            .base
            .position = Vector3 {
            x: yaw.rotate_left(3) as i16,
            y: 47,
            z: yaw.rotate_right(5) as i16,
        };
        f.seed(&mut source);
        f.advance(&mut source);
    }
}

#[test]
fn continuous_grid_visits_retain_independent_flags_cells_pose_and_history() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source);
    f.plane(&mut source, |x, z| (x ^ z) & 1 != 0);
    f.vary(0);
    f.records().auxiliary.as_mut().unwrap().mode = 0x10;
    f.seed(&mut source);
    let mut blocked = 0;
    let mut open = 0;
    for visit in 0..8192_u16 {
        let delta = [
            ((visit % 23) as i16 - 11) * 41,
            ((visit % 19) as i16 - 9) * 37,
        ];
        let position = &mut f
            .native
            .objects
            .get_mut(f.native.owner)
            .unwrap()
            .base
            .position;
        position.x = position.x.wrapping_add(delta[0]);
        position.z = position.z.wrapping_add(delta[1]);
        for (offset, delta) in [(12, delta[0]), (16, delta[1])] {
            let address = WRAM + u32::from(OWNER) + offset;
            source.bus.write16(
                address,
                source.bus.read16(address).wrapping_add(delta as u16),
            );
        }
        let bias = visit as u8;
        f.context.diagonal_tie_bias = Some(bias);
        source.bus.write8(0x3F, bias);
        let mode = if visit & 16 == 0 { 0x10 } else { 0 };
        f.records().auxiliary.as_mut().unwrap().mode = mode;
        source.bus.write8(WRAM + SLOT + 0x6AA0, mode);
        f.advance(&mut source);
        if f.records().motion.unwrap().contact_flags & 0x40 != 0 {
            blocked += 1;
        } else {
            open += 1;
        }
    }
    assert!(
        blocked > 1000 && open > 1000,
        "blocked {blocked}, open {open}"
    );
}

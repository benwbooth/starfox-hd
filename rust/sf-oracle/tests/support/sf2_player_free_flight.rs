//! Whole free-flight mode, including real nested effects and their independent
//! lifetimes. This is a child of the shared-flight fixture, not a source-output
//! replay or a replacement of any called original routine.
use super::*;
use sf2_game::player_free_flight::{self, FreeFlightContext};
use sf2_game::player_surface_effect::{self, SurfaceEffectInputs, SurfaceEffectPhase};
use sf2_game::Behavior;

impl Fixture {
    fn free_values(&self) -> Vec<(u32, u16, bool)> {
        let records = self
            .native
            .world
            .player(&self.native.objects, self.native.owner)
            .unwrap();
        let mode = records.mode_selection.unwrap();
        let motion = records.motion.unwrap();
        let mut values = vec![
            (WRAM + SLOT + 0x6AA1, u16::from(mode.requested), true),
            (
                WRAM + SLOT + 0x6BEC,
                u16::from(mode.transition_control),
                true,
            ),
            (WRAM + SLOT + 0x6B64, u16::from(mode.surface_control), true),
            (WRAM + SLOT + 0x6B9B, u16::from(mode.cue_control), true),
            (
                WRAM + SLOT + 0x6B94,
                u16::from(motion.walker_contact_control),
                true,
            ),
            (
                WRAM + SLOT + 0x6AA4,
                u16::from(motion.walker_motion_control),
                true,
            ),
            (WRAM + SLOT + 0x6AF7, motion.surface_height as u16, false),
            (
                0x7024E0,
                self.native.world.surface_clipping_plane_height.unwrap() as u16,
                false,
            ),
        ];
        for (id, actor) in self.native.objects.active_objects() {
            let base = WRAM + u32::from(address(Some(id)));
            values.push((base + 0x1CC7, u16::from(actor.base.behavior_phase), true));
            values.push((
                base + 0x1CE4,
                actor.extension.path_state.script_value,
                false,
            ));
            let Behavior::SurfaceEffect(phase) = actor.base.behavior else {
                continue;
            };
            for (offset, value, byte) in [
                (
                    0x19,
                    match phase {
                        SurfaceEffectPhase::Initialize => 0xC61E,
                        SurfaceEffectPhase::Active => 0xC631,
                    },
                    false,
                ),
                (0x1B, 7, true),
                (0x0A, u16::from(actor.base.target_speed), true),
                (0x13, u16::from(actor.base.child_number), true),
                (
                    0x2B,
                    address(actor.base.effect_origin.map(|origin| origin.slot())),
                    false,
                ),
                (0x1CD8, address(actor.extension.parent), false),
                (0x1CC8, actor.extension.depth_offset, false),
                (
                    0x1CCA,
                    u16::from(actor.extension.path_state.animation.shape.packed()),
                    true,
                ),
                (0x1CF0, u16::from(actor.extension.spawn_group), true),
            ] {
                values.push((base + offset, value, byte));
            }
        }
        values
    }

    fn free_seed(&self, source: &mut Source) {
        self.seed(source);
        for (field, value, byte) in self.free_values() {
            if byte {
                source.bus.write8(field, value as u8)
            } else {
                source.bus.write16(field, value)
            }
        }
    }

    fn free_compare(&mut self, source: &Source) {
        self.compare(source);
        for (field, value, byte) in self.free_values() {
            assert_eq!(
                value,
                if byte {
                    u16::from(source.bus.read8(field))
                } else {
                    source.bus.read16(field)
                },
                "free-flight field {field:06X}"
            );
        }
        for (id, actor) in self.native.objects.active_objects() {
            if !matches!(actor.base.behavior, Behavior::SurfaceEffect(_)) {
                continue;
            }
            let base = u32::from(address(Some(id)));
            for (offset, mask, flag) in [
                (0x20, 0x20, actor.base.flags.scaled_sprite),
                (
                    0x20,
                    8,
                    actor.extension.path_state.needs_path_initialization,
                ),
                (0x21, 1, actor.base.flags.collision_disabled),
                (0x22, 4, actor.base.flags.general_search_eligible),
                (
                    0x23,
                    4,
                    actor.extension.path_state.motion.attached_coordinates,
                ),
                (
                    0x24,
                    4,
                    actor.base.flags.exclude_from_shape_footprint_search,
                ),
                (0x25, 1, actor.base.flags.remove_with_parent),
                (0x26, 8, actor.base.contacts.run_when_paused),
            ] {
                assert_eq!(
                    source.bus.read8(base + offset) & mask != 0,
                    flag,
                    "free-flight effect {base:04X} flags {offset:X}:{mask:X}"
                );
            }
        }
    }

    fn free_step(&mut self, source: &mut Source, input: SurfaceEffectInputs, steering_target: u16) {
        source.bus.write16(0x1D16, 0);
        source.bus.write16(0x0A, steering_target);
        source.bus.write16(4, address(input.origin));
        source.bus.write8(2, input.lateral.unwrap() as u8);
        source.bus.write8(8, input.vertical.unwrap() as u8);
        source.bus.write8(0x97, input.forward.unwrap() as u8);
        let original = call(
            &mut source.bus,
            0x06E32C,
            &Entry {
                x: OWNER,
                dbr: 0x7E,
                p: 0x20,
                ..Default::default()
            },
        );
        assert!(
            original.returned,
            "complete original free-flight mode must return"
        );
        player_free_flight::advance(
            &mut self.native.objects,
            &mut self.native.world,
            &mut self.runtime.resources,
            self.native.owner,
            FreeFlightContext {
                mode: FlightModeContext {
                    steering: SteeringContext {
                        inherited_response_target: Some(steering_target),
                    },
                    flight: self.context,
                },
                protected_effect: input,
            },
        )
        .unwrap();
        self.free_compare(source);
    }

    fn effects_step(&mut self, source: &mut Source) -> usize {
        source.bus.write16(0x1D16, 0);
        let effects: Vec<_> = self
            .native
            .objects
            .active_objects()
            .filter_map(|(id, actor)| {
                if let Behavior::SurfaceEffect(phase) = actor.base.behavior {
                    Some((id, phase))
                } else {
                    None
                }
            })
            .collect();
        let mut retired = 0;
        for (id, phase) in effects {
            source.run(
                match phase {
                    SurfaceEffectPhase::Initialize => 0x07C61E,
                    SurfaceEffectPhase::Active => 0x07C631,
                },
                None,
                0,
                address(Some(id)),
                true,
            );
            player_surface_effect::step(&mut self.native.objects, &self.native.world, id).unwrap();
            self.free_compare(source);
            if self
                .native
                .objects
                .get(id)
                .unwrap()
                .base
                .flags
                .remove_after_tick
            {
                let result = call_near(
                    &mut source.bus,
                    0x7F335A,
                    &Entry {
                        x: address(Some(id)),
                        dbr: 0x7E,
                        p: 0x20,
                        ..Default::default()
                    },
                );
                assert!(result.returned);
                self.native.objects.remove(id).unwrap();
                retired += 1;
                self.free_compare(source);
            }
        }
        retired
    }
}

#[test]
fn complete_free_flight_matches_original_surface_effect_and_mode_transition_handoffs() {
    let mut source = Source::new(&rom(), 0);
    source.bus.enable_gsu();
    let mut effects = 0;
    let mut transitions = 0;
    for seed in 0..8192_u16 {
        let mut f = Fixture::new(&mut source);
        let low = seed as u8;
        let world = &mut f.native.world;
        world.surface_clipping_plane_height = Some(-1234);
        world.strategy_clock = seed;
        world.environment_plane_height = Some(if seed & 4 != 0 {
            seed.rotate_left(5) as i16
        } else {
            0
        });
        world.player_carry_mode = Some(low & 7);
        world.primary_player = (seed & 1 != 0).then_some(f.native.owner);
        world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(seed.wrapping_mul(197)),
            pressed: Buttons::from_bits(seed.rotate_left(7)),
        });
        world.scene.player_configuration = Some(if seed % 3 == 0 { 9 } else { 0 });
        world.reflect_all_contacts = Some(seed % 7 == 0);
        let owner = f.native.objects.get_mut(f.native.owner).unwrap();
        owner.extension.surface_contact.supporting_object = (seed % 5 != 0).then_some(f.collider);
        owner.extension.surface_contact.flags = low & 7;
        owner.extension.path_state.motion.carry_selected_player = seed % 7 == 0;
        owner.base.behavior_phase = 197;
        let origin = f.native.objects.get_mut(f.collider).unwrap();
        origin.base.pitch = Angle::from_units(low.rotate_left(3));
        origin.base.yaw = Angle::from_units(low.rotate_left(5));
        origin.extension.path_state.script_value = seed.wrapping_mul(199);
        let record = f.records();
        record.auxiliary.as_mut().unwrap().mode = [0x11, 0x12, 0x20, 0x30][usize::from(seed % 4)];
        record.auxiliary.as_mut().unwrap().action_flags = low.rotate_left(3);
        record.visit.as_mut().unwrap().pilot_code = (seed % 6) as u8;
        record
            .contact
            .as_mut()
            .unwrap()
            .hit
            .hold_secondary_protection = seed % 3 != 0;
        record.motion.as_mut().unwrap().walker_contact_control = low.rotate_left(5);
        record.motion.as_mut().unwrap().surface_height = seed.rotate_left(9) as i16;
        record.mode_selection.as_mut().unwrap().requested = (low & 0xF0) | (seed % 5) as u8;
        record.mode_selection.as_mut().unwrap().transition_control = low;
        record.mode_selection.as_mut().unwrap().surface_control = low.rotate_left(2);
        record
            .mode_selection
            .as_mut()
            .unwrap()
            .set_request_inhibited(seed % 7 == 0);
        record.vertical.as_mut().unwrap().control_flags = low;
        record.vertical.as_mut().unwrap().limit_flags = !low;
        f.free_seed(&mut source);
        f.plane(&mut source);
        f.free_step(
            &mut source,
            SurfaceEffectInputs {
                origin: Some(f.collider),
                lateral: Some(low as i8),
                vertical: Some(low.rotate_left(3) as i8),
                forward: Some(low.rotate_left(7) as i8),
            },
            seed.rotate_left(7),
        );
        effects += f
            .native
            .objects
            .active_objects()
            .filter(|(_, actor)| matches!(actor.base.behavior, Behavior::SurfaceEffect(_)))
            .count();
        transitions += usize::from(
            f.native
                .objects
                .get(f.native.owner)
                .unwrap()
                .base
                .behavior_phase
                != 197,
        );
    }
    assert!(
        effects > 1000 && transitions > 1000,
        "real mode effects and transitions required: {effects}, {transitions}"
    );
}

#[test]
fn complete_free_flight_matches_original_independent_controls_effect_lifetimes_and_pool_reuse() {
    let mut source = Source::new(&rom(), 0);
    source.bus.enable_gsu();
    let mut f = Fixture::new(&mut source);
    f.native.world.surface_clipping_plane_height = Some(-1234);
    f.records().auxiliary.as_mut().unwrap().action_flags = 5;
    f.records()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .hold_secondary_protection = true;
    f.records().vertical.as_mut().unwrap().profile.up_pitch = 16;
    f.records().vertical.as_mut().unwrap().profile.down_pitch = 240;
    f.records()
        .vertical
        .as_mut()
        .unwrap()
        .profile
        .upper_height_offset = 300;
    f.records()
        .vertical
        .as_mut()
        .unwrap()
        .profile
        .lower_height_offset = -300;
    f.free_seed(&mut source);
    let mut moved = 0;
    let mut retired = 0;
    for tick in 0..8192_u16 {
        // Only external controls/clocks are shared after initial seeding.
        // Pose, RNG, children, pool order and histories evolve independently.
        let held = tick.wrapping_mul(19) & 0x0F30;
        let pressed = if tick % 37 == 0 { 0x2000 } else { held & 0x30 };
        f.native.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(held),
            pressed: Buttons::from_bits(pressed),
        });
        source.bus.write16(0x1938, held);
        source.bus.write16(0x1936, pressed);
        f.native.world.strategy_clock = tick;
        source.bus.write8(0xC4, tick as u8);
        let carry = if tick % 11 < 5 { 0 } else { 2 };
        f.records().mode_selection.as_mut().unwrap().surface_control = carry;
        source.bus.write8(WRAM + SLOT + 0x6B64, carry);
        f.records().motion.as_mut().unwrap().walker_motion_control = (tick % 3) as u8;
        source.bus.write8(WRAM + SLOT + 0x6AA4, (tick % 3) as u8);
        let plane = if tick % 7 == 0 {
            0
        } else {
            (tick % 511) as i16 - 255
        };
        f.native.world.environment_plane_height = Some(plane);
        source.bus.write16(0x1E0F, plane as u16);
        let original = call(
            &mut source.bus,
            0x069075,
            &Entry {
                x: OWNER,
                dbr: 0x7E,
                p: 0x20,
                ..Default::default()
            },
        );
        assert!(original.returned);
        player_roll::prepare_shoulders(&f.native.objects, &mut f.native.world, f.native.owner)
            .unwrap();
        player_motion::capture_position(&f.native.objects, &mut f.native.world, f.native.owner)
            .unwrap();
        for (position, previous) in [(12, 0x6AC7), (14, 0x6AC9), (16, 0x6ACB)] {
            source.bus.write16(
                WRAM + SLOT + previous,
                source.bus.read16(u32::from(OWNER) + position),
            );
        }
        f.seed_context(&mut source);
        let before = f.native.objects.get(f.native.owner).unwrap().base.position;
        f.free_step(
            &mut source,
            SurfaceEffectInputs {
                origin: Some(f.proxy),
                lateral: Some(tick as i8),
                vertical: Some(tick.rotate_left(3) as i8),
                forward: Some(-29),
            },
            tick.rotate_left(7),
        );
        moved += usize::from(f.native.objects.get(f.native.owner).unwrap().base.position != before);
        retired += f.effects_step(&mut source);
    }
    assert!(
        moved > 1000 && retired > 2000,
        "independent movement and real effect retirement required: {moved}, {retired}"
    );
}

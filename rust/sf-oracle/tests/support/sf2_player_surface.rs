//! Entire unmodified two-wing surface response and real nested installers.
use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::collision_surface::SurfaceMode;
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::player_surface::{
    self, PlayerSurface, SurfaceContext, SurfaceError, SurfaceResponse,
};
use sf2_game::player_surface_particle::ParticleError;
use sf2_game::view_transition::ViewTransitionMode;
use sf2_game::weapon_dispatch::WeaponState;
use sf2_game::{Angle, Behavior, SoundEvent, Vector3};

#[derive(Clone, Copy)]
struct Case {
    mode: u8,
    hold: bool,
    material: u8,
    configuration: u8,
    carry_mode: u8,
    carrying: bool,
    view: u16,
    primary: bool,
    height: i16,
    plane: i16,
    upper: i16,
    pitch: u8,
    roll: u8,
    yaw: u8,
    bank: i16,
    fine_pitch: u16,
    recoil: i16,
    pilot: u8,
    number: u8,
}
impl Default for Case {
    fn default() -> Self {
        Self {
            mode: 1,
            hold: false,
            material: 0,
            configuration: 0,
            carry_mode: 0,
            carrying: false,
            view: 0,
            primary: true,
            height: 0,
            plane: 0,
            upper: 0,
            pitch: 0,
            roll: 0,
            yaw: 0,
            bank: 0x12AB,
            fine_pitch: 0x935A,
            recoil: 0,
            pilot: 0,
            number: 77,
        }
    }
}

struct Fixture {
    native: Native,
    runtime: PathRuntime,
    case: Case,
    alternate_thrust_target: Option<i8>,
    last_response: Option<SurfaceResponse>,
}
impl Fixture {
    fn new(source: &mut Source, count: usize, case: Case) -> Self {
        let mut native = Native::new(source, count, case.roll, case.yaw, 0x7FF0);
        let mut runtime = PathRuntime::default();
        let movement = native
            .world
            .player(&native.objects, native.owner)
            .unwrap()
            .flight_displacement;
        player_storage::replace(
            &mut native.objects,
            &mut native.world,
            &mut runtime,
            native.owner,
            PlayerStorageInputs {
                pilot_code: case.pilot,
                reserve_shield: 0,
                score: Default::default(),
            },
        )
        .unwrap();
        let storage =
            player_storage::get_mut(&native.objects, &mut runtime.resources, native.owner).unwrap();
        storage.fine_pitch = case.fine_pitch;
        let proxy = native
            .objects
            .active_ids()
            .iter()
            .copied()
            .find(|id| *id != native.owner)
            .unwrap();
        native.world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        native.world.surface_mode = Some(SurfaceMode { flags: case.mode });
        native.world.player_carry_mode = Some(case.carry_mode);
        native.world.scene.player_configuration = Some(case.configuration);
        native.world.view_transition_mode = Some(ViewTransitionMode { flags: case.view });
        native.world.primary_player = case.primary.then_some(native.owner);
        let records = native
            .world
            .player_mut(&native.objects, native.owner)
            .unwrap();
        records.surface = Some(PlayerSurface {
            plane_height: case.plane,
            material: case.material,
        });
        records
            .vertical
            .as_mut()
            .unwrap()
            .profile
            .upper_height_offset = case.upper;
        records.pose.as_mut().unwrap().shoulder_bank = case.bank;
        records
            .contact
            .as_mut()
            .unwrap()
            .hit
            .hold_secondary_protection = case.hold;
        records.contact.as_mut().unwrap().hit.camera_pitch_recoil = case.recoil;
        records.flight_displacement = movement;
        let actor = native.objects.get_mut(native.owner).unwrap();
        actor.base.position.y = case.height;
        actor.base.pitch = Angle::from_units(case.pitch);
        actor.extension.path_state.motion.carry_selected_player = case.carrying;
        source.bus.write16(0x14D6, address(Some(proxy)));
        source
            .bus
            .write16(0x12C3, if case.primary { OWNER } else { 0 });
        source.bus.write8(0x1B4D, case.mode);
        source.bus.write8(0x1DE2, case.configuration);
        source.bus.write8(0x1E13, case.carry_mode);
        source.bus.write16(0x1B84, case.view);
        source.bus.write8(0, case.number);
        source.bus.write8(0x1DB9, 197);
        source.bus.write8(0x1DBA, 255);
        source
            .bus
            .write16(u32::from(OWNER) + 14, case.height as u16);
        source.bus.write8(u32::from(OWNER) + 0x12, case.pitch);
        source.bus.write8(
            u32::from(OWNER) + 0x21,
            if case.carrying { 0x20 } else { 0 },
        );
        for (field, value) in [
            (0x6A7D, case.plane as u16),
            (0x6BF5, case.upper as u16),
            (0x6AD8, case.bank as u16),
            (0x6AB9, case.fine_pitch),
            (0x6B3B, case.recoil as u16),
        ] {
            source.bus.write16(WRAM + SLOT + field, value);
        }
        source
            .bus
            .write8(WRAM + SLOT + 0x6B7D, if case.hold { 0xB5 } else { 0x35 });
        source.bus.write8(WRAM + SLOT + 0x6A82, case.material);
        source.bus.write8(WRAM + SLOT + 0x6BFF, case.pilot);
        source.bus.write16(0x1D16, 0);
        Self {
            native,
            runtime,
            case,
            alternate_thrust_target: Some(197_u8 as i8),
            last_response: None,
        }
    }

    fn run(&mut self, source: &mut Source, fatal: bool) {
        let native = &mut self.native;
        let mut expected = *native.world.player(&native.objects, native.owner).unwrap();
        source.bus.write16(0x1D16, 0);
        source.run(0x07DD6F, fatal.then_some(0x008032), 0, OWNER, true);
        let actual = player_surface::respond(
            &mut native.objects,
            &mut native.world,
            &mut self.runtime.resources,
            native.owner,
            SurfaceContext {
                wing_child_number: Some(self.case.number),
                alternate_thrust_target: self.alternate_thrust_target,
            },
        );
        if fatal {
            assert!(
                matches!(
                    actual,
                    Err(SurfaceError::ObjectPoolExhausted
                        | SurfaceError::Particle(ParticleError::ObjectPoolExhausted))
                ),
                "{actual:?}"
            );
        } else {
            assert_eq!(
                actual,
                Ok(SurfaceResponse {
                    contact: source.last_carry,
                    effect_events: source.bus.read8(0x1DBA),
                    alternate_thrust_target: Some(source.bus.read8(0x1DB9) as i8),
                })
            );
            let response = actual.unwrap();
            self.alternate_thrust_target = response.alternate_thrust_target;
            self.last_response = Some(response);
        }
        expected.pose.as_mut().unwrap().shoulder_bank =
            source.bus.read16(WRAM + SLOT + 0x6AD8) as i16;
        expected.contact.as_mut().unwrap().hit.camera_pitch_recoil =
            source.bus.read16(WRAM + SLOT + 0x6B3B) as i16;
        assert_eq!(
            *native.world.player(&native.objects, native.owner).unwrap(),
            expected
        );
        assert_eq!(
            player_storage::get(&native.objects, &self.runtime.resources, native.owner)
                .unwrap()
                .fine_pitch,
            source.bus.read16(WRAM + SLOT + 0x6AB9)
        );
        let owner = native.objects.get(native.owner).unwrap();
        assert_eq!(
            owner.extension.clipping_plane.selector_byte(),
            source.bus.read8(WRAM + u32::from(OWNER) + 0x1CEF)
        );
        let proxy = native.world.weapons.unwrap().fallback.unwrap();
        let base = u32::from(address(Some(proxy)));
        let probe = native.objects.get(proxy).unwrap();
        assert_eq!(
            probe.base.position,
            Vector3 {
                x: source.bus.read16(base + 12) as i16,
                y: source.bus.read16(base + 14) as i16,
                z: source.bus.read16(base + 16) as i16
            }
        );
        let events: Vec<_> = native
            .world
            .audio
            .take_events()
            .into_iter()
            .flatten()
            .map(|event| {
                let SoundEvent::Authored(cue) = event else {
                    panic!("unexpected sound");
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
        for (i, event) in events.iter().enumerate() {
            assert_eq!(source.bus.read16(0x1CF6 + i as u32 * 2), *event);
        }
        native.compare_pool(source);
        for (id, actor) in native.objects.active_objects() {
            if actor.base.behavior != Behavior::FollowPath {
                continue;
            }
            let base = u32::from(address(Some(id)));
            assert_eq!(source.bus.read16(base + 4), 0xC08C);
            assert_eq!(source.bus.read16(base + 0x19), 0x7E1E);
            assert_eq!(source.bus.read8(base + 0x1B), 0x7F);
            assert_eq!(source.bus.read16(base + 0x2B), 0xF536);
            assert_eq!(source.bus.read8(base + 0x13), actor.base.child_number);
            assert_eq!(
                actor.base.path,
                Some(sf2_game::authored_paths::ALTERNATE_EXHAUST)
            );
            for (offset, value) in [
                (12, actor.base.position.x),
                (14, actor.base.position.y),
                (16, actor.base.position.z),
                (0x1CCF, actor.extension.relative_position.x),
                (0x1CD1, actor.extension.relative_position.y),
                (0x1CD3, actor.extension.relative_position.z),
            ] {
                assert_eq!(source.bus.read16(WRAM + base + offset) as i16, value);
            }
            for (offset, value) in [
                (0x12, actor.base.pitch.units()),
                (0x14, actor.base.yaw.units()),
                (0x16, actor.base.roll.units()),
                (0x2D, actor.base.hit_points),
                (0x2E, actor.base.attack_power),
                (0x1CF0, actor.extension.spawn_group),
            ] {
                assert_eq!(source.bus.read8(WRAM + base + offset), value);
            }
            assert_eq!(
                source.bus.read16(WRAM + base + 0x1CD8),
                address(actor.extension.parent)
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
                source.bus.read8(base + 0x26) & 8 != 0,
                actor.base.contacts.run_when_paused
            );
        }
    }
}

#[test]
fn surface_to_speed_handoff_retains_original_events_and_height_targets_across_visits() {
    use sf2_game::path_program::ActionGate;
    use sf2_game::player_speed::{self, SpeedContext};
    use sf2_game::{Buttons, InputState};
    let mut source = Source::new(&rom(), 0);
    for seed in 0..32_u8 {
        let case = Case {
            material: seed % 6,
            pilot: seed % 8,
            upper: (u16::from(seed) * 2003) as i16,
            height: if seed & 1 == 0 { -20 } else { 20 },
            configuration: if seed & 2 == 0 { 9 } else { 0 },
            ..Default::default()
        };
        let mut fixture = Fixture::new(&mut source, 2, case);
        fixture
            .native
            .objects
            .get_mut(fixture.native.owner)
            .unwrap()
            .base
            .speed = seed.wrapping_mul(17);
        fixture
            .native
            .world
            .player_mut(&fixture.native.objects, fixture.native.owner)
            .unwrap()
            .speed
            .as_mut()
            .unwrap()
            .thrust = seed.wrapping_mul(13) as i8;
        source
            .bus
            .write8(u32::from(OWNER) + 0x18, seed.wrapping_mul(17));
        source
            .bus
            .write8(WRAM + SLOT + 0x6B62, seed.wrapping_mul(13));
        for visit in 0..24_u16 {
            let surface_mode = if visit % 7 == 0 { 0 } else { 1 };
            let hold = visit % 11 == 0;
            fixture.native.world.surface_mode = Some(SurfaceMode {
                flags: surface_mode,
            });
            fixture
                .native
                .world
                .player_mut(&fixture.native.objects, fixture.native.owner)
                .unwrap()
                .contact
                .as_mut()
                .unwrap()
                .hit
                .hold_secondary_protection = hold;
            source.bus.write8(0x1B4D, surface_mode);
            source
                .bus
                .write8(WRAM + SLOT + 0x6B7D, if hold { 0x80 } else { 0 });
            fixture.run(&mut source, false);
            let native = &mut fixture.native;
            let mode = if visit % 3 == 0 { 0 } else { 1 };
            let actions = [4, 0, 0x20, 0x40][usize::from(visit / 6)];
            let held = if visit % 3 == 1 { 0x300 } else { 0 };
            let gate = u8::from(visit % 13 == 0);
            native.world.action_gate = Some(ActionGate { code: gate });
            native.world.strategy_clock = visit;
            native.world.processed_player_input = Some(InputState {
                held: Buttons::from_bits(held),
                pressed: Buttons::default(),
            });
            let auxiliary = native
                .world
                .player_mut(&native.objects, native.owner)
                .unwrap()
                .auxiliary
                .as_mut()
                .unwrap();
            auxiliary.mode = mode;
            auxiliary.action_flags = actions;
            source.bus.write8(0x1D72, gate);
            source.bus.write16(0x1938, held);
            source.bus.write16(0xC4, visit);
            source.bus.write8(WRAM + SLOT + 0x6AA0, mode);
            source.bus.write8(WRAM + SLOT + 0x6B77, actions);
            source.run(0x06F05D, Some(0x06F1FB), 0, OWNER, true);
            let context = SpeedContext::from(fixture.last_response.unwrap());
            player_speed::advance(
                &mut native.objects,
                &mut native.world,
                native.owner,
                context,
            )
            .unwrap();
            assert_eq!(
                native.objects.get(native.owner).unwrap().base.speed,
                source.bus.read8(u32::from(OWNER) + 0x18)
            );
            assert_eq!(
                native
                    .world
                    .player(&native.objects, native.owner)
                    .unwrap()
                    .speed
                    .unwrap()
                    .thrust,
                source.bus.read8(WRAM + SLOT + 0x6B62) as i8
            );
            // The next source service inherits the alternate target written
            // by speed. Mirror its branch semantics from native inputs only,
            // never importing a reference-machine result into native state.
            if gate == 0 {
                let pilot = usize::from(if case.pilot < 6 { case.pilot } else { 0 });
                fixture.alternate_thrust_target = match actions {
                    0x40 => Some([105, 107, 92, 90, 125, 123][pilot]),
                    0x20 => Some([-32, -32, -28, -26, -41, -43][pilot]),
                    0 if held == 0 => Some(0),
                    _ => fixture.alternate_thrust_target,
                };
            }
        }
    }
}

#[test]
fn surface_all_modes_materials_and_gate_combinations_match_original() {
    let mut source = Source::new(&rom(), 0);
    for mode in 0..=u8::MAX {
        for gate in 0..32_u8 {
            let case = Case {
                mode,
                material: mode,
                hold: gate & 1 != 0,
                view: if gate & 2 != 0 { 0xFFFF } else { 0xFFFD },
                primary: gate & 4 != 0,
                carry_mode: gate >> 3 & 1,
                carrying: gate & 16 != 0,
                ..Default::default()
            };
            Fixture::new(&mut source, 2, case).run(&mut source, false);
        }
    }
}

#[test]
fn surface_probe_rotation_signed_plane_boundaries_and_high_byte_feedback_match_original() {
    let mut source = Source::new(&rom(), 0);
    for roll in 0..=u8::MAX {
        for height in [
            i16::MIN,
            i16::MIN + 1,
            -36,
            -6,
            -5,
            -4,
            -1,
            0,
            1,
            4,
            5,
            6,
            36,
            i16::MAX,
        ] {
            for configuration in [0, 9] {
                let case = Case {
                    roll,
                    pitch: roll.rotate_left(2),
                    yaw: roll.rotate_left(4),
                    height,
                    configuration,
                    material: roll,
                    upper: height.wrapping_neg().wrapping_add((roll as i8) as i16),
                    plane: (roll as i8) as i16,
                    bank: (u16::from(roll) * 257) as i16,
                    recoil: if roll & 1 == 0 { 0 } else { -1 },
                    ..Default::default()
                };
                Fixture::new(&mut source, 2, case).run(&mut source, false);
            }
        }
    }
}

#[test]
fn surface_all_pilots_child_numbers_and_upper_contacts_bypass_effect_suppression_match_original() {
    let mut source = Source::new(&rom(), 0);
    for pilot in 0..=u8::MAX {
        for number in [0, 1, 16, 127, 128, 255] {
            let case = Case {
                pilot,
                number,
                configuration: 9,
                upper: -300,
                material: 4,
                carrying: true,
                carry_mode: 1,
                primary: pilot & 1 != 0,
                ..Default::default()
            };
            Fixture::new(&mut source, 2, case).run(&mut source, false);
        }
    }
}

#[test]
fn surface_both_allocation_sites_preserve_partial_state_at_original_fatal_pool_boundary() {
    let mut source = Source::new(&rom(), 0);
    for material in [0, 1, 4, 5, 255] {
        for count in [58, 59, 60] {
            Fixture::new(
                &mut source,
                count,
                Case {
                    material,
                    ..Default::default()
                },
            )
            .run(&mut source, count >= 59);
        }
    }
}

#[test]
fn surface_continuous_visits_preserve_independent_pose_and_effects_across_all_early_skips() {
    let mut source = Source::new(&rom(), 0);
    for seed in 0..32_u8 {
        let mut fixture = Fixture::new(
            &mut source,
            2,
            Case {
                pilot: seed,
                number: seed,
                upper: -512,
                ..Default::default()
            },
        );
        for visit in 0..24_u8 {
            let mode = if visit % 7 == 0 { 0 } else { 1 };
            let hold = visit % 5 == 0;
            let view = if visit % 3 == 0 { 2 } else { 0 };
            let height = (i16::from(visit) - 12) * 3;
            let roll = seed.wrapping_add(visit.wrapping_mul(5));
            let native = &mut fixture.native;
            native.world.surface_mode = Some(SurfaceMode { flags: mode });
            native.world.view_transition_mode = Some(ViewTransitionMode { flags: view });
            let records = native
                .world
                .player_mut(&native.objects, native.owner)
                .unwrap();
            records
                .contact
                .as_mut()
                .unwrap()
                .hit
                .hold_secondary_protection = hold;
            records.surface.as_mut().unwrap().material = visit % 6;
            let actor = native.objects.get_mut(native.owner).unwrap();
            actor.base.position.y = height;
            actor.base.roll = Angle::from_units(roll);
            source.bus.write8(0x1B4D, mode);
            source.bus.write16(0x1B84, view);
            source
                .bus
                .write8(WRAM + SLOT + 0x6B7D, if hold { 0x80 } else { 0 });
            source.bus.write8(WRAM + SLOT + 0x6A82, visit % 6);
            source.bus.write16(u32::from(OWNER) + 14, height as u16);
            source.bus.write8(u32::from(OWNER) + 0x16, roll);
            fixture.run(&mut source, false);
        }
    }
}

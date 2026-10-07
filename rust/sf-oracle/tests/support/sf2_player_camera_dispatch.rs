//! Full original dispatcher, all three primary modes, and real auxiliary and
//! continuity consumers. Native/original outputs remain independently retained.
use super::{rom, AuxiliaryFixture, Source, Task, OWNER, SLOT, TASKS, WRAM};
use sf2_game::player_camera_dispatch::{self, PlayerCameraDispatch};
use sf2_game::player_camera_tracking::TrackingStyle as Style;
use sf2_game::player_storage;
use sf2_game::world_occupancy::{MarkerCoverage, OccupancyChange, WorldOccupancy, WorldRectangle};
use sf2_game::Vector3;

struct DispatchFixture {
    auxiliary: AuxiliaryFixture,
}
impl DispatchFixture {
    fn new(source: &mut Source) -> Self {
        let mut auxiliary = AuxiliaryFixture::new();
        let f = &mut auxiliary.ground.position.height.camera.inner;
        f.records().camera_surface = Some(Default::default());
        f.records().camera_dispatch = Some(Default::default());
        f.records().mode_selection = Some(Default::default());
        f.world.action_gate = Some(Default::default());
        f.world.camera_height_limits = Some((-600, 0));
        f.world.camera_projection_offset = Some(0);
        f.world.horizon_disabled = Some(false);
        let mut occupancy = WorldOccupancy::default();
        for z in 0..128_u16 {
            for group in 0..16_u16 {
                let mut byte = 0;
                for bit in 0..8 {
                    let x = group * 8 + bit;
                    if (x ^ z) & 3 == 0 {
                        byte |= 1 << bit;
                        let marker = MarkerCoverage::from_rectangle(WorldRectangle {
                            x: (x * 512) as i16,
                            z: (z * 512) as i16,
                            width: 1,
                            depth: 1,
                        })
                        .unwrap();
                        occupancy.apply(&marker, OccupancyChange::Mark);
                    }
                }
                source
                    .bus
                    .write8(WRAM + 0xCF36 + u32::from(z * 16 + group), byte);
            }
        }
        f.world.occupancy = Some(occupancy);
        Self { auxiliary }
    }
    fn values(&self) -> Vec<(u32, u16, bool)> {
        let f = &self.auxiliary.ground.position.height.camera.inner;
        let r = f.world.player(&f.objects, f.owner).unwrap();
        let state = r.camera_dispatch.unwrap();
        let mut values = vec![];
        for (field, value) in [
            (WRAM + 0x1D72, f.world.action_gate.unwrap().code),
            (
                WRAM + 0x1D9D,
                0x25 | u8::from(f.world.horizon_disabled.unwrap()) * 0x80,
            ),
            (
                WRAM + SLOT + 0x6A9C,
                if state.style.is_some() { 7 } else { 0 },
            ),
            (
                WRAM + SLOT + 0x6B64,
                r.mode_selection.unwrap().surface_control,
            ),
            (
                WRAM + SLOT + 0x6B65,
                0xA5 | u8::from(state.projection_correction_disabled) * 0x40,
            ),
        ] {
            values.push((field, u16::from(value), true));
        }
        for (field, value) in [
            (
                WRAM + SLOT + 0x6A9A,
                match state.style {
                    None => 0,
                    Some(Style::Normal) => 0x8048,
                    Some(Style::ProjectionCorrected) => 0x8089,
                    Some(Style::Surface) => 0x80B3,
                },
            ),
            (
                WRAM + SLOT + 0x6AF7,
                r.motion.unwrap().surface_height as u16,
            ),
            (WRAM + SLOT + 0x6A7D, r.surface.unwrap().plane_height as u16),
            (
                WRAM + 0x1E32,
                f.world.camera_height_limits.unwrap().0 as u16,
            ),
            (
                WRAM + 0x1E34,
                f.world.camera_height_limits.unwrap().1 as u16,
            ),
            (
                WRAM + 0x1E52,
                f.world.camera_projection_offset.unwrap() as u16,
            ),
        ] {
            values.push((field, value, false));
        }
        values
    }
    fn seed(&mut self, source: &mut Source) {
        self.auxiliary.seed(source);
        for (field, value, byte) in self.values() {
            if byte {
                source.bus.write8(field, value as u8);
            } else {
                source.bus.write16(field, value);
            }
        }
    }
    fn verify(&self, source: &Source, phase: &str) {
        self.auxiliary.verify(source, phase);
        for (field, value, byte) in self.values() {
            let original = if byte {
                u16::from(source.bus.read8(field))
            } else {
                source.bus.read16(field)
            };
            assert_eq!(value, original, "{phase} field={field:06X}");
        }
    }
    fn step(&mut self, source: &mut Source) {
        source.run(0x078000, None, 0, OWNER, true);
        let f = &mut self.auxiliary.ground.position.height.camera.inner;
        player_camera_dispatch::advance(
            &mut f.objects,
            &mut f.world,
            &mut self.auxiliary.ground.runtime,
            f.owner,
        )
        .unwrap();
        self.verify(source, "complete camera dispatcher");
    }
    fn varied(&mut self, word: u16, style: Option<Style>) {
        self.auxiliary
            .varied(word, TASKS[usize::from(word >> 8) % TASKS.len()]);
        let f = &mut self.auxiliary.ground.position.height.camera.inner;
        *f.records().camera_dispatch.as_mut().unwrap() = PlayerCameraDispatch {
            style,
            projection_correction_disabled: word & 32 != 0,
            outside_occupied_world: word & 64 != 0,
        };
        f.records().mode_selection.as_mut().unwrap().surface_control = word as u8;
        f.records().auxiliary.as_mut().unwrap().action_flags = (word >> 8) as u8;
        f.records()
            .contact
            .as_mut()
            .unwrap()
            .hit
            .camera_pitch_recoil = word as i16;
        f.records()
            .contact
            .as_mut()
            .unwrap()
            .hit
            .hold_secondary_protection = word & 128 != 0;
        f.records().occupancy_exempt = Some(word & 256 != 0);
        f.records()
            .camera_surface
            .as_mut()
            .unwrap()
            .returning_below_plane = word & 512 != 0;
        f.records().camera_ground.as_mut().unwrap().hold_pitch = word & 1024 != 0;
        f.records()
            .camera_ground
            .as_mut()
            .unwrap()
            .follow_environment_plane = word & 2048 != 0;
        f.records().consumable.as_mut().unwrap().recovery_blocked = word & 4096 != 0;
        f.records().boundary.as_mut().unwrap().return_position = Vector3 {
            x: word as i16,
            y: word.rotate_left(3) as i16,
            z: !word as i16,
        };
        f.records().surface.as_mut().unwrap().plane_height = word.rotate_left(7) as i16;
        f.world.environment_plane_height = Some(word.rotate_left(5) as i16);
        f.world.player_carry_mode = Some(if word & 2 == 0 { 1 } else { 2 });
        f.objects
            .get_mut(f.owner)
            .unwrap()
            .extension
            .path_state
            .motion
            .carry_selected_player = word & 4 != 0;
        f.world.camera_height_limits = Some((word as i16, word.rotate_left(3) as i16));
        f.world.camera_projection_offset = Some(!word as i16);
        f.world.horizon_disabled = Some(word & 8 != 0);
        f.world.scene.player_view_control = Some(word as u8);
        f.world.action_gate.as_mut().unwrap().code = 0;
        f.world
            .view_transition_mode
            .as_mut()
            .unwrap()
            .set_active(false);
        player_storage::get_mut(
            &f.objects,
            &mut self.auxiliary.ground.runtime.resources,
            f.owner,
        )
        .unwrap()
        .fine_yaw = word;
    }
}

#[test]
fn camera_dispatch_matches_original_all_action_bytes_and_scripted_view_gates() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = DispatchFixture::new(&mut source);
    for style in [
        None,
        Some(Style::Normal),
        Some(Style::ProjectionCorrected),
        Some(Style::Surface),
    ] {
        for word in 0..=u16::MAX {
            fixture.varied(word, style);
            let f = &mut fixture.auxiliary.ground.position.height.camera.inner;
            f.world.action_gate.as_mut().unwrap().code = word as u8;
            f.world
                .view_transition_mode
                .as_mut()
                .unwrap()
                .set_active(word & 256 != 0);
            fixture.seed(&mut source);
            fixture.step(&mut source);
        }
    }
}

#[test]
fn camera_dispatch_all_three_modes_match_original_word_geometry_controls_and_auxiliary_tasks() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = DispatchFixture::new(&mut source);
    for style in [Style::Normal, Style::ProjectionCorrected, Style::Surface] {
        for word in 0..=u16::MAX {
            fixture.varied(word, Some(style));
            fixture.seed(&mut source);
            fixture.step(&mut source);
        }
    }
}

#[test]
fn camera_projection_matches_original_every_wrapped_height_midpoint_and_inhibition() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = DispatchFixture::new(&mut source);
    for gate in 0..3 {
        for word in 0..=u16::MAX {
            fixture.varied(word, Some(Style::ProjectionCorrected));
            let f = &mut fixture.auxiliary.ground.position.height.camera.inner;
            f.world
                .view_transition_mode
                .as_mut()
                .unwrap()
                .set_active(gate == 1);
            f.records()
                .camera_dispatch
                .as_mut()
                .unwrap()
                .projection_correction_disabled = gate == 2;
            f.world.camera_height_limits = Some((word as i16, word.rotate_left(7) as i16));
            f.objects.get_mut(f.view).unwrap().base.position.y = word.wrapping_mul(733) as i16;
            fixture.seed(&mut source);
            source.run(0x079489, None, 0, OWNER, true);
            let f = &mut fixture.auxiliary.ground.position.height.camera.inner;
            player_camera_dispatch::advance_projection(&f.objects, &mut f.world, f.owner).unwrap();
            fixture.verify(&source, "projection correction");
        }
    }
}

#[test]
fn camera_dispatch_retains_independent_modes_and_history_through_complete_continuity() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = DispatchFixture::new(&mut source);
    fixture.varied(0x1257, Some(Style::Normal));
    fixture.seed(&mut source);
    for tick in 0..8192_u16 {
        let f = &mut fixture.auxiliary.ground.position.height.camera.inner;
        let style = [
            None,
            Some(Style::Normal),
            Some(Style::ProjectionCorrected),
            Some(Style::Surface),
        ][usize::from(tick / 31) % 4];
        f.records().camera_dispatch.as_mut().unwrap().style = style;
        source.bus.write16(
            WRAM + SLOT + 0x6A9A,
            match style {
                None => 0,
                Some(Style::Normal) => 0x8048,
                Some(Style::ProjectionCorrected) => 0x8089,
                Some(Style::Surface) => 0x80B3,
            },
        );
        source
            .bus
            .write8(WRAM + SLOT + 0x6A9C, if style.is_some() { 7 } else { 0 });
        let code = if tick % 17 == 0 { 1 } else { 0 };
        f.world.action_gate.as_mut().unwrap().code = code;
        source.bus.write8(WRAM + 0x1D72, code);
        f.world
            .view_transition_mode
            .as_mut()
            .unwrap()
            .set_active(tick % 19 == 0);
        source
            .bus
            .write16(WRAM + 0x1B84, f.world.view_transition_mode.unwrap().flags);
        let task = [
            Task::None,
            Task::Initialize(super::Style::EncounterFocus),
            Task::Initialize(super::Style::Retreat),
            Task::Handoff,
        ][usize::from(tick / 251) % 4];
        if tick % 251 == 0 {
            f.records().camera_auxiliary.as_mut().unwrap().task = task;
            source
                .bus
                .write16(WRAM + SLOT + 0x6A9D, super::source_task(task));
            source
                .bus
                .write8(WRAM + SLOT + 0x6A9F, if task == Task::None { 0 } else { 7 });
        }
        let result = sf_oracle::call_near(
            &mut source.bus,
            0x069A26,
            &sf_oracle::Entry {
                x: OWNER,
                dbr: 0x7E,
                p: 0x20,
                ..Default::default()
            },
        );
        assert!(result.returned);
        let f = &mut fixture.auxiliary.ground.position.height.camera.inner;
        player_camera_dispatch::advance_with_continuity(
            &mut f.objects,
            &mut f.world,
            &mut fixture.auxiliary.ground.runtime,
            f.owner,
        )
        .unwrap();
        fixture.verify(&source, "complete retained camera and continuity");
    }
}

#[test]
fn normal_camera_plane_clamp_matches_original_every_height_word_and_task() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = DispatchFixture::new(&mut source);
    for style in [
        None,
        Some(Style::Normal),
        Some(Style::ProjectionCorrected),
        Some(Style::Surface),
    ] {
        for word in 0..=u16::MAX {
            fixture.varied(word, style);
            fixture.seed(&mut source);
            source.run(0x079D36, None, 0, OWNER, true);
            let f = &mut fixture.auxiliary.ground.position.height.camera.inner;
            player_camera_dispatch::clamp_normal_to_plane(&mut f.objects, &f.world, f.owner)
                .unwrap();
            fixture.verify(&source, "post-continuity plane limit");
        }
    }
}

#[test]
fn free_flight_camera_installer_matches_original_all_modes_tasks_and_protection_gates() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = DispatchFixture::new(&mut source);
    for style in [
        None,
        Some(Style::Normal),
        Some(Style::ProjectionCorrected),
        Some(Style::Surface),
    ] {
        for task in TASKS {
            for mode_and_protection in 0..512_u16 {
                fixture.varied(mode_and_protection.wrapping_mul(73), style);
                let f = &mut fixture.auxiliary.ground.position.height.camera.inner;
                f.records().auxiliary.as_mut().unwrap().mode = mode_and_protection as u8;
                f.records().camera_auxiliary.as_mut().unwrap().task = task;
                f.records()
                    .contact
                    .as_mut()
                    .unwrap()
                    .hit
                    .hold_secondary_protection = mode_and_protection & 256 != 0;
                fixture.seed(&mut source);
                source.run(0x06869C, Some(0x06871A), 0, OWNER, true);
                let f = &mut fixture.auxiliary.ground.position.height.camera.inner;
                player_camera_dispatch::select_free_flight_camera(
                    &mut f.objects,
                    &mut f.world,
                    f.owner,
                )
                .unwrap();
                fixture.verify(&source, "free flight camera installation prefix");
            }
        }
    }
}

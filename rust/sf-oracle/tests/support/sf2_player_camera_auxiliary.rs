//! Unmodified auxiliary task dispatch, real installers and view continuity.
use super::super::PROXY;
use super::{rom, GroundFixture, Source, OWNER, SLOT, VIEW, WRAM};
use sf2_game::path_motion::PublishedPlayerMotion;
use sf2_game::path_scene_state::{CameraTrackingTarget, EncounterCameraFocus, EncounterHandoff};
use sf2_game::player_camera_auxiliary::{self, AuxiliaryCameraTask as Task, OrbitStyle as Style};
use sf2_game::view_transition::FixedViewAngles;
use sf2_game::{Angle, Vector3};
use sf_oracle::{call_near, Entry};

const TASKS: [Task; 10] = [
    Task::None,
    Task::Handoff,
    Task::Initialize(Style::EncounterFocus),
    Task::Orbit(Style::EncounterFocus),
    Task::Initialize(Style::FlightDistance),
    Task::Orbit(Style::FlightDistance),
    Task::Initialize(Style::ModeDistance),
    Task::Orbit(Style::ModeDistance),
    Task::Initialize(Style::Retreat),
    Task::Orbit(Style::Retreat),
];

fn source_task(task: Task) -> u16 {
    match task {
        Task::None => 0,
        Task::Handoff => 0x9E34,
        Task::Initialize(Style::EncounterFocus) => 0x9ECA,
        Task::Orbit(Style::EncounterFocus) => 0x9EE0,
        Task::Initialize(Style::FlightDistance) => 0x9EF7,
        Task::Orbit(Style::FlightDistance) => 0x9F0D,
        Task::Initialize(Style::ModeDistance) => 0x9F17,
        Task::Orbit(Style::ModeDistance) => 0x9F2D,
        Task::Initialize(Style::Retreat) => 0x9F44,
        Task::Orbit(Style::Retreat) => 0x9F68,
    }
}

struct AuxiliaryFixture {
    ground: GroundFixture,
}
impl AuxiliaryFixture {
    fn new() -> Self {
        let mut ground = GroundFixture::new();
        ground.position.height.check_prepared = false;
        let f = &mut ground.position.height.camera.inner;
        f.records().camera_auxiliary = Some(Default::default());
        f.records().flight_displacement = Some(Default::default());
        f.world.handoff = Some(Default::default());
        f.world.camera_focus = Some(Default::default());
        f.world.camera_tracking = Some(CameraTrackingTarget {
            actor: Some(f.owner),
        });
        f.world.published_motion = Some(Default::default());
        f.world.encounter_timer_steps = Some(0);
        f.world.scene.player_configuration = Some(0);
        Self { ground }
    }
    fn values(&self) -> Vec<(u32, u16, bool)> {
        let f = &self.ground.position.height.camera.inner;
        let records = f.world.player(&f.objects, f.owner).unwrap();
        let state = records.camera_auxiliary.unwrap();
        let view = f.objects.get(f.view).unwrap();
        let proxy = f.objects.get(f.proxy).unwrap();
        let handoff = f.world.handoff.unwrap();
        let focus = f.world.camera_focus.unwrap().position;
        let published = f.world.published_motion.unwrap().position;
        let mut values = vec![];
        for (field, value) in [
            (
                WRAM + SLOT + 0x6A9F,
                if state.task == Task::None { 0 } else { 7 },
            ),
            (
                WRAM + VIEW + 0x1CE2,
                view.extension.path_state.script_parameter,
            ),
            (
                WRAM + PROXY + 0x15,
                proxy.extension.path_state.repeat_counter,
            ),
            (WRAM + 0x1DE2, f.world.scene.player_configuration.unwrap()),
        ] {
            values.push((field, u16::from(value), true));
        }
        for (field, value) in [
            (WRAM + SLOT + 0x6A9D, source_task(state.task)),
            (WRAM + SLOT + 0x6B54, state.retreat_distance as u16),
            (
                WRAM + SLOT + 0x6B0B,
                records.flight_displacement.unwrap().x as u16,
            ),
            (
                WRAM + SLOT + 0x6B0D,
                records.flight_displacement.unwrap().y as u16,
            ),
            (
                WRAM + SLOT + 0x6B0F,
                records.flight_displacement.unwrap().z as u16,
            ),
            (WRAM + VIEW + 0x32, view.base.velocity.x as u16),
            (WRAM + VIEW + 0x34, view.base.velocity.y as u16),
            (WRAM + VIEW + 0x36, view.base.velocity.z as u16),
            (WRAM + VIEW + 0x1CE4, view.extension.path_state.script_value),
            (WRAM + 0x1D88, handoff.x as u16),
            (WRAM + 0x1D8C, handoff.z as u16),
            (WRAM + 0x1D8E, handoff.heading_word),
            (WRAM + 0x1E01, focus.x as u16),
            (WRAM + 0x1E03, focus.y as u16),
            (WRAM + 0x1E05, focus.z as u16),
            (WRAM + 0xD7EC, published.x as u16),
            (WRAM + 0xD7EE, published.y as u16),
            (WRAM + 0xD7F0, published.z as u16),
            (WRAM + 0x1C0A, f.world.encounter_timer_steps.unwrap()),
            (
                WRAM + 0x1DFF,
                match f.world.camera_tracking.unwrap().actor {
                    Some(id) if id == f.owner => OWNER,
                    Some(id) if id == f.proxy => PROXY as u16,
                    Some(id) if id == f.view => VIEW as u16,
                    None => 0,
                    _ => panic!("unmapped target"),
                },
            ),
        ] {
            values.push((field, value, false));
        }
        values
    }
    fn seed(&mut self, source: &mut Source) {
        self.ground.seed(source);
        for (field, value, byte) in self.values() {
            if byte {
                source.bus.write8(field, value as u8);
            } else {
                source.bus.write16(field, value);
            }
        }
    }
    fn verify(&self, source: &Source, phase: &str) {
        self.ground.verify(source, phase);
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
        source.run(0x079DF6, None, 0, OWNER, true);
        let f = &mut self.ground.position.height.camera.inner;
        player_camera_auxiliary::advance(
            &mut f.objects,
            &mut f.world,
            &mut self.ground.runtime,
            f.owner,
        )
        .unwrap();
        self.verify(source, "auxiliary footer");
    }
    fn varied(&mut self, word: u16, task: Task) {
        let f = &mut self.ground.position.height.camera.inner;
        let r = f.records();
        let state = r.camera_auxiliary.as_mut().unwrap();
        state.task = task;
        state.retreat_distance = word as i16;
        r.auxiliary.as_mut().unwrap().mode = word as u8;
        r.flight_displacement = Some(Vector3 {
            x: word as i16,
            y: word.rotate_left(5) as i16,
            z: !word as i16,
        });
        f.world.handoff = Some(EncounterHandoff {
            x: word.wrapping_mul(73) as i16,
            z: !word as i16,
            heading_word: word,
            player_flags: 0xAC,
        });
        f.world.camera_focus = Some(EncounterCameraFocus {
            position: Vector3 {
                x: word.rotate_left(7) as i16,
                y: -111,
                z: word.wrapping_mul(977) as i16,
            },
        });
        f.world.published_motion = Some(PublishedPlayerMotion {
            position: Vector3 {
                x: !word as i16,
                y: word.rotate_left(3) as i16,
                z: word.wrapping_mul(919) as i16,
            },
            delta: Default::default(),
        });
        f.world.encounter_timer_steps = Some(word);
        f.world.scene.player_configuration = Some(if word & 1 == 0 { 9 } else { 0 });
        f.world.surface_mode.as_mut().unwrap().flags = word as u8;
        f.world.camera_tracking = Some(CameraTrackingTarget {
            actor: Some(if word & 2 == 0 { f.owner } else { f.proxy }),
        });
        let actor = f.objects.get_mut(f.owner).unwrap();
        actor.base.position = Vector3 {
            x: word as i16,
            y: word.wrapping_mul(733) as i16,
            z: word.rotate_left(11) as i16,
        };
        let proxy = f.objects.get_mut(f.proxy).unwrap();
        proxy.base.position = Vector3 {
            x: 197,
            y: -253,
            z: -1941,
        };
        proxy.base.roll = Angle::from_units(word as u8);
        let view = f.objects.get_mut(f.view).unwrap();
        view.base.position = Vector3 {
            x: word.rotate_left(3) as i16,
            y: word as i16,
            z: !word as i16,
        };
        view.base.velocity = Vector3 {
            x: 1751,
            y: -799,
            z: 973,
        };
        FixedViewAngles {
            pitch: word.rotate_left(3),
            yaw: word,
            roll: !word,
        }
        .write_to(view);
        view.extension.path_state.platform_carry.saved_position = Vector3 {
            x: word.rotate_left(5) as i16,
            y: word.wrapping_mul(79) as i16,
            z: word as i16,
        };
        view.extension.path_state.motion_delta = Vector3 {
            x: word as i16,
            y: word.rotate_left(3) as i16,
            z: !word as i16,
        };
        view.extension.path_state.script_parameter = word as u8;
        view.extension.path_state.script_value = word;
        self.ground.runtime.steering.unchanged_axes = 0xD7;
    }
}

#[test]
fn auxiliary_camera_all_tasks_match_original_full_word_geometry_history_and_timer_boundaries() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = AuxiliaryFixture::new();
    for task in TASKS {
        for word in 0..=u16::MAX {
            fixture.varied(word, task);
            fixture.seed(&mut source);
            fixture.step(&mut source);
        }
    }
}

#[test]
fn auxiliary_camera_map_installers_match_original_every_blend_control_byte() {
    use sf2_game::view_blend::ViewBlendControl;
    let mut source = Source::new(&rom(), 0);
    let mut fixture = AuxiliaryFixture::new();
    for (entry, task) in [
        (0x0DC75C, Task::Initialize(Style::ModeDistance)),
        (0x0DC783, Task::Handoff),
        (0x0DC795, Task::Initialize(Style::Retreat)),
        (0x0DC7BC, Task::Initialize(Style::FlightDistance)),
        (0x0DC7E3, Task::Initialize(Style::EncounterFocus)),
        (0x0DC80A, Task::None),
    ] {
        for flags in 0..=255_u16 {
            fixture.varied(flags.wrapping_mul(257), Task::Orbit(Style::Retreat));
            let f = &mut fixture.ground.position.height.camera.inner;
            ViewBlendControl {
                capture_position: flags & 8 != 0,
                capture_rotation: flags & 16 != 0,
                discard_capture: flags & 32 != 0,
                position_active: flags & 64 != 0,
                rotation_active: flags & 128 != 0,
                fast_position_recovery: flags & 1 != 0,
            }
            .write_to(f.objects.get_mut(f.view).unwrap());
            fixture.seed(&mut source);
            let result = call_near(
                &mut source.bus,
                entry,
                &Entry {
                    x: OWNER,
                    y: SLOT as u16,
                    dbr: 0x7E,
                    p: 0x20,
                    ..Default::default()
                },
            );
            assert!(result.returned);
            let f = &mut fixture.ground.position.height.camera.inner;
            player_camera_auxiliary::install(&mut f.objects, &mut f.world, f.owner, task).unwrap();
            fixture.verify(&source, "map installer");
        }
    }
}

#[test]
fn auxiliary_camera_retreat_matches_original_every_distance_in_flight_and_ground() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = AuxiliaryFixture::new();
    for mode in [0x11, 0x21] {
        for distance in 0..=u16::MAX {
            fixture.varied(distance, Task::Orbit(Style::Retreat));
            let f = &mut fixture.ground.position.height.camera.inner;
            f.records().auxiliary.as_mut().unwrap().mode = mode;
            f.world.scene.player_configuration = Some(9);
            fixture.seed(&mut source);
            fixture.step(&mut source);
        }
    }
}

#[test]
fn auxiliary_camera_tasks_retain_independent_state_through_real_continuity_and_mode_switches() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = AuxiliaryFixture::new();
    fixture.varied(0xE5B7, Task::None);
    fixture.seed(&mut source);
    for tick in 0..8192_u16 {
        // These are external commands, never original output replay.
        if tick % 127 == 0 {
            let (entry, task) = [
                (0x0DC7E3, Task::Initialize(Style::EncounterFocus)),
                (0x0DC75C, Task::Initialize(Style::ModeDistance)),
                (0x0DC795, Task::Initialize(Style::Retreat)),
                (0x0DC783, Task::Handoff),
                (0x0DC7BC, Task::Initialize(Style::FlightDistance)),
                (0x0DC80A, Task::None),
            ][usize::from(tick / 127) % 6];
            let result = call_near(
                &mut source.bus,
                entry,
                &Entry {
                    x: OWNER,
                    y: SLOT as u16,
                    dbr: 0x7E,
                    p: 0x20,
                    ..Default::default()
                },
            );
            assert!(result.returned);
            let f = &mut fixture.ground.position.height.camera.inner;
            player_camera_auxiliary::install(&mut f.objects, &mut f.world, f.owner, task).unwrap();
            fixture.verify(&source, "retained installation");
        }
        let f = &mut fixture.ground.position.height.camera.inner;
        f.records().auxiliary.as_mut().unwrap().mode = (tick >> 4) as u8;
        source.bus.write8(WRAM + SLOT + 0x6AA0, (tick >> 4) as u8);
        let position = Vector3 {
            x: tick.wrapping_mul(31) as i16,
            y: tick.wrapping_neg() as i16,
            z: tick.wrapping_mul(97) as i16,
        };
        f.objects.get_mut(f.owner).unwrap().base.position = position;
        for (field, value) in [(12, position.x), (14, position.y), (16, position.z)] {
            source
                .bus
                .write16(WRAM + u32::from(OWNER) + field, value as u16);
        }
        fixture.step(&mut source);
        source.run(0x0797FB, None, 0, OWNER, true);
        let f = &mut fixture.ground.position.height.camera.inner;
        sf2_game::view_blend::advance(&mut f.objects, &f.world).unwrap();
        fixture.verify(&source, "retained continuity");
    }
}

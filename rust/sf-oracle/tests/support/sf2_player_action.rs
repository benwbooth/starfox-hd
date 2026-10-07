//! Execute the unmodified parallel-action dispatcher and every callback in
//! both source-complete native streams. No service or timing branch is patched.
use super::{rom, Source, WRAM};
use sf2_game::path_program::{ProjectileTrigger, SelectedAuxiliaryState};
use sf2_game::path_scene_state::SceneTransitionControl;
use sf2_game::path_sound::MusicControlRequest;
use sf2_game::player_action::{
    self, PlayerAction, PlayerActionState, PlayerServiceFlags, ScenePalette,
};
use sf2_game::player_camera_auxiliary::{AuxiliaryCameraTask, OrbitStyle, PlayerCameraAuxiliary};
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use sf2_game::view_blend::ViewBlendControl;
use sf2_game::view_transition::ViewTransitionMode;
use sf2_game::{
    Behavior, Button, Buttons, InputState, Object, ObjectId, ObjectKind, ObjectStore, RandomState,
    ShapeId,
};

const OWNER: u16 = 0x0500;
const SLOT: u32 = 64;
const VIEW: u32 = 0x033F;

fn action_address(action: Option<PlayerAction>) -> u16 {
    match action {
        None => 0,
        Some(PlayerAction::TriggeredProjectile) => 0xBDDA,
        Some(PlayerAction::ForcedRetreat) => 0xBF63,
    }
}

pub(super) struct Fixture {
    pub(super) objects: ObjectStore,
    pub(super) world: ScenePathWorld,
    pub(super) owner: ObjectId,
    view: ObjectId,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        let view = objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        let mut world = ScenePathWorld::new(RandomState::default());
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    action: Some(PlayerActionState::default()),
                    mission: Some(Default::default()),
                    contact: Some(Default::default()),
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode: 0,
                        action_flags: 0,
                        stored_world_position: Default::default(),
                        stored_rotation: Default::default(),
                    }),
                    camera_auxiliary: Some(PlayerCameraAuxiliary::default()),
                    ..Default::default()
                },
            )
            .unwrap();
        world.fixed_players[0] = Some(view);
        world.view_transition_mode = Some(ViewTransitionMode { flags: 0 });
        world.scene_transition = Some(SceneTransitionControl::default());
        world.player_view_options_enabled = Some(true);
        world.projectile_trigger = Some(ProjectileTrigger::default());
        world.player_service_flags = Some(PlayerServiceFlags::default());
        world.scene.player_configuration = Some(0);
        world.palette = Some(ScenePalette {
            colors: std::array::from_fn(|i| (i as u16 * 251) | 0x8000),
            saved_colors: [123; 128],
        });
        Self {
            objects,
            world,
            owner,
            view,
        }
    }

    pub(super) fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }

    pub(super) fn seed(&self, source: &mut Source, pressed: bool) {
        let r = self.world.player(&self.objects, self.owner).unwrap();
        let action = r.action.unwrap();
        source.bus.write16(u32::from(OWNER) + 0x2B, SLOT as u16);
        for (address, value) in [
            (0x1B84, self.world.view_transition_mode.unwrap().flags),
            (0x1B78, self.world.scene_transition.unwrap().phase_word),
            (SLOT + 0x6C13, action_address(action.action)),
            (SLOT + 0x6C16, action.elapsed),
            (SLOT + 0x6C18, action.auxiliary_counter),
            (SLOT + 0x6C1A, action.total_updates),
        ] {
            source.bus.write16(WRAM + address, value);
        }
        let camera = match r.camera_auxiliary.unwrap().task {
            AuxiliaryCameraTask::Handoff => 0x9DF6,
            AuxiliaryCameraTask::Initialize(OrbitStyle::Retreat) => 0x9F44,
            other => panic!("unexpected camera fixture {other:?}"),
        };
        source.bus.write16(WRAM + SLOT + 0x6A9D, camera);
        let view = ViewBlendControl::capture(self.objects.get(self.view).unwrap());
        for (address, value) in [
            (
                SLOT + 0x6C15,
                if action.action.is_some() { 0x0D } else { 0 },
            ),
            (SLOT + 0x6A9F, 7),
            (SLOT + 0x6B77, r.auxiliary.unwrap().action_flags),
            (SLOT + 0x6BE2, r.contact.unwrap().hit.secondary_protection),
            (SLOT + 0x6BE3, r.contact.unwrap().hit.recovery),
            (
                0x1DE1,
                0x57 | if self.world.player_view_options_enabled.unwrap() {
                    0x80
                } else {
                    0
                },
            ),
            (0x1DE2, self.world.scene.player_configuration.unwrap()),
            (0x1E59, self.world.projectile_trigger.unwrap().activation),
            (0x1E0D, self.world.player_service_flags.unwrap().bits()),
            (0x1936, if pressed { 0x40 } else { 0 }),
            (0x1CDA, 201),
            (0x1CD9, 231),
            (
                VIEW + 0x21,
                0xE7 | u8::from(view.capture_position) * 8 | u8::from(view.capture_rotation) * 16,
            ),
        ] {
            source.bus.write8(WRAM + address, value);
        }
        for (i, (&color, &saved)) in self
            .world
            .palette
            .as_ref()
            .unwrap()
            .colors
            .iter()
            .zip(&self.world.palette.as_ref().unwrap().saved_colors)
            .enumerate()
        {
            source.bus.write16(WRAM + 0xEFE5 + i as u32 * 2, color);
            source.bus.write16(WRAM + 0xF2E5 + i as u32 * 2, saved);
        }
    }

    pub(super) fn visit(&mut self, source: &mut Source, pressed: bool) {
        source.run(0x0DBCCF, None, 0, OWNER, true);
        player_action::advance(
            &mut self.objects,
            &mut self.world,
            self.owner,
            InputState {
                held: Buttons::from_bits(Button::X as u16),
                pressed: Buttons::from_bits(if pressed { Button::X as u16 } else { 0 }),
            },
        )
        .unwrap();
        self.compare(source);
    }

    pub(super) fn compare(&self, source: &Source) {
        let r = self.world.player(&self.objects, self.owner).unwrap();
        let action = r.action.unwrap();
        for (address, value) in [
            (0x1B84, self.world.view_transition_mode.unwrap().flags),
            (0x1B78, self.world.scene_transition.unwrap().phase_word),
            (SLOT + 0x6C13, action_address(action.action)),
            (SLOT + 0x6C16, action.elapsed),
            (SLOT + 0x6C18, action.auxiliary_counter),
            (SLOT + 0x6C1A, action.total_updates),
        ] {
            assert_eq!(
                source.bus.read16(WRAM + address),
                value,
                "action {action:?} word {address:04X}"
            );
        }
        let view = ViewBlendControl::capture(self.objects.get(self.view).unwrap());
        for (address, value) in [
            (
                SLOT + 0x6C15,
                if action.action.is_some() { 0x0D } else { 0 },
            ),
            (SLOT + 0x6B77, r.auxiliary.unwrap().action_flags),
            (SLOT + 0x6BE2, r.contact.unwrap().hit.secondary_protection),
            (SLOT + 0x6BE3, r.contact.unwrap().hit.recovery),
            (
                0x1DE1,
                0x57 | if self.world.player_view_options_enabled.unwrap() {
                    0x80
                } else {
                    0
                },
            ),
            (0x1E59, self.world.projectile_trigger.unwrap().activation),
            (0x1E0D, self.world.player_service_flags.unwrap().bits()),
            (
                VIEW + 0x21,
                0xE7 | u8::from(view.capture_position) * 8 | u8::from(view.capture_rotation) * 16,
            ),
        ] {
            assert_eq!(
                source.bus.read8(WRAM + address),
                value,
                "action {action:?} byte {address:04X}"
            );
        }
        let camera = match r.camera_auxiliary.unwrap().task {
            AuxiliaryCameraTask::Handoff => 0x9DF6,
            AuxiliaryCameraTask::Initialize(OrbitStyle::Retreat) => 0x9F44,
            other => panic!("unexpected camera result {other:?}"),
        };
        assert_eq!(source.bus.read16(WRAM + SLOT + 0x6A9D), camera);
        assert_eq!(source.bus.read8(WRAM + SLOT + 0x6A9F), 7);
        match self.world.audio.pending_music_control() {
            Some(request) => {
                let value = match request {
                    MusicControlRequest::InterceptionReady => 1,
                    MusicControlRequest::ForcedRetreat => 5,
                    MusicControlRequest::EncounterExit => 2,
                    MusicControlRequest::EncounterProgressTransition => 7,
                    MusicControlRequest::EncounterProgressComplete => 3,
                };
                assert_eq!(source.bus.read8(WRAM + 0x1CDA), value);
                assert_eq!(source.bus.read8(WRAM + 0x1CD9), 0);
            }
            None => {
                assert_eq!(source.bus.read8(WRAM + 0x1CDA), 201);
                assert_eq!(source.bus.read8(WRAM + 0x1CD9), 231);
            }
        }
        for (i, (&color, &saved)) in self
            .world
            .palette
            .as_ref()
            .unwrap()
            .colors
            .iter()
            .zip(&self.world.palette.as_ref().unwrap().saved_colors)
            .enumerate()
        {
            assert_eq!(source.bus.read16(WRAM + 0xEFE5 + i as u32 * 2), color);
            assert_eq!(source.bus.read16(WRAM + 0xF2E5 + i as u32 * 2), saved);
        }
    }

    pub(super) fn prepare(&mut self, action: Option<PlayerAction>, time: u16, value: u16) {
        let r = self.records();
        r.action = Some(PlayerActionState {
            action,
            elapsed: time,
            auxiliary_counter: value,
            total_updates: u16::MAX,
        });
        r.auxiliary.as_mut().unwrap().action_flags = value as u8;
        r.contact.as_mut().unwrap().hit.secondary_protection = value as u8;
        r.contact.as_mut().unwrap().hit.recovery = (value >> 8) as u8;
        r.camera_auxiliary.as_mut().unwrap().task = AuxiliaryCameraTask::Handoff;
        self.world.scene_transition.as_mut().unwrap().phase_word = value;
        self.world.player_view_options_enabled = Some(true);
        self.world.audio.take_music_control();
        self.world.projectile_trigger.as_mut().unwrap().activation = 0;
        self.world.player_service_flags = Some(PlayerServiceFlags::from_bits(value as u8));
        self.world.palette.as_mut().unwrap().saved_colors.fill(123);
        ViewBlendControl::default().write_to(self.objects.get_mut(self.view).unwrap());
    }
}

#[test]
fn original_parallel_actions_match_every_elapsed_word_with_full_callback_execution() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    for action in [
        PlayerAction::ForcedRetreat,
        PlayerAction::TriggeredProjectile,
    ] {
        for time in 0..=u16::MAX {
            f.prepare(Some(action), time, time.rotate_left(7));
            let pressed = time & 1 != 0;
            f.seed(&mut source, pressed);
            f.visit(&mut source, pressed);
        }
    }
}

#[test]
fn original_retreat_preserves_every_transition_word_and_action_flag() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    for word in 0..=u16::MAX {
        f.prepare(Some(PlayerAction::ForcedRetreat), 0, word);
        f.seed(&mut source, false);
        f.visit(&mut source, false);
    }
}

#[test]
fn original_parallel_actions_observe_pause_inactivity_and_live_trigger_across_visits() {
    let mut source = Source::new(&rom(), 0);
    for action in [
        None,
        Some(PlayerAction::ForcedRetreat),
        Some(PlayerAction::TriggeredProjectile),
    ] {
        for seed in 0..=255u16 {
            let mut f = Fixture::new();
            f.prepare(action, 0, seed * 257);
            f.world.scene.player_configuration = Some(seed as u8);
            f.seed(&mut source, false);
            for visit in 0..48u16 {
                let paused = visit % 5 == seed % 5;
                f.world.view_transition_mode.as_mut().unwrap().flags =
                    0xF5FD | u16::from(paused) * 2;
                source
                    .bus
                    .write16(WRAM + 0x1B84, f.world.view_transition_mode.unwrap().flags);
                f.world.projectile_trigger.as_mut().unwrap().activation = seed as u8;
                source.bus.write8(WRAM + 0x1E59, seed as u8);
                source
                    .bus
                    .write8(WRAM + 0x1936, if visit % 3 == 0 { 0x40 } else { 0 });
                f.visit(&mut source, visit % 3 == 0);
            }
        }
    }
}

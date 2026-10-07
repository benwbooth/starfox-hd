//! Execute complete, unmodified player-frame status callers. The native
//! values are retained independently; no original output seeds native state.
use super::{rom, Source, WRAM};
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_status;
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use sf2_game::{
    Behavior, Object, ObjectId, ObjectKind, ObjectStore, RandomState, ShapeId, SoundEvent,
};

const OWNER: u16 = 0x0500;
const SLOT: u32 = 64;

struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    runtime: PathRuntime,
    owner: ObjectId,
}

impl Fixture {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut runtime = PathRuntime::default();
        player_storage::initialize(
            &mut objects,
            &mut world,
            &mut runtime,
            owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 19,
                score: Default::default(),
            },
        )
        .unwrap();
        world.active_shield_capacity = Some(32);
        world.view_transition_mode = Some(Default::default());
        world.player_carry_mode = Some(1);
        Self {
            objects,
            world,
            runtime,
            owner,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn storage(&mut self) -> &mut player_storage::PlayerStorage {
        player_storage::get_mut(&self.objects, &mut self.runtime.resources, self.owner).unwrap()
    }
    fn values(&self) -> Vec<(u32, u8)> {
        let r = self.world.player(&self.objects, self.owner).unwrap();
        let actor = self.objects.get(self.owner).unwrap();
        let c = actor.base.contacts;
        let hit = r.contact.unwrap().hit;
        vec![
            (
                SLOT + 0x6C38,
                player_storage::get(&self.objects, &self.runtime.resources, self.owner)
                    .unwrap()
                    .retained_shield,
            ),
            (SLOT + 0x6C00, hit.reserve_shield),
            (SLOT + 0x6C0A, r.status.unwrap().shield_warning_history),
            (SLOT + 0x6BE3, hit.recovery),
            (SLOT + 0x6BE2, hit.secondary_protection),
            (
                SLOT + 0x6B7D,
                0x37 | u8::from(hit.hold_secondary_protection) * 0x80,
            ),
            (SLOT + 0x6ADA, r.pose.unwrap().heading_return_bank as u8),
            (SLOT + 0x6B9B, r.mode_selection.unwrap().cue_control),
            (SLOT + 0x6AA1, r.mode_selection.unwrap().requested),
            (SLOT + 0x6BEC, r.mode_selection.unwrap().transition_control),
            (u32::from(OWNER) + 0x20, 0xD9 | u8::from(c.hit_marked) * 2),
            (
                u32::from(OWNER) + 0x21,
                0x9E | u8::from(actor.base.flags.collision_disabled)
                    | u8::from(actor.extension.path_state.motion.carry_selected_player) * 0x20,
            ),
            (
                u32::from(OWNER) + 0x22,
                0xF7 | u8::from(c.suppress_contacts_next_epoch) * 8,
            ),
            (u32::from(OWNER) + 0x2D, actor.base.hit_points),
            (0x1DD5, self.world.active_shield_capacity.unwrap()),
            (0x1E13, self.world.player_carry_mode.unwrap()),
        ]
    }
    fn seed(&self, source: &mut Source) {
        source.bus.write16(u32::from(OWNER) + 0x2B, SLOT as u16);
        source.bus.write16(
            0x12C3,
            if self.world.primary_player == Some(self.owner) {
                OWNER
            } else {
                0
            },
        );
        source.bus.write16(
            0x1B84,
            if self.world.view_transition_mode.unwrap().active() {
                2
            } else {
                0
            },
        );
        source.bus.write16(0xC4, self.world.strategy_clock);
        source.bus.write16(0x1D16, 0);
        for (field, value) in self.values() {
            source.bus.write8(WRAM + field, value);
        }
    }
    fn verify(&mut self, source: &Source, phase: &str) {
        for (field, value) in self.values() {
            assert_eq!(
                source.bus.read8(WRAM + field),
                value,
                "{phase} field={field:04X}"
            );
        }
        let cues: Vec<_> = self
            .world
            .audio
            .take_events()
            .into_iter()
            .flatten()
            .map(|event| {
                let SoundEvent::Authored(cue) = event else {
                    panic!("unexpected sound")
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
        assert_eq!(
            source.bus.read16(0x1D16) as usize,
            cues.len() * 2,
            "{phase} cue count"
        );
        for (index, cue) in cues.into_iter().enumerate() {
            assert_eq!(
                source.bus.read16(0x1CF6 + index as u32 * 2),
                cue,
                "{phase} cue {index}"
            );
        }
    }
    fn shield(&mut self, source: &mut Source) {
        source.run(0x07AF5B, None, 0, OWNER, true);
        player_status::advance_shield(
            &self.objects,
            &mut self.world,
            &mut self.runtime.resources,
            self.owner,
        )
        .unwrap();
        self.verify(source, "shield display/warnings");
    }
    fn filters(&mut self, source: &mut Source) {
        source.run(0x069195, None, 0, OWNER, true);
        player_status::advance_filters(&mut self.objects, &mut self.world, self.owner).unwrap();
        self.verify(source, "contact filters and heading bank");
    }
    fn cues(&mut self, source: &mut Source) {
        source.run(0x068FE2, None, 0, OWNER, true);
        player_status::advance_transform_cues(&self.objects, &mut self.world, self.owner).unwrap();
        self.verify(source, "transformation cues");
    }
}

#[test]
fn player_status_shield_display_matches_original_every_display_and_live_byte() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    for branch in 0..4_u8 {
        f.world.primary_player = (branch & 1 != 0).then_some(f.owner);
        f.objects.get_mut(f.owner).unwrap().base.hit_points = branch & 2;
        for word in 0..=u16::MAX {
            f.storage().retained_shield = word as u8;
            f.records().contact.as_mut().unwrap().hit.reserve_shield = (word >> 8) as u8;
            f.records().status.as_mut().unwrap().shield_warning_history = word.rotate_left(3) as u8;
            f.world.active_shield_capacity = Some(word.rotate_right(3) as u8);
            f.seed(&mut source);
            f.shield(&mut source);
        }
    }
}

#[test]
fn player_status_warning_matches_original_every_history_and_shield_with_pending_display() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    for branch in 0..4_u8 {
        f.world.primary_player = (branch & 1 != 0).then_some(f.owner);
        f.objects.get_mut(f.owner).unwrap().base.hit_points = branch & 2;
        for word in 0..=u16::MAX {
            f.storage().retained_shield = 0xA5;
            f.records().contact.as_mut().unwrap().hit.reserve_shield = word as u8;
            f.records().status.as_mut().unwrap().shield_warning_history = (word >> 8) as u8;
            f.seed(&mut source);
            f.shield(&mut source);
        }
    }
}

#[test]
fn player_status_filters_match_original_every_recovery_and_protection_byte_and_all_banks() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    for phase in 0..4_u8 {
        f.world
            .view_transition_mode
            .as_mut()
            .unwrap()
            .set_active(phase & 1 != 0);
        f.records()
            .contact
            .as_mut()
            .unwrap()
            .hit
            .hold_secondary_protection = phase & 2 != 0;
        for word in 0..=u16::MAX {
            f.records().contact.as_mut().unwrap().hit.recovery = word as u8;
            f.records()
                .contact
                .as_mut()
                .unwrap()
                .hit
                .secondary_protection = (word >> 8) as u8;
            f.records().pose.as_mut().unwrap().heading_return_bank = word.rotate_left(5) as i8;
            f.world.strategy_clock = word.rotate_right(7);
            f.objects.get_mut(f.owner).unwrap().base.contacts.hit_marked = word & 1 != 0;
            f.seed(&mut source);
            f.filters(&mut source);
        }
    }
}

#[test]
fn player_status_transform_cues_match_original_all_control_and_material_bytes_and_sides() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    for branch in 0..4_u8 {
        f.world.primary_player = (branch & 1 != 0).then_some(f.owner);
        f.objects
            .get_mut(f.owner)
            .unwrap()
            .extension
            .path_state
            .motion
            .carry_selected_player = branch & 2 != 0;
        for word in 0..=u16::MAX {
            f.records().mode_selection.as_mut().unwrap().cue_control = word as u8;
            f.records().mode_selection.as_mut().unwrap().requested = word.rotate_left(2) as u8;
            f.records()
                .mode_selection
                .as_mut()
                .unwrap()
                .transition_control = word.rotate_right(3) as u8;
            f.world.player_carry_mode = Some((word >> 8) as u8);
            f.seed(&mut source);
            f.cues(&mut source);
        }
    }
}

#[test]
fn player_status_retained_services_match_original_live_damage_and_display_acknowledgement() {
    let mut source = Source::new(&rom(), 0);
    for seed in 0..32_u16 {
        let mut f = Fixture::new();
        f.storage().retained_shield = seed as u8;
        f.records().status.as_mut().unwrap().shield_warning_history = seed as u8;
        f.records().contact.as_mut().unwrap().hit.reserve_shield = seed as u8;
        f.records().contact.as_mut().unwrap().hit.recovery = seed as u8;
        f.records()
            .contact
            .as_mut()
            .unwrap()
            .hit
            .secondary_protection = seed as u8;
        f.records().pose.as_mut().unwrap().heading_return_bank = seed.wrapping_mul(7) as i8;
        f.records().mode_selection.as_mut().unwrap().cue_control = seed.wrapping_mul(11) as u8;
        f.seed(&mut source);
        for visit in 0..256_u16 {
            let shield = (seed * 7 + visit / 9) as u8 & 63;
            f.records().contact.as_mut().unwrap().hit.reserve_shield = shield;
            source.bus.write8(WRAM + SLOT + 0x6C00, shield);
            if visit % 3 == 0 {
                f.storage().retained_shield &= 0x7F;
                source.bus.write8(
                    WRAM + SLOT + 0x6C38,
                    source.bus.read8(WRAM + SLOT + 0x6C38) & 0x7F,
                );
            }
            if visit % 41 == 0 {
                let timer = if visit & 1 == 0 { 0x7F } else { 0xFF };
                f.records().mode_selection.as_mut().unwrap().cue_control = timer;
                source.bus.write8(WRAM + SLOT + 0x6B9B, timer);
            }
            let paused = visit % 7 == 0;
            f.world
                .view_transition_mode
                .as_mut()
                .unwrap()
                .set_active(paused);
            source.bus.write16(0x1B84, if paused { 2 } else { 0 });
            f.world.strategy_clock = visit;
            source.bus.write16(0xC4, visit);
            source.bus.write16(0x1D16, 0);
            f.shield(&mut source);
            source.bus.write16(0x1D16, 0);
            f.filters(&mut source);
            source.bus.write16(0x1D16, 0);
            f.cues(&mut source);
        }
    }
}

#[test]
fn player_status_visit_prefix_matches_original_all_pilot_action_bytes_and_live_warning_gates() {
    use sf2_game::player_visit;
    use sf2_game::{InputState, Vector3};
    const OBSTACLE: u32 = 0x0600;
    const VIEW: u32 = 0x033F;
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    let mut obstacle = Object::new(
        ObjectKind::Enemy,
        ShapeId::TITLE_CRAFT,
        Behavior::Unassigned,
    );
    obstacle.base.flags.proximity_warning_source = true;
    obstacle.base.position.z = 70;
    let obstacle = f.objects.allocate(obstacle).unwrap();
    f.world.fixed_players[0] = Some(f.owner);
    source.bus.write16(0x12A8, OWNER);
    source.bus.write16(u32::from(OWNER), OBSTACLE as u16);
    source.bus.write16(OBSTACLE, 0);
    source.bus.write8(u32::from(OWNER) + 0x26, 0);
    source.bus.write8(OBSTACLE + 0x26, 0x20);
    source.bus.write16(
        OBSTACLE + 4,
        0xBC9C + ShapeId::TITLE_CRAFT.catalog_index() as u16 * 28,
    );
    source.bus.write16(OBSTACLE + 16, 70);
    for offset in [12, 14, 16, 18, 20, 22] {
        source.bus.write16(VIEW + offset, 0);
    }
    source.bus.write16(WRAM + SLOT + 0x6C13, 0); // Actual absent action stream.
    for mode in [0x11, 0x20] {
        f.records().auxiliary.as_mut().unwrap().mode = mode;
        source.bus.write8(WRAM + SLOT + 0x6AA0, mode);
        for paused in [false, true] {
            f.world
                .view_transition_mode
                .as_mut()
                .unwrap()
                .set_active(paused);
            for word in 0..=u16::MAX {
                let pilot = (word >> 8) as u8;
                let actions = word as u8;
                f.records().visit.as_mut().unwrap().pilot_code = pilot;
                f.records().visit.as_mut().unwrap().shield_warning_clock = 10;
                f.records().auxiliary.as_mut().unwrap().action_flags = actions;
                let actor = f.objects.get_mut(f.owner).unwrap();
                actor.extension.path_state.script_value = word;
                actor.base.flags.view_side_filter = true;
                actor.base.velocity = Vector3 {
                    x: word as i16,
                    y: word.rotate_left(3) as i16,
                    z: !word as i16,
                };
                actor.extension.path_state.motion_delta = Vector3 {
                    x: !word as i16,
                    y: word as i16,
                    z: word.rotate_right(3) as i16,
                };
                f.objects
                    .get_mut(obstacle)
                    .unwrap()
                    .base
                    .flags
                    .proximity_warning_latched = false;
                f.seed(&mut source);
                source.bus.write8(WRAM + SLOT + 0x6BFF, pilot);
                source.bus.write8(WRAM + SLOT + 0x6B77, actions);
                source.bus.write8(WRAM + SLOT + 0x6BE8, 10);
                source.bus.write16(WRAM + u32::from(OWNER) + 0x1CE4, word);
                source.bus.write8(u32::from(OWNER) + 0x25, 0x20);
                source.bus.write8(OBSTACLE + 0x22, 0);
                let actor = f.objects.get(f.owner).unwrap();
                for (offset, value) in [
                    (0x32, actor.base.velocity.x),
                    (0x34, actor.base.velocity.y),
                    (0x36, actor.base.velocity.z),
                    (0x1CC1, actor.extension.path_state.motion_delta.x),
                    (0x1CC3, actor.extension.path_state.motion_delta.y),
                    (0x1CC5, actor.extension.path_state.motion_delta.z),
                ] {
                    source
                        .bus
                        .write16(WRAM + u32::from(OWNER) + offset, value as u16);
                }
                source.run(0x069C27, Some(0x069D09), 0, OWNER, true);
                player_visit::begin(&mut f.objects, &mut f.world, f.owner, InputState::default())
                    .unwrap();
                f.verify(&source, "whole player visit prefix");
                assert_eq!(
                    source.bus.read8(OBSTACLE + 0x22) & 0x10 != 0,
                    f.objects
                        .get(obstacle)
                        .unwrap()
                        .base
                        .flags
                        .proximity_warning_latched
                );
                assert_eq!(
                    source.bus.read8(u32::from(OWNER) + 0x25) & 0x20 != 0,
                    f.objects.get(f.owner).unwrap().base.flags.view_side_filter
                );
                assert_eq!(
                    source.bus.read16(WRAM + u32::from(OWNER) + 0x1CE4),
                    f.objects
                        .get(f.owner)
                        .unwrap()
                        .extension
                        .path_state
                        .script_value
                );
                assert_eq!(
                    source.bus.read8(0x1DD6),
                    f.world.active_charge_threshold.unwrap()
                );
                assert_eq!(
                    source.bus.read8(0x1DD1),
                    f.world.scene.active_shield.unwrap()
                );
                let published = f.world.published_motion.unwrap();
                for (field, value) in [
                    (0xD7EC, published.position.x),
                    (0xD7EE, published.position.y),
                    (0xD7F0, published.position.z),
                    (0x1E1C, published.delta.x),
                    (0x1E1E, published.delta.y),
                    (0x1E20, published.delta.z),
                ] {
                    assert_eq!(
                        source.bus.read16(WRAM + field) as i16,
                        value,
                        "publication {field:04X}"
                    );
                }
            }
        }
    }
}

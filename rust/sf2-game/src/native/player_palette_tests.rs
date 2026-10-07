use super::*;
use crate::path_player_control::PlayerTargetControl;
use crate::player_action::ScenePalette;
use crate::player_consumable::PlayerConsumableControl;
use crate::scene_path_world::PlayerPathRecords;
use crate::{Behavior, Object, ObjectKind, RandomState, ShapeId};

fn fixture(control: u8, delay: u8) -> (ObjectStore, ScenePathWorld, ObjectId) {
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let mut world = ScenePathWorld::new(RandomState::default());
    world.primary_player = Some(owner);
    world
        .bind_player(
            &objects,
            owner,
            PlayerPathRecords {
                target_control: Some(PlayerTargetControl {
                    transition_delay: delay,
                    ..Default::default()
                }),
                consumable: Some(PlayerConsumableControl {
                    projectile_blockers: TriggeredUseBlockers::from_control(control),
                    ..Default::default()
                }),
                palette_effects: Some(PlayerPaletteControl::from_control(control)),
                ..Default::default()
            },
        )
        .unwrap();
    world.palette = Some(ScenePalette {
        colors: [0; 128],
        saved_colors: [0; 128],
    });
    world.palette_refresh_requested = Some(false);
    (objects, world, owner)
}

#[test]
fn palette_last_delay_visit_restores_immediately_and_shares_consumable_blockers() {
    let (objects, mut world, owner) = fixture(0, 2);
    world.palette.as_mut().unwrap().saved_colors[65] = 4;
    advance_primary(&objects, &mut world).unwrap();
    let record = world.player(&objects, owner).unwrap();
    assert_eq!(record.target_control.unwrap().transition_delay, 1);
    assert_eq!(record.consumable.unwrap().projectile_blockers.bits(), 0x18);
    assert!(record.consumable.unwrap().projectile_blockers.blocked());
    assert_eq!(
        world.palette.as_ref().unwrap().colors[65],
        3 | 1 << 5 | 1 << 10
    );
    advance_primary(&objects, &mut world).unwrap();
    let record = world.player(&objects, owner).unwrap();
    assert_eq!(record.target_control.unwrap().transition_delay, 0);
    assert_eq!(record.consumable.unwrap().projectile_blockers.bits(), 0x10);
    assert_eq!(world.palette.as_ref().unwrap().colors[65], 4);
}

#[test]
fn palette_flash_preserves_transparent_colors_but_clamps_overbright_components_down() {
    let (objects, mut world, _) = fixture(8, 0);
    world.palette.as_mut().unwrap().colors.fill(u16::MAX);
    advance_primary(&objects, &mut world).unwrap();
    let palette = world.palette.as_ref().unwrap();
    for (index, &color) in palette.colors.iter().enumerate() {
        assert_eq!(
            color,
            if index < 64 || index % 16 == 0 {
                u16::MAX
            } else {
                31 | 28 << 5 | 28 << 10
            }
        );
    }
    assert_eq!(world.palette_refresh_requested, Some(true));
}

#[test]
fn palette_progress_pulse_changes_earlier_banks_and_preserves_its_request_latch() {
    let (objects, mut world, owner) = fixture(0xE0, 0);
    let color = 3 | 4 << 5 | 5 << 10 | 0x8000;
    world.palette.as_mut().unwrap().colors.fill(color);
    world.palette.as_mut().unwrap().saved_colors.fill(0);
    advance_primary(&objects, &mut world).unwrap();
    assert_eq!(world.palette.as_ref().unwrap().colors[15], color);
    assert_eq!(
        world.palette.as_ref().unwrap().colors[16],
        3 | 3 << 5 | 3 << 10
    );
    assert_eq!(
        world.palette.as_ref().unwrap().colors[64],
        3 | 3 << 5 | 3 << 10
    );
    world.strategy_clock = 8;
    advance_primary(&objects, &mut world).unwrap();
    assert_eq!(
        world.palette.as_ref().unwrap().colors[16],
        3 | 3 << 5 | 3 << 10
    );
    assert_eq!(
        world.palette.as_ref().unwrap().colors[64],
        2 | 2 << 5 | 2 << 10
    );
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .palette_effects
            .unwrap()
            .bits(),
        0xE0
    );
    for flags in 0..=255 {
        let mut control = PlayerPaletteControl::from_control(flags);
        control.request_progress_pulse();
        assert_eq!(
            control.bits(),
            if flags & 0x80 == 0 {
                0xE0
            } else {
                flags & 0xE0
            }
        );
    }
}

#[test]
fn palette_restore_finishes_one_visit_after_arrival_and_compares_unused_high_bit() {
    let (objects, mut world, owner) = fixture(0xD0, 0);
    world.palette.as_mut().unwrap().colors[64] = 1;
    advance_primary(&objects, &mut world).unwrap();
    assert_eq!(world.palette.as_ref().unwrap().colors[64], 0);
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .consumable
            .unwrap()
            .projectile_blockers
            .bits(),
        0x10
    );
    advance_primary(&objects, &mut world).unwrap();
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .consumable
            .unwrap()
            .projectile_blockers
            .bits(),
        0
    );
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .palette_effects
            .unwrap()
            .bits(),
        0x80
    );
    let record = world.player_mut(&objects, owner).unwrap();
    record.consumable.as_mut().unwrap().projectile_blockers =
        TriggeredUseBlockers::from_control(0x10);
    world.palette.as_mut().unwrap().saved_colors[64] = 0x8000;
    for _ in 0..4 {
        advance_primary(&objects, &mut world).unwrap();
    }
    assert_eq!(world.palette.as_ref().unwrap().colors[64], 0);
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .consumable
            .unwrap()
            .projectile_blockers
            .bits(),
        0x10
    );
}

#[test]
fn palette_idle_visit_needs_no_palette_and_missing_inputs_retain_completed_prefix() {
    let (objects, mut world, owner) = fixture(0x80, 0);
    world.palette = None;
    advance_primary(&objects, &mut world).unwrap();
    assert_eq!(world.palette_refresh_requested, Some(false));
    world
        .player_mut(&objects, owner)
        .unwrap()
        .target_control
        .as_mut()
        .unwrap()
        .transition_delay = 3;
    assert_eq!(
        advance_primary(&objects, &mut world),
        Err(PaletteError::MissingPalette)
    );
    let record = world.player(&objects, owner).unwrap();
    assert_eq!(record.target_control.unwrap().transition_delay, 2);
    assert_eq!(record.consumable.unwrap().projectile_blockers.bits(), 0x18);
    assert_eq!(world.palette_refresh_requested, Some(true));
    world.player_mut(&objects, owner).unwrap().consumable = None;
    assert_eq!(
        advance_primary(&objects, &mut world),
        Err(PaletteError::MissingConsumableControl(owner))
    );
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .target_control
            .unwrap()
            .transition_delay,
        1
    );
}

#[test]
fn palette_completion_clears_blocker_before_diagnosing_missing_progress_state() {
    let (objects, mut world, owner) = fixture(0x10, 0);
    world.player_mut(&objects, owner).unwrap().palette_effects = None;
    assert_eq!(
        advance_primary(&objects, &mut world),
        Err(PaletteError::MissingPaletteControl(owner))
    );
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .consumable
            .unwrap()
            .projectile_blockers
            .bits(),
        0
    );
    assert_eq!(world.palette_refresh_requested, Some(true));
}

#[test]
fn palette_path_lock_and_consumable_share_one_delay_and_admission_lifetime() {
    use crate::path_equipment::SelectedEquipment;
    use crate::player_consumable;
    let (mut objects, mut world, owner) = fixture(0, 0);
    world.view_transition_mode = Some(Default::default());
    world.spawn_defaults = Some(crate::ObjectSpawnDefaults {
        group: 0,
        run_when_paused: false,
    });
    world.projectile_trigger = Some(Default::default());
    let record = world.player_mut(&objects, owner).unwrap();
    record.action = Some(Default::default());
    record.equipment = Some(SelectedEquipment {
        packed_consumables: 1,
        consumable_type: 1,
        weapon_level: 1,
    });
    record
        .target_control
        .as_mut()
        .unwrap()
        .lock_to_projectile(owner, Default::default());
    let mut visits = 0;
    loop {
        visits += 1;
        advance_primary(&objects, &mut world).unwrap();
        if !world
            .player(&objects, owner)
            .unwrap()
            .consumable
            .unwrap()
            .projectile_blockers
            .blocked()
        {
            break;
        }
        assert!(!player_consumable::use_item(&mut objects, &mut world, owner).unwrap());
        assert_eq!(objects.len(), 1);
        assert_eq!(
            world
                .player(&objects, owner)
                .unwrap()
                .equipment
                .unwrap()
                .packed_consumables,
            1
        );
        assert!(visits < 64);
    }
    assert_eq!(visits, 37);
    assert!(
        world
            .player(&objects, owner)
            .unwrap()
            .target_control
            .unwrap()
            .configuration_locked
    );
    assert!(player_consumable::use_item(&mut objects, &mut world, owner).unwrap());
    assert_eq!(objects.len(), 2);
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .equipment
            .unwrap()
            .packed_consumables,
        0
    );
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .action
            .unwrap()
            .action,
        Some(crate::player_action::PlayerAction::TriggeredProjectile)
    );
}

#[test]
fn palette_control_survives_motion_reset_but_full_replacement_zeroes_it() {
    use crate::path_runtime::PathRuntime;
    use crate::player_storage::{self, PlayerStorageInputs};
    let (mut objects, mut world, owner) = fixture(0xE0, 19);
    let mut runtime = PathRuntime::default();
    let inputs = PlayerStorageInputs {
        pilot_code: 2,
        reserve_shield: 40,
        score: Default::default(),
    };
    player_storage::replace(&mut objects, &mut world, &mut runtime, owner, inputs).unwrap();
    assert_eq!(
        world.player(&objects, owner).unwrap().palette_effects,
        Some(PlayerPaletteControl::default())
    );
    let record = world.player_mut(&objects, owner).unwrap();
    record.palette_effects = Some(PlayerPaletteControl::from_control(0xA0));
    record.target_control.as_mut().unwrap().transition_delay = 19;
    record.consumable.as_mut().unwrap().projectile_blockers =
        TriggeredUseBlockers::from_control(0x18);
    crate::player_motion_reset::reset(&objects, &mut world, &mut runtime.resources, owner).unwrap();
    let record = world.player(&objects, owner).unwrap();
    assert_eq!(record.palette_effects.unwrap().bits(), 0xA0);
    assert_eq!(record.target_control.unwrap().transition_delay, 19);
    assert_eq!(record.consumable.unwrap().projectile_blockers.bits(), 0x18);
    player_storage::replace(&mut objects, &mut world, &mut runtime, owner, inputs).unwrap();
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .palette_effects
            .unwrap()
            .bits(),
        0
    );
}

#[test]
fn palette_scene_fault_cannot_repeat_delay_after_missing_palette_is_supplied() {
    use crate::path_program::PathCatalog;
    use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
    use crate::strategy_schedule::StrategyCompletion;
    struct Callbacks;
    impl SceneCallbacks for Callbacks {
        type Error = ();
        fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
            panic!("not dispatched")
        }
        fn death_override(
            _: &mut SceneActors<'_, Self>,
            _: ObjectId,
        ) -> Result<Option<StrategyCompletion>, ()> {
            panic!("not dispatched")
        }
        fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
            panic!("not dispatched")
        }
    }
    let (mut objects, mut world, owner) = fixture(0, 3);
    world.palette = None;
    let mut execution = SceneExecution::default();
    let catalog = PathCatalog::new(vec![]).unwrap();
    let mut callbacks = Callbacks;
    let mut scene = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 10,
    };
    assert_eq!(
        scene.advance_player_palette(),
        Err(SceneError::PlayerPalette(PaletteError::MissingPalette))
    );
    scene.world.palette = Some(ScenePalette {
        colors: [0; 128],
        saved_colors: [0; 128],
    });
    assert_eq!(scene.advance_player_palette(), Err(SceneError::Faulted));
    assert_eq!(
        scene
            .world
            .player(scene.objects, owner)
            .unwrap()
            .target_control
            .unwrap()
            .transition_delay,
        2
    );
    assert_eq!(scene.world.palette.as_ref().unwrap().colors, [0; 128]);
}

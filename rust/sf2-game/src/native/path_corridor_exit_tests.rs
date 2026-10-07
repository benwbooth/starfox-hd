//! Real scene borrows, source-ordered writes and lazy corridor dependencies.
use super::tests::setup;
use super::*;
use crate::path_control::PlayerTarget;
use crate::path_invocation::InvocationWorld;
use crate::path_scene_state::EncounterHandoff;
use crate::player_boundary::{BoundaryError, PlayerBoundary, RegionInputs};
use crate::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use crate::{Angle, Behavior, ObjectKind, ShapeId, Vector3};

fn cursor(path: u16, command_index: u16) -> PathCursor {
    PathCursor {
        path: crate::PathId::from_catalog_index(path),
        command_index,
    }
}

struct Fixture {
    runtime: PathRuntime,
    objects: ObjectStore,
    owner: ObjectId,
    selected: ObjectId,
    world: ScenePathWorld,
}
impl Fixture {
    fn new() -> Self {
        let (runtime, mut objects, owner, random) = setup();
        let selected = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        let mut world = ScenePathWorld::new(random);
        world.primary_player = Some(owner);
        world.secondary_player = Some(selected);
        world.handoff = Some(EncounterHandoff::default());
        for player in [owner, selected] {
            world
                .bind_player(
                    &objects,
                    player,
                    PlayerPathRecords {
                        auxiliary: Some(SelectedAuxiliaryState {
                            mode: 0xB2,
                            action_flags: 0xF7,
                            stored_world_position: Default::default(),
                            stored_rotation: Default::default(),
                        }),
                        boundary: Some(PlayerBoundary {
                            reset_control: 211,
                            ..Default::default()
                        }),
                        steering: Some(Default::default()),
                        contact: Some(Default::default()),
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        Self {
            runtime,
            objects,
            owner,
            selected,
            world,
        }
    }
    fn run(&mut self, statement: Statement) -> Result<(), ProgramError> {
        let catalog = PathCatalog::new(vec![vec![
            statement,
            Statement::Control(ControlCommand::Hold),
        ]])
        .unwrap();
        self.objects.get_mut(self.owner).unwrap().base.path = Some(cursor(0, 0));
        let mut world = self
            .world
            .path_world(&self.objects, self.owner, PlayerTarget::Secondary)
            .unwrap();
        self.runtime
            .enter_program(&catalog, &mut self.objects, self.owner, &mut world, 4)
            .map(|exit| assert_eq!(exit.step, ControlStep::Movement))
    }
}

#[test]
fn corridor_reset_uses_live_selected_records_and_preserves_every_other_field() {
    let mut f = Fixture::new();
    for flags in 0..=u8::MAX {
        for control in 0..=u8::MAX {
            let selected = f.world.player_mut(&f.objects, f.selected).unwrap();
            selected.auxiliary.as_mut().unwrap().action_flags = flags;
            selected.boundary.as_mut().unwrap().reset_control = control;
            let mut expected = *selected;
            let primary = *f.world.player(&f.objects, f.owner).unwrap();
            f.run(Statement::ResetSelectedRegion { next: cursor(0, 1) })
                .unwrap();
            expected.auxiliary.as_mut().unwrap().action_flags &= !0x04;
            expected.boundary.as_mut().unwrap().reset_control = 0;
            assert_eq!(*f.world.player(&f.objects, f.selected).unwrap(), expected);
            assert_eq!(*f.world.player(&f.objects, f.owner).unwrap(), primary);
        }
    }
}

#[test]
fn entry_protects_primary_but_does_not_reassert_a_consumed_start_bit() {
    let mut f = Fixture::new();
    for flags in 0..=u8::MAX {
        f.world.handoff.as_mut().unwrap().player_flags = flags;
        f.world
            .player_mut(&f.objects, f.owner)
            .unwrap()
            .contact
            .as_mut()
            .unwrap()
            .hit
            .secondary_protection = flags;
        let secondary = *f.world.player(&f.objects, f.selected).unwrap();
        f.run(Statement::BeginCorridorEntry { next: cursor(0, 1) })
            .unwrap();
        assert_eq!(
            f.world.handoff.unwrap().player_flags,
            if flags & 4 == 0 { flags | 5 } else { flags }
        );
        assert_eq!(
            f.world
                .player(&f.objects, f.owner)
                .unwrap()
                .contact
                .unwrap()
                .hit
                .secondary_protection,
            63
        );
        assert_eq!(*f.world.player(&f.objects, f.selected).unwrap(), secondary);
    }
    f.world.handoff = None;
    f.world
        .player_mut(&f.objects, f.owner)
        .unwrap()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .secondary_protection = 1;
    assert_eq!(
        f.run(Statement::BeginCorridorEntry { next: cursor(0, 1) }),
        Err(ProgramError::MissingEncounterHandoff)
    );
    assert_eq!(
        f.world
            .player(&f.objects, f.owner)
            .unwrap()
            .contact
            .unwrap()
            .hit
            .secondary_protection,
        63
    );
    assert_eq!(
        f.objects.get(f.owner).unwrap().base.path,
        Some(cursor(0, 0))
    );
}

#[test]
fn region_heading_is_retained_even_on_rejection_and_active_regions_skip_the_proxy() {
    let mut f = Fixture::new();
    f.objects.get_mut(f.owner).unwrap().base.yaw = Angle::from_units(95);
    f.objects.get_mut(f.selected).unwrap().base.position = Vector3 { x: 0, y: 301, z: 0 };
    f.world
        .player_mut(&f.objects, f.selected)
        .unwrap()
        .auxiliary
        .as_mut()
        .unwrap()
        .action_flags = 0;
    f.runtime.region = RegionInputs {
        heading_offset: Angle::from_units(128),
        half_width: 100,
        half_height: 300,
        activation_radius: 1000,
    };
    let before = *f.world.player(&f.objects, f.selected).unwrap();
    f.run(Statement::InstallSelectedRegion { next: cursor(0, 1) })
        .unwrap();
    assert_eq!(f.runtime.region.heading_offset.units(), 192);
    assert_eq!(*f.world.player(&f.objects, f.selected).unwrap(), before);
    f.world
        .player_mut(&f.objects, f.selected)
        .unwrap()
        .auxiliary
        .as_mut()
        .unwrap()
        .action_flags = 4;
    f.run(Statement::InstallSelectedRegion { next: cursor(0, 1) })
        .unwrap();
    let selected = f.world.player(&f.objects, f.selected).unwrap();
    assert_eq!(selected.steering.unwrap().locked_heading.units(), 0);
    assert_eq!(
        (
            selected.boundary.unwrap().half_width,
            selected.boundary.unwrap().half_height
        ),
        (100, 300)
    );
    assert_eq!(selected.boundary.unwrap().reset_control, 211);
    f.world
        .player_mut(&f.objects, f.selected)
        .unwrap()
        .auxiliary
        .as_mut()
        .unwrap()
        .action_flags = 0;
    f.objects.get_mut(f.selected).unwrap().base.position.y = 0;
    assert_eq!(
        f.run(Statement::InstallSelectedRegion { next: cursor(0, 1) }),
        Err(ProgramError::Boundary(BoundaryError::MissingProxy))
    );
    assert_eq!(f.runtime.region.heading_offset.units(), 64);
}

#[test]
fn rejected_probes_do_not_require_output_records_and_reset_keeps_its_partial_write() {
    let mut f = Fixture::new();
    let selected = f.world.player_mut(&f.objects, f.selected).unwrap();
    selected.boundary = None;
    selected.steering = None;
    f.runtime.region.activation_radius = 0;
    f.run(Statement::InstallSelectedRegion { next: cursor(0, 1) })
        .unwrap();
    assert_eq!(
        f.run(Statement::ResetSelectedRegion { next: cursor(0, 1) }),
        Err(ProgramError::Boundary(BoundaryError::MissingBoundary(
            f.selected
        )))
    );
    assert_eq!(
        f.world
            .player(&f.objects, f.selected)
            .unwrap()
            .auxiliary
            .unwrap()
            .action_flags,
        0xF3
    );
}

#[test]
fn corridor_camera_clear_publishes_into_the_actual_camera_record() {
    let mut f = Fixture::new();
    for roll in [None, Some(65535)] {
        f.world.published_camera_roll = roll;
        f.run(Statement::ClearPublishedCameraRoll { next: cursor(0, 1) })
            .unwrap();
        assert_eq!(f.world.published_camera_roll, Some(0));
    }
}

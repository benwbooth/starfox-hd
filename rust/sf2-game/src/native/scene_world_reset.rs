//! Complete transition-world clear (`$0D:C956..C96B`) and the identical
//! player-entry prefix with region-selection reset (`$06:83F1..840F`).
//! Actor retirement remains deferred; region definitions are not erased.

use super::scene_path_world::ScenePathWorld;
use super::scene_proxy::SceneProxyError;
use super::world_occupancy::WorldOccupancy;
use super::ObjectStore;

const NO_SELECTED_REGION: u8 = u8::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionSelection {
    /// Action-stream cleanup disables scanning but retains both selections.
    Retain,
    /// Player entry also replaces both selection publications.
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldResetError {
    Proxies(SceneProxyError),
    MissingSpawnDefaults,
}

pub fn clear(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    selection: RegionSelection,
) -> Result<(), WorldResetError> {
    super::scene_clear::clear(objects, &mut world.proxies).map_err(WorldResetError::Proxies)?;
    // Zero registration count disables scanning without deleting authored
    // region definitions. Selection publications have separate ownership.
    world.region_registration_count = Some(0);
    if selection == RegionSelection::Clear {
        let defaults = world
            .spawn_defaults
            .as_mut()
            .ok_or(WorldResetError::MissingSpawnDefaults)?;
        defaults.group = NO_SELECTED_REGION;
        world.secondary_region_group = Some(NO_SELECTED_REGION);
    }
    // This operation publishes every occupancy cell, so no prior plane is
    // required. Marker erasure subsequently opens traversable world cells.
    world.occupancy = Some(WorldOccupancy::fully_occupied());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program_resources::ProgramResources;
    use crate::program_state::ProgramData;
    use crate::world_occupancy::{
        MarkerCoverage, OccupancyChange, WorldRectangle, CELLS_PER_AXIS, CELL_SIZE,
    };
    use crate::{
        Behavior, Object, ObjectKind, ObjectSpawnDefaults, PathCursor, PathId, RandomState,
        ShapeId, Vector3,
    };

    fn world() -> ScenePathWorld {
        let mut world = ScenePathWorld::new(RandomState::new([13, 27, 83, 177]));
        world.region_registration_count = Some(173);
        world.secondary_region_group = Some(47);
        world.spawn_defaults = Some(ObjectSpawnDefaults {
            group: 81,
            run_when_paused: true,
        });
        world.occupancy = Some(WorldOccupancy::default());
        world
    }

    #[test]
    fn transition_clear_retains_every_region_selection_and_replaces_all_occupancy() {
        let mut objects = ObjectStore::new();
        let mut world = world();
        for primary in 0..=u8::MAX {
            for secondary in 0..=u8::MAX {
                world.spawn_defaults.as_mut().unwrap().group = primary;
                world.secondary_region_group = Some(secondary);
                world.region_registration_count = Some(primary ^ secondary);
                clear(&mut objects, &mut world, RegionSelection::Retain).unwrap();
                assert_eq!(world.region_registration_count, Some(0));
                assert_eq!(world.spawn_defaults.unwrap().group, primary);
                assert_eq!(world.secondary_region_group, Some(secondary));
                assert!(world.spawn_defaults.unwrap().run_when_paused);
            }
        }
        let occupancy = world.occupancy.as_ref().unwrap();
        for z in 0..CELLS_PER_AXIS {
            for x in 0..CELLS_PER_AXIS {
                assert!(occupancy.contains(Vector3 {
                    x: (x as u16 * CELL_SIZE) as i16,
                    y: -317,
                    z: (z as u16 * CELL_SIZE) as i16
                }));
            }
        }
        // Repeating cleanup must fill a cell erased by an intervening map.
        let marker = MarkerCoverage::from_rectangle(WorldRectangle {
            x: 0,
            z: 0,
            width: 1,
            depth: 1,
        })
        .unwrap();
        world
            .occupancy
            .as_mut()
            .unwrap()
            .apply(&marker, OccupancyChange::Erase);
        assert!(!world
            .occupancy
            .as_ref()
            .unwrap()
            .contains(Vector3::default()));
        clear(&mut objects, &mut world, RegionSelection::Retain).unwrap();
        assert!(world
            .occupancy
            .as_ref()
            .unwrap()
            .contains(Vector3::default()));
    }

    #[test]
    fn player_entry_clears_both_groups_without_resetting_unrelated_mode_or_actor_lifetimes() {
        let mut objects = ObjectStore::new();
        let mut world = world();
        let mut actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::Unassigned);
        actor.base.flags.general_search_eligible = true;
        actor.extension.spawn_group = 81;
        let owner = objects.allocate(actor).unwrap();
        let random = world.random.clone();
        clear(&mut objects, &mut world, RegionSelection::Clear).unwrap();
        assert_eq!(world.region_registration_count, Some(0));
        assert_eq!(
            world.spawn_defaults.unwrap(),
            ObjectSpawnDefaults {
                group: NO_SELECTED_REGION,
                run_when_paused: true
            }
        );
        assert_eq!(world.secondary_region_group, Some(NO_SELECTED_REGION));
        assert_eq!(world.random, random);
        assert_eq!(objects.active_ids(), &[owner]);
        let actor = objects.get(owner).unwrap();
        assert!(actor.base.flags.remove_after_tick);
        assert_eq!(actor.extension.spawn_group, 81);
    }

    #[test]
    fn fully_written_inputs_need_no_previous_values_but_retained_mode_cannot_be_invented() {
        let mut objects = ObjectStore::new();
        let mut world = ScenePathWorld::new(RandomState::new([1, 2, 3, 4]));
        clear(&mut objects, &mut world, RegionSelection::Retain).unwrap();
        assert_eq!(world.region_registration_count, Some(0));
        assert_eq!(world.spawn_defaults, None);
        assert_eq!(world.secondary_region_group, None);
        world.occupancy = None;
        world.region_registration_count = Some(89);
        assert_eq!(
            clear(&mut objects, &mut world, RegionSelection::Clear),
            Err(WorldResetError::MissingSpawnDefaults)
        );
        assert_eq!(world.region_registration_count, Some(0));
        assert_eq!(world.secondary_region_group, None);
        assert_eq!(world.occupancy, None);
        world.spawn_defaults = Some(ObjectSpawnDefaults {
            group: 91,
            run_when_paused: false,
        });
        clear(&mut objects, &mut world, RegionSelection::Clear).unwrap();
        assert_eq!(world.secondary_region_group, Some(NO_SELECTED_REGION));
        assert_eq!(world.occupancy, Some(WorldOccupancy::fully_occupied()));
    }

    #[test]
    fn proxy_failure_precedes_region_and_occupancy_publication() {
        let mut objects = ObjectStore::new();
        let mut world = world();
        let mut actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::Unassigned);
        actor.base.flags.general_search_eligible = true;
        let owner = objects.allocate(actor).unwrap();
        let proxy = world
            .proxies
            .capture_actor(
                &mut objects,
                owner,
                PathCursor {
                    path: PathId::from_catalog_index(0),
                    command_index: 0,
                },
                &ProgramResources::<ProgramData>::default(),
            )
            .unwrap()
            .unwrap();
        world.proxies.release(proxy).unwrap();
        assert_eq!(
            clear(&mut objects, &mut world, RegionSelection::Clear),
            Err(WorldResetError::Proxies(SceneProxyError::MissingProxy(
                proxy
            )))
        );
        assert_eq!(world.region_registration_count, Some(173));
        assert_eq!(world.secondary_region_group, Some(47));
        assert_eq!(world.spawn_defaults.unwrap().group, 81);
        assert_eq!(world.occupancy, Some(WorldOccupancy::default()));
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.extension.scene_proxy, None);
        assert!(!actor.base.flags.remove_after_tick);
    }
}

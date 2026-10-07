use super::*;
use crate::collision_surface::SurfaceMode;
use crate::intro_material::{DepthGroup, FlatMaterial};
use crate::player_action::ScenePalette;
use crate::player_mode_selection::PlayerModeSelection;
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::{Behavior, Object, ObjectKind, RandomState, ShapeId};

fn fixture() -> (ObjectStore, ScenePathWorld, ObjectId) {
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let mut world = ScenePathWorld::new(RandomState::new([5, 91, 137, 242]));
    world
        .bind_player(
            &objects,
            owner,
            PlayerPathRecords {
                mode_selection: Some(PlayerModeSelection::default()),
                ..Default::default()
            },
        )
        .unwrap();
    world.surface_mode = Some(SurfaceMode { flags: 2 });
    world.environment_plane_height = Some(-571);
    world.render_environment = SceneRenderEnvironment {
        lighting: SceneLighting {
            depth_colors: DepthColorFamily::from_catalog_index(4),
            thresholds: Some(DepthThresholdTable::OPENING),
        },
        plane_height: Some(222),
        ambient_height_gate: Some(31337),
        ambient_control: Some(AmbientParticleControl::from_bits(0xA59D)),
        ambient_palette: Some(SurfaceParticlePalette::Negative),
        ambient_size: Some(731),
    };
    world.palette = Some(ScenePalette {
        colors: std::array::from_fn(|i| (i as u16).wrapping_mul(313)),
        saved_colors: std::array::from_fn(|i| !(i as u16).wrapping_mul(191)),
    });
    (objects, world, owner)
}

#[test]
fn both_palette_entries_update_live_and_saved_colors_but_only_one_requests_refresh() {
    for initial in [None, Some(false), Some(true)] {
        for side in [SurfaceViewSide::Negative, SurfaceViewSide::Nonnegative] {
            let (_, mut world, _) = fixture();
            world.palette_refresh_requested = initial;
            let before = world.palette.clone().unwrap();
            replace_polygon_palette(&mut world, side).unwrap();
            let palette = world.palette.as_ref().unwrap();
            assert_eq!(palette.colors[..112], before.colors[..112]);
            assert_eq!(palette.saved_colors[..112], before.saved_colors[..112]);
            assert_eq!(palette.colors[112..], palette.saved_colors[112..]);
            assert_eq!(
                &palette.colors[112..],
                match side {
                    SurfaceViewSide::Negative => PolygonPaletteId::EladardSurface.colors(),
                    SurfaceViewSide::Nonnegative => PolygonPaletteId::CatalogOne.colors(),
                }
            );
            assert_eq!(
                world.palette_refresh_requested,
                if side == SurfaceViewSide::Negative {
                    Some(true)
                } else {
                    initial
                }
            );
        }
    }
}

#[test]
fn independent_surface_bits_preserve_each_branches_actual_retained_state() {
    for flags in 0..=255 {
        let (objects, mut world, owner) = fixture();
        world
            .player_mut(&objects, owner)
            .unwrap()
            .mode_selection
            .as_mut()
            .unwrap()
            .surface_control = flags;
        let palette = world.palette.clone();
        let random = world.random.bytes();
        publish_environment(&objects, &mut world, owner).unwrap();
        let output = world.render_environment;
        let carried = flags & SURFACE_CARRY != 0;
        assert_eq!(
            output.lighting.depth_colors,
            Some(if carried {
                DepthColorFamily::STANDARD
            } else {
                DepthColorFamily::SURFACE_UNCARRIED
            })
        );
        assert_eq!(
            output.lighting.thresholds,
            Some(if carried {
                DepthThresholdTable::SURFACE_CARRIED
            } else {
                DepthThresholdTable::SURFACE_UNCARRIED
            })
        );
        assert_eq!(output.ambient_size, Some(16));
        if flags & NONNEGATIVE_VIEW_HEIGHT == 0 {
            assert_eq!(output.plane_height, Some(-571));
            assert_eq!(output.ambient_height_gate, Some((-1500_i16) as u16));
            assert_eq!(output.ambient_control.unwrap().bits(), 0);
            assert_eq!(
                output.ambient_palette,
                Some(SurfaceParticlePalette::Negative)
            );
        } else {
            assert_eq!(output.plane_height, Some(0));
            assert_eq!(output.ambient_height_gate, Some(31337));
            assert_eq!(output.ambient_control.unwrap().bits(), 0xA5BF);
            assert_eq!(
                output.ambient_palette,
                Some(SurfaceParticlePalette::Nonnegative)
            );
            assert!(output.ambient_control.unwrap().renders_shapes());
            assert!(output.ambient_control.unwrap().rises());
        }
        assert_eq!(world.palette, palette);
        assert_eq!(world.random.bytes(), random);
        assert_eq!(world.palette_refresh_requested, None);
    }
}

#[test]
fn other_modes_do_not_require_or_modify_player_or_render_inputs() {
    for mode in 0..=255 {
        if mode & 7 == 2 {
            continue;
        }
        let (mut objects, mut world, owner) = fixture();
        world.surface_mode = Some(SurfaceMode { flags: mode });
        world.render_environment = Default::default();
        world.environment_plane_height = None;
        objects.remove(owner).unwrap();
        publish_environment(&objects, &mut world, owner).unwrap();
        assert_eq!(world.render_environment, SceneRenderEnvironment::default());
    }
}

#[test]
fn missing_inputs_preserve_the_source_order_of_completed_publications() {
    let (objects, mut world, owner) = fixture();
    world.environment_plane_height = None;
    let before = world.render_environment;
    assert_eq!(
        publish_environment(&objects, &mut world, owner),
        Err(SurfaceRenderError::MissingEnvironmentPlane)
    );
    assert_ne!(world.render_environment.lighting, before.lighting);
    assert_eq!(
        world.render_environment.ambient_height_gate,
        Some((-1500_i16) as u16)
    );
    assert_eq!(world.render_environment.plane_height, before.plane_height);
    assert_eq!(
        world.render_environment.ambient_control,
        before.ambient_control
    );
    assert_eq!(world.render_environment.ambient_size, before.ambient_size);
    world
        .player_mut(&objects, owner)
        .unwrap()
        .mode_selection
        .as_mut()
        .unwrap()
        .surface_control = NONNEGATIVE_VIEW_HEIGHT;
    world.render_environment.ambient_control = None;
    assert_eq!(
        publish_environment(&objects, &mut world, owner),
        Err(SurfaceRenderError::MissingAmbientControl)
    );
    assert_eq!(world.render_environment.plane_height, Some(0));
    assert_eq!(world.render_environment.ambient_size, before.ambient_size);
    world.palette = None;
    assert_eq!(
        replace_polygon_palette(&mut world, SurfaceViewSide::Negative),
        Err(SurfaceRenderError::MissingPalette)
    );
    assert_eq!(world.palette_refresh_requested, Some(true));
}

#[test]
fn replacement_branch_does_not_require_prior_ambient_control_and_retaining_branch_needs_no_plane() {
    let (objects, mut world, owner) = fixture();
    world.render_environment = SceneRenderEnvironment::default();
    publish_environment(&objects, &mut world, owner).unwrap();
    assert_eq!(
        world.render_environment.ambient_control,
        Some(AmbientParticleControl::default())
    );
    world.render_environment.ambient_height_gate = None;
    world.environment_plane_height = None;
    world
        .player_mut(&objects, owner)
        .unwrap()
        .mode_selection
        .as_mut()
        .unwrap()
        .surface_control = NONNEGATIVE_VIEW_HEIGHT;
    publish_environment(&objects, &mut world, owner).unwrap();
    assert_eq!(world.render_environment.ambient_height_gate, None);
    assert_eq!(world.render_environment.plane_height, Some(0));
    assert_eq!(
        world.render_environment.ambient_control.unwrap().bits(),
        0x22
    );
}

#[test]
fn selected_lighting_and_polygon_colors_feed_the_existing_native_material_resolver() {
    let (objects, mut world, owner) = fixture();
    let material = FlatMaterial::from_word(0x3E0B).unwrap();
    let mut outputs = Vec::new();
    for flags in [0, 2, 4, 6] {
        world
            .player_mut(&objects, owner)
            .unwrap()
            .mode_selection
            .as_mut()
            .unwrap()
            .surface_control = flags;
        replace_polygon_palette(
            &mut world,
            if flags & 4 == 0 {
                SurfaceViewSide::Negative
            } else {
                SurfaceViewSide::Nonnegative
            },
        )
        .unwrap();
        publish_environment(&objects, &mut world, owner).unwrap();
        let lighting = world.render_environment.lighting;
        let group = lighting.depth_group(3072, 0).unwrap();
        let pair = lighting
            .palette_pair(material, group, true, [0; 3], [0; 3])
            .unwrap();
        let palette = world.palette.as_ref().unwrap();
        outputs.push((
            group,
            pair,
            palette.colors[112 + usize::from(pair & 15)],
            palette.colors[112 + usize::from(pair >> 4)],
        ));
    }
    assert_ne!(outputs[0], outputs[1]);
    // Both authored threshold rows are [-6, -8, -10]; at depth 12*256
    // all three wrapped comparisons are nonnegative.
    assert_eq!(outputs[0].0, DepthGroup::Farthest);
    assert_ne!(outputs[0], outputs[2]);
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("surface rendering must not dispatch a strategy")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected map continuation")
    }
}

#[test]
fn scene_host_latches_render_and_palette_failures_without_replaying_partial_updates() {
    for palette_failure in [false, true] {
        let (mut objects, mut world, owner) = fixture();
        world.environment_plane_height = None;
        world.palette = None;
        let mut execution = SceneExecution::default();
        let catalog = crate::authored_paths::catalog();
        let mut callbacks = Callbacks;
        let mut host = SceneActors {
            objects: &mut objects,
            world: &mut world,
            execution: &mut execution,
            catalog: &catalog,
            callbacks: &mut callbacks,
            statement_budget: 1,
        };
        let result = if palette_failure {
            host.replace_player_surface_palette(SurfaceViewSide::Negative)
        } else {
            host.publish_player_surface_environment(owner)
        };
        assert_eq!(
            result,
            Err(SceneError::PlayerSurfaceRender(if palette_failure {
                SurfaceRenderError::MissingPalette
            } else {
                SurfaceRenderError::MissingEnvironmentPlane
            }))
        );
        assert!(host.execution.is_faulted());
        let before = host.world.render_environment;
        assert_eq!(
            host.publish_player_surface_environment(owner),
            Err(SceneError::Faulted)
        );
        assert_eq!(
            host.replace_player_surface_palette(SurfaceViewSide::Nonnegative),
            Err(SceneError::Faulted)
        );
        assert_eq!(host.world.render_environment, before);
    }
}

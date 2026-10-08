//! The frame's render-view setup for the selected fixed view (`$7F:1561`,
//! `$7F:1573..1736`, started by `$7F:7A1E` at frame start). It publishes the
//! view matrix the display services project markers through (`$157C`, copied
//! into GSU RAM by `$03:8E0A`) and the flight window's viewport (`$03:8AAA`).
//! The render origin it also computes goes only to the GSU's mesh pass.

use super::intro_camera::IntroCameraView;
use super::intro_draw::ViewTransform;
use super::intro_motion::AttractCameraAngles;
use super::intro_projection::ProjectionViewport;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::view_transition::FixedViewAngles;
use super::ObjectStore;

/// The 3D window of the flight screen: centre (112, 96), 224 x 192 pixels.
pub const FLIGHT_VIEWPORT: ProjectionViewport = ProjectionViewport {
    center: [0x70, 0x60],
    left: 0,
    right: 0xDF,
    top: 0,
    bottom: 0xBF,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderViewError {
    World(WorldInputError),
    MissingFixedView,
}

/// Build the view transform from the fixed view's fine angles and rear
/// distance, and publish its matrix and the viewport for marker projection.
pub fn setup(objects: &ObjectStore, world: &mut ScenePathWorld) -> Result<ViewTransform, RenderViewError> {
    let view_id = world.fixed_players[0].ok_or(RenderViewError::MissingFixedView)?;
    let view = objects
        .get(view_id)
        .ok_or(RenderViewError::World(WorldInputError::MissingActor(view_id)))?;
    let angles = FixedViewAngles::capture(view);
    let transform = ViewTransform::from_camera(
        IntroCameraView {
            position: view.base.position,
            angles: AttractCameraAngles {
                pitch: angles.pitch,
                yaw: angles.yaw,
                roll: angles.roll,
            },
        },
        view.base.view_rear_distance,
    );
    world.marker_projection.view_matrix = Some(transform.matrix);
    world.marker_projection.viewport = Some(FLIGHT_VIEWPORT);
    Ok(transform)
}

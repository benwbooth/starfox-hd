use super::object::{Angle, ObjectId, ObjectLifetimeId, ShapeId, Vector3};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Rotation {
    pub pitch: Angle,
    pub yaw: Angle,
    pub roll: Angle,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Camera {
    pub position: Vector3,
    pub rotation: Rotation,
}

/// Decoded material-set identity. The contained value is interpreted only by
/// the generated asset catalog and renderer adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterialSetId(u16);

impl MaterialSetId {
    pub const fn from_catalog_token(token: u16) -> Self {
        Self(token)
    }

    pub const fn catalog_token(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AnimationState {
    pub shape_frame: u8,
    pub color_frame: u8,
    pub explosion_frame: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderFlags {
    pub visible: bool,
    pub casts_shadow: bool,
    pub highlighted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderObject {
    pub object: ObjectId,
    pub lifetime: ObjectLifetimeId,
    pub shape: ShapeId,
    pub material_set: MaterialSetId,
    pub position: Vector3,
    pub rotation: Rotation,
    pub sort_depth: i16,
    pub animation: AnimationState,
    pub depth_offset: u8,
    pub texture_scroll_x: u8,
    pub texture_scroll_y: u8,
    pub flags: RenderFlags,
}

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

/// Scene clipping selection, separate from visibility and collision. Source
/// extension `$1CEF` is copied intact to draw-record byte `$1E`. Zero disables
/// clipping; nonzero values select scene planes. Keep the complete byte rather
/// than collapsing the independently authored first/second planes to a flag.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ClippingPlaneSelection(u8);

impl ClippingPlaneSelection {
    pub const DISABLED: Self = Self(0);
    pub const FIRST: Self = Self(1);
    pub const SECOND: Self = Self(2);

    pub const fn from_selector_byte(value: u8) -> Self {
        Self(value)
    }

    pub const fn selector_byte(self) -> u8 {
        self.0
    }
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
    pub scaled_sprite: bool,
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
    pub clipping_plane: ClippingPlaneSelection,
    pub texture_scroll_x: u8,
    pub texture_scroll_y: u8,
    pub flags: RenderFlags,
}

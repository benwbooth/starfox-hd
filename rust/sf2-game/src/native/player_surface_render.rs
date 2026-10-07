//! Surface-crossing palette entries ($07:EB78/EB94) and the complete
//! lighting/environment publication tail ($07:C355..C440). The two live
//! surface flags have independent meanings; this service does not infer a
//! crossing, allocate an effect, or advance the player or display clocks.

use super::intro_material::{DepthColorFamily, DepthThresholdTable, SceneLighting};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore};
use sf2_data::palettes::PolygonPaletteId;
use sf2_data::surface_particles::SurfaceParticlePalette;

#[cfg(test)]
#[path = "player_surface_render_tests.rs"]
mod tests;

const SURFACE_MODE_MASK: u8 = 7;
const SPECIAL_SURFACE_MODE: u8 = 2;
const SURFACE_CARRY: u8 = 0x02;
const NONNEGATIVE_VIEW_HEIGHT: u8 = 0x04;
const POLYGON_PALETTE_START: usize = 112;
const AMBIENT_HEIGHT_GATE: i16 = -1500;
const AMBIENT_PARTICLE_SIZE: u16 = 16;

/// Side of the environment plane selected by the *wrapped signed* view-Y
/// subtraction. The crossing producer owns that comparison and its latch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceViewSide {
    Negative,
    Nonnegative,
}

/// Shared ambient-particle generation flags (1BC), not an actor's flags.
/// Preserve the other authored behaviors when the surface service enables
/// rising shape particles. The opposite branch clears the entire word.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AmbientParticleControl(u16);

impl AmbientParticleControl {
    const SHAPES: u16 = 0x02;
    const RISING: u16 = 0x20;

    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }
    pub const fn bits(self) -> u16 {
        self.0
    }
    pub const fn renders_shapes(self) -> bool {
        self.0 & Self::SHAPES != 0
    }
    pub const fn rises(self) -> bool {
        self.0 & Self::RISING != 0
    }
    fn enable_rising_shapes(&mut self) {
        self.0 |= Self::SHAPES | Self::RISING;
    }
}

/// Canonical scene rendering publications. None means that its real
/// producer has not run. Surface updates preserve inherited values wherever
/// the source does; this is not a preselected underwater rendering preset.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SceneRenderEnvironment {
    pub lighting: SceneLighting,
    /// Shared rendering plane (18B9); distinct from collision's plane.
    pub plane_height: Option<i16>,
    /// Ambient generation's unsigned view-height comparison (1E11).
    pub ambient_height_gate: Option<u16>,
    pub ambient_control: Option<AmbientParticleControl>,
    pub ambient_palette: Option<SurfaceParticlePalette>,
    /// Whole shape-size publication, not the number of ambient particles.
    pub ambient_size: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRenderError {
    World(WorldInputError),
    MissingPalette,
    MissingSurfaceMode,
    MissingModeSelection(ObjectId),
    MissingEnvironmentPlane,
    MissingAmbientControl,
}
impl From<WorldInputError> for SurfaceRenderError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

/// Replace both live and saved polygon colors, without touching the other
/// 112 colors. Only the negative-side entry requests a display refresh,
/// and that request precedes its first palette write.
pub fn replace_polygon_palette(
    world: &mut ScenePathWorld,
    side: SurfaceViewSide,
) -> Result<(), SurfaceRenderError> {
    let selection = match side {
        SurfaceViewSide::Nonnegative => PolygonPaletteId::CatalogOne,
        SurfaceViewSide::Negative => {
            world.palette_refresh_requested = Some(true);
            PolygonPaletteId::EladardSurface
        }
    };
    let palette = world
        .palette
        .as_mut()
        .ok_or(SurfaceRenderError::MissingPalette)?;
    palette.colors[POLYGON_PALETTE_START..].copy_from_slice(selection.colors());
    palette.saved_colors[POLYGON_PALETTE_START..].copy_from_slice(selection.colors());
    Ok(())
}

/// Publish the source tail's lighting and ambient configuration using live
/// player state. Non-special modes do not borrow any player/ambient inputs.
/// The palette change is a distinct earlier service, not repeated here.
pub fn publish_environment(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), SurfaceRenderError> {
    let mode = world
        .surface_mode
        .ok_or(SurfaceRenderError::MissingSurfaceMode)?;
    if mode.flags & SURFACE_MODE_MASK != SPECIAL_SURFACE_MODE {
        return Ok(());
    }
    let flags = world
        .player(objects, owner)?
        .mode_selection
        .ok_or(SurfaceRenderError::MissingModeSelection(owner))?
        .surface_control;
    let environment = &mut world.render_environment;
    environment.lighting = if flags & SURFACE_CARRY == 0 {
        SceneLighting {
            depth_colors: Some(DepthColorFamily::SURFACE_UNCARRIED),
            thresholds: Some(DepthThresholdTable::SURFACE_UNCARRIED),
        }
    } else {
        SceneLighting {
            depth_colors: Some(DepthColorFamily::STANDARD),
            thresholds: Some(DepthThresholdTable::SURFACE_CARRIED),
        }
    };
    if flags & NONNEGATIVE_VIEW_HEIGHT == 0 {
        environment.ambient_height_gate = Some(AMBIENT_HEIGHT_GATE as u16);
        environment.plane_height = Some(
            world
                .environment_plane_height
                .ok_or(SurfaceRenderError::MissingEnvironmentPlane)?,
        );
        environment.ambient_control = Some(AmbientParticleControl::default());
        environment.ambient_palette = Some(SurfaceParticlePalette::Negative);
    } else {
        environment.plane_height = Some(0);
        environment
            .ambient_control
            .as_mut()
            .ok_or(SurfaceRenderError::MissingAmbientControl)?
            .enable_rising_shapes();
        environment.ambient_palette = Some(SurfaceParticlePalette::Nonnegative);
    }
    environment.ambient_size = Some(AMBIENT_PARTICLE_SIZE);
    Ok(())
}

//! The stage loop's scene setup (`$03:8325`, single-player path), between
//! the mission launch and the scene's first frame: the display goes dark,
//! the stage clock and scene publications reset, the layout setup
//! (`$03:B19D`) and the object pool (`$03:85AE`, a fresh `ObjectStore`).
//! The prologue map, the excluded proxy and the scene player's first visit
//! (`$03:A55D`), the stage map and the frame loop's reseed (`$03:8A68`)
//! follow, run by the caller with the existing native services.
//!
//! Hardware, loader and GSU setup (the screen state, `$03:8F57` loads,
//! `$7F:1737`, `$0D:D5B7`, `$03:84F9`) produce no scene state and are not
//! modeled.

use super::map_streaming::RegionGroups;
use super::scene_display::{DisplayBand, FadeRequest, Intensity, SceneDisplay};
use super::scene_path_world::ScenePathWorld;
use super::stage_controller::StageClock;

/// 1B84: the two-player path ($03:83F2) is not ported.
const MODE_TWO_PLAYER: u16 = 0x0001;
/// `$03:83C1` sets 0010 and `$03:83C7` clears 0100.
const STAGE_MODE_SET: u16 = 0x0010;
const STAGE_MODE_CLEAR: u16 = 0x0100;
const CLOCK_STEP_FRAMES: u16 = 0xF0;
const BLINK_PERIOD: u16 = 4;
const NO_SELECTED_REGION: u8 = u8::MAX;
/// `$03:B2DB`: layout 0's viewport word (1916).
const FLIGHT_LAYOUT: u16 = 0x00C0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupError {
    MissingViewMode,
    MissingStageControl,
    MissingSceneGate,
    MissingHandoff,
    MissingHealthDisplay,
    MissingSceneDisplay,
    UnportedTwoPlayer,
    UnportedLayout(u8),
}

/// `$03:8325`: the scene setup's stores, in source order. `layout` is the
/// launch's layout selector (1AA5).
pub fn begin_scene(world: &mut ScenePathWorld, layout: u8) -> Result<(), SetupError> {
    // $03:8333..833E: forced blank, every band blank, the fade cleared.
    let display = world.scene_display.as_mut().ok_or(SetupError::MissingSceneDisplay)?;
    display.publish_all_bands(DisplayBand::BLANK_DARK);
    display.request = FadeRequest::Idle;
    display.progress = Intensity::DARK;
    let mode = world.view_transition_mode.as_mut().ok_or(SetupError::MissingViewMode)?;
    if mode.flags & MODE_TWO_PLAYER != 0 {
        return Err(SetupError::UnportedTwoPlayer);
    }
    mode.flags = (mode.flags | STAGE_MODE_SET) & !STAGE_MODE_CLEAR;
    world.scene_gate_flags.as_mut().ok_or(SetupError::MissingSceneGate)?.hud_ready = false;
    // $03:83D0..83E5.
    let stage = world.stage.as_mut().ok_or(SetupError::MissingStageControl)?;
    stage.clock = StageClock {
        step_countdown: CLOCK_STEP_FRAMES,
        step_frames: 0,
        blink_countdown: BLINK_PERIOD,
        ..stage.clock
    };
    world.encounter_timer_steps = Some(0);
    // $03:8437..8453.
    world.handoff.as_mut().ok_or(SetupError::MissingHandoff)?.player_flags = 0;
    world.camera_projection_base = Some(0);
    world.health_display.as_mut().ok_or(SetupError::MissingHealthDisplay)?.maximum = 0;
    world.strategy_clock = 0;
    // $03:845D: the layout setup.
    match layout {
        0 => {
            world.stage_layout = Some(FLIGHT_LAYOUT);
            world.reflect_all_contacts = Some(true);
        }
        other => return Err(SetupError::UnportedLayout(other)),
    }
    // $03:848C..8494: no selected region, no regions.
    world.region_groups = Some(RegionGroups { current: NO_SELECTED_REGION, previous: NO_SELECTED_REGION });
    world.map_regions = Some(Default::default());
    Ok(())
}

/// The dark display the setup leaves before any map request.
pub fn dark_display(blank_hold: u8, interval: (u8, u8)) -> SceneDisplay {
    SceneDisplay {
        request: FadeRequest::Idle,
        progress: Intensity::DARK,
        bands: [DisplayBand::BLANK_DARK; 3],
        blank_hold,
        interval_remaining: interval.0,
        interval_reload: interval.1,
    }
}

//! The mission stage launch (`$03:B90E..BA14`), run once when the strategic
//! map hands an encounter to the stage loop: the stage's HUD and display
//! modes, the presentation director's stage-start request, the encounter's
//! campaign flags and clock snapshot, and the location's map
//! (`$03:BA17`, then the location handler, then the stage loop `$03:BE74`).
//!
//! Only location 7 (the Star Wolf interceptions) is ported; the alternate
//! launch variants it does not take fault.

use super::presentation_director::{self, DirectorError};
use super::scene_map::MapCursor;
use super::scene_path_world::ScenePathWorld;

/// 1B84: the two-player mode (01) and the location handler's 0200.
const MODE_TWO_PLAYER: u16 = 0x0001;
const MODE_LOCATION_CLEARS: u16 = 0x0200;
/// 1B86 bit 0010: a stage was launched; bit 0020 selects the continued
/// launch ($03:B97F), which is not ported.
const RESULT_LAUNCHED: u16 = 0x0010;
const RESULT_CONTINUED: u16 = 0x0020;
/// 1B8A bit 0080 marks the alternate interception launch ($03:B971).
const CAMPAIGN_ALTERNATE: u16 = 0x0080;
/// 1B9C bits 02/04/08: the stage's display flags.
const DISPLAY_STAGE: u16 = 0x000E;
/// 1B88 bit 0001: locations from 6 on.
const EVENT_LATE_LOCATION: u16 = 0x0001;
const LATE_LOCATION: u16 = 6;
const HUD_MODE_FLIGHT: u8 = 4;
const HUD_LAYOUT_FLIGHT: u8 = 2;
/// `$0B:8C66`: the stage-start iris, holding the stage; its inline
/// parameter is returned to the launch.
const STAGE_START_IRIS: u8 = 3;
const STAGE_START_PARAMETER: u16 = 0x1E;
/// `$03:BA4F`: the location class byte.
const LOCATION_CLASSES: [u8; 14] = [3, 4, 5, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0];
/// `$03:BA17`: each location's handler and scene number.
const LOCATION_SCENES: [u16; 14] = [
    0x0062, 0x0076, 0x008A, 0x009E, 0x00B2, 0x00C3, 0x0115, 0x0129, 0x013D, 0x0151, 0x00D4,
    0x00E2, 0x00F9, 0x0107,
];
const INTERCEPTION_LOCATION: u16 = 7;
/// `$03:B9DF..BA0B`: location 7's layouts 8 and 0A choose other scenes.
const PLANET_LAYOUT: u8 = 0x08;
const RIVAL_LAYOUT: u8 = 0x0A;
const BLOCKADE_LAYOUT: u8 = 0x0B;
const PLANET_SCENE: u16 = 0x0162;
const RIVAL_SCENE: u16 = 0x0173;
/// E087 bit 0400 selects the planet layout's scene.
const SCENARIO_PLANET: u16 = 0x0400;

/// The launch's stage-level publications that only the HUD, display and
/// campaign owners read.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StageLaunchState {
    /// 1B9E: the HUD service's mode ($04:83B3 table).
    pub hud_mode: u8,
    /// 1BA2: the HUD layout ($04:A403).
    pub hud_layout: u8,
    /// 1C6E: the director's stage-start parameter, read at $03:C1EB.
    pub presentation_parameter: u16,
    /// 1CB8 bit 08: the display interrupt's stage mode. The interrupt owns
    /// the byte's other bits.
    pub stage_interrupt: bool,
    /// 1C67 bit 02.
    pub stage_transition: bool,
    /// 1BB7: the location number, one-based.
    pub mission_number: u8,
    /// 1BB8: the location class ($03:BA4F).
    pub location_class: u8,
    /// 1B6E: the location's scene number ($03:BA19).
    pub scene_number: u16,
    /// DA5D: the elapsed-step clock at launch.
    pub elapsed_at_launch: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissionLaunch {
    /// 1BB5 and 1BA5.
    pub location: u16,
    pub layout: u8,
    /// 1B8A, the campaign's interception flags.
    pub campaign_flags: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchError {
    MissingStageControl,
    MissingViewMode,
    MissingDisplayFlags,
    MissingSceneEvents,
    MissingScenarioFlags,
    Director(DirectorError),
    UnportedAlternateLaunch,
    UnportedContinuedLaunch,
    UnportedBlockadeLayout,
    UnportedLocation(u16),
}

impl From<DirectorError> for LaunchError {
    fn from(error: DirectorError) -> Self {
        Self::Director(error)
    }
}

/// `$03:B90E`: launch the encounter. Returns the stage's frame map, which
/// the stage loop's first frames dispatch.
pub fn launch(world: &mut ScenePathWorld, mission: MissionLaunch) -> Result<MapCursor, LaunchError> {
    let mut launch = StageLaunchState {
        hud_mode: HUD_MODE_FLIGHT,
        hud_layout: HUD_LAYOUT_FLIGHT,
        ..Default::default()
    };
    // 1AA5 and D7D8 are also cleared; nothing reads them back.
    world.view_transition_mode.as_mut().ok_or(LaunchError::MissingViewMode)?.flags &= !MODE_TWO_PLAYER;
    let stage = world.stage.as_mut().ok_or(LaunchError::MissingStageControl)?;
    stage.result_flags |= RESULT_LAUNCHED;
    let continued = stage.result_flags & RESULT_CONTINUED != 0;
    *world.scene_display_flags.as_mut().ok_or(LaunchError::MissingDisplayFlags)? |= DISPLAY_STAGE;
    launch.stage_interrupt = true;
    // $0B:8C17 clears the director's request; $0B:8C66 requests the iris.
    presentation_director::clear_request(world)?;
    presentation_director::request(world, STAGE_START_IRIS, true)?;
    launch.presentation_parameter = STAGE_START_PARAMETER;
    launch.stage_transition = true;
    if mission.campaign_flags & CAMPAIGN_ALTERNATE != 0 {
        return Err(LaunchError::UnportedAlternateLaunch);
    }
    if continued {
        return Err(LaunchError::UnportedContinuedLaunch);
    }
    launch.mission_number = (mission.location as u8).wrapping_add(1);
    let stage = world.stage.as_mut().ok_or(LaunchError::MissingStageControl)?;
    launch.elapsed_at_launch = stage.clock.elapsed_steps;
    let events = world.scene_events.as_mut().ok_or(LaunchError::MissingSceneEvents)?;
    events.bits &= !EVENT_LATE_LOCATION;
    if (mission.location as i16) >= LATE_LOCATION as i16 {
        events.bits |= EVENT_LATE_LOCATION;
    }
    let index = usize::from(mission.location);
    launch.location_class = *LOCATION_CLASSES.get(index).ok_or(LaunchError::UnportedLocation(mission.location))?;
    launch.scene_number = LOCATION_SCENES[index];
    if mission.location == INTERCEPTION_LOCATION {
        match mission.layout {
            PLANET_LAYOUT => {
                let scenario = world.scenario_flags.ok_or(LaunchError::MissingScenarioFlags)?;
                if scenario & SCENARIO_PLANET != 0 {
                    launch.scene_number = PLANET_SCENE;
                }
            }
            RIVAL_LAYOUT => launch.scene_number = RIVAL_SCENE,
            BLOCKADE_LAYOUT => return Err(LaunchError::UnportedBlockadeLayout),
            _ => {}
        }
    }
    // $03:E312 loads the two pilots' portraits and palettes for display.
    let map = match mission.location {
        // $03:BBA5: map $05:E995 (192E = 05, 1657 = 6995).
        INTERCEPTION_LOCATION => {
            world.view_transition_mode.as_mut().ok_or(LaunchError::MissingViewMode)?.flags &= !MODE_LOCATION_CLEARS;
            super::authored_maps::STAR_WOLF_INTERCEPTION
        }
        other => return Err(LaunchError::UnportedLocation(other)),
    };
    world.stage.as_mut().ok_or(LaunchError::MissingStageControl)?.launch = launch;
    Ok(map)
}

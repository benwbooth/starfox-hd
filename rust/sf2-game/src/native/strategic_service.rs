//! The radar-region service (`$7F:539C`), run each scene frame after the
//! HUD's target services: on the stage's blink frames it advances the
//! strategic map's simulation (`$7F:535E` -> `$7F:537D`), so the campaign
//! map keeps moving while a stage is played.

use super::path_sound::AuthoredCue;
use super::path_control::PlayerTarget;
use super::scene_path_world::ScenePathWorld;
use super::strategic_sim::{SceneLinks, SimError, StrategicInputs, StrategicMap, TickOutput};
use super::SoundEvent;

/// 1B84: scripted view (02), the scripted-view transition (10) and the
/// stage hold (08) stop the service.
const MODE_STOPS: u16 = 0x001A;
/// 1B86 bit 0020.
const RESULT_CONTINUED: u16 = 0x0020;
/// 1B8A bits owned by the campaign: 0010 interceptions possible, 0080 the
/// alternate interception; 0020 is `interception_active`, 0200 the planet
/// damage hold.
const CAMPAIGN_INTERCEPTIONS: u16 = 0x0010;
const CAMPAIGN_ALTERNATE: u16 = 0x0080;
const CAMPAIGN_INTERCEPTION_ACTIVE: u16 = 0x0020;
const CAMPAIGN_PLANET_HOLD: u16 = 0x0200;
/// `$7F:5369`: eight table entries, all the same tick.
const SERVICE_ENTRIES: u16 = 8;

/// The strategic map as the stage carries it, with the campaign inputs the
/// simulation reads.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StrategicWorld {
    pub map: StrategicMap,
    /// 1C08: the service's table offset (even).
    pub service: u16,
    /// DAF3/DAF6: the player's map position.
    pub player: (u8, u8),
    /// DA3B, 1BA3, E089.
    pub batch_bonus: u16,
    pub satellite_timing: u16,
    pub satellite_busy: u16,
    /// 1B8A bits 0010 and 0080.
    pub campaign_flags: u16,
    /// The terrain grid (`$7F:D400`).
    pub terrain: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceError {
    MissingViewMode,
    MissingSceneGate,
    MissingStageControl,
    MissingStrategicMap,
    MissingSceneEvents,
    MissingEncounterResult,
    MissingInterception,
    MissingMapRegion,
    MissingNodeVariant,
    MissingDifficulty,
    InvalidService(u16),
    Simulation(SimError),
}

/// `$7F:539C`.
pub fn advance(world: &mut ScenePathWorld) -> Result<(), ServiceError> {
    let mode = world.view_transition_mode.ok_or(ServiceError::MissingViewMode)?.flags;
    if mode & MODE_STOPS != 0 {
        return Ok(());
    }
    if !world.scene_gate_flags.ok_or(ServiceError::MissingSceneGate)?.hud_ready {
        return Ok(());
    }
    let stage = world.stage.ok_or(ServiceError::MissingStageControl)?;
    if !stage.signals.blink || stage.result_flags & RESULT_CONTINUED != 0 {
        return Ok(());
    }
    let strategic = world.strategic.as_ref().ok_or(ServiceError::MissingStrategicMap)?;
    if strategic.service % 2 != 0 || strategic.service / 2 >= SERVICE_ENTRIES {
        return Err(ServiceError::InvalidService(strategic.service));
    }
    let interception = world.interception_active.ok_or(ServiceError::MissingInterception)?;
    let mut links = SceneLinks {
        stage_results: stage.result_flags,
        scene_events: world.scene_events.ok_or(ServiceError::MissingSceneEvents)?.bits,
        campaign_events: strategic.campaign_flags & (CAMPAIGN_INTERCEPTIONS | CAMPAIGN_ALTERNATE)
            | if interception { CAMPAIGN_INTERCEPTION_ACTIVE } else { 0 }
            | if stage.planet.suspended { CAMPAIGN_PLANET_HOLD } else { 0 },
        event_word: stage.event_word,
        planet_damage: stage.planet.pending,
        map_region: u16::from(world.scene.map_region.ok_or(ServiceError::MissingMapRegion)?),
        encounter_result: world.encounter_result.ok_or(ServiceError::MissingEncounterResult)?.word,
        node_variant: world.scene.node_presentation_variant.ok_or(ServiceError::MissingNodeVariant)?,
    };
    let difficulty = match world.campaign.ok_or(ServiceError::MissingDifficulty)?.difficulty {
        super::Difficulty::Normal => 0,
        super::Difficulty::Hard => 1,
        super::Difficulty::Expert => 2,
    };
    let mut output = TickOutput::default();
    let strategic = world.strategic.as_mut().ok_or(ServiceError::MissingStrategicMap)?;
    let inputs = StrategicInputs {
        difficulty,
        player: strategic.player,
        batch_bonus: strategic.batch_bonus,
        satellite_timing: strategic.satellite_timing,
        satellite_busy: strategic.satellite_busy,
        terrain: &strategic.terrain,
    };
    strategic.map.tick(&mut links, inputs, &mut output).map_err(ServiceError::Simulation)?;
    world.scene_events.as_mut().ok_or(ServiceError::MissingSceneEvents)?.bits = links.scene_events;
    world.interception_active = Some(links.campaign_events & CAMPAIGN_INTERCEPTION_ACTIVE != 0);
    let stage = world.stage.as_mut().ok_or(ServiceError::MissingStageControl)?;
    stage.event_word = links.event_word;
    stage.planet.pending = links.planet_damage;
    world.scene.map_region = Some(links.map_region as u8);
    world.encounter_result.as_mut().ok_or(ServiceError::MissingEncounterResult)?.word = links.encounter_result;
    world.scene.node_presentation_variant = Some(links.node_variant);
    // `$7F:6E09` queues each cue word.
    for cue in output.cues {
        world.audio.queue(SoundEvent::Authored(AuthoredCue::new(cue as u8, 0, PlayerTarget::Primary)));
    }
    Ok(())
}

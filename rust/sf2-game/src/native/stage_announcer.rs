//! The stage's map-event announcer (`$7F:7763`, from the display service
//! `$04:FCC8` via `$7F:7034`): while no message is showing, the first
//! pending strategic map event (DB33) is claimed and its radio message is
//! requested, such as the planet-under-attack report, whose wording follows
//! the planet's remaining health.

use super::scene_path_world::ScenePathWorld;

/// `$7F:77D4`: each event bit, its message (0 picks by planet health) and
/// the message's voice.
const EVENTS: [(u16, u8, u8); 10] = [
    (0x0002, 0x6E, 0x03),
    (0x0004, 0x00, 0x02),
    (0x0008, 0x12, 0x02),
    (0x0001, 0x13, 0x02),
    (0x0010, 0x51, 0x00),
    (0x0020, 0x50, 0x00),
    (0x0040, 0x52, 0x00),
    (0x0080, 0x87, 0x00),
    (0x0100, 0xD1, 0x02),
    (0x0200, 0xC5, 0x02),
];
const FLIGHT_STAGE: u16 = 1;
/// The director's stage-start request, not yet started.
const PENDING_IRIS: u16 = 3;

/// The announcement's presentation words (F566, F55E, F560). The message
/// itself goes to the radio event word (1E84), which an authored path
/// shows and clears.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StageMessage {
    pub voice: u16,
    pub progress: u16,
    pub timer: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnouncerError {
    MissingStageControl,
    MissingDirector,
    MissingRadioEvent,
    MissingStrategicMap,
}

/// `$04:FCC8`'s stage branch, as far as it holds scene state: the
/// announcer runs in the stage's display mode (1CB8 bit 08).
pub fn advance(world: &mut ScenePathWorld) -> Result<(), AnnouncerError> {
    let stage = world.stage.ok_or(AnnouncerError::MissingStageControl)?;
    if !stage.launch.stage_interrupt {
        return Ok(());
    }
    if stage.kind & 0x00FF == FLIGHT_STAGE
        && world.director.ok_or(AnnouncerError::MissingDirector)?.request == PENDING_IRIS
    {
        return Ok(());
    }
    if world.radio_event.ok_or(AnnouncerError::MissingRadioEvent)?.number != 0 {
        return Ok(());
    }
    let health = stage.planet.health;
    let events = &mut world.strategic.as_mut().ok_or(AnnouncerError::MissingStrategicMap)?.map.globals.map_events;
    if let Some((message, announcement)) = announce(events, health) {
        world.radio_event.as_mut().ok_or(AnnouncerError::MissingRadioEvent)?.number = message;
        world.stage_message = Some(announcement);
    }
    Ok(())
}

/// `$7F:7784..77B9`: with no message showing, claim the first pending map
/// event; every event bit before it is cleared too. Returns the message
/// and its presentation words.
pub fn announce(events: &mut u16, planet_health: u16) -> Option<(u16, StageMessage)> {
    for (bit, message, voice) in EVENTS {
        let pending = *events & bit != 0;
        *events &= !bit;
        if !pending {
            continue;
        }
        let message = if message != 0 { u16::from(message) } else { planet_report(planet_health) };
        return Some((message, StageMessage { voice: u16::from(voice), progress: 0, timer: 0 }));
    }
    None
}

/// `$7F:77BD`: the report's wording by the planet's health.
fn planet_report(health: u16) -> u16 {
    if health < 0x0F {
        0x11
    } else if health < 0x32 {
        0x10
    } else {
        0x0F
    }
}

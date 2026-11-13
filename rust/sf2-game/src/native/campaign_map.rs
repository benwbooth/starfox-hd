//! The ported strategic map as the game runs it: one `MapVisit` carried
//! across the campaign, stepped one display frame at a time, and the
//! hand-offs its exits make to the other programs (pilot selection, a
//! stage, the campaign's scenes).
//!
//! Retail's map program waits for each finished picture, so a program
//! frame follows every eighth frame interrupt; here the program frame runs
//! in the frame whose interrupt finished the picture. Leaving runs the
//! exit's fade to its end within the frame that requested it.

use super::state::Pilot;
use super::strategic_entry::EntryInputs;
use super::strategic_exit::ExitInputs;
use super::strategic_screen::{MapPad, ScreenOutput};
use super::strategic_visit::{MapVisit, VisitError};
use super::RandomState;

/// 1B88 bits: back from a stage (0010) and its objective met (0002).
const EVENTS_FROM_STAGE: u16 = 0x0010;
const EVENTS_CLEARED: u16 = 0x0002;
/// OAM (128 entries and the high table), then CGRAM.
const OAM_BYTES: usize = 0x220;
const CGRAM_BYTES: usize = 0x200;
/// OAM, CGRAM, then each craft's frame set and sector.
pub const SPRITE_LAYER_BYTES: usize = OAM_BYTES + CGRAM_BYTES + 4;
/// 1B86 bit 0080: the tutorial map.
const RESULTS_TUTORIAL: u16 = 0x0080;
/// 1B84 bit 0080, set by the front end before the first entry.
const MODE_FRONT_END: u16 = 0x0080;
/// 1DD1.. the pilots' starting shields.
const FULL_SHIELD: u8 = 0x20;

/// The words the front end leaves for a new campaign's first map entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewCampaign {
    /// D7F2: 0 normal, 1 hard, 2 expert.
    pub difficulty: u8,
    /// 1C02: the campaign's seed word.
    pub seed: u16,
    /// 1C00: the random word.
    pub random: u16,
    /// 1C06..1C07: the encounter variants the front end's scenes leave.
    pub missile_kinds: [u8; 2],
    /// The shared generator's bytes and the byte after them (DP E0..E4).
    pub generator: [u8; 4],
    pub generator_tail: u8,
    /// D7E1..D7E3.
    pub tallies: [u8; 3],
}

/// Where the map handed the game when it left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapHandoff {
    /// The tutorial map ended (mode 6): the pilots are chosen next.
    PilotSelection,
    /// The ship reached a place (mode 2) or met a unit (mode 1): the stage
    /// at this location and layout (1BB5, 1BA5).
    Stage(StageRequest),
    /// A campaign scene (mode 4) with its scene number (1B76): 0x14 the
    /// planet fell, 0x20 the final stage, 0x28 a threat's place.
    Scene(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageRequest {
    pub location: u8,
    pub layout: u8,
    /// Whether the ship met a unit (mode 1) rather than reaching a place.
    pub unit: bool,
    /// 1B96 bit 0080: the continued launch.
    pub continued: bool,
}

/// A unit as the map screen shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapUnitView {
    pub category: u8,
    pub variant: u8,
    pub x: u8,
    pub y: u8,
}

/// How a stage ended, as the map's return reads it.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StageOutcome {
    /// 1B88 bit 0002: the stage's objective was met.
    pub cleared: bool,
    /// 1BAD: the stage's result word.
    pub mission_result: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignMap {
    pub visit: MapVisit,
    /// DP E4.
    pub generator_tail: u8,
    /// 1C02 and D7E1..D7E3, which the campaign setup reads.
    seed: u16,
    tallies: [u8; 3],
    /// Whether the map is between an exit and its next entry.
    away: bool,
    /// The main loop has a program frame to run (after the entry, or once
    /// the wait for a picture ended).
    program_pending: bool,
    /// What the frames since the last call queued.
    pub output: ScreenOutput,
}

impl CampaignMap {
    /// A new campaign: the front end's words, then the first entry (the
    /// tutorial map).
    pub fn new_campaign(campaign: NewCampaign) -> Result<Self, VisitError> {
        let mut visit = MapVisit::default();
        visit.links.difficulty = u16::from(campaign.difficulty);
        visit.links.scene.stage_results = RESULTS_TUTORIAL;
        visit.links.mode = MODE_FRONT_END;
        visit.links.pilots = [1, 0];
        visit.links.pilot_shields = [FULL_SHIELD; 4];
        visit.links.random = campaign.random;
        visit.links.missile_kinds = campaign.missile_kinds;
        visit.rng = RandomState::new(campaign.generator);
        let mut map = Self {
            visit,
            generator_tail: campaign.generator_tail,
            seed: campaign.seed,
            tallies: campaign.tallies,
            away: true,
            program_pending: false,
            output: ScreenOutput::default(),
        };
        map.enter()?;
        Ok(map)
    }

    /// After pilot selection: the chosen pilots (1E14/1E15, by roster
    /// index) and the primary's shields, then the campaign's first entry.
    pub fn begin_campaign(&mut self, pilots: [Option<Pilot>; 2]) -> Result<(), VisitError> {
        let index = |pilot: Option<Pilot>| {
            pilot.map_or(0, |pilot| {
                Pilot::ALL.iter().position(|&p| p == pilot).unwrap_or(0) as u8
            })
        };
        self.visit.links.pilots = [index(pilots[0]), index(pilots[1])];
        if let Some(primary) = pilots[0] {
            let shield = primary.craft_profile().maximum_shield;
            self.visit.links.pilot_shields[0] = shield;
            self.visit.links.pilot_shields[1] = shield;
        }
        self.enter()
    }

    /// Back from a stage: its outcome, then the entry, which runs the
    /// stage's results on the map.
    pub fn return_from_stage(&mut self, outcome: StageOutcome) -> Result<(), VisitError> {
        let events = &mut self.visit.links.scene.scene_events;
        *events |= EVENTS_FROM_STAGE;
        if outcome.cleared {
            *events |= EVENTS_CLEARED;
        } else {
            *events &= !EVENTS_CLEARED;
        }
        self.visit.links.mission_result = outcome.mission_result;
        self.enter()
    }

    /// The ship's place on the map, in map pixels.
    pub fn ship(&self) -> (u8, u8) {
        (
            (self.visit.screen.ship.x >> 8) as u8,
            (self.visit.screen.ship.y >> 8) as u8,
        )
    }

    /// The map's units in list order: category, variant and place.
    pub fn units(&self) -> Vec<MapUnitView> {
        let map = &self.visit.map;
        let mut units = Vec::new();
        let mut cursor = map.unit_head;
        while let Some(id) = cursor {
            let unit = map.unit(id);
            units.push(MapUnitView {
                category: unit.category as u8,
                variant: unit.variant as u8,
                x: unit.x,
                y: unit.y,
            });
            cursor = unit.next;
        }
        units
    }

    /// The sprite list (OAM) and the palette (CGRAM) the map screen shows,
    /// while the map runs.
    pub fn sprite_layer(&self) -> Option<[u8; SPRITE_LAYER_BYTES]> {
        if self.away {
            return None;
        }
        let mut layer = [0u8; SPRITE_LAYER_BYTES];
        let oam = &self.visit.sprites.oam;
        let palette = &self.visit.hud.palette;
        let (left, right) = layer.split_at_mut(OAM_BYTES);
        left.copy_from_slice(oam.get(..OAM_BYTES)?);
        let (palette_bytes, craft) = right.split_at_mut(CGRAM_BYTES);
        palette_bytes.copy_from_slice(palette.get(..CGRAM_BYTES)?);
        // The crafts' frames (`$04:D68B`), whose tiles are streamed: the
        // pilot's frame set and the sector of each craft's course.
        let links = &self.visit.links;
        let screen = &self.visit.screen;
        craft.copy_from_slice(&[
            (links.pilots[0] >> 1) & 3,
            screen.ship.sector as u8,
            (links.pilots[1] >> 1) & 3,
            screen.wingmate.sector as u8,
        ]);
        Some(layer)
    }

    /// The cursor's place on the map.
    pub fn cursor(&self) -> (u8, u8) {
        (self.visit.screen.cursor_x, self.visit.screen.cursor_y)
    }

    /// Whether the map is running (entered and not yet left).
    pub fn active(&self) -> bool {
        !self.away
    }

    fn enter(&mut self) -> Result<(), VisitError> {
        let inputs = EntryInputs {
            seed: self.seed,
            tallies: self.tallies,
            generator_tail: self.generator_tail,
        };
        self.visit.enter(inputs, &mut self.output)?;
        self.away = false;
        self.program_pending = true;
        Ok(())
    }

    /// One display frame with this frame's pad. Returns the hand-off when
    /// the map left in it.
    pub fn frame(&mut self, pad: MapPad) -> Result<Option<MapHandoff>, VisitError> {
        if self.away {
            return Ok(None);
        }
        // Each frame's cues and frames are the caller's to read before the
        // next.
        self.output = ScreenOutput::default();
        if self.program_pending {
            self.program_pending = false;
            self.visit.program_frame(&mut self.output)?;
        }
        self.visit.interrupt(pad, &mut self.output)?;
        if !self.visit.program_frame_due() {
            return Ok(None);
        }
        // $04:AEC9..AED1: the wait is over; the exit, else the next frame.
        if self.visit.exit_due() {
            return self.leave(pad).map(Some);
        }
        self.visit.begin_program_frame();
        self.program_pending = true;
        Ok(None)
    }

    fn leave(&mut self, pad: MapPad) -> Result<MapHandoff, VisitError> {
        let inputs = ExitInputs {
            dark_after: u16::MAX,
            pad,
            generator_tail: self.generator_tail,
        };
        self.visit.leave(inputs, &mut self.output)?;
        self.away = true;
        let launch = self.visit.launch;
        Ok(match launch.mode & 0x00FF {
            6 => MapHandoff::PilotSelection,
            1 | 2 => MapHandoff::Stage(StageRequest {
                location: self.visit.links.launch_location as u8,
                layout: self.visit.links.launch_layout as u8,
                unit: launch.mode & 0x00FF == 1,
                continued: launch.options & 0x0080 != 0,
            }),
            _ => MapHandoff::Scene(launch.scene),
        })
    }
}

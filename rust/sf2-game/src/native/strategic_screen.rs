//! The strategic map screen's frame (`$04:B5DA`), run from the map's frame
//! interrupt before the simulation tick: the cursor and its snapping to
//! places, the place info and the place menu, the ship's travel along a
//! line stepper toward the cursor or a chosen place, the wingmate that
//! trails it, the map clock, and the screen's warning cues.
//!
//! The selection script (`$04:CD47`, started by Select), the end-of-game
//! services (`$04:D377`) and the two-player start are not ported; reaching
//! them faults.

use sf_core::aim_angle::atan16;
use sf_core::snes_trig::SINTAB;

use super::strategic_sim::{MapPlace, PlaceId, SceneLinks, SimError, StrategicInputs, StrategicMap, TickOutput, UnitId};

// Pad bits (1292 held, 1296 pressed).
const PAD_B: u16 = 0x8000;
const PAD_SELECT: u16 = 0x2000;
const PAD_START: u16 = 0x1000;
const PAD_DIRECTIONS: u16 = 0x0F00;
const PAD_LEFT: u16 = 0x0200;
const PAD_RIGHT: u16 = 0x0100;
const PAD_X: u16 = 0x0040;
const PAD_CANCEL: u16 = 0x4040;
// The held directions as the high byte reads them.
const HELD_UP: u8 = 0x08;
const HELD_DOWN: u8 = 0x04;
const HELD_LEFT: u8 = 0x02;
const HELD_RIGHT: u8 = 0x01;

// DA8B: the screen's interface flags.
const UI_SNAPPED: u16 = 0x0001;
const UI_MOVING: u16 = 0x0002;
const UI_LAUNCHED: u16 = 0x0004;
const UI_ORDERED: u16 = 0x0008;
const UI_REPEAT: u16 = 0x0020;
const UI_FAST: u16 = 0x0040;
const UI_ON_SHIP: u16 = 0x0080;
const UI_COUNTDOWN: u16 = 0x0100;
const UI_CLEARS_COUNTER: u16 = 0x0200;
const UI_MENU: u16 = 0x0400;
const UI_MENU_CLOSED: u16 = 0x0800;

// DA8D: what the cursor is over.
const HOVER_PLACE: u16 = 0x0001;
const HOVER_ATTACKED: u16 = 0x0002;
const HOVER_DEFENDED: u16 = 0x0004;
const HOVER_ANNOUNCED: u16 = 0x0008;
const HOVER_DESTINATION: u16 = 0x0010;
const HOVER_FINAL: u16 = 0x0020;
const HOVER_UNIT: u16 = 0x0040;

// DB01: the ship's travel.
const TRAVEL_SETTING_OFF: u16 = 0x0001;
const TRAVEL_UNDER_WAY: u16 = 0x0002;
const TRAVEL_X_DONE: u16 = 0x0004;
const TRAVEL_Y_DONE: u16 = 0x0008;
const TRAVEL_ARRIVED: u16 = 0x0010;
const TRAVEL_DOCKED: u16 = 0x0100;
const TRAVEL_HELD: u16 = 0x0400;

// DB23: the wingmate.
const WING_FOLLOWING: u16 = 0x0002;
const WING_CLOSE: u16 = 0x0200;
const WING_HELD: u16 = 0x0400;

// Place flags (+1C).
const PLACE_ATTACKED: u16 = 0x0004;
const PLACE_DEFENDED: u16 = 0x0008;
const PLACE_CLOSED: u16 = 0x100E;
const PLACE_VISITED: u16 = 0x0080;
const PLACE_MENU_HIDDEN: u16 = 0x007C;
const PLACE_LISTED: u16 = 0x1000;
const PLACE_SHOWN: u16 = 0x2000;
const PLACE_SEALED: u16 = 0x8000;
// Unit flags (+2E) that hide a unit from the cursor.
const UNIT_UNPICKABLE: u16 = 0x8740;

// 1B8C: the map's holds.
const HOLD_CLOCK: u16 = 0x0001;
const HOLD_UNITS_PICKABLE: u16 = 0x0002;
const HOLD_SHIP: u16 = 0x0004;
const HOLD_CURSOR: u16 = 0x0008;
const HOLD_INPUT: u16 = 0x0010;
const HOLD_TWO_PLAYER_A: u16 = 0x0020;
const HOLD_TWO_PLAYER_B: u16 = 0x0040;
const HOLD_STOP: u16 = 0x0023;
// 1B92.
const SPEED_STEPPED: u16 = 0x0002;
const SPEED_FAST_FORWARD: u16 = 0x0008;
const SPEED_TOGGLED: u16 = 0x0020;
const SPEED_FAST_TRAVEL: u16 = 0x0400;
// 1B84.
const MODE_SCRIPT: u16 = 0x0010;
// 1B86.
const RESULT_WING_DOWN: u16 = 0x0002;
const RESULT_TWO_PLAYER: u16 = 0x0080;
// 1B88.
const EVENT_SCREEN_HELD: u16 = 0x0120;
const EVENT_ENCOUNTER: u16 = 0x0020;
const EVENT_ARRIVAL_CLEARS: u16 = 0x0004;
const EVENT_ARRIVAL_SETS: u16 = 0x0060;
const EVENT_PURSUIT: u16 = 0x0400;
// 1B8A.
const CAMPAIGN_FINAL: u16 = 0x0008;

const CURSOR_ACCELERATION: u16 = 0x0080;
const CURSOR_TOP_SPEED: u16 = 0x0400;
const CURSOR_SNAP_STEP: u8 = 0x11;
const CURSOR_X_RANGE: (u8, u8) = (0x0C, 0xF4);
const CURSOR_Y_RANGE: (u8, u8) = (0x10, 0xAC);
const PLACE_REACH: u8 = 8;
const UNIT_REACH: u8 = 4;
const WING_FAR: u16 = 0x0C;
const WING_NEAR: u16 = 0x02;
const MENU_CHOICES: u16 = 7;
const CLOCK_FRAMES_PER_STEP: u16 = 0x3C;
const ELAPSED_LIMIT: u16 = 999;
const COUNTDOWN_START: u16 = 0x0F;
const COUNTDOWN_PULSE: u16 = 0x0C;
/// `$04:F02E`: the countdown's twelfth frame clears the pulses' bit 0200.
const PULSE_MASK: u16 = 0xFDFF;
const SERVICE_MASK: u16 = 0x000F;
/// The two-player Start's request (D9FD).
const TWO_PLAYER_TIMELINE: u16 = 0x16;
const SELECT_SCRIPT: u16 = 0x26;

// `$7F:6E09` cues.
const CUE_HOVER: u16 = 0x0B;
const CUE_ORDER: u16 = 0x0C;
const CUE_CANCEL: u16 = 0x08;
const CUE_TWO_PLAYER: u16 = 0x0E;
const CUE_SCRIPT: u16 = 0x2F;

/// `$7F:6D30`: the cue for each 1C0E event bit (the bytes past the seventh
/// are the following routine's code, read as the source reads them).
const EVENT_CUES: [u8; 16] = [
    0x61, 0x62, 0x0F, 0x2B, 0x63, 0x61, 0x3C, 0xDA, 0x5A, 0x8B, 0x08, 0xC2, 0x30, 0x22, 0x58, 0x1D,
];
/// `$04:B789`: the threat cue's repeat by the threat countdown (DA71):
/// `(at least, repeat frames, cue)`, the last row for every other value.
const THREAT_CUES: [(u16, u8, u8); 6] = [
    (0x021C, 0x14, 0xC5),
    (0x01A4, 0x10, 0xC5),
    (0x00F0, 0x0D, 0xC5),
    (0x0078, 0x0A, 0xC6),
    (0x003C, 0x08, 0xC6),
    (0x0000, 0x06, 0xC7),
];
/// The planet health (DB47) below which each warning plays.
const PLANET_DAMAGED: u16 = 0x33;
const PLANET_CRITICAL: u16 = 0x15;

/// One of the four drifting pulses (DA97 + 14k): 24-bit positions and
/// steps, and a flag word.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pulse {
    pub x: u32,
    pub y: u32,
    pub dx: u32,
    pub dy: u32,
    pub flags: u16,
}

/// The player's ship on the map.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapShip {
    /// DAF2/DAF5: 8.8 positions (the high bytes, DAF3/DAF6, are what the
    /// simulation reads).
    pub x: u16,
    pub y: u16,
    /// DAF7: the course; DB09 its sixteenth.
    pub heading: u16,
    pub sector: u16,
    /// DAFB.
    pub speed: u16,
    /// DAFD/DAFF: the line stepper's per-frame steps.
    pub step_x: u16,
    pub step_y: u16,
    /// DB01.
    pub travel: u16,
    /// DB07: the place the ship is bound for.
    pub destination: Option<PlaceId>,
    /// DB0D/DB0E/DB0F: the line's spans and error term.
    pub span_x: u8,
    pub span_y: u8,
    pub error: u8,
    /// DAF4: the byte between the positions, read by a word load.
    pub x_pad: u8,
    /// DB03/DB05: the scripted flight's steps and countdown.
    pub flight_steps: u16,
    pub flight_countdown: u16,
    /// DB11/DB12: the travel target; DB13 the byte after it, which the
    /// arrival's word store carries into the first pulse.
    pub target_x: u8,
    pub target_y: u8,
    pub target_pad: u8,
}

/// The wingmate trailing the ship.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapWingmate {
    /// DB14/DB17: 8.8 positions.
    pub x: u16,
    pub y: u16,
    /// DB19, DB1B, DB1D/DB1F, DB21, DB23.
    pub heading: u16,
    pub speed: u16,
    pub velocity_x: u16,
    pub velocity_y: u16,
    pub sector: u16,
    pub flags: u16,
}

/// The place info panel's words (DB2D..DB57).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlaceInfo {
    pub shown: u16,
    pub number: u16,
    pub detail: u16,
    pub x: u16,
    pub y: u16,
    pub kind: u16,
}

/// The screen's own state.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapScreen {
    /// DA91/DA92.
    pub cursor_x: u8,
    pub cursor_y: u8,
    /// D978/D97A/D97C: 8.8 velocities and the acceleration they share.
    pub cursor_velocity_x: u16,
    pub cursor_velocity_y: u16,
    pub cursor_speed: u16,
    /// DA8B, DA8D.
    pub interface: u16,
    pub hover: u16,
    /// DA8F.
    pub countdown: u16,
    /// DA93..DA96: the chosen place's position and where the ship arrived.
    pub selected_x: u8,
    pub selected_y: u8,
    pub arrival_x: u8,
    pub arrival_y: u8,
    pub pulses: [Pulse; 4],
    /// DA5F, DA5B.
    pub clock_frames: u16,
    pub elapsed_steps: u16,
    /// DA75.
    pub threat_cue_timer: u16,
    /// DA1B: the Select handler's state; DA7F the script it starts.
    pub select_state: u16,
    pub script: u16,
    /// DAE7, DAE9, DAEF, DAED: the place menu.
    pub menu_choice: u16,
    pub menu_home: u16,
    pub menu_confirm: u16,
    pub menu_place: Option<PlaceId>,
    pub ship: MapShip,
    pub wingmate: MapWingmate,
    pub info: PlaceInfo,
    /// DB55: the place under the cursor; DB53 the one ordered.
    pub hovered_place: Option<PlaceId>,
    pub ordered_place: Option<PlaceId>,
    /// E09F: the unit under the cursor.
    pub hovered_unit: Option<UnitId>,
    /// 1C12: the planet warnings given; 1C10 the last one.
    pub planet_warnings: u16,
    pub planet_warning: u8,
    /// F4EC..F4EF: the planet palette flash's countdown, its reload and its
    /// palette offset word.
    pub warning_flash: [u8; 4],
    /// 1CE5: the travel sound's mode.
    pub travel_sound: u8,
    /// DA81, DA83, DA85: the running script's position in `$04:CD6E`,
    /// its subject and its countdown.
    pub script_offset: u16,
    pub script_subject: ScriptSubject,
    pub script_countdown: u16,
    /// DA87/DA89: the ship's position kept across the warp's spin.
    pub saved_ship: (u16, u16),
    /// DB25..DB27: the warp's target and the byte after it; DB28 the byte
    /// after that (DB27/DB28 are also the final choice's countdown word).
    pub warp_target: [u8; 3],
    pub warp_spare: u8,
    /// D99C/D99E: the player's position words kept for the interceptors.
    pub saved_player: (u16, u16),
    /// E0A1: a unit's motion kept across a script's fast-forward.
    pub saved_motion: u16,
    pub campaign: MapCampaign,
}

/// DA83: a unit, or, after `$04:CE95`, a table index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptSubject {
    Unit(UnitId),
    Value(u16),
}

impl Default for ScriptSubject {
    fn default() -> Self {
        Self::Value(0)
    }
}

/// The campaign's wave and marker bookkeeping that the scripts advance.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapCampaign {
    /// D99A: markers left to place; DAEB the marker kind cursor; DA2F
    /// markers placed. (1BCA/1BCC are scratch words several services share.)
    pub marker_count: u16,
    pub marker_cursor: u16,
    pub markers_placed: u16,
    /// DB51: the wave cursor; D98F the wave's remaining ships (low byte);
    /// DA41 the escorts' running number; DA3F the start position cursor.
    pub wave_cursor: u16,
    pub wave_ships: u16,
    pub wave_escorts: u16,
    pub start_cursor: u16,
    /// DA59: bases remaining.
    pub bases_left: u16,
    /// D9B6..D9B9: the last new place's position and spawn target, which
    /// the next placement inherits where it sets fewer.
    pub placement: [u8; 4],
}

/// Campaign words the screen reads, and the few it writes back.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScreenLinks {
    /// The words the simulation shares with the stage (1B86, 1B88, 1B8A,
    /// 1C0E, DB4B, DB5B, D79D, 1E09).
    pub scene: SceneLinks,
    /// 1C08: the service table offset.
    pub service: u16,
    /// 1B84 (read).
    pub mode: u16,
    /// DB47: the planet's health; DB49 its damage in percent.
    pub planet_health: u16,
    pub planet_damage_percent: u16,
    /// 1E14/1E15: the two pilots.
    pub pilots: [u8; 2],
    /// DB4D and E07B: the planet and the station places; DB63 the unit a
    /// pursuit heads for.
    pub planet_place: Option<PlaceId>,
    pub station_place: Option<PlaceId>,
    pub pursuit_unit: Option<UnitId>,
    /// E08D: the satellite's sortie escort.
    pub sortie_escort: Option<UnitId>,
    /// DB5D/DB5F: the place and unit the warp scripts move.
    pub warp_place: Option<PlaceId>,
    pub warp_unit: Option<UnitId>,
    /// 1C1F/1C21: the map's pad copies for its dialogs (held, pressed),
    /// cleared when the speed toggles.
    pub menu_pad: [u16; 2],
    /// DA7D and DB29, written on arrival; D9FD the campaign timeline's
    /// state (the two-player Start requests state 16).
    pub arrival_word: u16,
    pub final_stage: u16,
    pub timeline: u16,
    /// D7F2, DA3B, 1BA3: the simulation's inputs (E089 is the simulation's
    /// satellite hold).
    pub difficulty: u16,
    pub batch_bonus: u16,
    pub satellite_timing: u16,
    /// 1B9C: the display flags (the markers set bit 0080).
    pub display_flags: u16,
    /// D7F8: the campaign's stage flags (bit 0100: bases remain).
    pub stage_flags: u16,
    /// 1C6E: the presentation countdown the stage start leaves; 1C67 the
    /// presentation flags (bit 0002 cleared when it runs out).
    pub presentation_countdown: u16,
    pub presentation_flags: u16,
    /// F582: the message the map's text box shows; EF576/EF578 its state.
    pub message: u16,
    pub text_state: [u16; 2],
    /// 1DD1/1DD5/1DD7/1DDB: the pilots' shields (current, full) as bytes.
    pub pilot_shields: [u8; 4],
    /// 1BB5/1BA5: the next mission's location and layout.
    pub launch_location: u16,
    pub launch_layout: u16,
    /// D79D, D7F4, 1BF2: the last mission's result words.
    pub mission_result: u16,
    pub mission_rank: u16,
    pub mission_extra: u16,
    /// 1C00: the frame's random word.
    pub random: u16,
    /// 1C06/1C07: the missile kinds' offsets.
    pub missile_kinds: [u8; 2],
    /// 1B9E: the HUD service's mode (2 on the map).
    pub hud_mode: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapPad {
    /// 1292 and 1296.
    pub held: u16,
    pub pressed: u16,
}

/// The ship sprite chosen by `$04:D6BD`: the pilot's frame set and the
/// sixteenth of the course.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ShipFrame {
    pub pilot: u8,
    pub sector: u8,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ScreenOutput {
    /// `$7F:6E09` cues in order.
    pub cues: Vec<u16>,
    /// `$7F:6DF8`: the radio voice (1CDA, with 1CD9 cleared).
    pub radio: Option<u8>,
    /// 1C17/1C19 and 1C1B/1C1D: the ship's and the wingmate's frames.
    pub ship_frame: Option<ShipFrame>,
    pub wingmate_frame: Option<ShipFrame>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenError {
    Unported(u32),
    MissingPlace(u32),
    MissingUnit(u32),
    /// `$04:CCAE`/`$04:CCC4` search forever when no listed place matches.
    EmptyMenu,
    /// `$04:EF6A`: the place pool is empty (the source halts).
    PlacePoolExhausted,
    /// A script entry the dispatcher would jump through as code.
    InvalidScriptStep(u16),
    TableOverrun(u32),
    Simulation(SimError),
}

impl From<SimError> for ScreenError {
    fn from(error: SimError) -> Self {
        Self::Simulation(error)
    }
}

/// `$04:B5DA`: one map screen frame.
pub fn frame(
    screen: &mut MapScreen,
    map: &mut StrategicMap,
    links: &mut ScreenLinks,
    terrain: &[u8],
    pad: MapPad,
    output: &mut ScreenOutput,
) -> Result<(), ScreenError> {
    links.service = links.service.wrapping_add(2) & SERVICE_MASK;
    let mut frame = Frame { screen, map, links, terrain, pad, output };
    // $04:B5FB; the service table's entries all return.
    if frame.links.scene.scene_events & EVENT_SCREEN_HELD != 0 {
        return Ok(());
    }
    frame.run()?;
    if frame.map.globals.campaign_flags != 0 {
        frame.final_services()?;
    }
    frame.run_script()?;
    frame.menu()
}

pub(super) struct Frame<'a> {
    pub(super) screen: &'a mut MapScreen,
    pub(super) map: &'a mut StrategicMap,
    pub(super) links: &'a mut ScreenLinks,
    pub(super) terrain: &'a [u8],
    pub(super) pad: MapPad,
    pub(super) output: &'a mut ScreenOutput,
}

impl Frame<'_> {
    pub(super) fn place(&self, id: Option<PlaceId>, site: u32) -> Result<&MapPlace, ScreenError> {
        id.map(|id| &self.map.places[usize::from(id.0)]).ok_or(ScreenError::MissingPlace(site))
    }

    pub(super) fn cue(&mut self, cue: u16) {
        self.output.cues.push(cue);
    }

    /// The simulation's inputs as the screen's frame sees them.
    pub(super) fn sim_inputs(&self) -> StrategicInputs<'_> {
        StrategicInputs {
            difficulty: self.links.difficulty,
            player: ((self.screen.ship.x >> 8) as u8, (self.screen.ship.y >> 8) as u8),
            batch_bonus: self.links.batch_bonus,
            satellite_timing: self.links.satellite_timing,
            satellite_busy: self.map.globals.satellite_hold,
            terrain: self.terrain,
        }
    }

    /// `$7F:66CF`: one unit's behavior, motion and meeting check.
    pub(super) fn run_unit(&mut self, id: UnitId) -> Result<(), ScreenError> {
        let mut scene = self.links.scene;
        let mut tick = TickOutput::default();
        let terrain = self.terrain;
        let inputs = StrategicInputs { terrain, ..self.sim_inputs() };
        self.map.behave(id, &mut scene, inputs, &mut tick)?;
        self.map.move_unit(id)?;
        self.map.check_meeting(id, &mut scene, inputs)?;
        self.links.scene = scene;
        self.output.cues.extend(tick.cues);
        Ok(())
    }

    /// `$04:D68B`: the ship's frame from its sector.
    pub(super) fn show_ship(&mut self) {
        self.output.ship_frame = Some(ShipFrame {
            pilot: (self.links.pilots[0] >> 1) & 3,
            sector: self.screen.ship.sector as u8,
        });
    }

    /// `$04:B629`.
    fn run(&mut self) -> Result<(), ScreenError> {
        self.select_button()?;
        self.screen.info.shown &= !0x0001;
        let hold = self.map.globals.hold;
        if self.screen.interface & UI_COUNTDOWN != 0 {
            self.count_down();
        } else if self.screen.interface & UI_MENU == 0 && hold & HOLD_CURSOR == 0 {
            self.screen.interface &= !(0x0088);
            self.screen.hover &= !HOVER_PLACE;
            self.move_cursor();
            self.hover()?;
        }
        self.buttons();
        self.advance_clock();
        if self.map.globals.hold & HOLD_SHIP == 0 && self.screen.ship.travel & TRAVEL_HELD == 0 {
            self.choose_target()?;
            self.face_target();
            self.show_ship();
            self.travel()?;
        }
        if self.links.scene.stage_results & RESULT_WING_DOWN == 0 && self.screen.wingmate.flags & WING_HELD == 0 {
            self.output.wingmate_frame = Some(ShipFrame {
                pilot: (self.links.pilots[1] >> 1) & 3,
                sector: self.screen.wingmate.sector as u8,
            });
            self.follow();
        }
        self.warnings();
        if self.map.globals.speed_flags & SPEED_TOGGLED != 0 {
            self.links.menu_pad = [0, 0];
            self.map.globals.speed_flags &= !SPEED_TOGGLED;
            self.screen.interface &= !UI_FAST;
        }
        Ok(())
    }

    /// `$04:B8A0`: Start in the two-player game, otherwise Select's script.
    fn select_button(&mut self) -> Result<(), ScreenError> {
        let hold = self.map.globals.hold;
        if self.links.scene.stage_results & RESULT_TWO_PLAYER != 0 {
            if hold & (HOLD_TWO_PLAYER_A | HOLD_TWO_PLAYER_B) == 0 && self.pad.pressed & PAD_START != 0 {
                self.cue(CUE_TWO_PLAYER);
                self.links.timeline = TWO_PLAYER_TIMELINE;
            }
            return Ok(());
        }
        match self.screen.select_state {
            // $04:B8E1.
            0 => {
                if hold & HOLD_INPUT != 0
                    || self.pad.pressed & PAD_SELECT == 0
                    || self.links.scene.stage_results & RESULT_WING_DOWN != 0
                {
                    return Ok(());
                }
                self.stop();
                self.links.mode |= MODE_SCRIPT;
                self.screen.script = SELECT_SCRIPT;
                self.cue(CUE_SCRIPT);
                self.screen.select_state += 2;
                Ok(())
            }
            // $04:B912: wait for the script, then $04:B918 (its `$06:A325`
            // is not ported).
            2 if self.screen.script != 0 => Ok(()),
            2 | 4 => Err(ScreenError::Unported(0x04B918)),
            other => Err(ScreenError::Unported(0x04B8D4 + u32::from(other))),
        }
    }

    /// `$04:B887`.
    fn stop(&mut self) {
        self.map.globals.hold |= HOLD_STOP;
        self.screen.ship.travel &= !(TRAVEL_SETTING_OFF | TRAVEL_UNDER_WAY);
        self.screen.interface &= !(UI_LAUNCHED | UI_REPEAT);
        self.screen.hover &= !(HOVER_DESTINATION | HOVER_FINAL);
    }

    /// `$04:F013`: the order countdown drifts the pulses.
    fn count_down(&mut self) {
        let mut mask = 0xFFFF;
        if self.screen.countdown == 0 {
            return;
        }
        self.screen.countdown -= 1;
        if self.screen.countdown == 0 {
            self.map.globals.stored_counter = 0;
            return;
        }
        if self.screen.countdown == COUNTDOWN_PULSE {
            mask = PULSE_MASK;
        }
        for pulse in &mut self.screen.pulses {
            pulse.x = pulse.x.wrapping_add(pulse.dx) & 0xFF_FFFF;
            pulse.y = pulse.y.wrapping_add(pulse.dy) & 0xFF_FFFF;
            pulse.flags &= mask;
        }
    }

    /// `$04:F07A`: the held directions accelerate the cursor; over a place
    /// it steps a fixed distance and snaps.
    fn move_cursor(&mut self) {
        let screen = &mut *self.screen;
        if self.pad.held & PAD_DIRECTIONS == 0 {
            screen.interface &= !UI_MOVING;
            screen.cursor_speed = 0;
            screen.cursor_velocity_x = 0;
            screen.cursor_velocity_y = 0;
            return;
        }
        let speed = screen.cursor_speed.wrapping_add(CURSOR_ACCELERATION);
        screen.cursor_speed = if speed < CURSOR_TOP_SPEED { speed } else { CURSOR_TOP_SPEED };
        screen.interface |= UI_MOVING;
        let held = (self.pad.held >> 8) as u8;
        let speed = (screen.cursor_speed >> 8) as u8;
        if screen.hover & HOVER_PLACE == 0 {
            if held & HELD_UP != 0 {
                set_high(&mut screen.cursor_velocity_y, speed.wrapping_neg());
            } else if held & HELD_DOWN != 0 {
                set_high(&mut screen.cursor_velocity_y, speed);
            } else {
                screen.cursor_velocity_y = 0;
            }
            if held & HELD_LEFT != 0 {
                set_high(&mut screen.cursor_velocity_x, speed.wrapping_neg());
            } else if held & HELD_RIGHT != 0 {
                set_high(&mut screen.cursor_velocity_x, speed);
            } else {
                screen.cursor_velocity_x = 0;
            }
            step_cursor(screen);
            return;
        }
        // $04:F113: the word store of -0x21 also lands on the speed's low
        // byte, which is cleared below.
        if held & HELD_UP != 0 {
            set_high(&mut screen.cursor_velocity_y, 0xDF);
            screen.cursor_speed = (screen.cursor_speed & 0xFF00) | 0x00FF;
        } else if held & HELD_DOWN != 0 {
            set_high(&mut screen.cursor_velocity_y, CURSOR_SNAP_STEP);
        }
        if held & HELD_LEFT != 0 {
            set_high(&mut screen.cursor_velocity_x, 0xDF);
        } else if held & HELD_RIGHT != 0 {
            set_high(&mut screen.cursor_velocity_x, CURSOR_SNAP_STEP);
        }
        step_cursor(screen);
        screen.interface |= UI_SNAPPED;
        screen.cursor_speed = 0;
        screen.cursor_velocity_x = 0;
        screen.cursor_velocity_y = 0;
    }

    /// `$04:F19F`: what the cursor is over.
    fn hover(&mut self) -> Result<(), ScreenError> {
        self.screen.hover &= !0x00C7;
        let cursor = (self.screen.cursor_x, self.screen.cursor_y);
        let hit = self.place_under(cursor);
        let guarded = self.map.globals.guarded_place;
        let Some(id) = hit.filter(|&id| Some(id) != guarded) else {
            return self.hover_unit(cursor);
        };
        let planet = self.links.planet_place == Some(id);
        let station = self.links.station_place == Some(id);
        if !planet && station && self.map.globals.hold & HOLD_UNITS_PICKABLE == 0 {
            return self.hover_unit(cursor);
        }
        let place = self.map.places[usize::from(id.0)];
        self.screen.selected_x = place.x as u8;
        set_low(&mut self.screen.info.x, place.x as u8);
        self.screen.selected_y = place.y as u8;
        set_low(&mut self.screen.info.y, place.y as u8);
        set_low(&mut self.screen.info.number, place.menu_index as u8);
        set_low(&mut self.screen.info.detail, place.info as u8);
        set_low(&mut self.screen.info.kind, place.kind as u8);
        self.screen.info.shown |= 0x0001;
        if planet {
            return Ok(());
        }
        if station {
            if self.map.globals.satellite_guarding != 0 {
                self.screen.info.number = 0x000A;
            }
            return Ok(());
        }
        self.screen.hover |= HOVER_PLACE;
        self.screen.hovered_place = Some(id);
        if self.screen.hover & HOVER_ANNOUNCED == 0 {
            self.screen.hover |= HOVER_ANNOUNCED;
            self.cue(CUE_HOVER);
        }
        if place.flags & PLACE_ATTACKED != 0 {
            self.screen.hover |= HOVER_ATTACKED;
        } else if place.flags & PLACE_DEFENDED != 0 {
            self.screen.hover |= HOVER_DEFENDED;
        }
        Ok(())
    }

    /// `$04:F263`: no place; units are pickable while the map is held.
    fn hover_unit(&mut self, cursor: (u8, u8)) -> Result<(), ScreenError> {
        self.screen.hover &= !HOVER_ANNOUNCED;
        if self.map.globals.hold & HOLD_UNITS_PICKABLE == 0 {
            return Ok(());
        }
        // $04:F29A.
        let mut cursor_unit = self.map.unit_head;
        while let Some(id) = cursor_unit {
            let unit = &self.map.units[usize::from(id.0)];
            if unit.flags & UNIT_UNPICKABLE == 0 && near8((unit.x, unit.y), cursor, UNIT_REACH) {
                self.screen.hover |= HOVER_UNIT;
                self.screen.hovered_unit = Some(id);
                return Ok(());
            }
            cursor_unit = unit.next;
        }
        Ok(())
    }

    /// `$04:F2C4`: the first shown, unlisted place within reach.
    fn place_under(&self, cursor: (u8, u8)) -> Option<PlaceId> {
        let mut next = self.map.place_head;
        while let Some(id) = next {
            let place = &self.map.places[usize::from(id.0)];
            if place.flags & PLACE_LISTED == 0
                && place.flags & PLACE_SHOWN != 0
                && near8((place.x as u8, place.y as u8), cursor, PLACE_REACH)
            {
                return Some(id);
            }
            next = place.next;
        }
        None
    }

    /// `$04:B7A1`: B orders the ship; X or Y cancels.
    fn buttons(&mut self) {
        if self.links.mode & MODE_SCRIPT != 0 {
            return;
        }
        if self.pad.held & PAD_DIRECTIONS != 0 {
            self.screen.interface &= !UI_REPEAT;
        }
        let ship = self.ship_position();
        if (self.screen.cursor_x, self.screen.cursor_y) == ship {
            self.screen.interface |= UI_ON_SHIP;
        }
        if self.map.globals.hold & HOLD_INPUT != 0 {
            return;
        }
        if self.pad.pressed & PAD_B != 0
            && (self.screen.hover & (HOVER_PLACE | HOVER_UNIT) != 0 || self.screen.interface & UI_ON_SHIP == 0)
        {
            self.map.globals.hold &= !HOLD_STOP;
            if self.screen.interface & UI_REPEAT != 0 {
                self.screen.countdown = 0;
            } else {
                self.screen.ship.travel = (self.screen.ship.travel & !0x004C) | TRAVEL_SETTING_OFF | TRAVEL_UNDER_WAY;
                self.screen.interface |= UI_LAUNCHED | UI_ORDERED | UI_REPEAT;
                self.screen.countdown = COUNTDOWN_START;
                self.screen.ordered_place = self.screen.hovered_place;
                self.cue(CUE_ORDER);
                if self.screen.interface & UI_CLEARS_COUNTER != 0 {
                    self.screen.interface &= !UI_CLEARS_COUNTER;
                    self.map.globals.stored_counter = 0;
                }
            }
        }
        if self.pad.pressed & PAD_CANCEL != 0 && self.map.globals.hold & HOLD_CLOCK == 0 {
            self.stop();
            self.cue(CUE_CANCEL);
        }
        if self.map.globals.speed_flags & SPEED_TOGGLED != 0 {
            if self.screen.interface & UI_FAST != 0 {
                self.map.globals.speed_flags |= SPEED_FAST_TRAVEL;
            } else {
                self.map.globals.speed_flags &= !SPEED_FAST_TRAVEL;
            }
        }
    }

    /// `$04:B962`: the map clock, sixty frames to a step.
    fn advance_clock(&mut self) {
        let globals = &mut self.map.globals;
        globals.speed_flags &= !SPEED_STEPPED;
        if globals.hold & HOLD_CLOCK != 0 {
            return;
        }
        let step = if globals.speed_flags & SPEED_FAST_FORWARD != 0 { 4 } else { 1 };
        let screen = &mut *self.screen;
        screen.clock_frames = screen.clock_frames.wrapping_add(step);
        if (screen.clock_frames.wrapping_sub(CLOCK_FRAMES_PER_STEP) as i16) < 0 {
            return;
        }
        screen.clock_frames -= CLOCK_FRAMES_PER_STEP;
        globals.speed_flags |= SPEED_STEPPED;
        screen.elapsed_steps = if screen.elapsed_steps < ELAPSED_LIMIT { screen.elapsed_steps + 1 } else { ELAPSED_LIMIT };
    }

    fn ship_position(&self) -> (u8, u8) {
        ((self.screen.ship.x >> 8) as u8, (self.screen.ship.y >> 8) as u8)
    }

    /// `$04:D56B`: where the setting-off ship heads.
    fn choose_target(&mut self) -> Result<(), ScreenError> {
        if self.links.scene.scene_events & EVENT_ENCOUNTER != 0 || self.screen.ship.travel & TRAVEL_SETTING_OFF == 0 {
            return Ok(());
        }
        if self.links.scene.scene_events & EVENT_PURSUIT != 0 {
            let id = self.links.pursuit_unit.ok_or(ScreenError::MissingUnit(0x04D5DF))?;
            let unit = &self.map.units[usize::from(id.0)];
            self.screen.ship.target_x = unit.x;
            self.screen.ship.target_y = unit.y;
            return Ok(());
        }
        if self.screen.hover & HOVER_PLACE == 0 {
            self.screen.hover &= !(HOVER_DESTINATION | HOVER_FINAL);
            self.screen.ship.target_x = self.screen.cursor_x;
            self.screen.ship.target_y = self.screen.cursor_y;
            return Ok(());
        }
        let id = self.screen.ordered_place;
        let place = *self.place(id, 0x04D58D)?;
        self.screen.ship.target_x = place.x as u8;
        self.screen.ship.target_y = place.y as u8;
        self.screen.hover &= !(HOVER_DESTINATION | HOVER_FINAL);
        if place.flags & PLACE_DEFENDED != 0 {
            self.screen.hover |= HOVER_FINAL;
        } else if place.flags & PLACE_CLOSED == 0 {
            self.screen.ship.destination = id;
            self.screen.hover |= HOVER_DESTINATION;
        }
        Ok(())
    }

    /// `$04:D5F1`: the ship faces its target.
    pub(super) fn face_target(&mut self) {
        let travel = self.screen.ship.travel;
        let target = if travel & TRAVEL_SETTING_OFF != 0 {
            (self.screen.ship.target_x, self.screen.ship.target_y)
        } else if travel & TRAVEL_UNDER_WAY != 0 {
            return;
        } else {
            (self.screen.cursor_x, self.screen.cursor_y)
        };
        let ship = self.ship_position();
        if ship == target {
            return;
        }
        let (heading, sector) = bearing(ship, target);
        self.screen.ship.heading = heading;
        self.screen.ship.sector = sector;
        if self.screen.wingmate.flags & (WING_HELD | WING_FOLLOWING) == 0 {
            self.screen.wingmate.heading = heading;
            self.screen.wingmate.sector = sector;
        }
    }

    /// `$04:D6F1`: the ship sets off along a stepped line and travels it.
    fn travel(&mut self) -> Result<(), ScreenError> {
        let ship = &mut self.screen.ship;
        if ship.travel & TRAVEL_SETTING_OFF != 0 {
            let (along_x, along_y) = rotate(ship.speed.wrapping_neg(), (ship.heading >> 8) as u8);
            let (along_x, along_y) = (along_x.wrapping_abs_i16(), along_y.wrapping_abs_i16());
            let step = if along_x >= along_y { along_x } else { along_y };
            let span_x = u16::from(ship.target_x).wrapping_sub(ship.x >> 8);
            let span_y = u16::from(ship.target_y).wrapping_sub(ship.y >> 8);
            ship.step_x = if (span_x as i16) < 0 { step.wrapping_neg() } else { step };
            ship.step_y = if (span_y as i16) < 0 { step.wrapping_neg() } else { step };
            ship.span_x = span_x.wrapping_abs_i16() as u8;
            ship.span_y = span_y.wrapping_abs_i16() as u8;
            ship.error = 0;
            ship.travel &= !TRAVEL_SETTING_OFF;
        } else if ship.travel & TRAVEL_UNDER_WAY == 0 {
            return Ok(());
        }
        // $04:D783.
        let fast = self.map.globals.speed_flags & SPEED_FAST_TRAVEL != 0;
        let ship = &mut self.screen.ship;
        let mut line = Line {
            x: ship.x,
            y: ship.y,
            target: (ship.target_x, ship.target_y),
            step_x: if fast { ship.step_x << 3 } else { ship.step_x },
            step_y: if fast { ship.step_y << 3 } else { ship.step_y },
            span_x: u16::from(ship.span_x),
            span_y: u16::from(ship.span_y),
            error: u16::from(ship.error),
            flags: ship.travel,
        };
        line.step();
        ship.x = line.x;
        ship.y = line.y;
        ship.travel = line.flags;
        ship.error = line.error as u8;
        if line.flags & TRAVEL_ARRIVED == 0 {
            return Ok(());
        }
        self.screen.interface &= !(UI_LAUNCHED | UI_REPEAT);
        self.screen.ship.travel = TRAVEL_DOCKED;
        if self.screen.hover & HOVER_FINAL != 0 {
            // $04:D865.
            let globals = &mut self.map.globals;
            if globals.hold & HOLD_CLOCK != 0
                || self.links.scene.scene_events & EVENT_ENCOUNTER != 0
                || globals.campaign_flags != 0
            {
                return Ok(());
            }
            self.screen.hover &= !HOVER_FINAL;
            globals.hold = (globals.hold | 0x003F) & !HOLD_SHIP;
            globals.campaign_flags |= 0x0100;
            self.links.scene.campaign_events |= CAMPAIGN_FINAL;
            self.links.final_stage = 4;
            return Ok(());
        }
        if self.screen.hover & HOVER_DESTINATION == 0 {
            return Ok(());
        }
        let destination = self.screen.ship.destination;
        if self.place(destination, 0x04D819)?.flags & PLACE_SEALED != 0 {
            return Ok(());
        }
        // $04:D827: two word stores; the second carries DB13 into the
        // first pulse's low byte.
        let ship = self.screen.ship;
        self.screen.arrival_x = ship.target_x;
        self.screen.arrival_y = ship.target_y;
        self.screen.pulses[0].x = (self.screen.pulses[0].x & 0xFF_FF00) | u32::from(ship.target_pad);
        self.map.globals.encounter_request = 2;
        self.screen.hover &= !HOVER_DESTINATION;
        self.links.scene.scene_events = (self.links.scene.scene_events & !EVENT_ARRIVAL_CLEARS) | EVENT_ARRIVAL_SETS;
        self.map.globals.hold |= 0x001E;
        self.links.arrival_word = 0;
        let index = usize::from(destination.ok_or(ScreenError::MissingPlace(0x04D857))?.0);
        self.map.places[index].flags |= PLACE_VISITED;
        Ok(())
    }

    /// `$04:D8A5`: the wingmate closes on the ship.
    fn follow(&mut self) {
        let ship = self.ship_position();
        let wing = &mut self.screen.wingmate;
        wing.speed = self.screen.ship.speed;
        let point = (wing.x >> 8, wing.y >> 8);
        let centre = (u16::from(ship.0), u16::from(ship.1));
        if self.screen.ship.travel & TRAVEL_UNDER_WAY != 0 {
            wing.flags &= !WING_CLOSE;
            if near16(centre, point, WING_FAR) {
                wing.flags &= !WING_FOLLOWING;
                return;
            }
        } else {
            if wing.flags & WING_CLOSE != 0 {
                return;
            }
            wing.speed = self.screen.ship.speed << 2;
            if near16(centre, point, WING_NEAR) {
                wing.flags = (wing.flags & !WING_FOLLOWING) | WING_CLOSE;
                return;
            }
        }
        wing.flags |= WING_FOLLOWING;
        // $04:D925.
        let (heading, sector) = bearing((point.0 as u8, point.1 as u8), ship);
        wing.heading = heading;
        wing.sector = sector;
        self.step_wingmate();
    }

    /// `$04:D96A`: the wingmate moves along its course.
    pub(super) fn step_wingmate(&mut self) {
        let wing = &mut self.screen.wingmate;
        let (velocity_x, velocity_y) = rotate(wing.speed.wrapping_neg(), (wing.heading >> 8) as u8);
        wing.velocity_x = velocity_x;
        wing.velocity_y = velocity_y;
        wing.x = wing.x.wrapping_add(velocity_x);
        wing.y = wing.y.wrapping_add(velocity_y);
    }

    /// `$04:B6BB`: the planet's warnings, the travel sound, the event cues
    /// and the threat cue's repeat.
    fn warnings(&mut self) {
        let health = self.links.planet_health;
        let warning = match self.screen.planet_warnings {
            0 if health < PLANET_DAMAGED => Some(2),
            1 if health < PLANET_CRITICAL => {
                // Two word stores.
                self.screen.warning_flash[..3].copy_from_slice(&[4, 4, 0]);
                Some(3)
            }
            _ => None,
        };
        if let Some(voice) = warning {
            self.screen.planet_warnings += 1;
            self.screen.planet_warning = voice;
            self.output.radio = Some(voice);
        }
        self.screen.travel_sound = if self.screen.ship.travel & TRAVEL_UNDER_WAY == 0 {
            0
        } else if self.screen.interface & UI_FAST != 0 {
            8
        } else {
            4
        };
        let events = self.links.scene.event_word;
        if events != 0 {
            let cue = EVENT_CUES[events.trailing_zeros() as usize];
            self.cue(u16::from(cue));
        }
        self.links.scene.event_word = 0;
        if !self.map.globals.threat_cue {
            return;
        }
        if self.screen.threat_cue_timer != 0 {
            self.screen.threat_cue_timer -= 1;
            if self.screen.threat_cue_timer != 0 {
                return;
            }
        }
        let countdown = self.map.globals.threat_countdown;
        let &(_, repeat, cue) = if (countdown as i16) < 0 {
            &THREAT_CUES[THREAT_CUES.len() - 1]
        } else {
            THREAT_CUES.iter().find(|&&(least, _, _)| countdown >= least).unwrap_or(&THREAT_CUES[THREAT_CUES.len() - 1])
        };
        self.screen.threat_cue_timer = u16::from(repeat);
        self.cue(u16::from(cue));
    }

    /// `$04:D377`: while a campaign sequence holds the map (1B94), the
    /// screen drives the units it involves (by D7FC).
    fn final_services(&mut self) -> Result<(), ScreenError> {
        match self.map.globals.stored_target {
            0 => Ok(()),
            // $04:D3CB: the met unit.
            2 => {
                let id = self.map.globals.met_unit.ok_or(ScreenError::MissingUnit(0x04D3CB))?;
                self.move_unit(id)
            }
            // $04:D3D3: the places, then the fighters.
            4 => {
                let mut scene = self.links.scene;
                let terrain = self.terrain;
                let inputs = StrategicInputs { terrain, ..self.sim_inputs() };
                self.map.advance_places(&mut scene, inputs)?;
                self.links.scene = scene;
                self.each_unit(0x0020, |frame, id| frame.move_unit(id))
            }
            // $04:D3A4: the units' first pass, then the carriers' behaviors
            // and the motion of carriers and active units.
            6 => {
                let mut scene = self.links.scene;
                let terrain = self.terrain;
                let inputs = StrategicInputs { terrain, ..self.sim_inputs() };
                self.map.advance_units(&mut scene, inputs)?;
                self.links.scene = scene;
                let mut next = self.map.unit_head;
                while let Some(id) = next {
                    next = self.map.unit(id).next;
                    if self.map.unit(id).flags & 0x0010 != 0 {
                        self.behave_unit(id)?;
                    }
                    if self.map.unit(id).flags & 0x0050 != 0 {
                        self.map.move_unit(id)?;
                    }
                }
                Ok(())
            }
            // $04:D405: the satellite's escort closes on its target.
            8 => {
                let id = self.links.sortie_escort.ok_or(ScreenError::MissingUnit(0x04D405))?;
                self.move_unit(id)?;
                let (x, y) = if self.map.globals.satellite_guarding != 0 {
                    let place = *self.place(self.map.globals.guarded_place, 0x04CC5B)?;
                    (place.x as u8, place.y as u8)
                } else {
                    let target = self.map.globals.satellite_target.ok_or(ScreenError::MissingUnit(0x04CC6C))?;
                    let unit = self.map.unit(target);
                    (unit.x, unit.y)
                };
                let unit = self.map.unit_mut(id);
                unit.target_x = (unit.target_x & 0xFF00) | u16::from(x);
                unit.target_y = (unit.target_y & 0xFF00) | u16::from(y);
                let unit = *self.map.unit(id);
                if near8((unit.target_x as u8, unit.target_y as u8), (unit.x, unit.y), unit.radius as u8) {
                    self.map.globals.satellite_flags |= 0x0040;
                }
                Ok(())
            }
            // $04:D38D: the missiles.
            0x0A => self.each_unit(0x0080, |frame, id| frame.move_unit(id)),
            // $04:D3EE: the interceptors.
            0x0C => self.each_unit(0x0008, |frame, id| frame.move_unit(id)),
            other => Err(ScreenError::Unported(0x04D37E + u32::from(other))),
        }
    }

    /// The units with any of `flags`, each through `run`; the next unit is
    /// taken before the call.
    fn each_unit(
        &mut self,
        flags: u16,
        mut run: impl FnMut(&mut Self, UnitId) -> Result<(), ScreenError>,
    ) -> Result<(), ScreenError> {
        let mut next = self.map.unit_head;
        while let Some(id) = next {
            next = self.map.unit(id).next;
            if self.map.unit(id).flags & flags != 0 {
                run(self, id)?;
            }
        }
        Ok(())
    }

    /// `$7F:66A6`: one unit's behavior.
    fn behave_unit(&mut self, id: UnitId) -> Result<(), ScreenError> {
        let mut scene = self.links.scene;
        let mut tick = TickOutput::default();
        let terrain = self.terrain;
        let inputs = StrategicInputs { terrain, ..self.sim_inputs() };
        self.map.behave(id, &mut scene, inputs, &mut tick)?;
        self.links.scene = scene;
        self.output.cues.extend(tick.cues);
        Ok(())
    }

    /// `$7F:66BB`: one unit's behavior and motion.
    fn move_unit(&mut self, id: UnitId) -> Result<(), ScreenError> {
        self.behave_unit(id)?;
        self.map.move_unit(id)?;
        Ok(())
    }

    /// `$04:CC7E`: the place menu steps through the listed places.
    fn menu(&mut self) -> Result<(), ScreenError> {
        if self.screen.interface & UI_MENU == 0 {
            return Ok(());
        }
        let pressed = self.pad.pressed;
        if pressed & PAD_B != 0 {
            self.screen.interface &= !UI_MENU;
            if self.screen.menu_choice != self.screen.menu_confirm {
                return Ok(());
            }
            self.close_menu();
        } else if pressed & PAD_X != 0 {
            self.close_menu();
        } else if pressed & (PAD_RIGHT | PAD_LEFT) != 0 {
            let forward = pressed & PAD_RIGHT != 0;
            for _ in 0..MENU_CHOICES {
                let choice = self.screen.menu_choice;
                // The source's `BMI`/`BPL` on the stepped choice.
                self.screen.menu_choice = if forward {
                    let next = choice.wrapping_add(1);
                    if (next.wrapping_sub(MENU_CHOICES) as i16) < 0 { next } else { 0 }
                } else {
                    let next = choice.wrapping_sub(1);
                    if (next as i16) >= 0 { next } else { MENU_CHOICES - 1 }
                };
                if let Some(id) = self.menu_place() {
                    self.screen.menu_place = Some(id);
                    self.cue(CUE_HOVER);
                    return self.show_menu_place();
                }
            }
            return Err(ScreenError::EmptyMenu);
        }
        Ok(())
    }

    /// `$04:CCEF`.
    fn close_menu(&mut self) {
        self.screen.interface &= !UI_MENU;
        self.screen.hover &= !HOVER_PLACE;
        self.screen.menu_choice = self.screen.menu_home;
        self.screen.interface |= UI_MENU_CLOSED;
    }

    /// `$04:CD31`: the menu's place in the info panel; two word stores, the
    /// second over the first's high byte.
    pub(super) fn show_menu_place(&mut self) -> Result<(), ScreenError> {
        let place = *self.place(self.screen.menu_place, 0x04CD31)?;
        self.screen.info.number = place.menu_index;
        self.screen.selected_x = place.x as u8;
        self.screen.selected_y = place.y as u8;
        self.screen.arrival_x = (place.y >> 8) as u8;
        Ok(())
    }

    /// `$04:CD09`: the listed place at the menu's choice.
    pub(super) fn menu_place(&self) -> Option<PlaceId> {
        let mut next = self.map.place_head;
        while let Some(id) = next {
            let place = &self.map.places[usize::from(id.0)];
            if place.flags & PLACE_MENU_HIDDEN == 0
                && place.flags & PLACE_LISTED != 0
                && place.menu_index == self.screen.menu_choice
            {
                return Some(id);
            }
            next = place.next;
        }
        None
    }
}

/// `$04:DA02`'s line stepper state.
struct Line {
    x: u16,
    y: u16,
    target: (u8, u8),
    step_x: u16,
    step_y: u16,
    span_x: u16,
    span_y: u16,
    error: u16,
    flags: u16,
}

impl Line {
    /// `$04:DA02`: one step along the major axis, the minor axis by the
    /// error term; each axis stops once its whole coordinate is reached.
    fn step(&mut self) {
        let (target_x, target_y) = self.target;
        if (self.x >> 8) as u8 == target_x && (self.y >> 8) as u8 == target_y {
            self.flags |= TRAVEL_ARRIVED;
            return;
        }
        if self.span_x == 0 {
            self.y = self.y.wrapping_add(self.step_y);
        } else if self.span_y == 0 {
            self.x = self.x.wrapping_add(self.step_x);
        } else if (self.span_x.wrapping_sub(self.span_y) as i16) >= 0 {
            if self.flags & TRAVEL_X_DONE == 0 {
                self.x = self.x.wrapping_add(self.step_x);
            }
            if self.flags & TRAVEL_Y_DONE == 0 {
                let error = self.error.wrapping_add(self.span_y);
                self.error = if (error.wrapping_sub(self.span_x) as i16) < 0 {
                    error
                } else {
                    self.y = self.y.wrapping_add(self.step_y);
                    error.wrapping_sub(self.span_x)
                };
            }
        } else {
            if self.flags & TRAVEL_Y_DONE == 0 {
                self.y = self.y.wrapping_add(self.step_y);
            }
            if self.flags & TRAVEL_X_DONE == 0 {
                let error = self.error.wrapping_add(self.span_x);
                self.error = if (error.wrapping_sub(self.span_y) as i16) < 0 {
                    error
                } else {
                    self.x = self.x.wrapping_add(self.step_x);
                    error.wrapping_sub(self.span_y)
                };
            }
        }
        if (self.x >> 8) as u8 == target_x {
            self.flags |= TRAVEL_X_DONE;
        }
        if (self.y >> 8) as u8 == target_y {
            self.flags |= TRAVEL_Y_DONE;
        }
        if self.flags & (TRAVEL_X_DONE | TRAVEL_Y_DONE) == TRAVEL_X_DONE | TRAVEL_Y_DONE {
            self.flags |= TRAVEL_ARRIVED;
        }
    }
}

trait WrappingAbs {
    fn wrapping_abs_i16(self) -> u16;
}

impl WrappingAbs for u16 {
    fn wrapping_abs_i16(self) -> u16 {
        if (self as i16) < 0 { self.wrapping_neg() } else { self }
    }
}

fn set_high(word: &mut u16, byte: u8) {
    *word = (*word & 0x00FF) | (u16::from(byte) << 8);
}

fn set_low(word: &mut u16, byte: u8) {
    *word = (*word & 0xFF00) | u16::from(byte);
}

/// `$04:F16A`: the cursor moves by its velocities' high bytes, kept on
/// the map.
fn step_cursor(screen: &mut MapScreen) {
    let clamp = |value: u8, (low, high): (u8, u8)| {
        if value < low {
            low
        } else if value >= high {
            high
        } else {
            value
        }
    };
    screen.cursor_x = clamp(screen.cursor_x.wrapping_add((screen.cursor_velocity_x >> 8) as u8), CURSOR_X_RANGE);
    screen.cursor_y = clamp(screen.cursor_y.wrapping_add((screen.cursor_velocity_y >> 8) as u8), CURSOR_Y_RANGE);
}

/// `$04:F2FB` with byte operands: `point` within `reach` of `centre`,
/// the sums wrapping as bytes.
pub(super) fn near8(centre: (u8, u8), point: (u8, u8), reach: u8) -> bool {
    let axis = |c: u8, p: u8| c.wrapping_add(reach) >= p && c.wrapping_sub(reach) < p;
    axis(centre.0, point.0) && axis(centre.1, point.1)
}

/// `$04:F2FB` with word operands.
fn near16(centre: (u16, u16), point: (u16, u16), reach: u16) -> bool {
    let axis = |c: u16, p: u16| c.wrapping_add(reach) >= p && c.wrapping_sub(reach) < p;
    axis(centre.0, point.0) && axis(centre.1, point.1)
}

/// `$04:D66B`: the course from `from` to `to`, as `$7F:6A46` measures it
/// (rounded up at the half), and its sixteenth.
pub(super) fn bearing(from: (u8, u8), to: (u8, u8)) -> (u16, u16) {
    let dx = u16::from(from.0).wrapping_sub(u16::from(to.0));
    let dy = u16::from(from.1).wrapping_sub(u16::from(to.1));
    let mut heading = atan16(dx as i16, dy as i16);
    if heading & 0x0080 != 0 {
        heading = heading.wrapping_add(1);
    }
    let sector = (((heading >> 8) as u8).wrapping_add(8) >> 4) as u16;
    (heading, sector)
}

/// `$7F:69C7` as the screen reads it: the middle words of
/// `(-value * cos) >> 2` and `(value * sin) >> 2`.
pub(super) fn rotate(value: u16, angle: u8) -> (u16, u16) {
    let index = angle.wrapping_add(0x40);
    let first = i32::from(SINTAB[usize::from(index.wrapping_add(0x40))]);
    let second = i32::from(SINTAB[usize::from(index)]);
    let negated = i32::from((value as i16).wrapping_neg());
    let x = ((negated * first) as u32) >> 2;
    let y = ((i32::from(value as i16) * second) as u32) >> 2;
    ((x >> 8) as u16, (y >> 8) as u16)
}

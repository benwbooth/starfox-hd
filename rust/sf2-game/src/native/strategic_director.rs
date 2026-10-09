//! The map program's frame (`$04:B9A3`), run on the map's main thread each
//! frame between the screen frames: the campaign's directors that start
//! the map scripts and hand encounters to the mission launch.
//!
//! - the stage-return aftermath (DA0D, `$04:BB5F` table): the ship's
//!   return, the met unit's fate, the pilot dialogs it waits for;
//! - the campaign timeline (D9FD, `$04:C0F6` table): day-scheduled events
//!   (`$04:EF74`), the missile salvos, the waves, the attacks on the bases
//!   and the planet's final defence;
//! - the planet threat warnings (DA69, `$04:C9C7`), the satellite's sortie
//!   (E089, `$04:CB47`), the base recapture (DA15), the ambush (DA17), the
//!   map clearing (DA19), the exit request (DA21) and the final-stage
//!   choice (DB29, `$04:B9F5`).
//!
//! D7FA is the handshake word the bank-06 pilot dialogs answer; the
//! director only sets and tests its bits.

use super::strategic_screen::{Frame, MapPad, MapScreen, ScreenError, ScreenLinks, ScreenOutput};
use super::strategic_sim::{
    byte_at, PatternEntry, PlaceId, StrategicMap, UnitId, CARRIER_PATTERN, CARRIER_STRENGTH, CARRIER_WORD,
};

/// The director's own words.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapDirector {
    /// DA0D/DA0F/DA1D/DA13: the aftermath state, its continuation, the
    /// dialog hold bits and the message wait.
    pub aftermath: u16,
    pub aftermath_next: u16,
    pub dialog_hold: u16,
    pub message_wait: u16,
    /// DA21: the exit request's state.
    pub exit_state: u16,
    /// D9FF/DA05: the timeline's continuation and delay (its state, D9FD,
    /// is in the screen links); DA01/DA03 the schedule cursor and steps.
    pub timeline_next: u16,
    pub timeline_delay: u16,
    pub schedule_cursor: u16,
    pub schedule_steps: u16,
    /// D7FA: the dialog handshake.
    pub handshake: u16,
    /// DA15, DA17, DA19: the recapture, ambush and clearing states;
    /// DA07/DA0B the alert sub-state and its timer.
    pub recapture: u16,
    pub ambush: u16,
    pub clearing: u16,
    pub alert: u16,
    pub alert_timer: u16,
    /// DA6D: the threat warning's delay.
    pub threat_delay: u16,
    /// DB37: alerts collected for F55C.
    pub pending_alerts: u16,
    /// 1B8E: the map holds kept across a sequence.
    pub saved_hold: u16,
    /// D9C2/D9C4: the final-stage choice and whether it is open.
    pub choice: u16,
    pub choosing: u16,
    /// E08B: the satellite sortie's timer (its escort, E08D, is in the
    /// screen links).
    pub sortie_timer: u16,
    /// D9B2: the missile salvo's remaining index.
    pub salvo_index: u16,
    pub tally: MapTally,
}

/// The campaign's progress counters.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapTally {
    /// DA25: the goal; DA29 enemies left; DA2D marked places left.
    pub goal: u16,
    pub enemies_left: u16,
    pub marks_left: u16,
    /// DA33, DA37, DA3D, DA49, DA4F: cleared marks, fleets, fleets
    /// destroyed and the other tallies the goal sums.
    pub marks_cleared: u16,
    pub fleets: u16,
    pub fleets_cleared: u16,
    pub other_cleared: u16,
    pub bases_cleared: u16,
    /// DA4B and DA51: the other units and the bases a stage has cleared,
    /// counted with DA49 and DA4F.
    pub other_total: u16,
    pub bases_total: u16,
}

// D7FA bits.
const HS_TIMELINE: u16 = 0x0001;
const HS_BUSY: u16 = 0x0002;
const HS_THREATS: u16 = 0x0004;
const HS_THREAT_SHOWN: u16 = 0x0008;
const HS_CLEARING: u16 = 0x0010;
const HS_SHUTDOWN: u16 = 0x0020;
const HS_AMBUSH: u16 = 0x0040;
const HS_RECAPTURE: u16 = 0x0100;
const HS_RECAPTURING: u16 = 0x0200;
const HS_SORTIE: u16 = 0x0400;
const HS_SORTIE_RUNNING: u16 = 0x0800;
const HS_DIALOGS: u16 = 0x0555;
// D7F8 bits.
const SF_TIMELINE_WAIT: u16 = 0x0001;
const SF_CLEAR_REQUIRED: u16 = 0x0002;
const SF_AFTERMATH: u16 = 0x0004;
const SF_RETURNED: u16 = 0x0008;
const SF_RECAPTURE: u16 = 0x0010;
const SF_AMBUSH: u16 = 0x0020;
const SF_FINAL: u16 = 0x0080;
const SF_BASES: u16 = 0x0100;
const SF_BASES_FALLEN: u16 = 0x0200;
const SF_PLANET_LOST: u16 = 0x0400;

const HOLD_ALL: u16 = 0x003F;

/// `$04:EF74`: the campaign schedule: (day, handler) word pairs in three
/// lists (DA01 starts at a difficulty's list: bytes 00, 1A or 50), each
/// ended by day FFFF; day FFFE runs once the map is clear.
const SCHEDULE: [u8; 142] = [
    0x00, 0x00, 0x9C, 0xC2, 0x14, 0x00, 0xCE, 0xC1, 0x19, 0x00, 0x2B, 0xC2, 0x23, 0x00, 0xCE, 0xC1, 0x28, 0x00,
    0x04, 0xC2, 0xFE, 0xFF, 0x76, 0xC2, 0xFF, 0xFF, 0x00, 0x00, 0x9C, 0xC2, 0x26, 0x00, 0xB3, 0xC1, 0x2A, 0x00,
    0xBC, 0xC1, 0x2D, 0x00, 0xDA, 0xC1, 0x32, 0x00, 0xCE, 0xC1, 0x3C, 0x00, 0x04, 0xC2, 0x55, 0x00, 0x44, 0xC2,
    0x5F, 0x00, 0xCE, 0xC1, 0x6E, 0x00, 0x2B, 0xC2, 0x78, 0x00, 0x04, 0xC2, 0x82, 0x00, 0xCE, 0xC1, 0xBE, 0x00,
    0xDA, 0xC1, 0xFE, 0xFF, 0x76, 0xC2, 0xFF, 0xFF, 0x00, 0x00, 0x9C, 0xC2, 0x23, 0x00, 0xBC, 0xC1, 0x31, 0x00,
    0x04, 0xC2, 0x34, 0x00, 0xB3, 0xC1, 0x3C, 0x00, 0x5D, 0xC2, 0x5A, 0x00, 0x44, 0xC2, 0x5F, 0x00, 0xCE, 0xC1,
    0x6E, 0x00, 0x2B, 0xC2, 0x73, 0x00, 0xCE, 0xC1, 0x78, 0x00, 0x04, 0xC2, 0x91, 0x00, 0xCE, 0xC1, 0xA5, 0x00,
    0xDA, 0xC1, 0xEB, 0x00, 0xDA, 0xC1, 0x1D, 0x01, 0xDA, 0xC1, 0xFE, 0xFF, 0x76, 0xC2, 0xFF, 0xFF,
];
/// `$04:C32B`: the satellite alert bit by 1BA3.
const SATELLITE_ALERTS: [u8; 6] = [0x01, 0x00, 0x02, 0x00, 0x04, 0x00];
/// `$04:C36A`: the missile kind offsets by random pick.
const MISSILE_KIND_OFFSETS: [u8; 5] = [0x00, 0x01, 0x03, 0x02, 0x00];
/// `$04:E88A`: the salvo's target columns.
const SALVO_COLUMNS: [u8; 4] = [0x40, 0x70, 0xA0, 0xD0];
/// `$04:D533..D56A`: the order pulses' starts (x/y offsets from the ship),
/// steps (low words, high bytes) and flags, four of each.
const PULSE_STARTS: [u8; 0x38] = [
    0xD8, 0xFF, 0x28, 0x00, 0xD8, 0xFF, 0x28, 0x00, 0xD8, 0xFF, 0xD8, 0xFF, 0x28, 0x00, 0x28, 0x00, 0x70, 0x02,
    0x90, 0xFD, 0x70, 0x02, 0x90, 0xFD, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFF, 0x70, 0x02, 0x70, 0x02,
    0x90, 0xFD, 0x90, 0xFD, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x2C, 0x3A, 0x2C, 0x7A, 0x2C, 0xBA,
    0x2C, 0xFA,
];

// Dialog messages (F582) and text box words.
const MESSAGE_WARP: u16 = 0xA6CA;
const MESSAGE_SHIELDS_LOW: u16 = 0xA6E3;
const MESSAGE_SHIELDS_HIGH: u16 = 0xA6EA;
const MESSAGE_FINAL: u16 = 0xA712;

fn word_of(table: &[u8], index: usize, site: u32) -> Result<u16, ScreenError> {
    let low = byte_at(table, index, site).map_err(|_| ScreenError::TableOverrun(site))?;
    let high = byte_at(table, index + 1, site).map_err(|_| ScreenError::TableOverrun(site))?;
    Ok(u16::from(low) | (u16::from(high) << 8))
}

/// `$04:B9A3`: one map program frame.
pub fn step(
    director: &mut MapDirector,
    screen: &mut MapScreen,
    map: &mut StrategicMap,
    links: &mut ScreenLinks,
    terrain: &[u8],
    output: &mut ScreenOutput,
) -> Result<(), ScreenError> {
    let frame = Frame { screen, map, links, terrain, pad: MapPad::default(), output };
    let mut step = Step { frame, d: director };
    step.run()
}

struct Step<'a, 'b> {
    frame: Frame<'a>,
    d: &'b mut MapDirector,
}

impl Step<'_, '_> {
    fn hold(&mut self) -> &mut u16 {
        &mut self.frame.map.globals.hold
    }

    fn cue(&mut self, cue: u16) {
        self.frame.cue(cue);
    }

    fn flags(&self) -> u16 {
        self.frame.links.stage_flags
    }

    fn campaign(&self) -> u16 {
        self.frame.map.globals.campaign_flags
    }

    fn met_unit(&self, site: u32) -> Result<UnitId, ScreenError> {
        self.frame.map.globals.met_unit.ok_or(ScreenError::MissingUnit(site))
    }

    fn run(&mut self) -> Result<(), ScreenError> {
        let links = &mut *self.frame.links;
        if links.presentation_countdown != 0 {
            links.presentation_countdown -= 1;
            if links.presentation_countdown != 0 {
                return Ok(());
            }
            links.presentation_flags &= !0x0002;
        }
        self.exit_request()?;
        if self.frame.links.scene.scene_events & 0x0120 == 0 {
            self.aftermath()?;
            if self.frame.links.scene.campaign_events & 0x0008 == 0 {
                self.threats()?;
                self.sortie()?;
                self.clearing()?;
                self.timeline()?;
                self.recapture()?;
                self.ambush()?;
            }
            self.final_stage()?;
        }
        self.frame.screen.ship.travel &= !0x0100;
        Ok(())
    }

    /// `$04:DC80`.
    fn exit_request(&mut self) -> Result<(), ScreenError> {
        match self.d.exit_state {
            0 => {}
            2 => {
                *self.hold() |= HOLD_ALL;
                self.d.dialog_hold |= 0x0002;
                self.frame.map.globals.map_events |= 0x0002;
                self.d.exit_state = 4;
            }
            4 => {
                if self.d.dialog_hold & 0x0002 == 0 {
                    self.frame.map.globals.encounter_request = 3;
                    self.d.exit_state = 0;
                }
            }
            other => return Err(ScreenError::Unported(0x04DC87 + u32::from(other))),
        }
        Ok(())
    }

    // ---- the aftermath (DA0D) ----

    /// `$04:BCCF`: the sequence ends; wait for the dialogs, then 3E.
    fn aftermath_done(&mut self) {
        self.frame.screen.ship.travel &= !0x0400;
        *self.hold() |= 0x0004;
        self.d.aftermath = 0x68;
        self.d.aftermath_next = 0x3E;
        self.d.dialog_hold = 0x0001;
    }

    fn wait_script(&self) -> bool {
        self.frame.screen.script != 0
    }

    fn aftermath(&mut self) -> Result<(), ScreenError> {
        match self.d.aftermath {
            0x00..=0x08 => {}
            0x0A => {
                self.frame.screen.ship.speed = 0x0800;
                self.frame.map.globals.stored_counter = 3;
                self.d.aftermath = 0x0C;
            }
            0x0C => {
                let counter = &mut self.frame.map.globals.stored_counter;
                *counter = counter.wrapping_sub(1);
                if *counter == 0 {
                    self.d.aftermath = 0x0E;
                }
            }
            0x0E => {
                *self.hold() &= !0x0004;
                let screen = &mut *self.frame.screen;
                screen.ship.travel = (screen.ship.travel & !0x0200) | 0x0003;
                self.d.aftermath = 0x10;
                let [x, y, _] = screen.warp_target;
                set_high(&mut screen.ship.x, x);
                set_high(&mut screen.wingmate.x, x);
                set_high(&mut screen.ship.y, y);
                set_high(&mut screen.wingmate.y, y);
                screen.cursor_x = 0x28;
                screen.cursor_y = 0x90;
                screen.ship.heading = 0xD800;
                screen.wingmate.heading = 0xD800;
                screen.ship.sector = 0x000D;
                screen.wingmate.sector = 0x000D;
            }
            0x10 => {
                if self.frame.screen.ship.travel & 0x0100 != 0 {
                    self.d.aftermath = 0x12;
                    self.returned()?;
                }
            }
            0x12 => self.returned()?,
            0x14 => self.start_sequence(0x16, 0x12, true),
            0x16 | 0x1A => {
                if !self.wait_script() {
                    self.aftermath_done();
                }
            }
            0x18 => {
                self.d.aftermath = 0x1A;
                self.frame.screen.script = 4;
            }
            0x1C => self.start_sequence(0x1E, 0x10, true),
            0x1E => {
                if self.wait_script() {
                    return Ok(());
                }
                let unit = self.met_unit(0x04BD0D)?;
                let home = self.frame.map.unit(unit).home.ok_or(ScreenError::MissingPlace(0x04BD10))?;
                self.clear_fleet(home, unit)?;
                self.frame.links.scene.scene_events &= !0x0020;
                self.aftermath_done();
            }
            0x20 => {
                self.d.aftermath = 0x22;
                self.frame.screen.script = 4;
            }
            0x22 => {
                if self.wait_script() {
                    return Ok(());
                }
                let unit = self.met_unit(0x04BD32)?;
                self.frame.map.unit_mut(unit).flags &= !0x0001;
                self.aftermath_done();
            }
            0x24 => {
                self.d.aftermath = 0x26;
                let unit = self.met_unit(0x04BD47)?;
                if self.frame.map.unit(unit).flags & 0x0008 == 0 {
                    self.frame.screen.script = 0x0A;
                } else {
                    self.frame.map.globals.intercepted_unit = Some(unit);
                    self.frame.screen.script = 0x14;
                }
            }
            0x26 => {
                if self.wait_script() {
                    return Ok(());
                }
                let globals = &mut self.frame.map.globals;
                if globals.satellite_flags & 0x0800 == 0 {
                    return self.resolve_interception();
                }
                globals.satellite_guarding = 0;
                globals.satellite_flags &= !0x0C00;
                let satellite = globals.satellite.ok_or(ScreenError::MissingUnit(0x04BD7D))?;
                self.frame.map.unit_mut(satellite).flags2 &= 0xDFFF;
                self.aftermath_done();
            }
            0x28 => {
                let won = self.frame.links.scene.campaign_events & 0x0020 != 0
                    && (self.frame.links.mission_result as i16) >= 0
                    && self.frame.links.mission_result != 1
                    && self.frame.links.mission_rank < 2;
                if won {
                    self.frame.screen.script = 0x0A;
                } else {
                    let unit = self.met_unit(0x04BDAE)?;
                    self.frame.map.unit_mut(unit).flags &= !0x0001;
                    self.frame.screen.script = 0x0C;
                }
                self.d.aftermath = 0x2A;
            }
            0x2A | 0x30 => {
                if !self.wait_script() {
                    self.resolve_interception()?;
                }
            }
            0x2C => {
                self.frame.screen.script = 4;
                self.d.aftermath = 0x2E;
            }
            0x2E => {
                if self.wait_script() {
                    return Ok(());
                }
                let unit = self.met_unit(0x04BDE3)?;
                self.frame.map.unit_mut(unit).flags &= !0x0001;
                if self.frame.links.scene.campaign_events & 0x0020 == 0 {
                    self.aftermath_done();
                    return Ok(());
                }
                let rank = self.frame.links.mission_rank;
                if rank != 0 && (rank >= 2 || (self.frame.links.mission_result as i16) < 0) {
                    return self.resolve_interception();
                }
                self.frame.screen.script = 0x0A;
                self.d.aftermath = 0x30;
            }
            0x32 => {
                self.frame.map.globals.campaign_flags |= 0x0001;
                self.frame.map.globals.stored_target = 2;
                self.frame.screen.script = 8;
                self.d.aftermath = 0x34;
            }
            0x34 => {
                if self.wait_script() {
                    return Ok(());
                }
                self.frame.map.globals.campaign_flags &= !0x0001;
                self.resolve_interception()?;
            }
            0x36 => self.resolve_interception()?,
            0x38 => {
                if !self.wait_script() {
                    self.aftermath_done();
                }
            }
            0x3A => {
                self.d.aftermath = 0x68;
                self.d.aftermath_next = 0x3C;
                self.d.dialog_hold = 0x0001;
            }
            0x3C => {
                *self.hold() &= !0x0003;
                self.frame.links.scene.scene_events &= !0x0040;
                self.d.aftermath = 0;
            }
            0x3E => {
                let speed = self.frame.map.globals.speed_flags;
                if speed & 0x4000 != 0 {
                    self.d.pending_alerts |= 0x0200;
                } else if speed & 0x8000 != 0 {
                    self.d.pending_alerts |= 0x0800;
                } else {
                    return self.dialog_round();
                }
                self.post_alerts();
                self.d.aftermath = 0x40;
                self.d.message_wait = 1;
            }
            0x40 => {
                if self.d.message_wait == 0 {
                    self.dialog_round()?;
                }
            }
            0x42 => self.dialog_round()?,
            0x44 => {
                if self.d.handshake & HS_BUSY == 0 {
                    self.dialog(0x0004, 0x52);
                }
            }
            0x46 => {
                if self.d.handshake & HS_BUSY == 0 {
                    self.order_pause()?;
                }
            }
            0x48 => self.dialog_with_final()?,
            0x4A => {
                if self.d.handshake & 0x0080 == 0 {
                    self.dialog(0x0100, 0x56);
                }
            }
            0x4C => self.dialog(0x0010, 0x4E),
            0x4E => {
                if self.d.handshake & 0x0020 == 0 {
                    self.dialog_with_final()?;
                }
            }
            0x50 => self.dialog(0x0004, 0x52),
            0x52 => {
                if self.d.handshake & 0x0008 == 0 {
                    self.dialog(0x0400, 0x5A);
                }
            }
            0x54 => self.dialog(0x0100, 0x56),
            0x56 => {
                if self.d.handshake & 0x0200 != 0 {
                    return Ok(());
                }
                if self.flags() & SF_PLANET_LOST != 0 {
                    self.frame.links.stage_flags &= !SF_PLANET_LOST;
                    self.d.handshake |= HS_TIMELINE;
                    self.d.aftermath = 0x46;
                    return Ok(());
                }
                self.order_pause()?;
            }
            0x58 => self.dialog(0x0400, 0x5A),
            0x5A => {
                if self.d.handshake & HS_SORTIE_RUNNING == 0 {
                    self.dialog(0x0010, 0x4E);
                }
            }
            0x5C => self.order()?,
            0x5E => self.ordered()?,
            0x60 => {
                if self.d.message_wait == 0 {
                    self.order()?;
                }
            }
            0x62 => {
                if self.d.message_wait == 0 {
                    self.release();
                }
            }
            0x64 => {
                if self.frame.map.globals.stored_counter == 0 {
                    self.d.handshake |= 0x0515;
                    self.frame.links.stage_flags |= SF_RETURNED;
                    self.d.aftermath = 0x66;
                    self.frame.map.globals.stored_counter = 3;
                    *self.hold() &= !0x0013;
                }
            }
            0x66 => {
                let counter = &mut self.frame.map.globals.stored_counter;
                *counter = counter.wrapping_sub(1);
                if *counter == 0 {
                    self.d.aftermath = 0;
                    *self.hold() &= !0x0020;
                    self.frame.links.scene.scene_events &= !0x0040;
                    self.frame.links.scene.campaign_events |= 0x0001;
                }
            }
            0x68 => {
                if self.d.dialog_hold & 0x0001 == 0 {
                    self.d.aftermath = self.d.aftermath_next;
                    self.frame.links.scene.campaign_events &= !0x0200;
                }
            }
            other => return Err(ScreenError::Unported(0x04BB5F + u32::from(other))),
        }
        Ok(())
    }

    /// `$04:BC77`/`$04:BCEE`: hold the ship and start a script.
    fn start_sequence(&mut self, next: u16, script: u16, hold_ship: bool) {
        *self.hold() &= !0x0004;
        if hold_ship {
            self.frame.screen.ship.travel |= 0x0400;
        }
        self.d.aftermath = next;
        self.frame.screen.script = script;
    }

    /// `$04:BC47`: the ship is back on the map once the wingmate is.
    fn returned(&mut self) -> Result<(), ScreenError> {
        if self.frame.screen.wingmate.flags & 0x0002 != 0 {
            return Ok(());
        }
        // $04:BC5C.
        let mut next = self.frame.map.place_head;
        while let Some(id) = next {
            let place = &mut self.frame.map.places[usize::from(id.0)];
            if place.flags & 0x0001 != 0 {
                place.flags &= 0xFDFF;
            }
            next = place.next;
        }
        self.frame.screen.ship.speed = 0x0258;
        // $04:C034.
        if self.frame.links.difficulty != 0 {
            return self.order();
        }
        self.frame.links.message = MESSAGE_WARP;
        self.d.message_wait = 1;
        self.d.aftermath = 0x60;
        Ok(())
    }

    /// `$04:BE46`: the interception's outcome restores the intercepted unit.
    fn resolve_interception(&mut self) -> Result<(), ScreenError> {
        if self.frame.links.scene.campaign_events & 0x0020 == 0 {
            self.aftermath_done();
            return Ok(());
        }
        self.frame.links.scene.campaign_events &= !0x0020;
        let result = self.frame.links.mission_result;
        if (result as i16) < 0 {
            self.d.aftermath = 0x38;
            self.frame.screen.script = 0x14;
            return Ok(());
        }
        let globals = self.frame.map.globals;
        let unit = globals.intercepted_unit.ok_or(ScreenError::MissingUnit(0x04BE5C))?;
        let record = self.frame.map.unit_mut(unit);
        record.motion = globals.intercepted_motion;
        record.behavior = globals.intercepted_behavior;
        if result == 1 || self.frame.links.mission_extra == 0 {
            self.aftermath_done();
            return Ok(());
        }
        self.d.aftermath = 0x38;
        self.frame.screen.script = 0x0E;
        Ok(())
    }

    /// `$04:BED1`: the dialogs after a return start.
    fn dialog_round(&mut self) -> Result<(), ScreenError> {
        self.d.handshake |= HS_TIMELINE;
        self.frame.links.stage_flags &= !SF_RETURNED;
        self.d.aftermath = 0x44;
        Ok(())
    }

    /// `$04:BEED` and its siblings: one pilot dialog request.
    fn dialog(&mut self, request: u16, next: u16) {
        self.d.handshake = (self.d.handshake & !HS_DIALOGS) | request;
        self.d.aftermath = next;
    }

    /// `$04:BF41`: the dialog that also raises the final one when due.
    fn dialog_with_final(&mut self) -> Result<(), ScreenError> {
        self.dialog(0x0040, 0x4A);
        if self.flags() & SF_AMBUSH != 0 {
            self.d.handshake |= 0x0080;
        }
        if self.d.handshake & 0x0080 == 0 {
            self.dialog(0x0100, 0x56);
        }
        Ok(())
    }

    /// `$04:BFAA`: release the guards, rescan the threats and order.
    fn order_pause(&mut self) -> Result<(), ScreenError> {
        self.release_guard()?;
        self.rescan_threats()?;
        self.order()
    }

    /// `$04:BFB1`: the order prompt with its pulses.
    fn order(&mut self) -> Result<(), ScreenError> {
        self.d.handshake &= !HS_DIALOGS;
        self.frame.links.stage_flags |= SF_CLEAR_REQUIRED;
        self.frame.screen.interface |= 0x0100;
        self.d.aftermath = 0x5E;
        self.frame.map.globals.stored_counter = 1;
        self.frame.screen.countdown = 0x10;
        self.start_pulses()?;
        self.cue(0x0B);
        Ok(())
    }

    /// `$04:BFDF`.
    fn ordered(&mut self) -> Result<(), ScreenError> {
        if self.frame.map.globals.stored_counter != 0 {
            return Ok(());
        }
        *self.hold() = (*self.hold() & !HOLD_ALL) | 0x0033;
        self.frame.screen.interface &= !0x0100;
        self.frame.map.globals.campaign_flags &= !0x0100;
        let screen = &mut *self.frame.screen;
        screen.cursor_x = (screen.ship.x >> 8) as u8;
        screen.cursor_y = (screen.ship.y >> 8) as u8;
        if self.frame.links.difficulty != 0 {
            self.release();
            return Ok(());
        }
        let [current, full, _, _] = self.frame.links.pilot_shields;
        self.frame.links.message = if full >> 1 <= current { MESSAGE_SHIELDS_LOW } else { MESSAGE_SHIELDS_HIGH };
        self.d.aftermath = 0x62;
        self.d.message_wait = 1;
        Ok(())
    }

    /// `$04:C06B`: the map is released to the player.
    fn release(&mut self) {
        self.frame.links.mode &= !0x0010;
        *self.hold() &= !0x0010;
        self.frame.links.scene.campaign_events &= !0x0008;
        self.d.aftermath = 0x64;
        self.frame.screen.interface |= 0x0200;
        self.frame.map.globals.stored_counter = 1;
    }

    /// `$04:D4D8`: the order pulses start around the ship.
    fn start_pulses(&mut self) -> Result<(), ScreenError> {
        let ship_x = (self.frame.screen.ship.x >> 8) as u16;
        let ship_y = (self.frame.screen.ship.y >> 8) as u16;
        for k in 0..4 {
            let at = 2 * k;
            let word = |base: usize| word_of(&PULSE_STARTS, base + at, 0x04D4DE);
            let x = ship_x.wrapping_add(word(0x00)?);
            let y = ship_y.wrapping_add(word(0x08)?);
            let dx = word(0x10)?;
            let dx_high = byte_at(&PULSE_STARTS, 0x18 + at, 0x04D503).map_err(|_| ScreenError::TableOverrun(0x04D503))?;
            let dy = word(0x20)?;
            let dy_high = byte_at(&PULSE_STARTS, 0x28 + at, 0x04D515).map_err(|_| ScreenError::TableOverrun(0x04D515))?;
            let flags = word(0x30)?;
            let pulse = &mut self.frame.screen.pulses[k];
            pulse.x = (pulse.x & 0x0000FF) | (u32::from(x) << 8);
            pulse.y = (pulse.y & 0x0000FF) | (u32::from(y) << 8);
            pulse.dx = u32::from(dx) | (u32::from(dx_high) << 16);
            pulse.dy = u32::from(dy) | (u32::from(dy_high) << 16);
            pulse.flags = flags;
        }
        Ok(())
    }

    /// `$04:B4B0`: a fleet's place and unit leave the map.
    fn clear_fleet(&mut self, place: PlaceId, unit: UnitId) -> Result<(), ScreenError> {
        let globals = &mut self.frame.map.globals;
        if globals.threat_place == Some(place) {
            globals.threat_kind = 0;
            globals.threat_place = None;
            self.frame.links.scene.scene_events &= !0x0080;
        }
        self.free_place(place);
        // $04:E082.
        self.frame.map.free_unit(unit);
        let globals = &mut self.frame.map.globals;
        globals.unit_count = globals.unit_count.wrapping_sub(1);
        let bonus = &mut self.frame.links.batch_bonus;
        *bonus = bonus.wrapping_sub(1);
        let tally = &mut self.d.tally;
        tally.fleets = tally.fleets.wrapping_sub(1);
        tally.enemies_left = tally.enemies_left.wrapping_sub(1);
        tally.fleets_cleared = tally.fleets_cleared.wrapping_add(1);
        self.last_enemy();
        Ok(())
    }

    /// `$04:B42C`: a marked place is cleared.
    fn clear_mark(&mut self, id: PlaceId) {
        let place = &mut self.frame.map.places[usize::from(id.0)];
        place.flags |= 0x1000;
        place.marker = 0;
        if place.state != 0 {
            place.flags &= 0xF7FF;
            place.state = 1;
        }
        place.info = 0x000F;
        self.frame.screen.campaign.markers_placed = self.frame.screen.campaign.markers_placed.wrapping_sub(1);
        let tally = &mut self.d.tally;
        tally.marks_left = tally.marks_left.wrapping_sub(1);
        tally.enemies_left = tally.enemies_left.wrapping_sub(1);
        tally.marks_cleared = tally.marks_cleared.wrapping_add(1);
        self.last_enemy();
    }

    /// `$04:B465`.
    fn last_enemy(&mut self) {
        if self.d.tally.enemies_left == 1 {
            self.frame.map.globals.speed_flags |= 0x4000;
        }
    }

    /// `$04:E093`: unlink a place and push it on the free list.
    fn free_place(&mut self, id: PlaceId) {
        let map = &mut *self.frame.map;
        let index = usize::from(id.0);
        map.places[index].flags = 0;
        map.places[index].status = 0;
        let (prev, next) = (map.places[index].prev, map.places[index].next);
        match prev {
            None => {
                map.place_head = next;
                if let Some(next) = next {
                    map.places[usize::from(next.0)].prev = None;
                }
            }
            Some(prev) => {
                map.places[usize::from(prev.0)].next = next;
                if let Some(next) = next {
                    map.places[usize::from(next.0)].prev = Some(prev);
                }
            }
        }
        map.places[index].next = map.place_free;
        map.place_free = Some(id);
    }

    /// `$04:C689`: the collected alerts go up; the message closes.
    fn post_alerts(&mut self) {
        self.frame.map.globals.alerts = self.d.pending_alerts;
        self.d.pending_alerts = 0;
        self.frame.links.message = 0;
    }

    /// `$04:C673`: hold the map for an alert, then the delay (state 0A).
    fn alert_hold(&mut self) {
        self.d.saved_hold = self.frame.map.globals.hold;
        *self.hold() |= HOLD_ALL;
        self.alert_delay();
    }

    /// `$04:C67F`.
    fn alert_delay(&mut self) {
        self.post_alerts();
        self.frame.links.timeline = 0x0A;
    }

    fn restore_hold(&mut self) {
        self.frame.map.globals.hold = self.d.saved_hold;
    }

    /// `$04:B88D`.
    fn stop_ship(&mut self) {
        let screen = &mut *self.frame.screen;
        screen.ship.travel &= !0x0003;
        screen.interface &= !0x0024;
        screen.hover &= !0x0030;
    }

    // ---- the timeline (D9FD) ----

    fn timeline(&mut self) -> Result<(), ScreenError> {
        if self.d.handshake & HS_TIMELINE == 0 {
            return Ok(());
        }
        self.timeline_state(self.frame.links.timeline)
    }

    fn timeline_state(&mut self, state: u16) -> Result<(), ScreenError> {
        match state {
            0x00 | 0x14 => Ok(()),
            0x02 => self.schedule(),
            0x04 => {
                self.alert_hold();
                Ok(())
            }
            0x06 | 0x08 => {
                self.alert_delay();
                Ok(())
            }
            0x0A => {
                self.d.timeline_delay = self.d.timeline_delay.wrapping_sub(1);
                if self.d.timeline_delay == 0 {
                    self.frame.links.timeline = self.d.timeline_next;
                }
                Ok(())
            }
            0x0C => {
                // $04:C180.
                if self.all_clear() {
                    self.frame.links.stage_flags |= SF_AFTERMATH;
                } else if self.flags() & SF_TIMELINE_WAIT != 0 {
                    return Ok(());
                }
                self.stop_ship();
                self.frame.links.timeline = self.d.timeline_next;
                self.timeline_state(self.frame.links.timeline)
            }
            0x0E => {
                // $04:C6AF: the goal reached.
                let tally = self.d.tally;
                let (sum, carry) = tally.marks_cleared.overflowing_add(tally.fleets_cleared);
                let (sum, carry) = add_carry(sum, tally.other_cleared, carry);
                let (sum, _) = add_carry(sum, tally.bases_cleared, carry);
                if (sum.wrapping_sub(tally.goal) as i16) >= 0 {
                    self.d.handshake |= HS_BUSY;
                    self.frame.links.timeline = self.d.timeline_next;
                }
                Ok(())
            }
            0x10 => {
                self.frame.screen.script = 0x18;
                self.frame.links.timeline = 0x12;
                self.opening()
            }
            0x12 => self.opening(),
            0x16 => {
                self.frame.links.scene.scene_events |= 0x8000;
                self.frame.map.globals.encounter_request = 1;
                *self.hold() |= HOLD_ALL;
                self.frame.links.timeline = 2;
                Ok(())
            }
            0x18 | 0x1A | 0x1C => {
                self.hold_aftermath();
                self.satellite_alert()
            }
            0x1E => self.satellite_alert(),
            0x20 => self.missile_launch(),
            0x22 => {
                if self.campaign() != 0 {
                    return Ok(());
                }
                self.d.handshake |= HS_BUSY;
                self.frame.map.globals.campaign_flags |= 0x0020;
                self.frame.map.globals.stored_target = 0x0A;
                self.d.saved_hold = self.frame.map.globals.hold;
                self.salvo()?;
                self.frame.links.timeline = 0x24;
                Ok(())
            }
            0x24 => {
                if self.frame.map.globals.launches_pending != 0 {
                    return Ok(());
                }
                self.d.handshake &= !HS_BUSY;
                self.frame.map.globals.campaign_flags &= !0x0020;
                self.frame.map.globals.stored_target = 0;
                self.frame.links.timeline = 2;
                self.restore_hold();
                Ok(())
            }
            0x26 => {
                if self.frame.links.batch_bonus | self.frame.map.globals.cruiser_count != 0 || self.campaign() != 0 {
                    return Ok(());
                }
                self.wave_alert()
            }
            0x28 => self.wave_alert(),
            0x2A => {
                self.frame.launch_wave()?;
                self.frame.map.globals.interceptors = 2;
                self.frame.links.scene.stage_results |= 0x0400;
                self.frame.map.globals.stored_target = 6;
                self.frame.links.timeline = 0x2C;
                Ok(())
            }
            0x2C => {
                if self.frame.links.batch_bonus == 0 || self.frame.map.globals.interceptors != 0 {
                    return Ok(());
                }
                self.frame.map.globals.campaign_flags &= !0x0004;
                self.d.handshake &= !HS_BUSY;
                self.frame.map.globals.stored_target = 0;
                self.frame.links.timeline = 2;
                if self.flags() & (SF_AFTERMATH | SF_RETURNED) != 0 {
                    self.frame.links.stage_flags &= !SF_AFTERMATH;
                    self.d.aftermath = 0x5C;
                } else {
                    self.restore_hold();
                }
                Ok(())
            }
            0x2E => {
                if self.frame.map.globals.escort_count != 0 || self.campaign() != 0 {
                    return Ok(());
                }
                if self.flags() & SF_CLEAR_REQUIRED != 0 && !self.all_clear() {
                    return Ok(());
                }
                self.hold_aftermath();
                self.frame.map.globals.campaign_flags |= 0x0080;
                self.d.handshake |= HS_BUSY;
                self.d.pending_alerts |= 0x0002;
                self.d.timeline_delay = 0x28;
                self.d.timeline_next = 0x30;
                self.alert_hold();
                Ok(())
            }
            0x30 => {
                self.frame.links.scene.stage_results |= 0x1000;
                self.frame.screen.script = 0x20;
                self.frame.links.timeline = 0x32;
                self.final_choice()
            }
            0x32 => self.final_choice(),
            0x34 => {
                if !self.wait_script() {
                    self.final_ready();
                }
                Ok(())
            }
            0x36 => {
                let globals = self.frame.map.globals;
                if self.d.tally.enemies_left | globals.missile_count | globals.unit_count != 0 {
                    return Ok(());
                }
                self.frame.links.stage_flags &= !SF_RECAPTURE;
                if self.campaign() != 0 {
                    return Ok(());
                }
                self.stop_ship();
                self.d.handshake |= HS_BUSY;
                self.frame.map.globals.pass_holds |= 0x0020;
                self.d.pending_alerts |= 0x2000;
                self.d.timeline_delay = 0x50;
                self.d.timeline_next = 0x38;
                self.alert_hold();
                Ok(())
            }
            0x38 => {
                self.frame.screen.script = 0x1E;
                self.frame.links.timeline += 2;
                Ok(())
            }
            0x3A => {
                if self.wait_script() {
                    return Ok(());
                }
                self.restore_hold();
                self.frame.map.globals.campaign_flags |= 0x0040;
                self.d.pending_alerts |= 0x0400;
                self.d.timeline_delay = 0x28;
                self.d.timeline_next = 0x3C;
                self.frame.links.scene.campaign_events |= 0x0080;
                self.frame.post_guard(None)?;
                self.frame.map.globals.stored_target = 0x0C;
                self.frame.map.globals.launches_pending = 1;
                self.stop_ship();
                self.frame.output.radio = Some(8);
                self.cue(0x61);
                self.alert_hold();
                Ok(())
            }
            0x3C | 0x3E => {
                if self.frame.map.globals.launches_pending != 0 {
                    return Ok(());
                }
                self.frame.links.scene.campaign_events &= !0x0040;
                self.frame.links.stage_flags |= SF_BASES;
                self.release_guard()?;
                self.frame.map.globals.campaign_flags &= !0x0040;
                self.d.handshake &= !HS_BUSY;
                self.frame.map.globals.stored_target = 0;
                self.frame.links.timeline = 0x40;
                self.frame.links.scene.scene_events &= !0x0040;
                *self.hold() &= !0x0002;
                self.d.aftermath = 0;
                Ok(())
            }
            0x40 => {
                if self.frame.map.globals.interceptor_count == 0 {
                    self.frame.links.timeline = 0;
                    self.d.clearing = 2;
                }
                Ok(())
            }
            0x42 => self.release_guard(),
            0x44 => {
                if self.d.tally.marks_left != 0 {
                    return Ok(());
                }
                self.frame.links.stage_flags &= !SF_RECAPTURE;
                self.frame.map.globals.pass_holds &= !0x0008;
                self.schedule()
            }
            0x46 => {
                if self.frame.links.batch_bonus != 0 {
                    return Ok(());
                }
                self.end_ambush();
                self.schedule()
            }
            other => Err(ScreenError::Unported(0x04C0F6 + u32::from(other))),
        }
    }

    /// `$04:C1A3`: no marks, fleets or units remain.
    fn all_clear(&self) -> bool {
        self.frame.screen.campaign.markers_placed | self.frame.links.batch_bonus | self.frame.map.globals.unit_count
            == 0
    }

    /// `$04:C13E`: run the schedule's due rows.
    fn schedule(&mut self) -> Result<(), ScreenError> {
        loop {
            self.frame.links.stage_flags &= !SF_AFTERMATH;
            let at = usize::from(self.d.schedule_cursor);
            let day = word_of(&SCHEDULE, at, 0x04C147)?;
            if day == 0xFFFF {
                return Ok(());
            }
            let due = if self.all_clear() {
                self.frame.links.stage_flags &= !SF_CLEAR_REQUIRED;
                true
            } else {
                day == 0xFFFE
                    || day == self.frame.screen.elapsed_steps
                    || (day.wrapping_sub(self.frame.screen.elapsed_steps) as i16) < 0
            };
            if !due {
                return Ok(());
            }
            let handler = word_of(&SCHEDULE, at + 2, 0x04C16F)?;
            self.d.schedule_cursor = self.d.schedule_cursor.wrapping_add(4);
            self.d.schedule_steps = self.d.schedule_steps.wrapping_add(1);
            if !self.scheduled(handler)? {
                return Ok(());
            }
        }
    }

    /// A schedule row's handler; returns whether the schedule runs on.
    fn scheduled(&mut self, handler: u16) -> Result<bool, ScreenError> {
        match handler {
            0xC1B3 => self.frame.map.globals.pass_holds |= 0x0008,
            0xC1BC => {
                self.frame.map.globals.pass_holds &= !0x0020;
                let base = self.frame.map.globals.base.ok_or(ScreenError::MissingPlace(0x04C1C2))?;
                self.frame.map.places[usize::from(base.0)].state = 2;
            }
            0xC1CE => {
                self.frame.links.stage_flags |= SF_BASES;
                self.frame.screen.campaign.bases_left = self.frame.screen.campaign.bases_left.wrapping_add(1);
            }
            0xC1DA => {
                if self.frame.screen.campaign.markers_placed != 0 {
                    self.frame.links.stage_flags |= SF_RECAPTURE;
                    self.frame.links.timeline = 0x44;
                    return Ok(false);
                }
            }
            0xC204 => {
                if self.frame.links.batch_bonus != 0 {
                    self.frame.links.stage_flags |= SF_AMBUSH;
                    self.d.ambush = 2;
                    self.frame.links.timeline = 0x46;
                    return Ok(false);
                }
            }
            0xC22B => {
                self.chain(0x1C)?;
                return Ok(false);
            }
            0xC244 => {
                self.chain(0x26)?;
                return Ok(false);
            }
            0xC25D => {
                self.chain(0x2E)?;
                return Ok(false);
            }
            0xC276 => {
                self.chain(0x36)?;
                return Ok(false);
            }
            0xC29C => {
                self.timeline_state(0x10)?;
                return Ok(false);
            }
            other => return Err(ScreenError::InvalidScriptStep(other)),
        }
        Ok(true)
    }

    /// `$04:C22B` and its siblings: run the state now, or after the map is
    /// clear when clearing is required (state 0C).
    fn chain(&mut self, state: u16) -> Result<(), ScreenError> {
        if self.flags() & SF_CLEAR_REQUIRED == 0 {
            self.frame.links.timeline = state;
            return self.timeline_state(state);
        }
        self.d.timeline_next = state;
        self.frame.links.stage_flags |= SF_TIMELINE_WAIT;
        self.frame.links.timeline = 0x0C;
        Ok(())
    }

    /// `$04:C2A8`: the opening script, then the first stage or the map.
    fn opening(&mut self) -> Result<(), ScreenError> {
        if self.wait_script() {
            return Ok(());
        }
        if self.frame.links.scene.stage_results & 0x0001 != 0 {
            self.d.aftermath = 0x0A;
            *self.hold() = (*self.hold() & !HOLD_ALL) | 0x003B;
            self.frame.links.timeline = 2;
        } else {
            *self.hold() = (*self.hold() | 0x0002) & !0x0060;
            self.frame.links.timeline = 0x14;
        }
        Ok(())
    }

    /// `$04:C45E`.
    fn hold_aftermath(&mut self) {
        if self.flags() & (SF_AFTERMATH | SF_RETURNED) != 0 {
            self.stop_ship();
            *self.hold() |= HOLD_ALL;
        }
    }

    /// `$04:C306`: the satellite's alert, then the missiles (state 20).
    fn satellite_alert(&mut self) -> Result<(), ScreenError> {
        self.frame.links.stage_flags |= SF_FINAL;
        self.d.handshake |= HS_BUSY;
        self.d.timeline_next = 0x20;
        self.d.timeline_delay = 0x28;
        let bit = word_of(&SATELLITE_ALERTS, usize::from(self.frame.links.satellite_timing), 0x04C321)?;
        self.d.pending_alerts |= bit;
        self.alert_hold();
        Ok(())
    }

    /// `$04:C331`: the missile launch request.
    fn missile_launch(&mut self) -> Result<(), ScreenError> {
        let difficulty = self.frame.links.difficulty;
        let pick = if difficulty == 0 { 0 } else { usize::from(self.frame.links.random & 0x0003) };
        let offset = |index| byte_at(&MISSILE_KIND_OFFSETS, index, 0x04C33F).map_err(|_| ScreenError::TableOverrun(0x04C33F));
        self.frame.links.missile_kinds[0] = offset(pick)?;
        if difficulty as u8 == 2 {
            self.frame.links.missile_kinds[1] = offset(pick + 1)?;
        }
        self.restore_hold();
        self.frame.map.globals.encounter_request = 4;
        self.frame.links.timeline = 0x22;
        Ok(())
    }

    /// `$04:E7CD`: a missile salvo from the station.
    fn salvo(&mut self) -> Result<(), ScreenError> {
        let count = self.frame.map.globals.missile_count;
        self.frame.map.globals.launches_pending = count;
        self.d.salvo_index = count.wrapping_sub(1);
        loop {
            let id = self.frame.new_unit()?;
            let index = usize::from(self.d.salvo_index);
            let column = byte_at(&SALVO_COLUMNS, index, 0x04E806).map_err(|_| ScreenError::TableOverrun(0x04E806))?;
            let offset = *self.frame.links.missile_kinds.get(index).ok_or(ScreenError::Unported(0x04E815))?;
            let kind = 0x1Cu8.wrapping_add(offset);
            let entry = PatternEntry {
                pattern: byte_at(&CARRIER_PATTERN, usize::from(kind), 0x00B0F8)?,
                word: byte_at(&CARRIER_WORD, usize::from(kind), 0x00B118)?,
                strength: byte_at(&CARRIER_STRENGTH, usize::from(kind), 0x00B138)?,
            };
            let unit = self.frame.map.unit_mut(id);
            unit.flags |= 0x4080;
            unit.category = 9;
            unit.variant = 4;
            unit.strength = 1;
            unit.count = 5;
            set_low(&mut unit.target_x, column);
            unit.kind = u16::from(kind);
            self.frame.map.place_in_slot(id, entry)?;
            let unit = self.frame.map.unit_mut(id);
            unit.x_fraction &= 0x00FF;
            unit.x = 0xEC;
            unit.y_fraction &= 0x00FF;
            unit.y = 0x18;
            unit.speed = 0x2000;
            unit.target_y = 0x0060;
            self.frame.map.aim(id);
            let unit = self.frame.map.unit_mut(id);
            unit.behavior = 3;
            unit.motion = 1;
            unit.sprite = 0xFFFF;
            unit.timer = 1;
            unit.radius = 8;
            unit.proximity_delay = 0x18;
            let globals = &mut self.frame.map.globals;
            globals.missile_salvo = globals.missile_salvo.wrapping_add(1);
            self.d.salvo_index = self.d.salvo_index.wrapping_sub(1);
            if (self.d.salvo_index as i16) < 0 {
                break;
            }
        }
        self.frame.links.scene.event_word |= 0x0020;
        Ok(())
    }

    /// `$04:C3D0`: the wave's alert, then the wave (state 2A).
    fn wave_alert(&mut self) -> Result<(), ScreenError> {
        self.hold_aftermath();
        self.d.handshake |= HS_BUSY;
        self.d.timeline_delay = 0x28;
        self.d.timeline_next = 0x2A;
        self.frame.map.globals.campaign_flags |= 0x0004;
        self.d.pending_alerts |= if self.frame.links.difficulty == 2 { 0x0040 } else { 0x0020 };
        self.alert_hold();
        Ok(())
    }

    /// `$04:C4C3`: the final choice's script.
    fn final_choice(&mut self) -> Result<(), ScreenError> {
        if self.wait_script() {
            return Ok(());
        }
        self.frame.links.scene.stage_results &= !0x1000;
        if self.frame.links.scene.campaign_events & 0x0004 == 0 {
            self.final_ready();
            return Ok(());
        }
        self.frame.links.scene.campaign_events &= !0x0004;
        self.frame.screen.menu_choice = 6;
        self.frame.screen.menu_confirm = 6;
        self.frame.screen.menu_place = self.frame.map.globals.guarded_place;
        self.frame.screen.script = 0x24;
        self.frame.links.timeline = 0x34;
        Ok(())
    }

    /// `$04:C4FE`.
    fn final_ready(&mut self) {
        self.frame.links.stage_flags |= SF_BASES_FALLEN | SF_RECAPTURE;
        self.d.handshake |= HS_RECAPTURE;
        self.frame.links.timeline = 0;
    }

    /// `$04:C5FF`: a guard becomes the planet's attacker.
    fn release_guard(&mut self) -> Result<(), ScreenError> {
        if self.flags() & SF_BASES == 0 || self.frame.links.scene.campaign_events & 0x0040 != 0 {
            return Ok(());
        }
        if self.frame.map.globals.interceptor_count == 0 {
            self.frame.links.stage_flags &= !SF_BASES;
            return Ok(());
        }
        let mut next = self.frame.map.unit_head;
        while let Some(id) = next {
            let unit = *self.frame.map.unit(id);
            if unit.flags2 & 0x0400 != 0 {
                let record = self.frame.map.unit_mut(id);
                record.flags &= 0xDFFF;
                record.flags2 = (record.flags2 | 0x0100) & 0xFBFF;
                self.frame.links.scene.campaign_events |= 0x0040;
                let record = self.frame.map.unit_mut(id);
                record.behavior = 7;
                record.motion = 0;
                record.radius = 8;
                let home = unit.home.ok_or(ScreenError::MissingPlace(0x04C666))?;
                self.frame.map.places[usize::from(home.0)].flags &= 0x7FFF;
                return Ok(());
            }
            next = unit.next;
        }
        self.frame.links.stage_flags &= !SF_BASES;
        Ok(())
    }

    /// `$7F:5856`: the nearest threatened place by its countdown.
    fn rescan_threats(&mut self) -> Result<(), ScreenError> {
        let map = &mut *self.frame.map;
        map.globals.threat_nearest = 0xFFFF;
        let mut next = map.unit_head;
        while let Some(id) = next {
            let unit = *map.unit(id);
            if unit.flags & 0x0040 != 0 && self.frame.links.scene.scene_events & 0x0080 == 0 {
                let home = unit.home.ok_or(ScreenError::MissingPlace(0x7F587F))?;
                let place = map.places[usize::from(home.0)];
                if place.status & 0x0002 != 0 && place.warning_countdown < map.globals.threat_nearest {
                    map.globals.threat_place = Some(home);
                    map.globals.threat_countdown = place.warning_countdown;
                    map.globals.threat_nearest = place.warning_countdown;
                    map.globals.threat_cue = true;
                }
            }
            next = unit.next;
        }
        Ok(())
    }

    // ---- the recapture (DA15) ----

    fn recapture(&mut self) -> Result<(), ScreenError> {
        if self.d.handshake & HS_RECAPTURE == 0 || self.flags() & SF_RECAPTURE == 0 {
            return Ok(());
        }
        let campaign = self.campaign();
        if campaign != 0 && campaign & 0x0002 == 0 && campaign & 0x0080 == 0 {
            return Ok(());
        }
        match self.d.recapture {
            0 => {
                if self.flags() & SF_BASES_FALLEN == 0 {
                    if self.frame.screen.campaign.markers_placed == 0 {
                        self.frame.links.stage_flags = (self.flags() & !SF_RECAPTURE) | SF_PLANET_LOST;
                        self.frame.map.globals.pass_holds &= !0x0008;
                        self.frame.links.timeline = 2;
                        return Ok(());
                    }
                    if self.frame.map.globals.escort_count != 0 {
                        return Ok(());
                    }
                    if self.flags() & SF_CLEAR_REQUIRED != 0 && !self.all_clear() {
                        return Ok(());
                    }
                }
                self.d.handshake |= HS_RECAPTURING;
                self.d.recapture = 2;
            }
            2 => {
                if self.mark_targets() == 0 {
                    self.d.handshake &= !HS_RECAPTURING;
                    self.frame.links.stage_flags &= !SF_RECAPTURE;
                    self.frame.map.globals.pass_holds &= !0x0008;
                    self.d.recapture = 0;
                    return Ok(());
                }
                if self.flags() & SF_BASES_FALLEN == 0 {
                    self.hold_aftermath();
                    self.d.saved_hold = self.frame.map.globals.hold;
                } else {
                    self.frame.links.stage_flags &= !SF_BASES_FALLEN;
                }
                self.frame.map.globals.campaign_flags |= 0x0002;
                *self.hold() |= HOLD_ALL;
                self.d.alert_timer = 0x50;
                self.d.alert = 2;
                self.d.recapture = 4;
                self.d.pending_alerts |= 0x0100;
                self.recapture_alert()?;
            }
            4 => self.recapture_alert()?,
            6 => self.recapture_launch(),
            8 => self.recapture_wait(),
            0x0A => self.recapture_end(),
            other => return Err(ScreenError::Unported(0x04C731 + u32::from(other))),
        }
        Ok(())
    }

    /// `$04:C7DD`.
    fn recapture_alert(&mut self) -> Result<(), ScreenError> {
        self.alert_step()?;
        if self.d.alert == 0 {
            self.recapture_launch();
        }
        Ok(())
    }

    /// `$04:C7E6`.
    fn recapture_launch(&mut self) {
        self.frame.map.globals.stored_target = 4;
        self.frame.map.globals.pass_holds &= !0x0008;
        self.d.recapture = 8;
        self.recapture_wait();
    }

    /// `$04:C7FE`.
    fn recapture_wait(&mut self) {
        if self.frame.map.globals.fighter_launches == 0 {
            self.recapture_end();
        }
    }

    /// `$04:C804`.
    fn recapture_end(&mut self) {
        self.d.handshake &= !(HS_RECAPTURING | HS_RECAPTURE);
        self.frame.links.stage_flags &= !SF_RECAPTURE;
        self.frame.map.globals.campaign_flags &= !0x0002;
        self.frame.map.globals.stored_target = 0;
        self.d.recapture = 0;
        self.frame.links.message = 0;
        self.frame.links.timeline = 2;
        if self.campaign() & 0x0080 != 0 {
            self.frame.map.globals.campaign_flags &= !0x0080;
            self.d.handshake &= !HS_BUSY;
        }
        if self.flags() & (SF_AFTERMATH | SF_RETURNED) != 0 {
            self.frame.map.globals.pass_holds &= !0x0008;
            self.frame.links.stage_flags &= !SF_AFTERMATH;
            self.d.aftermath = 0x5C;
        } else {
            self.restore_hold();
            self.frame.map.globals.pass_holds &= !0x0008;
        }
    }

    /// `$04:C86D`: the shown, unlisted marked places start their attack;
    /// returns how many.
    fn mark_targets(&mut self) -> u16 {
        let mut count: u16 = 0;
        let mut next = self.frame.map.place_head;
        while let Some(id) = next {
            let place = &mut self.frame.map.places[usize::from(id.0)];
            if place.flags & 0x0001 != 0 && place.flags & 0x2000 != 0 && place.flags & 0x1800 == 0 {
                place.flags &= 0xFDFF;
                place.state = 2;
                place.timer = 1;
                count = count.wrapping_add(1);
            }
            next = place.next;
        }
        if count != 0 {
            self.frame.map.globals.fighter_launches = if (count.wrapping_sub(7) as i16) < 0 { count } else { 6 };
        }
        count
    }

    /// `$04:C6CE`: the alert sub-state (DA07).
    fn alert_step(&mut self) -> Result<(), ScreenError> {
        match self.d.alert {
            0 => {}
            2 | 4 | 6 => {
                self.post_alerts();
                self.d.alert = 8;
            }
            8 => {
                self.d.alert_timer = self.d.alert_timer.wrapping_sub(1);
                if self.d.alert_timer == 0 {
                    self.d.alert = 0;
                }
            }
            other => return Err(ScreenError::Unported(0x04C6D9 + u32::from(other))),
        }
        Ok(())
    }

    // ---- the ambush (DA17) ----

    fn ambush(&mut self) -> Result<(), ScreenError> {
        if self.d.handshake & HS_AMBUSH == 0 || self.flags() & SF_AMBUSH == 0 {
            return Ok(());
        }
        let campaign = self.campaign();
        if campaign != 0 && campaign & 0x0008 == 0 {
            return Ok(());
        }
        match self.d.ambush {
            0 => {}
            2 => {
                if self.frame.links.batch_bonus == 0 || !self.exposed_bases() {
                    self.end_ambush();
                    return Ok(());
                }
                self.frame.map.globals.campaign_flags |= 0x0008;
                self.d.saved_hold = self.frame.map.globals.hold;
                *self.hold() |= HOLD_ALL;
                self.d.pending_alerts |= 0x0080;
                self.d.alert_timer = 0x28;
                self.d.alert = 2;
                self.d.ambush = 4;
                self.ambush_alert()?;
            }
            4 => self.ambush_alert()?,
            6 => self.ambush_launch()?,
            other => return Err(ScreenError::Unported(0x04C8E0 + u32::from(other))),
        }
        Ok(())
    }

    /// `$04:C91D`.
    fn ambush_alert(&mut self) -> Result<(), ScreenError> {
        self.alert_step()?;
        if self.d.alert == 0 {
            self.ambush_launch()?;
        }
        Ok(())
    }

    /// `$04:C926`: the exposed bases' fleets set out.
    fn ambush_launch(&mut self) -> Result<(), ScreenError> {
        let mut next = self.frame.map.place_head;
        while let Some(id) = next {
            let place = self.frame.map.places[usize::from(id.0)];
            if place.flags & 0x0004 != 0 && place.status & 0x0002 == 0 {
                let unit = place.held_unit.ok_or(ScreenError::MissingUnit(0x04C98D))?;
                self.frame.map.unit_mut(unit).flags2 &= 0xFFFB;
                self.frame.map.set_course(unit, 0x00C8)?;
            }
            next = place.next;
        }
        self.restore_hold();
        self.frame.links.message = 0;
        self.frame.map.globals.campaign_flags &= !0x0008;
        self.end_ambush();
        Ok(())
    }

    /// `$04:C938`.
    fn end_ambush(&mut self) {
        self.d.handshake &= !0x0080;
        self.frame.links.stage_flags &= !SF_AMBUSH;
        self.d.ambush = 0;
        self.frame.links.timeline = 2;
    }

    /// `$04:C951`: any fleet base not yet on alert.
    fn exposed_bases(&self) -> bool {
        let mut next = self.frame.map.place_head;
        while let Some(id) = next {
            let place = &self.frame.map.places[usize::from(id.0)];
            if place.flags & 0x0004 != 0 && place.status & 0x0002 == 0 {
                return true;
            }
            next = place.next;
        }
        false
    }

    // ---- the planet threats (DA69) ----

    fn threats(&mut self) -> Result<(), ScreenError> {
        if self.d.handshake & HS_THREATS == 0 {
            return Ok(());
        }
        self.frame.map.globals.threat_cue = false;
        let Some(place_id) = self.frame.map.globals.threat_place else {
            return Ok(());
        };
        let index = usize::from(place_id.0);
        match self.frame.map.globals.threat_kind {
            0 => {}
            1 => self.frame.map.globals.threat_cue = true,
            2 => {
                self.frame.map.globals.threat_cue = true;
                self.d.handshake |= HS_THREAT_SHOWN;
                self.d.saved_hold = self.frame.map.globals.hold;
                *self.hold() |= HOLD_ALL;
                self.frame.map.globals.alerts |= 0x0010;
                self.d.threat_delay = 0x28;
                self.next_threat();
            }
            3 => {
                self.frame.map.globals.threat_cue = true;
                if self.threat_delay_done() {
                    self.next_threat();
                }
            }
            4 => {
                self.frame.map.globals.threat_cue = true;
                self.frame.links.scene.campaign_events |= 0x0002;
                self.frame.map.globals.encounter_request = 5;
                self.next_threat();
            }
            5 => {
                self.d.saved_hold = self.frame.map.globals.hold;
                let place = &mut self.frame.map.places[index];
                place.warning = 2;
                place.warning_rate = if place.status & 0x0001 != 0 { 0x20 } else { 0x1F };
                place.warning_bias = 0;
                self.cue(0x00FF);
                self.cue(0x00C4);
                self.next_threat();
            }
            6 => {
                if self.frame.map.globals.speed_flags & 0x1000 != 0 || self.frame.map.places[index].warning != 6 {
                    return Ok(());
                }
                self.frame.map.places[index].warning = 0;
                if self.frame.links.planet_health == 0 {
                    self.threat_cleared(index);
                    return Ok(());
                }
                self.d.threat_delay = 0x28;
                self.frame.map.globals.map_events |= 0x0001;
                self.next_threat();
            }
            7 => {
                if !self.threat_delay_done() {
                    return Ok(());
                }
                self.d.handshake &= !HS_THREAT_SHOWN;
                self.frame.links.scene.scene_events &= !0x0080;
                self.threat_cleared(index);
            }
            other => return Err(ScreenError::Unported(0x04C9C7 + u32::from(other) * 2)),
        }
        Ok(())
    }

    fn next_threat(&mut self) {
        let kind = &mut self.frame.map.globals.threat_kind;
        *kind = kind.wrapping_add(1);
    }

    /// `$04:CACF`.
    fn threat_delay_done(&mut self) -> bool {
        if self.d.threat_delay == 0 {
            return true;
        }
        self.d.threat_delay -= 1;
        false
    }

    /// `$04:CAAD`: the warning resets and the next threat is chosen.
    fn threat_cleared(&mut self, index: usize) {
        self.restore_hold();
        let place = &mut self.frame.map.places[index];
        place.warning_countdown = 0x0258;
        place.warning = 1;
        place.warning_rate = 2;
        place.warning_bias = 0;
        // $04:CADC.
        let map = &mut *self.frame.map;
        map.globals.threat_nearest = 0xFFFF;
        map.globals.threat_kind = 0;
        if self.frame.links.planet_health == 0 {
            return;
        }
        let mut next = map.place_head;
        while let Some(id) = next {
            let place = map.places[usize::from(id.0)];
            if place.flags & 0x0004 != 0 && place.status & 0x0002 != 0 && place.warning_countdown < map.globals.threat_nearest {
                map.globals.threat_place = Some(id);
                map.globals.threat_countdown = place.warning_countdown;
                map.globals.threat_nearest = place.warning_countdown;
                map.globals.threat_kind = 1;
            }
            next = place.next;
        }
    }

    // ---- the satellite sortie (E089) ----

    fn sortie(&mut self) -> Result<(), ScreenError> {
        if self.d.handshake & HS_SORTIE == 0 {
            return Ok(());
        }
        let campaign = self.campaign();
        if campaign != 0 && campaign & 0x0010 == 0 {
            return Ok(());
        }
        match self.frame.map.globals.satellite_hold {
            0 => {
                let globals = &mut self.frame.map.globals;
                if globals.satellite_flags & 0x0008 == 0 {
                    return Ok(());
                }
                globals.satellite_flags &= !0x0008;
                globals.satellite_hold += 1;
                globals.satellite_phase = globals.satellite_phase.wrapping_add(1);
                self.launch_sortie()?;
            }
            1 => self.launch_sortie()?,
            2 => {
                let globals = &mut self.frame.map.globals;
                if globals.satellite_flags & 0x0040 == 0 {
                    return Ok(());
                }
                globals.satellite_flags |= 0x0080;
                self.frame.map.globals.satellite_hold += 1;
                self.cue(0x008B);
                let escort = self.frame.links.sortie_escort.ok_or(ScreenError::MissingUnit(0x04CBF1))?;
                if self.frame.map.globals.satellite_guarding != 0 {
                    let satellite = self.frame.map.globals.satellite.ok_or(ScreenError::MissingUnit(0x04CBF9))?;
                    let mut scene = self.frame.links.scene;
                    self.frame.map.damage_planet(satellite, &mut scene);
                    self.frame.links.scene = scene;
                    self.frame.map.unit_mut(escort).behavior = 0x0D;
                } else {
                    self.frame.map.unit_mut(escort).behavior = 0x0C;
                    let target = self.frame.map.globals.satellite_target.ok_or(ScreenError::MissingUnit(0x04CC0D))?;
                    let mut scene = self.frame.links.scene;
                    self.frame.map.destroy(target, &mut scene)?;
                    self.frame.links.scene = scene;
                }
            }
            3 => {
                if self.frame.map.globals.satellite_flags & 0x0080 != 0 {
                    return Ok(());
                }
                if self.frame.map.globals.satellite_guarding == 0 {
                    return self.end_sortie();
                }
                self.frame.map.globals.alerts |= 0x1000;
                self.d.sortie_timer = 0x28;
                self.frame.map.globals.satellite_hold += 1;
                self.sortie_timer_step()?;
            }
            4 => self.sortie_timer_step()?,
            other => return Err(ScreenError::Unported(0x04CB47 + u32::from(other) * 2)),
        }
        Ok(())
    }

    /// `$04:CC32`.
    fn sortie_timer_step(&mut self) -> Result<(), ScreenError> {
        self.d.sortie_timer = self.d.sortie_timer.wrapping_sub(1);
        if self.d.sortie_timer == 0 {
            return self.end_sortie();
        }
        Ok(())
    }

    /// `$04:CC38`.
    fn end_sortie(&mut self) -> Result<(), ScreenError> {
        self.frame.map.globals.campaign_flags &= !0x0010;
        self.d.handshake &= !HS_SORTIE_RUNNING;
        self.frame.map.globals.satellite_flags &= !0x0040;
        self.restore_hold();
        self.frame.map.globals.satellite_hold = 0;
        Ok(())
    }

    /// `$04:CB66`: the satellite's escort sets out.
    fn launch_sortie(&mut self) -> Result<(), ScreenError> {
        self.d.handshake |= HS_SORTIE_RUNNING;
        self.d.saved_hold = self.frame.map.globals.hold;
        *self.hold() |= HOLD_ALL;
        self.frame.map.globals.campaign_flags |= 0x0010;
        self.frame.map.globals.stored_target = 8;
        let id = self.frame.map.allocate_unit()?;
        self.frame.links.sortie_escort = Some(id);
        let satellite = self.frame.map.globals.satellite.ok_or(ScreenError::MissingUnit(0x04CB91))?;
        let anchor = *self.frame.map.unit(satellite);
        let (target_x, target_y) = if self.frame.map.globals.satellite_guarding != 0 {
            let place = *self.frame.place(self.frame.map.globals.guarded_place, 0x04CC5B)?;
            (place.x as u8, place.y as u8)
        } else {
            let target = self.frame.map.globals.satellite_target.ok_or(ScreenError::MissingUnit(0x04CC6C))?;
            let unit = self.frame.map.unit(target);
            (unit.x, unit.y)
        };
        let unit = self.frame.map.unit_mut(id);
        unit.behavior = 0;
        unit.variant = 0;
        unit.x_fraction = (unit.x_fraction & 0x00FF) | (anchor.x_fraction & 0xFF00);
        unit.x = anchor.x;
        unit.y_fraction = (unit.y_fraction & 0x00FF) | (anchor.y_fraction & 0xFF00);
        unit.y = anchor.y;
        set_low(&mut unit.target_x, target_x);
        set_low(&mut unit.target_y, target_y);
        unit.speed = 0x2000;
        unit.flags |= 0x2000;
        unit.flags2 |= 0x0004;
        unit.motion = 3;
        unit.radius = 0x0C;
        self.frame.map.globals.satellite_flags &= !0x0004;
        self.cue(0x0064);
        self.frame.map.globals.satellite_hold += 1;
        Ok(())
    }

    // ---- the map clearing (DA19) ----

    fn clearing(&mut self) -> Result<(), ScreenError> {
        if self.d.handshake & HS_CLEARING == 0 {
            return Ok(());
        }
        match self.d.clearing {
            0 => {}
            2 => {
                self.d.handshake |= HS_SHUTDOWN;
                self.frame.links.scene.stage_results |= 0x0020;
                *self.hold() = 0xFFFF & !0x0002;
                self.frame.map.globals.pass_holds |= 0x0004;
                self.frame.screen.ship.travel &= !0x0002;
                self.d.clearing = 4;
            }
            4 => {
                if self.frame.map.globals.unit_count == 0 {
                    *self.hold() = (*self.hold() | 0x0002) & !0x0004;
                    self.frame.screen.script = 2;
                    self.d.clearing = 6;
                }
            }
            6 => {
                if !self.wait_script() {
                    // Two word stores.
                    self.frame.screen.arrival_x = 0xEC;
                    self.frame.screen.arrival_y = 0x18;
                    self.frame.screen.pulses[0].x &= 0xFF_FF00;
                    self.d.clearing = 8;
                }
            }
            8 => {
                let shields = &mut self.frame.links.pilot_shields;
                shields[0] = shields[1];
                shields[2] = shields[3];
                self.frame.links.launch_location = 0x000B;
                self.frame.links.launch_layout = self.frame.links.difficulty;
                self.frame.links.arrival_word = 8;
                self.frame.map.globals.encounter_request = 2;
            }
            0x0A => {
                self.destroy_first(0x8760, 0x008B)?;
                self.d.clearing = 0;
            }
            0x0C => {
                self.destroy_first(0x8F58, 0x002B)?;
                self.d.clearing = 0;
            }
            0x0E => {
                self.clear_first_mark();
                self.d.clearing = 0;
            }
            0x10 => {
                self.clear_first_fleet()?;
                self.d.clearing = 0;
            }
            0x12 => {
                self.clear_first_mark();
                self.clear_first_fleet()?;
                self.destroy_first(0x8760, 0x008B)?;
                self.destroy_first(0x8F58, 0x002B)?;
                self.d.clearing = 0;
            }
            other => return Err(ScreenError::Unported(0x04DAD1 + u32::from(other))),
        }
        Ok(())
    }

    /// `$04:DB13`.
    fn clear_first_mark(&mut self) {
        let mut next = self.frame.map.place_head;
        while let Some(id) = next {
            let place = self.frame.map.places[usize::from(id.0)];
            if place.flags & 0x0001 != 0 && place.flags & 0x0004 == 0 && place.flags & 0x1000 == 0 {
                self.clear_mark(id);
                return;
            }
            next = place.next;
        }
    }

    /// `$04:DB44`.
    fn clear_first_fleet(&mut self) -> Result<(), ScreenError> {
        let mut next = self.frame.map.place_head;
        while let Some(id) = next {
            let place = self.frame.map.places[usize::from(id.0)];
            if place.flags & 0x0004 != 0 {
                let unit = place.held_unit.ok_or(ScreenError::MissingUnit(0x04DB5D))?;
                self.clear_fleet(id, unit)?;
                return Ok(());
            }
            next = place.next;
        }
        Ok(())
    }

    /// `$04:DB6F`/`$04:DBB2`: the first unit outside `mask` is destroyed.
    fn destroy_first(&mut self, mask: u16, cue: u16) -> Result<(), ScreenError> {
        let Some(head) = self.frame.map.unit_head else {
            return Ok(());
        };
        let mut next = Some(head);
        while let Some(id) = next {
            let unit = *self.frame.map.unit(id);
            if unit.flags & mask == 0 {
                if unit.flags & 0x0008 != 0 {
                    let home = unit.home.ok_or(ScreenError::MissingPlace(0x04DB90))?;
                    self.frame.map.places[usize::from(home.0)].flags &= 0x7FFF;
                }
                let mut scene = self.frame.links.scene;
                self.frame.map.destroy(id, &mut scene)?;
                self.frame.links.scene = scene;
                break;
            }
            next = unit.next;
        }
        self.cue(cue);
        Ok(())
    }

    // ---- the final stage choice (DB29) ----

    fn final_stage(&mut self) -> Result<(), ScreenError> {
        match self.frame.links.final_stage {
            0 => {}
            2 | 4 => {
                if self.frame.screen.wingmate.flags & 0x0002 != 0 {
                    return Ok(());
                }
                *self.hold() |= HOLD_ALL;
                self.frame.screen.ship.travel |= 0x0200;
                self.frame.links.text_state = [0, 0x0069];
                self.frame.links.message = MESSAGE_FINAL;
                self.set_choice_countdown(4);
                self.cue(4);
                self.next_choice();
            }
            6 => {
                let countdown = self.choice_countdown().wrapping_sub(1);
                self.set_choice_countdown(countdown);
                if countdown == 0 {
                    self.d.choice = 0;
                    self.d.choosing = 1;
                    self.next_choice();
                }
            }
            8 => {
                let pressed = self.frame.links.menu_pad[1];
                if pressed & 0x8000 != 0 {
                    if self.d.choice != 0 {
                        self.d.choosing = 0;
                        return self.decline();
                    }
                    self.frame.links.text_state = [0, 0x006A];
                    self.frame.links.message = MESSAGE_FINAL;
                    self.cue(4);
                    self.d.choosing = 0;
                    self.next_choice();
                } else if pressed & 0x0040 != 0 {
                    self.d.choosing = 0;
                    return self.decline();
                } else if pressed & 0x0800 != 0 {
                    self.d.choice = self.d.choice.wrapping_sub(1) & 1;
                    self.cue(3);
                } else if pressed & 0x0400 != 0 {
                    self.d.choice = self.d.choice.wrapping_add(1) & 1;
                    self.cue(3);
                }
            }
            0x0A => {
                self.frame.screen.interface |= 0x0400;
                if let Some(id) = self.frame.menu_place() {
                    self.frame.screen.menu_place = Some(id);
                }
                self.frame.show_menu_place()?;
                self.frame.screen.menu_confirm = self.frame.screen.info.number;
                self.frame.screen.menu_home = self.frame.screen.menu_choice;
                self.next_choice();
            }
            0x0C => {
                let interface = self.frame.screen.interface;
                if interface & 0x0400 != 0 {
                    return Ok(());
                }
                if interface & 0x0800 != 0 {
                    return self.decline();
                }
                self.frame.screen.menu_confirm = self.frame.screen.info.number;
                self.next_choice();
            }
            0x0E => {
                self.frame.screen.script = 6;
                self.next_choice();
            }
            // $04:BAF7 clears a unit's flag through whatever the earlier
            // services left in the index; not ported.
            0x10 => return Err(ScreenError::Unported(0x04BAF7)),
            0x12 => {
                if !self.wait_script() {
                    self.choice_done();
                }
            }
            other => return Err(ScreenError::Unported(0x04B9F5 + u32::from(other))),
        }
        Ok(())
    }

    fn next_choice(&mut self) {
        self.frame.links.final_stage = self.frame.links.final_stage.wrapping_add(2);
    }

    /// DB27 (with DB28) doubles as the choice's countdown word.
    fn choice_countdown(&self) -> u16 {
        u16::from(self.frame.screen.warp_target[2]) | (u16::from(self.frame.screen.warp_spare) << 8)
    }

    fn set_choice_countdown(&mut self, value: u16) {
        self.frame.screen.warp_target[2] = value as u8;
        self.frame.screen.warp_spare = (value >> 8) as u8;
    }

    /// `$04:BB08`.
    fn decline(&mut self) -> Result<(), ScreenError> {
        self.cue(8);
        self.frame.screen.script = 0x22;
        self.frame.links.final_stage = 0x12;
        if !self.wait_script() {
            self.choice_done();
        }
        Ok(())
    }

    /// `$04:BB21`.
    fn choice_done(&mut self) {
        let shields = &mut self.frame.links.pilot_shields;
        shields[0] = shields[1];
        shields[2] = shields[3];
        self.frame.screen.hover &= !0x0001;
        self.d.aftermath = 0x5C;
        self.frame.screen.interface &= !0x0800;
        self.frame.links.final_stage = 0;
        self.frame.links.text_state[0] = 1;
    }
}

fn add_carry(a: u16, b: u16, carry: bool) -> (u16, bool) {
    let sum = u32::from(a) + u32::from(b) + u32::from(carry);
    (sum as u16, sum > 0xFFFF)
}

fn set_low(word: &mut u16, byte: u8) {
    *word = (*word & 0xFF00) | u16::from(byte);
}

fn set_high(word: &mut u16, byte: u8) {
    *word = (*word & 0x00FF) | (u16::from(byte) << 8);
}

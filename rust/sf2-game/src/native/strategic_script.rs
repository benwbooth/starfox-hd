//! The map screen's scripts (`$04:CD47`): short step lists the campaign
//! starts through DA7F for its set pieces (the warp, the satellite's
//! capture, an interceptor's fall, a new enemy wave and the guard
//! markers). One step runs per frame; a step either finishes and moves the
//! script on, or waits.

use super::strategic_screen::{bearing, near8, rotate, Frame, ScreenError, ScriptSubject};
use super::strategic_sim::{byte_at, word_at, PlaceId, UnitId, PLACE_CAPACITY, PROGRAMS, TERRAIN_ROW};

/// `$04:CD6E`: each script's offset (indexed by DA7F), then the step
/// lists: handler addresses, ending in 0001.
const SCRIPTS: [u8; 0xE6] = [
    0x28, 0x00, 0x32, 0x00, 0x2A, 0x00, 0x58, 0x00, 0x84, 0x00, 0x8A, 0x00, 0x9A, 0x00, 0xA0, 0x00, 0xA6, 0x00,
    0xB2, 0x00, 0x92, 0x00, 0xBE, 0x00, 0xC0, 0x00, 0xC0, 0x00, 0xC0, 0x00, 0x42, 0x00, 0xCE, 0x00, 0x68, 0x00,
    0x72, 0x00, 0xD6, 0x00, 0x01, 0x00, 0xD7, 0xD0, 0x0D, 0xD1, 0x1F, 0xD1, 0x01, 0x00, 0x07, 0xCF, 0x28, 0xCF,
    0x1F, 0xD1, 0xA2, 0xCF, 0x55, 0xCE, 0xE4, 0xCF, 0xFB, 0xCF, 0x01, 0x00, 0x22, 0xD0, 0xA2, 0xCF, 0x55, 0xCE,
    0xE4, 0xCF, 0xFB, 0xCF, 0xB4, 0xCF, 0x55, 0xCE, 0x7A, 0xCF, 0xF9, 0xCE, 0x4F, 0xCF, 0x01, 0x00, 0x5B, 0xCF,
    0xA2, 0xCF, 0x55, 0xCE, 0xE4, 0xCF, 0xFB, 0xCF, 0xB4, 0xCF, 0x4F, 0xD0, 0x7A, 0xCF, 0x65, 0xD0, 0xCF, 0xD0,
    0x0D, 0xD1, 0x1F, 0xD1, 0x01, 0x00, 0x5B, 0xCF, 0xA2, 0xCF, 0x55, 0xCE, 0xE4, 0xCF, 0xFB, 0xCF, 0xB4, 0xCF,
    0x4F, 0xD0, 0x7A, 0xCF, 0x01, 0x00, 0xDC, 0xD1, 0xEC, 0xD1, 0x01, 0x00, 0xF5, 0xD1, 0x55, 0xCE, 0x40, 0xD2,
    0x01, 0x00, 0x08, 0xD2, 0x55, 0xCE, 0x40, 0xD2, 0x01, 0x00, 0x47, 0xD2, 0xA8, 0xD2, 0x01, 0x00, 0x4F, 0xD2,
    0xA8, 0xD2, 0x01, 0x00, 0xD7, 0xD0, 0x0D, 0xD1, 0x1F, 0xD1, 0xCF, 0xD2, 0x55, 0xCE, 0x01, 0x00, 0xD7, 0xD0,
    0x0D, 0xD1, 0x1F, 0xD1, 0xE2, 0xD2, 0x0A, 0xD3, 0x01, 0x00, 0x01, 0x00, 0x61, 0xCE, 0x8D, 0xCE, 0x95, 0xCE,
    0xEA, 0xCE, 0x72, 0xCE, 0x8D, 0xCE, 0x01, 0x00, 0x95, 0xCE, 0xEA, 0xCE, 0x72, 0xCE, 0x01, 0x00, 0x37, 0xD1,
    0x4B, 0xD1, 0x7E, 0xD1, 0xC7, 0xD1, 0x01, 0x00, 0xEE, 0x81, 0xDA, 0xEE, 0x81, 0xDA,
];
const SCRIPT_END: u16 = 0x0001;
const SCRIPT_RUNNING: u16 = 0xFFFF;
/// `$04:CE8A`: markers per difficulty.
const MARKERS_BY_DIFFICULTY: [u8; 3] = [2, 3, 3];
/// `$04:EE52`: the marker kinds, one byte per marker: a header of row
/// offsets (the map program picks DAEB from them), then the rows.
const MARKER_KINDS: [u8; 0xCC] = [
    0x06, 0x00, 0x0C, 0x00, 0x14, 0x00, 0x18, 0x00, 0x14, 0x00, 0x54, 0x00, 0x01, 0x00, 0x03, 0x00, 0x04, 0x00,
    0x03, 0x01, 0x04, 0x01, 0x04, 0x03, 0x00, 0x01, 0x02, 0x00, 0x01, 0x03, 0x00, 0x01, 0x04, 0x05, 0x01, 0x00,
    0x00, 0x02, 0x03, 0x00, 0x02, 0x04, 0x00, 0x02, 0x05, 0x00, 0x03, 0x04, 0x05, 0x03, 0x00, 0x00, 0x04, 0x05,
    0x01, 0x02, 0x03, 0x04, 0x02, 0x01, 0x01, 0x02, 0x05, 0x01, 0x03, 0x04, 0x05, 0x03, 0x01, 0x01, 0x04, 0x05,
    0x04, 0x03, 0x02, 0x05, 0x03, 0x02, 0x02, 0x04, 0x05, 0x03, 0x04, 0x05, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
    0x00, 0x01, 0x03, 0x05, 0x04, 0x02, 0x00, 0x01, 0x04, 0x05, 0x02, 0x03, 0x05, 0x01, 0x00, 0x02, 0x03, 0x04,
    0x00, 0x02, 0x03, 0x04, 0x05, 0x01, 0x00, 0x02, 0x04, 0x01, 0x03, 0x05, 0x00, 0x02, 0x05, 0x03, 0x01, 0x04,
    0x00, 0x03, 0x04, 0x02, 0x05, 0x01, 0x05, 0x03, 0x00, 0x04, 0x01, 0x02, 0x00, 0x04, 0x05, 0x01, 0x02, 0x03,
    0x01, 0x02, 0x03, 0x04, 0x05, 0x00, 0x04, 0x02, 0x01, 0x05, 0x03, 0x00, 0x01, 0x02, 0x05, 0x03, 0x00, 0x04,
    0x01, 0x03, 0x04, 0x02, 0x00, 0x05, 0x05, 0x03, 0x01, 0x00, 0x02, 0x04, 0x01, 0x04, 0x05, 0x00, 0x03, 0x02,
    0x04, 0x03, 0x02, 0x01, 0x05, 0x00, 0x05, 0x03, 0x02, 0x01, 0x04, 0x00, 0x02, 0x04, 0x05, 0x00, 0x01, 0x03,
    0x03, 0x04, 0x05, 0x02, 0x00, 0x01,
];
/// `$04:E2FE`: a marked place's timer by kind (words).
const MARKER_TIMERS: [u8; 14] = [0x38, 0x04, 0x28, 0x05, 0xDC, 0x05, 0xB0, 0x04, 0x18, 0x06, 0x90, 0x06, 0xF0, 0x00];
/// `$04:E6F2`: the wave lists (word offsets, zero-ended), each wave a
/// ship count and per ship a program index and a placement offset; a
/// placement is the place's position, spawn target, target and route.
const WAVES: [u8; 0x49] = [
    0x06, 0x00, 0x0A, 0x00, 0x10, 0x00, 0x16, 0x00, 0x00, 0x00, 0x1D, 0x00, 0x24, 0x00, 0x00, 0x00, 0x2B, 0x00,
    0x32, 0x00, 0x00, 0x00, 0x02, 0x00, 0x39, 0x00, 0x02, 0x41, 0x00, 0x02, 0x04, 0x39, 0x00, 0x06, 0x41, 0x00,
    0x02, 0x08, 0x39, 0x00, 0x0A, 0x41, 0x00, 0x02, 0x0C, 0x39, 0x00, 0x0E, 0x41, 0x00, 0x02, 0x10, 0x39, 0x00,
    0x12, 0x41, 0x00, 0x50, 0x28, 0x20, 0x80, 0x30, 0x58, 0x08, 0x90, 0xE0, 0x60, 0x38, 0xA0, 0x70, 0x78, 0x40,
    0xA8,
];
/// `$04:8E0A`: where a wave's escorts start, by DA3F.
const ESCORT_STARTS: [u8; 8] = [0xDA, 0x1F, 0xD4, 0x19, 0xE4, 0x21, 0xD2, 0x0F];
/// `$04:E593`: a wave place's kind and menu index.
const WAVE_PLACE_KIND: u8 = 8;
const WAVE_PLACE_MENU: u8 = 7;

// Ship travel (DB01) and wingmate (DB23) bits.
const TRAVEL_SET_OFF: u16 = 0x0003;
const TRAVEL_UNDER_WAY: u16 = 0x0002;
const TRAVEL_WARP: u16 = 0x0423;
const TRAVEL_PARKED: u16 = 0x0200;
const TRAVEL_HELD: u16 = 0x0400;
const WING_FOLLOWING: u16 = 0x0002;
const WING_CLOSE: u16 = 0x0200;
const WING_HELD: u16 = 0x0400;
// Unit flags.
const UNIT_GROUNDED: u16 = 0x0002;
const UNIT_INTERCEPTOR: u16 = 0x0008;
const UNIT_TARGETED: u16 = 0x2000;
const UNIT_HIDDEN: u16 = 0x4000;
const UNIT_SHOWN_BURNING: u16 = 0x6000;
/// Place flag 4000: the place flashes.
const PLACE_FLASHING: u16 = 0x4000;
const UNIT2_GUARD: u16 = 0x0400;
// 1B92.
const SPEED_FAST_FORWARD: u16 = 0x0008;
const SPEED_SCRIPTED: u16 = 0x0800;
// 1B8C.
const HOLD_UNITS_PICKABLE: u16 = 0x0002;
const HOLD_SHIP: u16 = 0x0004;

const CUE_WARP: u16 = 0x1A;
const CUE_GONE: u16 = 0x8B;
const CUE_CAPTURE: u16 = 0x0A;
const CUE_RELEASE: u16 = 0x63;
const CUE_HAUL: u16 = 0x61;

const SLOW_SPEED: u16 = 0x0258;
const WARP_SPEED: u16 = 0x0800;

fn script_word(offset: u16) -> Result<u16, ScreenError> {
    word_at(&SCRIPTS, usize::from(offset), 0x04CD6E).map_err(|_| ScreenError::TableOverrun(0x04CD6E))
}

fn table_byte(table: &[u8], index: usize, site: u32) -> Result<u8, ScreenError> {
    byte_at(table, index, site).map_err(|_| ScreenError::TableOverrun(site))
}

fn table_word(table: &[u8], index: usize, site: u32) -> Result<u16, ScreenError> {
    word_at(table, index, site).map_err(|_| ScreenError::TableOverrun(site))
}

fn set_low(word: &mut u16, byte: u8) {
    *word = (*word & 0xFF00) | u16::from(byte);
}

fn set_high(word: &mut u16, byte: u8) {
    *word = (*word & 0x00FF) | (u16::from(byte) << 8);
}

/// `$04:D34C`: a point offset toward the map's centre.
fn toward_centre(point: (u8, u8), offset: (u8, u8)) -> (u8, u8) {
    let x = if point.0 < 0x80 { point.0.wrapping_add(offset.0) } else { point.0.wrapping_sub(offset.0) };
    let y = if point.1 >= 0x60 { point.1.wrapping_sub(offset.1) } else { point.1.wrapping_add(offset.1) };
    (x, y)
}

impl Frame<'_> {
    /// `$04:CD47`.
    pub(super) fn run_script(&mut self) -> Result<(), ScreenError> {
        let script = self.screen.script;
        if script == 0 {
            return Ok(());
        }
        if (script as i16) >= 0 {
            self.screen.script_offset = script_word(script)?;
            self.screen.script = SCRIPT_RUNNING;
        }
        let handler = script_word(self.screen.script_offset)?;
        if handler == SCRIPT_END {
            self.screen.script = 0;
            return Ok(());
        }
        self.step(handler)
    }

    /// `$04:CE4E`.
    fn next_step(&mut self) {
        self.screen.script_offset = self.screen.script_offset.wrapping_add(2);
    }

    fn subject(&self, site: u32) -> Result<UnitId, ScreenError> {
        match self.screen.script_subject {
            ScriptSubject::Unit(id) => Ok(id),
            ScriptSubject::Value(_) => Err(ScreenError::MissingUnit(site)),
        }
    }

    fn warp_unit(&self, site: u32) -> Result<UnitId, ScreenError> {
        self.links.warp_unit.ok_or(ScreenError::MissingUnit(site))
    }

    fn step(&mut self, handler: u16) -> Result<(), ScreenError> {
        match handler {
            0xCE55 => {
                let id = self.subject(0x04CE55)?;
                let unit = self.map.unit_mut(id);
                unit.timer = unit.timer.wrapping_sub(1);
                if unit.timer == 0 {
                    self.next_step();
                }
            }
            0xCE61 => {
                self.launch_wave()?;
                self.map.globals.interceptors = 2;
                self.map.globals.hold &= !HOLD_UNITS_PICKABLE;
                self.next_step();
            }
            0xCE72 => {
                self.map.globals.interceptors = 1;
                self.screen.campaign.marker_count = self.markers_for_difficulty()?;
                self.place_markers()?;
                self.next_step();
            }
            0xCE8D => {
                if self.map.globals.interceptors == 0 {
                    self.next_step();
                }
            }
            0xCE95 => self.place_guards()?,
            0xCEEA => {
                if self.map.globals.launches_pending == 0 {
                    self.map.globals.stored_target = 0;
                    self.next_step();
                }
            }
            0xCEF9 => {
                self.screen.script_countdown = 0x1E;
                self.screen.ship.speed = SLOW_SPEED;
                self.warp_set_off();
            }
            0xCF07 => {
                self.screen.ship.speed = 0x0960;
                self.warp_set_off();
            }
            0xCF28 => {
                self.fly_ship();
                let ship = ((self.screen.ship.x >> 8) as u8, (self.screen.ship.y >> 8) as u8);
                let target = (self.screen.ship.target_x, self.screen.ship.target_y);
                if !near8(target, ship, 4) {
                    return Ok(());
                }
                self.screen.ship.travel = (self.screen.ship.travel & !TRAVEL_UNDER_WAY) | TRAVEL_PARKED;
                let unit = self.map.unit_mut(self.links.warp_unit.ok_or(ScreenError::MissingUnit(0x04CF3D))?);
                unit.target_x = 0x00EC;
                unit.target_y = 0x0018;
                self.next_step();
            }
            0xCF4F => {
                self.fly_ship();
                self.screen.script_countdown = self.screen.script_countdown.wrapping_sub(1);
                if self.screen.script_countdown == 0 {
                    self.next_step();
                }
            }
            0xCF5B => {
                let place = *self.place(self.screen.menu_place, 0x04CF5B)?;
                let id = self.warp_unit(0x04CF5E)?;
                self.screen.warp_target[0] = place.x as u8;
                self.screen.warp_target[1] = place.y as u8;
                let unit = self.map.unit_mut(id);
                set_low(&mut unit.target_x, place.x as u8);
                set_low(&mut unit.target_y, place.y as u8);
                self.next_step();
            }
            0xCF7A => {
                let place = self.links.warp_place.ok_or(ScreenError::MissingPlace(0x04CF7A))?;
                let id = self.warp_unit(0x04CF7D)?;
                let [x, y, _] = self.screen.warp_target;
                let record = &mut self.map.places[usize::from(place.0)];
                set_low(&mut record.x, x);
                set_low(&mut record.y, y);
                let unit = self.map.unit_mut(id);
                unit.x = x;
                unit.y = y;
                unit.flags &= !UNIT_HIDDEN;
                self.next_step();
            }
            0xCFA2 => self.warp_flash(0x24, CUE_CAPTURE)?,
            0xCFB4 => self.warp_flash(0x25, CUE_RELEASE)?,
            0xCFE4 => {
                let id = self.warp_unit(0x04CFE4)?;
                self.map.unit_mut(id).sprite = 0xFFFF;
                self.map.aim(id);
                self.cue(CUE_HAUL);
                self.next_step();
            }
            0xCFFB => {
                let id = self.warp_unit(0x04CFFB)?;
                self.map.integrate(id);
                if !self.map.reached_target(id) {
                    return Ok(());
                }
                let unit = self.map.unit_mut(id);
                unit.x = unit.target_x as u8;
                unit.y = unit.target_y as u8;
                unit.sprite = 0;
                self.next_step();
            }
            0xD022 => {
                let id = self.warp_unit(0x04D022)?;
                let ship = self.screen.ship;
                let x = (ship.x >> 8) as u8;
                let y = (ship.y >> 8) as u8;
                self.screen.saved_player = (
                    u16::from(x) | (u16::from(ship.x_pad) << 8),
                    u16::from(y) | ((ship.heading & 0x00FF) << 8),
                );
                let (target_x, target_y) = toward_centre((x, y), (0x18, 0x18));
                let unit = self.map.unit_mut(id);
                set_low(&mut unit.target_x, target_x);
                set_low(&mut unit.target_y, target_y);
                // Two word stores of the offsets' scratch words.
                self.screen.warp_target = [target_x, target_y, 0];
                self.next_step();
            }
            0xD04F => {
                let id = self.subject(0x04D04F)?;
                let unit = self.map.unit_mut(id);
                unit.timer = unit.timer.wrapping_sub(1);
                if unit.timer == 0 {
                    unit.flags &= !UNIT_HIDDEN;
                    self.next_step();
                }
            }
            0xD065 => self.arrive_from_warp()?,
            0xD0CF => self.start_warp_flight(0x0F),
            0xD0D7 => self.start_warp_flight(0x19),
            0xD10D => {
                let ship = &mut self.screen.ship;
                ship.flight_countdown = ship.flight_countdown.wrapping_sub(1);
                if ship.flight_countdown == 0 {
                    ship.travel &= !TRAVEL_SET_OFF;
                    self.next_step();
                } else {
                    self.fly_ship();
                }
            }
            0xD11F => {
                if self.screen.wingmate.flags & WING_FOLLOWING != 0 {
                    return Ok(());
                }
                self.screen.ship.speed = SLOW_SPEED;
                self.screen.ship.travel &= !TRAVEL_HELD;
                self.next_step();
            }
            0xD137 => {
                let flags = self.screen.wingmate.flags;
                if flags & WING_FOLLOWING == 0 && flags & WING_CLOSE != 0 {
                    self.next_step();
                }
            }
            0xD14B => {
                self.screen.ship.travel |= TRAVEL_HELD;
                let wing = &mut self.screen.wingmate;
                wing.flags = (wing.flags | WING_HELD) & !WING_CLOSE;
                self.screen.ship.speed = WARP_SPEED;
                wing.speed = 0xF800;
                self.screen.ship.flight_countdown = 0x20;
                self.screen.saved_ship = (self.screen.ship.x, self.screen.ship.y);
                self.next_step();
            }
            0xD17E => {
                let (heading, carry) = self.screen.ship.heading.overflowing_add(0x0800);
                self.screen.ship.heading = heading;
                let wing = &mut self.screen.wingmate;
                wing.heading = wing.heading.wrapping_add(0x0800).wrapping_add(u16::from(carry));
                let ship = &mut self.screen.ship;
                ship.flight_countdown = ship.flight_countdown.wrapping_sub(1);
                if ship.flight_countdown != 0 {
                    self.step_ship();
                    self.step_wingmate();
                    return Ok(());
                }
                let (x, y) = self.screen.saved_ship;
                self.screen.ship.x = x;
                self.screen.wingmate.x = x;
                self.screen.ship.y = y;
                self.screen.wingmate.y = y;
                self.screen.ship.speed = SLOW_SPEED;
                self.screen.wingmate.speed = SLOW_SPEED;
                self.screen.ship.travel &= !TRAVEL_HELD;
                self.screen.wingmate.flags &= !WING_HELD;
                self.next_step();
            }
            0xD1C7 => {
                self.screen.wingmate.x = self.screen.ship.x;
                self.screen.wingmate.y = self.screen.ship.y;
                self.screen.wingmate.flags &= !WING_HELD;
                self.next_step();
            }
            0xD1DC => {
                let id = self.map.globals.met_unit.ok_or(ScreenError::MissingUnit(0x04D1DC))?;
                self.map.dive(id);
                self.map.globals.stored_counter = 1;
                self.next_step();
            }
            0xD1EC => {
                if self.map.globals.stored_counter == 0 {
                    self.next_step();
                }
            }
            0xD1F5 => {
                let id = self.map.globals.met_unit.ok_or(ScreenError::MissingUnit(0x04D1F5))?;
                self.screen.script_subject = ScriptSubject::Unit(id);
                self.burn(id, 0x1C, 0x3F);
                self.cue(CUE_GONE);
                self.next_step();
            }
            0xD208 => {
                let id = self.map.globals.intercepted_unit.ok_or(ScreenError::MissingUnit(0x04D208))?;
                self.screen.script_subject = ScriptSubject::Unit(id);
                let unit = *self.map.unit(id);
                let home = unit.home.ok_or(ScreenError::MissingPlace(0x04D20E))?;
                self.map.places[usize::from(home.0)].flags &= 0x7FFF;
                if unit.flags2 & UNIT2_GUARD == 0 {
                    self.links.scene.campaign_events &= !0x0040;
                    self.screen.campaign.bases_left = self.screen.campaign.bases_left.wrapping_sub(1);
                    if self.screen.campaign.bases_left == 0 {
                        self.links.stage_flags &= !0x0100;
                    }
                }
                self.burn(id, 0x1C, 0x3F);
                self.cue(CUE_GONE);
                self.next_step();
            }
            0xD240 => {
                let id = self.subject(0x04D240)?;
                let mut scene = self.links.scene;
                self.map.destroy(id, &mut scene)?;
                self.links.scene = scene;
                self.next_step();
            }
            0xD247 => {
                let id = self.map.globals.met_unit.ok_or(ScreenError::MissingUnit(0x04D247))?;
                self.start_fall(id);
            }
            0xD24F => {
                let id = self.map.globals.intercepted_unit.ok_or(ScreenError::MissingUnit(0x04D24F))?;
                self.start_fall(id);
            }
            0xD2A8 => {
                let id = self.subject(0x04D2A8)?;
                self.screen.script_countdown = self.screen.script_countdown.wrapping_sub(1);
                if self.screen.script_countdown != 0 {
                    return self.run_unit(id);
                }
                if self.map.globals.speed_flags & SPEED_SCRIPTED != 0 {
                    self.map.globals.speed_flags &= !SPEED_SCRIPTED;
                    self.map.unit_mut(id).motion = self.screen.saved_motion;
                }
                self.map.globals.speed_flags &= !SPEED_FAST_FORWARD;
                self.next_step();
            }
            0xD2CF => {
                let id = self.map.globals.met_unit.ok_or(ScreenError::MissingUnit(0x04D2CF))?;
                self.screen.script_subject = ScriptSubject::Unit(id);
                self.burn(id, 0x1E, 0x46);
                self.cue(CUE_GONE);
                self.next_step();
            }
            0xD2E2 => {
                let id = self.screen.ship.destination.ok_or(ScreenError::MissingPlace(0x04D2E2))?;
                let place = &mut self.map.places[usize::from(id.0)];
                place.flags |= PLACE_FLASHING;
                place.marker = 0x001D;
                place.arrivals = 0;
                self.screen.script_countdown = 0x48;
                self.cue(CUE_GONE);
                self.next_step();
            }
            0xD30A => {
                self.screen.script_countdown = self.screen.script_countdown.wrapping_sub(1);
                if self.screen.script_countdown == 0 {
                    let id = self.screen.ship.destination.ok_or(ScreenError::MissingPlace(0x04D310))?;
                    self.map.places[usize::from(id.0)].flags &= !PLACE_FLASHING;
                    self.next_step();
                }
            }
            other => return Err(ScreenError::InvalidScriptStep(other)),
        }
        Ok(())
    }

    fn markers_for_difficulty(&self) -> Result<u16, ScreenError> {
        // An eight-bit table read with a word index (D7F2).
        Ok(u16::from(table_byte(&MARKERS_BY_DIFFICULTY, usize::from(self.links.difficulty), 0x04CE7B)?))
    }

    /// `$04:CF0D`: the ship sets off for the warp target.
    fn warp_set_off(&mut self) {
        self.screen.ship.travel |= TRAVEL_WARP;
        let [x, y, pad] = self.screen.warp_target;
        self.screen.ship.target_x = x;
        self.screen.ship.target_y = y;
        self.screen.ship.target_pad = pad;
        self.face_target();
        self.show_ship();
        self.next_step();
    }

    /// `$04:CFC6`: the warp unit flashes.
    fn warp_flash(&mut self, sprite: u16, cue: u16) -> Result<(), ScreenError> {
        let id = self.warp_unit(0x04CFA2)?;
        self.map.unit_mut(id).sprite = sprite;
        self.cue(cue);
        let unit = self.map.unit_mut(id);
        unit.flags |= UNIT_HIDDEN;
        unit.frame = 0;
        unit.timer = 0x18;
        unit.animation_timer = 0x18;
        self.screen.script_subject = ScriptSubject::Unit(id);
        self.next_step();
        Ok(())
    }

    /// `$04:D065`: the ship and wingmate come out at the warp target,
    /// facing the base, or the guarded place over hostile ground.
    fn arrive_from_warp(&mut self) -> Result<(), ScreenError> {
        let [x, y, _] = self.screen.warp_target;
        set_high(&mut self.screen.ship.x, x);
        set_high(&mut self.screen.wingmate.x, x);
        set_high(&mut self.screen.ship.y, y);
        set_high(&mut self.screen.wingmate.y, y);
        let index = usize::from(y >> 3) * TERRAIN_ROW + usize::from(x >> 3);
        let cell = *self.terrain.get(index).ok_or(ScreenError::TableOverrun(0x7F695B))?;
        let facing = if cell == 0 || cell == 8 { self.map.globals.base } else { self.map.globals.guarded_place };
        let place = *self.place(facing, 0x04D09D)?;
        self.screen.ship.target_x = place.x as u8;
        self.screen.ship.target_y = place.y as u8;
        // $04:D0B7.
        let (heading, sector) = bearing((x, y), (place.x as u8, place.y as u8));
        self.screen.ship.heading = heading;
        self.screen.ship.sector = sector;
        self.screen.wingmate.heading = heading;
        self.screen.wingmate.sector = sector;
        self.next_step();
        Ok(())
    }

    /// `$04:D0DD`.
    fn start_warp_flight(&mut self, frames: u16) {
        let ship = &mut self.screen.ship;
        ship.flight_countdown = frames;
        ship.travel &= !TRAVEL_PARKED;
        self.map.globals.hold &= !HOLD_SHIP;
        ship.travel |= TRAVEL_HELD;
        self.cue(CUE_WARP);
        let ship = &mut self.screen.ship;
        ship.flight_steps = 5;
        ship.travel |= TRAVEL_SET_OFF;
        ship.speed = WARP_SPEED;
        self.next_step();
    }

    /// `$04:D440`: the ship flies its course, kept on the map.
    fn fly_ship(&mut self) {
        let ship = &mut self.screen.ship;
        let (step_x, step_y) = rotate(ship.speed.wrapping_neg(), (ship.heading >> 8) as u8);
        ship.step_x = step_x;
        ship.step_y = step_y;
        set_low(&mut ship.sector, (((ship.heading >> 8) as u8).wrapping_add(8)) >> 4);
        self.show_ship();
        let ship = &mut self.screen.ship;
        let x = ship.x.wrapping_add(ship.step_x);
        if (0x0C00..0xF400).contains(&x) {
            ship.x = x;
        }
        let y = ship.y.wrapping_add(ship.step_y);
        if (0x1000..0xAC00).contains(&y) {
            ship.y = y;
        }
    }

    /// `$04:D93A`: the ship moves along its course.
    fn step_ship(&mut self) {
        let ship = &mut self.screen.ship;
        let (step_x, step_y) = rotate(ship.speed.wrapping_neg(), (ship.heading >> 8) as u8);
        ship.step_x = step_x;
        ship.step_y = step_y;
        ship.x = ship.x.wrapping_add(step_x);
        ship.y = ship.y.wrapping_add(step_y);
    }

    /// `$04:D49A`/`$04:D4AB`: the unit burns out.
    pub(super) fn burn(&mut self, id: UnitId, sprite: u16, frames: u16) {
        let unit = self.map.unit_mut(id);
        unit.sprite = sprite;
        unit.timer = frames;
        unit.animation_timer = frames;
        unit.frame = 0;
        unit.motion = 0;
        unit.flags |= UNIT_SHOWN_BURNING;
        unit.behavior = 0x11;
    }

    /// `$04:D255`: the falling unit's countdown; interceptors keep their
    /// motion across a scripted fast-forward.
    fn start_fall(&mut self, id: UnitId) {
        self.screen.script_subject = ScriptSubject::Unit(id);
        let unit = *self.map.unit(id);
        let frames = if unit.flags & UNIT_INTERCEPTOR != 0 {
            if unit.flags2 & UNIT2_GUARD != 0 {
                self.screen.script_countdown = 1;
                self.next_step();
                return;
            }
            self.map.globals.speed_flags |= SPEED_SCRIPTED;
            self.screen.saved_motion = unit.motion;
            self.map.unit_mut(id).motion = 1;
            0x3C
        } else if unit.flags & UNIT_GROUNDED != 0 {
            0x1E
        } else {
            self.map.globals.speed_flags |= SPEED_FAST_FORWARD;
            0x28
        };
        self.screen.script_countdown = frames;
        self.cue(CUE_WARP);
        self.next_step();
    }

    /// `$04:CE95`: guards at the places of the marker kinds, from DAEB.
    fn place_guards(&mut self) -> Result<(), ScreenError> {
        let count = self.markers_for_difficulty()?;
        self.screen.campaign.marker_count = count;
        self.map.globals.launches_pending = count;
        self.screen.campaign.guards_left = count;
        let mut kind_index = self.screen.campaign.marker_cursor;
        'kinds: loop {
            self.screen.script_subject = ScriptSubject::Value(kind_index);
            let kind = u16::from(table_byte(&MARKER_KINDS, usize::from(kind_index), 0x04CEAE)?);
            self.screen.campaign.guard_kind = kind;
            let mut next = self.map.place_head;
            while let Some(id) = next {
                let place = self.map.places[usize::from(id.0)];
                if place.flags & 0x0001 != 0 && place.kind == kind {
                    self.post_guard(Some(id))?;
                    self.screen.campaign.guards_left = self.screen.campaign.guards_left.wrapping_sub(1);
                    if self.screen.campaign.guards_left == 0 {
                        break 'kinds;
                    }
                    kind_index = kind_index.wrapping_add(1);
                    continue 'kinds;
                }
                next = place.next;
            }
            break;
        }
        self.map.globals.stored_target = 0x000C;
        self.next_step();
        Ok(())
    }

    /// `$04:E430`: a guard leaves the station for the place, or, in the
    /// alternate interception, for the player's surroundings.
    /// The place is read only on the station path; the alternate path is
    /// also entered with no place in hand (`$04:C589`).
    pub(super) fn post_guard(&mut self, place_id: Option<PlaceId>) -> Result<(), ScreenError> {
        let id = self.new_unit()?;
        let alternate = self.links.scene.campaign_events & 0x0080 != 0;
        let place = match place_id {
            Some(place_id) => self.map.places[usize::from(place_id.0)],
            None if alternate => Default::default(),
            None => return Err(ScreenError::MissingPlace(0x04E44D)),
        };
        let base = self.map.globals.base;
        let (player_x, player_y) = self.screen.saved_player;
        let unit = self.map.unit_mut(id);
        unit.x_fraction &= 0x00FF;
        unit.x = 0xEC;
        unit.y_fraction &= 0x00FF;
        unit.y = 0x18;
        let pattern = if !alternate {
            set_low(&mut unit.target_x, place.x as u8);
            set_low(&mut unit.target_y, place.y as u8);
            unit.speed = 0x1800;
            unit.behavior = 5;
            0x31
        } else {
            unit.home = base;
            unit.flags2 |= UNIT2_GUARD;
            let (x, y) = toward_centre((player_x as u8, player_y as u8), (0x0E, 0x0E));
            set_low(&mut unit.target_x, x);
            set_low(&mut unit.target_y, y);
            unit.speed = 0x2000;
            unit.behavior = 3;
            0x32
        };
        self.map.assign_pattern(id, pattern)?;
        self.map.globals.interceptor_count = self.map.globals.interceptor_count.wrapping_add(1);
        let unit = self.map.unit_mut(id);
        unit.flags |= UNIT_INTERCEPTOR | UNIT_TARGETED;
        unit.category = 7;
        unit.variant = 6;
        unit.radius = 2;
        unit.motion = 3;
        unit.node_variant = 0;
        Ok(())
    }

    /// `$7F:6136`: a unit from the pool, counted.
    pub(super) fn new_unit(&mut self) -> Result<UnitId, ScreenError> {
        let id = self.map.allocate_unit()?;
        self.map.globals.unit_count = self.map.globals.unit_count.wrapping_add(1);
        Ok(id)
    }

    /// `$04:E245`: the next marker kinds' places become marked, each with
    /// a guard.
    fn place_markers(&mut self) -> Result<(), ScreenError> {
        loop {
            let kind = u16::from(table_byte(
                &MARKER_KINDS,
                usize::from(self.screen.campaign.marker_cursor),
                0x04E24B,
            )?);
            let mut next = self.map.place_head;
            let mut found = None;
            while let Some(id) = next {
                let place = &self.map.places[usize::from(id.0)];
                if place.flags & 0x0001 != 0 && place.kind == kind {
                    found = Some(id);
                    break;
                }
                next = place.next;
            }
            let id = found.ok_or(ScreenError::MissingPlace(0x04E26C))?;
            let index = usize::from(id.0);
            if self.links.scene.stage_results & 0x0100 == 0 {
                self.links.scene.stage_results |= 0x0100;
                self.map.places[index].timer = 1;
                self.map.places[index].flags &= 0xFDFF;
            } else {
                let kind = self.map.places[index].kind & 0x00FF;
                self.map.places[index].timer = table_word(&MARKER_TIMERS, usize::from(kind) * 2, 0x04E293)?;
            }
            let place = &mut self.map.places[index];
            place.flags = (place.flags | 0x2000) & 0xEFFF;
            place.state = 2;
            place.marker = 0x8000;
            let menu_index = place.menu_index;
            self.screen.campaign.marker_cursor = self.screen.campaign.marker_cursor.wrapping_add(1);
            let guarded = if self.links.difficulty != 2 {
                true
            } else if self.links.scene.stage_results & 0x1000 == 0 {
                false
            } else {
                let confirm = self.screen.menu_confirm;
                if confirm != 6 && confirm == menu_index {
                    self.links.scene.campaign_events |= 0x0004;
                }
                true
            };
            if guarded {
                self.map.places[index].flags |= 0x8000;
                self.station_guard(id)?;
            }
            self.screen.campaign.markers_placed = self.screen.campaign.markers_placed.wrapping_add(1);
            self.screen.campaign.marker_count = self.screen.campaign.marker_count.wrapping_sub(1);
            if self.screen.campaign.marker_count == 0 {
                break;
            }
        }
        self.links.display_flags |= 0x0080;
        Ok(())
    }

    /// `$04:E3B9`: a guard waits at the marked place.
    fn station_guard(&mut self, place_id: PlaceId) -> Result<(), ScreenError> {
        self.map.globals.interceptor_count = self.map.globals.interceptor_count.wrapping_add(1);
        let id = self.new_unit()?;
        let place = self.map.places[usize::from(place_id.0)];
        let marker_count = self.screen.campaign.marker_count;
        let unit = self.map.unit_mut(id);
        unit.home = Some(place_id);
        let x = (place.x as u8).wrapping_add(4);
        let y = (place.y as u8).wrapping_sub(4);
        unit.x = x;
        set_low(&mut unit.target_x, x);
        unit.y = y;
        set_low(&mut unit.target_y, y);
        unit.x_fraction = 0;
        unit.y_fraction = 0;
        unit.behavior = 0;
        unit.category = 7;
        unit.flags = (unit.flags | UNIT_INTERCEPTOR) & !UNIT_TARGETED;
        unit.flags2 |= UNIT2_GUARD;
        unit.variant = 6;
        unit.radius = 0x0A;
        unit.motion = 0;
        unit.node_variant = marker_count;
        self.map.assign_pattern(id, 0x31)?;
        Ok(())
    }

    /// `$04:E571`: the next wave of the list arrives: its places and an
    /// escort for each.
    pub(super) fn launch_wave(&mut self) -> Result<(), ScreenError> {
        let cursor = self.screen.campaign.wave_cursor;
        let wave = table_word(&WAVES, usize::from(cursor), 0x04E577)?;
        if wave == 0 {
            return Ok(());
        }
        self.screen.campaign.wave_cursor = cursor.wrapping_add(2);
        let mut at = usize::from(wave);
        let ships = table_byte(&WAVES, at, 0x04E588)?;
        set_low(&mut self.links.batch_bonus, ships);
        set_low(&mut self.screen.campaign.wave_ships, ships);
        at += 1;
        loop {
            if self.map.globals.launch_count as u8 == 0 {
                return Ok(());
            }
            let program = table_byte(&WAVES, at, 0x04E5A5)?;
            let placement = usize::from(table_word(&WAVES, at + 1, 0x04E5AF)?);
            at += 3;
            let bytes: Vec<u8> = (0..8)
                .map(|k| table_byte(&WAVES, placement + k, 0x04E5B9))
                .collect::<Result<_, _>>()?;
            let id = self.new_place(&bytes[..4])?;
            let index = usize::from(id.0);
            let place = &mut self.map.places[index];
            set_low(&mut place.target_x, bytes[4]);
            set_low(&mut place.target_y, bytes[5]);
            set_low(&mut place.route_x, bytes[6]);
            set_low(&mut place.route_y, bytes[7]);
            place.flags |= 0x2004;
            place.status &= 0xFFFE;
            if self.screen.campaign.wave_ships & 0x0001 != 0 {
                place.status |= 0x0001;
            }
            place.program = table_word(&PROGRAMS, usize::from(program), 0x7F6C10)?;
            place.state = 1;
            self.launch_escort(id, (bytes[0], bytes[1]))?;
            self.screen.campaign.wave_escorts = self.screen.campaign.wave_escorts.wrapping_add(2);
            let remaining = (self.screen.campaign.wave_ships as u8).wrapping_sub(1);
            set_low(&mut self.screen.campaign.wave_ships, remaining);
            if remaining == 0 {
                return Ok(());
            }
        }
    }

    /// `$04:E4DA`: a place from the free list, first in the list, set at
    /// `placement` (position, spawn target).
    fn new_place(&mut self, placement: &[u8]) -> Result<PlaceId, ScreenError> {
        let id = self.map.place_free.ok_or(ScreenError::PlacePoolExhausted)?;
        let index = usize::from(id.0);
        if index >= PLACE_CAPACITY {
            return Err(ScreenError::MissingPlace(0x04E4E1));
        }
        self.map.place_free = self.map.places[index].next;
        let head = self.map.place_head;
        self.map.places[index].next = head;
        self.map.places[index].prev = None;
        self.map.place_head = Some(id);
        if let Some(next) = head {
            self.map.places[usize::from(next.0)].prev = Some(id);
        }
        let difficulty = self.links.difficulty as u8;
        let place = &mut self.map.places[index];
        set_low(&mut place.x, placement[0]);
        set_low(&mut place.cell_x, placement[0] >> 3);
        set_low(&mut place.y, placement[1]);
        set_low(&mut place.cell_y, placement[1] >> 3);
        set_low(&mut place.spawn_target_x, placement[2]);
        set_low(&mut place.spawn_target_y, placement[3]);
        set_low(&mut place.info, difficulty << 1);
        place.kind = u16::from(WAVE_PLACE_KIND);
        place.menu_index = u16::from(WAVE_PLACE_MENU);
        Ok(id)
    }

    /// `$04:E64E`: the wave place's escort sets out from the next start.
    fn launch_escort(&mut self, place_id: PlaceId, destination: (u8, u8)) -> Result<(), ScreenError> {
        let id = self.new_unit()?;
        let start = self.screen.campaign.start_cursor.wrapping_sub(1);
        self.screen.campaign.start_cursor = start;
        let at = usize::from(start.wrapping_mul(2));
        let x = table_byte(&ESCORT_STARTS, at, 0x04E65F)?.wrapping_add(8);
        let y = table_byte(&ESCORT_STARTS, at + 1, 0x04E669)?.wrapping_add(8);
        let unit = self.map.unit_mut(id);
        unit.x = x;
        unit.y = y;
        set_low(&mut unit.target_x, destination.0);
        set_low(&mut unit.target_y, destination.1);
        unit.speed = 0x1000;
        self.map.aim(id);
        let escorts = self.screen.campaign.wave_escorts;
        let unit = self.map.unit_mut(id);
        set_low(&mut unit.category, WAVE_PLACE_KIND);
        unit.flags |= 0x6040;
        unit.flags2 |= 0x0004;
        unit.motion = 0;
        unit.sprite = 0;
        unit.behavior = 1;
        unit.timer = 1;
        set_low(&mut unit.radius, 8);
        unit.home = Some(place_id);
        unit.variant = 7;
        unit.kind = escorts;
        unit.count = 0x32;
        unit.strength = 1;
        let place = &mut self.map.places[usize::from(place_id.0)];
        place.held_unit = Some(id);
        place.info = escorts;
        Ok(())
    }
}

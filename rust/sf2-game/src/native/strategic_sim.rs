//! The strategic map's real-time simulation (`$7F:537D`), run every fourth
//! frame on the strategic map and, through the radar-region service
//! (`$7F:539C` -> `$7F:535E`), during stages: the places' states and spawn
//! programs (`$7F:55FE`, `$7F:5943`), the units' behaviors (`$7F:5693`,
//! `$7F:5A8D`), the home base (`$7F:55A0`), the defense satellite
//! (`$7F:53E1`), the units' meetings with the player and the planets
//! (`$7F:649A`) and their motion (`$7F:66E6`).
//!
//! Places and units are the source's two linked record pools. Their
//! identities are slot numbers; the lists keep the source order, and a
//! reused unit keeps whatever its slot held, as in the source.

use sf_core::aim_angle::atan16;
use sf_core::snes_trig::SINTAB;

pub const PLACE_CAPACITY: usize = 15;
pub const UNIT_CAPACITY: usize = 23;
/// The encounter slot tables (D800, D7C2, D7A2, E805, E815).
pub const ENCOUNTER_SLOTS: usize = 16;
/// The terrain grid's row length (`$7F:D400`, 8-pixel cells).
pub const TERRAIN_ROW: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlaceId(pub u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitId(pub u8);

/// A place record (0x56 bytes): planets, bases and fleets on the map.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapPlace {
    pub next: Option<PlaceId>,
    pub prev: Option<PlaceId>,
    /// +04: the place's kind.
    pub kind: u16,
    /// +06/+08: the map screen's menu position and info line, which the
    /// simulation never reads.
    pub menu_index: u16,
    pub info: u16,
    /// +0C/+0E: map position (words; the low bytes are the coordinates).
    pub x: u16,
    pub y: u16,
    /// +10/+12: where its units head when spawned.
    pub spawn_target_x: u16,
    pub spawn_target_y: u16,
    /// +14/+16: the position's terrain cell (set when a wave places it).
    pub cell_x: u16,
    pub cell_y: u16,
    /// +18/+1A: the spawn program's position (`$7F:6C10`) and repeats.
    pub program: u16,
    pub program_repeats: u16,
    /// +1C/+1E: flags and status.
    pub flags: u16,
    pub status: u16,
    /// +20: the map screen's marker word.
    pub marker: u16,
    /// +22: units arrived here.
    pub arrivals: u16,
    /// +24: the heading its last departing unit took.
    pub heading: u16,
    /// +26: the last unit it spawned; +28 the unit it holds.
    pub spawned_unit: Option<UnitId>,
    pub held_unit: Option<UnitId>,
    /// +2A/+2C: a target position its departing units adopt.
    pub target_x: u16,
    pub target_y: u16,
    /// +2E/+30: the wave's route bytes.
    pub route_x: u16,
    pub route_y: u16,
    /// +32/+34: the threat warning's kind and countdown.
    pub warning: u16,
    pub warning_countdown: u16,
    /// +36: the state; +3A/+3C the remaining and batch spawn counts.
    pub state: u16,
    pub spawn_remaining: u16,
    pub spawn_batch: u16,
    /// +3E: the state timer.
    pub timer: u16,
    /// +4A/+4C.
    pub warning_rate: u16,
    pub warning_bias: u16,
}

/// A unit record (0x52 bytes): fleets, missiles, the satellite and others.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapUnit {
    pub next: Option<UnitId>,
    pub prev: Option<UnitId>,
    /// +04/+06: behavior and the behavior saved by a meeting.
    pub behavior: u16,
    pub saved_behavior: u16,
    /// +08: the behavior timer.
    pub timer: u16,
    /// +0A/+0C: the scripted program's step and timer.
    pub program: u16,
    pub program_timer: u16,
    /// +0E/+10: motion mode and the mode saved by a meeting.
    pub motion: u16,
    pub saved_motion: u16,
    /// +12/+14: the place it is bound to and the place that spawned it.
    pub home: Option<PlaceId>,
    pub origin: Option<PlaceId>,
    /// +1A..+1C and +1D..+1F: 8.16 fixed-point position.
    pub x_fraction: u16,
    pub x: u8,
    pub y_fraction: u16,
    pub y: u8,
    /// +20/+22: target position (words; the low bytes are coordinates).
    pub target_x: u16,
    pub target_y: u16,
    /// +24/+26: heading (fine angle) and speed.
    pub heading: u16,
    pub speed: u16,
    /// +28..+2A and +2B..+2D: 8.16 velocity.
    pub vx_fraction: u16,
    pub vx: u8,
    pub vy_fraction: u16,
    pub vy: u8,
    /// +2E/+30: flags.
    pub flags: u16,
    pub flags2: u16,
    /// +32..+3E: classification.
    pub category: u16,
    pub kind: u16,
    pub variant: u16,
    pub strength: u16,
    pub encounter_slot: u16,
    pub count: u16,
    pub node_variant: u16,
    /// +40: the proximity radius; +42 the delay before it checks.
    pub radius: u16,
    pub proximity_delay: u16,
    /// +44..+48: sprite, frame and animation timer.
    pub sprite: u16,
    pub frame: u16,
    pub animation_timer: u16,
    /// +4A..+4E: turn rate, turn bias and turn countdown.
    pub turn_rate: u16,
    pub turn_bias: u16,
    pub turn_countdown: u16,
}

/// The simulation's own shared words.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StrategicGlobals {
    /// 1B8C: 01 holds the places' warnings, the base and the satellite;
    /// 02 holds the whole tick.
    pub hold: u16,
    /// 1B90: per-pass holds.
    pub pass_holds: u16,
    /// 1B92: 0008 fast-forward, 0800 holds scripted programs.
    pub speed_flags: u16,
    /// 1B94.
    pub campaign_flags: u16,
    /// 1B74: set to 2 when a unit meets the player.
    pub encounter_request: u16,
    /// DA39, DA43, DA45, DA47, DA4D, DA53, DA55, DA57: unit counters.
    pub launch_count: u16,
    pub unit_count: u16,
    pub escort_count: u16,
    pub fighter_launches: u16,
    pub cruiser_count: u16,
    pub missile_count: u16,
    pub missile_salvo: u16,
    pub interceptor_count: u16,
    /// DA69..DA73: the most urgent threat to the base.
    pub threat_kind: u16,
    pub threat_place: Option<PlaceId>,
    pub threat_countdown: u16,
    pub threat_nearest: u16,
    /// D998, D9B0.
    pub interceptors: u16,
    pub launches_pending: u16,
    /// D9F9 (word read, byte written) and D9FB: spawn pattern cursors.
    pub spawn_pattern: u16,
    pub escort_pattern: u16,
    /// DB33: map events for the presentation.
    pub map_events: u16,
    /// DB3B: the last spawned unit's variant bit.
    pub spawn_variant_bit: u16,
    /// DB45: the place the satellite guards; DB61 the home base.
    pub guarded_place: Option<PlaceId>,
    pub base: Option<PlaceId>,
    /// E079/E085..E095: the defense satellite and its program.
    pub satellite: Option<UnitId>,
    pub satellite_phase: u16,
    pub satellite_flags: u16,
    pub satellite_hold: u16,
    pub satellite_target: Option<UnitId>,
    pub satellite_guarding: u16,
    pub satellite_facing: u16,
    /// E097..E09D: the unit meeting the player and an intercepted unit's
    /// saved modes.
    pub met_unit: Option<UnitId>,
    pub intercepted_unit: Option<UnitId>,
    pub intercepted_motion: u16,
    pub intercepted_behavior: u16,
    /// F55C.
    pub alerts: u16,
    /// 1CD3 bit 10.
    pub threat_cue: bool,
    /// D7FC, DA11.
    pub stored_target: u16,
    pub stored_counter: u16,
    /// D7FE and the encounter slot tables.
    pub slot_cursor: u16,
    pub slot_used: [u8; ENCOUNTER_SLOTS],
    pub slot_pattern: [u8; ENCOUNTER_SLOTS],
    pub slot_word: [u16; ENCOUNTER_SLOTS],
    pub slot_kind: [u8; ENCOUNTER_SLOTS],
    pub slot_strength: [u8; ENCOUNTER_SLOTS],
}

/// Scene words the simulation shares with the stage.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SceneLinks {
    /// 1B86 (read only), 1B88, 1B8A, 1C0E.
    pub stage_results: u16,
    pub scene_events: u16,
    pub campaign_events: u16,
    pub event_word: u16,
    /// DB4B: planet damage pending.
    pub planet_damage: u16,
    /// DB5B: the terrain cell under the met unit.
    pub map_region: u16,
    /// D79D and 1E09: the encounter result and node variant.
    pub encounter_result: u16,
    pub node_variant: u8,
}

/// What the tick reads but never writes.
#[derive(Debug, Clone, Copy)]
pub struct StrategicInputs<'a> {
    /// D7F2: 0..=2.
    pub difficulty: u16,
    /// DAF3/DAF6: the player's map position.
    pub player: (u8, u8),
    /// DA3B.
    pub batch_bonus: u16,
    /// 1BA3: the satellite's timer selector (byte offset).
    pub satellite_timing: u16,
    /// E089: the satellite stays on its third phase while nonzero.
    pub satellite_busy: u16,
    /// The terrain grid (`$7F:D400`).
    pub terrain: &'a [u8],
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StrategicMap {
    pub places: [MapPlace; PLACE_CAPACITY],
    pub units: [MapUnit; UNIT_CAPACITY],
    /// DB67/DB65 and E0A3/E0A5.
    pub place_head: Option<PlaceId>,
    pub place_free: Option<PlaceId>,
    pub unit_head: Option<UnitId>,
    pub unit_free: Option<UnitId>,
    pub globals: StrategicGlobals,
}

/// One tick's requests for other owners.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TickOutput {
    /// `$7F:6E09` cue numbers, in order.
    pub cues: Vec<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimError {
    Unported(u32),
    MissingSatellite,
    MissingBase,
    MissingPlace(u32),
    MissingUnit(u32),
    /// `$7F:6AD6`: the unit pool is empty (the source halts).
    UnitPoolExhausted,
    /// `$7F:645E` spins forever when every encounter slot is in use.
    EncounterSlotsExhausted,
    TableOverrun(u32),
}

// Unit flags (+2E).
const UNIT_MET: u16 = 0x0001;
const UNIT_GROUNDED: u16 = 0x0002;
const UNIT_ARRIVED: u16 = 0x0004;
const UNIT_INTERCEPTOR: u16 = 0x0008;
const UNIT_CARRIER: u16 = 0x0010;
const UNIT_FIGHTER: u16 = 0x0020;
const UNIT_ACTIVE: u16 = 0x0040;
const UNIT_MISSILE: u16 = 0x0080;
const UNIT_SPECIAL: u16 = 0x0800;
const UNIT_TARGETED: u16 = 0x2000;
const UNIT_HIDDEN: u16 = 0x4000;

const FAST_FORWARD: u16 = 0x0008;
const LIMIT_UNITS: u16 = 0x10;

/// The source's `CMP`/`BPL`: the difference's sign bit is clear.
fn at_least(value: u16, limit: u16) -> bool {
    value.wrapping_sub(limit) & 0x8000 == 0
}

impl StrategicMap {
    fn place(&self, id: Option<PlaceId>, site: u32) -> Result<MapPlace, SimError> {
        id.map(|id| self.places[usize::from(id.0)]).ok_or(SimError::MissingPlace(site))
    }
    fn place_mut(&mut self, id: Option<PlaceId>, site: u32) -> Result<&mut MapPlace, SimError> {
        id.map(|id| &mut self.places[usize::from(id.0)]).ok_or(SimError::MissingPlace(site))
    }
    pub(crate) fn unit(&self, id: UnitId) -> &MapUnit {
        &self.units[usize::from(id.0)]
    }
    pub(crate) fn unit_mut(&mut self, id: UnitId) -> &mut MapUnit {
        &mut self.units[usize::from(id.0)]
    }

    /// `$7F:537D`: one simulation tick.
    pub fn tick(
        &mut self,
        links: &mut SceneLinks,
        inputs: StrategicInputs<'_>,
        output: &mut TickOutput,
    ) -> Result<(), SimError> {
        if self.globals.hold & 0x0002 != 0 {
            return Ok(());
        }
        self.advance_places(links, inputs)?;
        self.advance_units(links, inputs)?;
        self.advance_base(links, inputs)?;
        self.advance_satellite(links, inputs)?;
        self.advance_units_second(links, inputs, output)?;
        self.check_meetings(links, inputs)?;
        self.advance_motion()
    }

    /// `$7F:6908`: the fast-forward speeds countdowns up fourfold.
    fn countdown_step(&self) -> u16 {
        if self.globals.speed_flags & FAST_FORWARD != 0 {
            4
        } else {
            1
        }
    }

    // ---- places ($7F:55FE) ----

    pub(crate) fn advance_places(&mut self, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        let mut cursor = self.place_head;
        while let Some(id) = cursor {
            let place = self.places[usize::from(id.0)];
            if place.flags & 0x0001 != 0 && place.flags & 0x1200 == 0 {
                match place.state & 0x00FF {
                    0 => {}
                    // $7F:5630.
                    1 => {
                        if self.globals.pass_holds & 0x0008 == 0 {
                            let row = byte_at(&PLACE_TIMER_ROWS, usize::from(inputs.difficulty), 0x7F563D)?;
                            let index = usize::from(row.wrapping_add(place.kind as u8));
                            let timer = word_at(&PLACE_TIMERS, index * 2, 0x7F564A)?;
                            let place = &mut self.places[usize::from(id.0)];
                            place.timer = timer;
                            place.state = place.state.wrapping_add(1);
                        }
                    }
                    // $7F:5656.
                    2 => {
                        if place.flags & 0x0280 == 0 {
                            self.place_timer(id);
                        }
                    }
                    // $7F:5663.
                    3 => {
                        if !at_least(self.globals.unit_count, LIMIT_UNITS)
                            && !at_least(self.globals.escort_count, 6)
                            && place.flags & 0x1A80 == 0
                        {
                            self.spawn_unit(id, links, inputs)?;
                            let place = &mut self.places[usize::from(id.0)];
                            place.flags |= 0x0800;
                            place.state = 0;
                        }
                    }
                    other => return Err(SimError::Unported(0x7F5626 + u32::from(other) * 2)),
                }
            }
            cursor = self.places[usize::from(id.0)].next;
        }
        Ok(())
    }

    /// `$7F:600A`: count the place's timer down; at zero or below its state
    /// advances. Returns whether it did.
    fn place_timer(&mut self, id: PlaceId) -> bool {
        let step = self.countdown_step();
        let place = &mut self.places[usize::from(id.0)];
        place.timer = place.timer.wrapping_sub(step);
        if place.timer == 0 || (place.timer as i16) < 0 {
            place.state = place.state.wrapping_add(1);
            return true;
        }
        false
    }

    // ---- the base ($7F:55A0) ----

    fn advance_base(&mut self, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        if self.globals.hold & 0x0001 != 0 || self.globals.pass_holds & 0x0020 != 0 {
            return Ok(());
        }
        let base = self.globals.base.ok_or(SimError::MissingBase)?;
        match self.places[usize::from(base.0)].state & 0x00FF {
            // $7F:55C7.
            0 => {
                if self.globals.satellite_guarding != 0 {
                    return Ok(());
                }
                let timer = if inputs.difficulty.wrapping_sub(1) == 0 { 0x0780 } else { 0x05A0 };
                let place = &mut self.places[usize::from(base.0)];
                place.timer = timer;
                place.state = place.state.wrapping_add(1);
            }
            1 => {
                self.place_timer(base);
            }
            // $7F:55E8.
            2 => {
                if !at_least(self.globals.unit_count, LIMIT_UNITS) {
                    self.spawn_unit(base, links, inputs)?;
                    let place = &mut self.places[usize::from(base.0)];
                    place.state = place.state.wrapping_add(1);
                }
            }
            3 => {}
            other => return Err(SimError::Unported(0x7F55BF + u32::from(other) * 2)),
        }
        Ok(())
    }

    // ---- units, first pass ($7F:5693) ----

    pub(crate) fn advance_units(&mut self, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        if self.globals.pass_holds & 0x0001 != 0 {
            return Ok(());
        }
        self.globals.threat_nearest = 0xFFFF;
        let mut cursor = self.unit_head;
        while let Some(id) = cursor {
            let next = self.unit(id).next;
            if self.unit(id).flags & UNIT_ACTIVE != 0 {
                match self.unit(id).behavior {
                    0 => {}
                    // $7F:56D1.
                    1 => {
                        if self.behavior_timer(id) {
                            self.unit_mut(id).behavior = 2;
                        }
                    }
                    // $7F:56DD.
                    2 => {
                        let unit = self.unit_mut(id);
                        unit.sprite = 0x19;
                        unit.frame = 0;
                        unit.behavior = 3;
                        unit.timer = 0x18;
                        unit.animation_timer = 0x18;
                        self.globals.launch_count = self.globals.launch_count.wrapping_sub(1);
                    }
                    // $7F:56FC.
                    3 => {
                        if self.behavior_timer(id) {
                            let unit = self.unit_mut(id);
                            unit.frame = 0;
                            unit.sprite = 0xFFFF;
                            unit.motion = 1;
                            unit.timer = 1;
                            unit.behavior = 4;
                        }
                    }
                    // $7F:5720.
                    4 => {
                        if self.unit(id).timer != 0 {
                            if self.reached_target(id) {
                                self.unit_mut(id).timer = 0;
                            }
                        } else {
                            let home = self.place(self.unit(id).home, 0x7F5731)?;
                            let unit = self.unit_mut(id);
                            unit.x = home.x as u8;
                            unit.y = (home.y as u8).wrapping_sub(1);
                            unit.sprite = 0x1A;
                            unit.frame = 0;
                            unit.behavior = 5;
                            unit.timer = 0x1B;
                            unit.animation_timer = 0x1B;
                            unit.motion = 0;
                            links.event_word |= 0x0010;
                        }
                    }
                    // $7F:576E.
                    5 => {
                        if self.behavior_timer(id) {
                            self.depart(id)?;
                        }
                    }
                    // $7F:5778.
                    6 => self.hold_home(id, links, inputs)?,
                    // $7F:57F4.
                    7 => self.warn_threat(id, links)?,
                    // $7F:57ED.
                    8 => {
                        self.warn_threat(id, links)?;
                        self.arrive(id, links, inputs)?;
                    }
                    other => return Err(SimError::Unported(0x7F56BE + u32::from(other) * 2)),
                }
            }
            cursor = next;
        }
        Ok(())
    }

    /// `$7F:5EE8`: count the behavior timer down; at zero, or past it, the
    /// behavior advances. Returns whether it did.
    fn behavior_timer(&mut self, id: UnitId) -> bool {
        let step = self.countdown_step();
        let unit = self.unit_mut(id);
        let (value, borrow) = unit.timer.overflowing_sub(step);
        unit.timer = value;
        if value == 0 || borrow {
            unit.behavior = unit.behavior.wrapping_add(1);
            return true;
        }
        false
    }

    /// `$7F:5920`: whether the unit is within its radius of its target.
    pub(crate) fn reached_target(&self, id: UnitId) -> bool {
        let unit = self.unit(id);
        within((u16::from(unit.x), u16::from(unit.y)), (unit.target_x, unit.target_y), unit.radius)
    }

    /// `$7F:58A6`: leave the home place for the place's target.
    fn depart(&mut self, id: UnitId) -> Result<(), SimError> {
        let home = self.place(self.unit(id).home, 0x7F58A6)?;
        let unit = self.unit_mut(id);
        unit.y = home.y as u8;
        unit.target_x = (unit.target_x & 0xFF00) | (home.target_x & 0x00FF);
        unit.target_y = (unit.target_y & 0xFF00) | (home.target_y & 0x00FF);
        unit.frame = 0;
        unit.sprite = 0x18;
        unit.animation_timer = 0x50;
        unit.flags = (unit.flags & !UNIT_TARGETED) | UNIT_HIDDEN;
        unit.flags2 |= 0x0014;
        unit.motion = 1;
        unit.behavior = 6;
        unit.sprite = 0x18;
        self.set_course(id, 0x19)
    }

    /// `$7F:590A`: set speed, steer at the target, and publish the heading
    /// to the home place.
    pub(crate) fn set_course(&mut self, id: UnitId, speed: u16) -> Result<(), SimError> {
        self.unit_mut(id).speed = speed;
        self.aim(id);
        let heading = self.unit(id).heading;
        self.place_mut(self.unit(id).home, 0x7F5911)?.heading = heading;
        Ok(())
    }

    /// `$7F:698F`: face the target, then recompute the velocity.
    pub(crate) fn aim(&mut self, id: UnitId) {
        let heading = self.heading_to_target(id);
        self.unit_mut(id).heading = heading;
        self.update_velocity(id);
    }

    /// `$7F:6964`: the fine angle of the unit as seen from its target.
    fn heading_to_target(&self, id: UnitId) -> u16 {
        let unit = self.unit(id);
        let dx = u16::from(unit.x).wrapping_sub(unit.target_x & 0x00FF);
        let dy = u16::from(unit.y).wrapping_sub(unit.target_y & 0x00FF);
        atan16(dx as i16, dy as i16)
    }

    /// `$7F:6995`/`$7F:69C7`: velocity from speed and heading.
    fn update_velocity(&mut self, id: UnitId) {
        let unit = self.unit(id);
        let index = ((unit.heading >> 8) as u8).wrapping_add(0x40);
        let x_factor = i16::from(SINTAB[usize::from(index.wrapping_add(0x40))]);
        let y_factor = i16::from(SINTAB[usize::from(index)]);
        let vx = scaled_product(unit.speed as i16, x_factor);
        let vy = scaled_product(unit.speed.wrapping_neg() as i16, y_factor);
        let unit = self.unit_mut(id);
        unit.vx_fraction = vx as u16;
        unit.vx = (vx >> 16) as u8;
        unit.vy_fraction = vy as u16;
        unit.vy = (vy >> 16) as u8;
    }

    /// `$7F:5778`: the place follows the unit; an arrival either runs the
    /// place's program or, for a unit flagged on a planet, turns the place
    /// into a warning (`$7F:5798`).
    fn hold_home(&mut self, id: UnitId, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        let unit = *self.unit(id);
        let home = self.place_mut(unit.home, 0x7F5778)?;
        home.x = (home.x & 0xFF00) | u16::from(unit.x);
        home.y = (home.y & 0xFF00) | u16::from(unit.y);
        if unit.flags & UNIT_ARRIVED == 0 {
            return self.place_arrival(id, links, inputs);
        }
        let unit = self.unit_mut(id);
        unit.flags &= 0xBFFB;
        unit.flags2 |= 0x0004;
        unit.motion = 0;
        unit.behavior = 8;
        let home_id = unit.home;
        let home = self.place_mut(home_id, 0x7F57B6)?;
        home.status |= 0x0002;
        home.warning_countdown = 0x0258;
        home.warning = 1;
        home.warning_rate = 2;
        home.warning_bias = 0;
        if links.stage_results & 0x0010 == 0 {
            self.globals.alerts |= 0x0008;
        } else {
            self.globals.map_events |= 0x0080;
        }
        Ok(())
    }

    /// `$7F:57F8`: count the home place's warning down and report it.
    fn warn_threat(&mut self, id: UnitId, links: &mut SceneLinks) -> Result<(), SimError> {
        if self.globals.hold & 0x0001 != 0 {
            return Ok(());
        }
        let home_id = self.unit(id).home;
        let home = self.place(home_id, 0x7F5800)?;
        if home.status & 0x0002 == 0 {
            return Ok(());
        }
        let mut expired = home.warning_countdown == 0;
        if !expired {
            let countdown = home.warning_countdown - 1;
            self.place_mut(home_id, 0x7F5812)?.warning_countdown = countdown;
            expired = countdown == 0;
            if !expired {
                if countdown >= self.globals.threat_nearest || links.scene_events & 0x0080 != 0 {
                    return Ok(());
                }
                self.globals.threat_place = home_id;
                self.globals.threat_countdown = countdown;
                self.globals.threat_nearest = countdown;
                self.globals.threat_kind = 1;
                return Ok(());
            }
        }
        // $7F:5838.
        if links.scene_events & 0x0080 != 0 {
            return Ok(());
        }
        self.globals.threat_place = home_id;
        self.globals.threat_countdown = self.place(home_id, 0x7F5843)?.warning_countdown;
        self.globals.threat_kind = 2;
        links.scene_events |= 0x0080;
        Ok(())
    }

    /// `$7F:57ED`: a stationed unit's arrival.
    fn arrive(&mut self, id: UnitId, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        self.place_arrival(id, links, inputs)
    }

    /// `$7F:5943`: an arrival at the home place runs the place's state.
    fn place_arrival(&mut self, id: UnitId, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        let home_id = self.unit(id).home.ok_or(SimError::MissingPlace(0x7F5944))?;
        let home = &mut self.places[usize::from(home_id.0)];
        home.arrivals = home.arrivals.wrapping_add(1);
        if self.globals.pass_holds & 0x0010 != 0 {
            return Ok(());
        }
        match self.places[usize::from(home_id.0)].state & 0x00FF {
            0 => Ok(()),
            1 => self.run_place_program(home_id, inputs),
            // $7F:5A3A.
            2 => {
                if self.places[usize::from(home_id.0)].flags & 0x0080 == 0 {
                    self.place_timer(home_id);
                }
                Ok(())
            }
            // $7F:5A47.
            3 => loop {
                if at_least(self.globals.unit_count, LIMIT_UNITS) || at_least(self.globals.cruiser_count, 2) {
                    return Ok(());
                }
                let place = &mut self.places[usize::from(home_id.0)];
                if place.spawn_remaining == 0 {
                    place.state = 1;
                    return Ok(());
                }
                place.spawn_remaining -= 1;
                self.spawn_unit(home_id, links, inputs)?;
            },
            // $7F:5A6D.
            4 => {
                if self.place_timer(home_id) {
                    let place = &mut self.places[usize::from(home_id.0)];
                    place.state = 0;
                    let held = place.held_unit.ok_or(SimError::MissingUnit(0x7F5A79))?;
                    self.unit_mut(held).flags2 &= !0x0004;
                    self.set_course(held, 0x0190)?;
                }
                Ok(())
            }
            other => Err(SimError::Unported(0x7F5961 + u32::from(other) * 2)),
        }
    }

    /// `$7F:596C`: one step of the place's spawn program (`$7F:6C10`
    /// rows: a kind byte, a timer word, a count byte and, for kinds 2 and
    /// 4, a repeat byte).
    fn run_place_program(&mut self, id: PlaceId, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        if at_least(self.globals.unit_count, LIMIT_UNITS) || at_least(self.globals.cruiser_count, 2) {
            return Ok(());
        }
        let index = usize::from(self.places[usize::from(id.0)].program);
        let kind = byte_at(&PROGRAMS, index, 0x7F5981)?;
        let next;
        match kind {
            0 => return Ok(()),
            1 => {
                self.load_program_row(id, index, inputs)?;
                next = Some(index + 4);
            }
            2 => {
                self.load_program_row(id, index, inputs)?;
                let repeats = byte_at(&PROGRAMS, index + 4, 0x7F59AE)?;
                let place = &mut self.places[usize::from(id.0)];
                place.flags |= 0x0100;
                place.program_repeats = u16::from(repeats);
                next = Some(index + 5);
            }
            3 => {
                let place = &mut self.places[usize::from(id.0)];
                if place.flags & 0x0100 != 0 {
                    return Ok(());
                }
                place.flags |= 0x0100;
                return self.repeat_program(id, index, inputs);
            }
            4 => {
                self.load_program_row(id, index, inputs)?;
                let repeats = byte_at(&PROGRAMS, index + 4, 0x7F59D3)?;
                self.places[usize::from(id.0)].program_repeats = u16::from(repeats);
                next = Some(index + 5);
            }
            5 => return self.repeat_program(id, index, inputs),
            other => return Err(SimError::Unported(0x7F598D + u32::from(other) * 2)),
        }
        let place = &mut self.places[usize::from(id.0)];
        if let Some(next) = next {
            place.program = next as u16;
        }
        place.state = place.state.wrapping_add(1);
        Ok(())
    }

    /// `$7F:59E0`: repeat the previous row, or step past the repeat.
    fn repeat_program(&mut self, id: PlaceId, index: usize, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        let place = &mut self.places[usize::from(id.0)];
        let repeats = place.program_repeats.wrapping_sub(1);
        if repeats == 0 {
            place.program = (index + 1) as u16;
            place.flags &= !0x0100;
            return Ok(());
        }
        place.program_repeats = repeats;
        let previous = index.checked_sub(5).ok_or(SimError::TableOverrun(0x7F59E9))?;
        self.load_program_row(id, previous, inputs)?;
        let place = &mut self.places[usize::from(id.0)];
        place.state = place.state.wrapping_add(1);
        Ok(())
    }

    /// `$7F:5A10`: a row's timer and spawn count.
    fn load_program_row(&mut self, id: PlaceId, index: usize, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        let timer = word_at(&PROGRAMS, index + 1, 0x7F5A10)?;
        let mut count = u16::from(byte_at(&PROGRAMS, index + 3, 0x7F5A17)?);
        if inputs.batch_bonus.wrapping_sub(1) == 0 {
            count = count.wrapping_add(1);
        }
        let place = &mut self.places[usize::from(id.0)];
        place.timer = timer;
        place.spawn_batch = count;
        place.spawn_remaining = count;
        place.flags |= 0x0400;
        Ok(())
    }

    // ---- the satellite ($7F:53E1) ----

    fn advance_satellite(&mut self, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        if self.globals.hold & 0x0001 != 0 {
            return Ok(());
        }
        let satellite = self.globals.satellite.ok_or(SimError::MissingSatellite)?;
        match self.globals.satellite_phase {
            // $7F:53FD.
            0 => {
                self.sweep(satellite);
                let unit = self.unit_mut(satellite);
                unit.timer = unit.timer.wrapping_sub(1);
                if unit.timer != 0 {
                    return Ok(());
                }
                unit.flags2 |= 0x0800;
                unit.timer = 0x10;
                self.globals.satellite_flags = (self.globals.satellite_flags | 0x0004) & !0x0002;
                links.event_word |= 0x0040;
                self.globals.satellite_phase = self.globals.satellite_phase.wrapping_add(1);
                Ok(())
            }
            1 => self.satellite_watch(satellite, links, inputs),
            // $7F:54D8.
            2 => {
                if inputs.satellite_busy != 0 {
                    return Ok(());
                }
                self.satellite_rest(inputs)
            }
            3 => self.satellite_rest(inputs),
            other => Err(SimError::Unported(0x7F53F5 + u32::from(other) * 2)),
        }
    }

    /// `$7F:542E`: the satellite looks for a unit in its sights.
    fn satellite_watch(&mut self, satellite: UnitId, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        if self.globals.satellite_flags & 0x0008 != 0 {
            if self.globals.satellite_guarding == 0 && links.stage_results & 0x0010 == 0 {
                return Ok(());
            }
            let target = self.globals.satellite_target.ok_or(SimError::MissingUnit(0x7F5444))?;
            self.globals.satellite_flags &= !0x0008;
            return self.satellite_fire(target, links, inputs);
        }
        self.sweep(satellite);
        let unit = self.unit_mut(satellite);
        unit.timer = unit.timer.wrapping_sub(1);
        if unit.timer != 0 {
            return Ok(());
        }
        unit.timer = 0x10;
        if self.globals.satellite_guarding != 0 {
            let guarded = self.place(self.globals.guarded_place, 0x7F546D)?;
            if self.in_sights(satellite, (guarded.x, guarded.y), 0x08) {
                self.globals.satellite_flags |= 0x0008;
            }
            return Ok(());
        }
        let mut cursor = self.unit_head;
        while let Some(id) = cursor {
            let unit = *self.unit(id);
            if unit.flags & 0xBFC9 == 0
                && self.in_sights(satellite, (u16::from(unit.x), u16::from(unit.y)), 0x0B)
            {
                self.globals.satellite_target = Some(id);
                if links.stage_results & 0x0010 != 0 {
                    return self.satellite_fire(id, links, inputs);
                }
                self.unit_mut(id).flags |= UNIT_TARGETED;
                self.globals.satellite_flags |= 0x0008;
                return Ok(());
            }
            cursor = unit.next;
        }
        Ok(())
    }

    /// `$7F:54CB`: the satellite destroys its target and rests.
    fn satellite_fire(&mut self, target: UnitId, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        self.globals.map_events |= 0x0200;
        self.destroy(target, links)?;
        self.satellite_rest(inputs)
    }

    /// `$7F:54DE`.
    fn satellite_rest(&mut self, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        let satellite = self.globals.satellite.ok_or(SimError::MissingSatellite)?;
        let timer = word_at(&SATELLITE_RESTS, usize::from(inputs.satellite_timing), 0x7F54F9)?;
        let unit = self.unit_mut(satellite);
        unit.flags2 &= !0x0800;
        unit.timer = timer;
        self.globals.satellite_flags = (self.globals.satellite_flags & !0x0004) | 0x0002;
        self.globals.satellite_phase = 0;
        Ok(())
    }

    /// `$7F:5532`/`$7F:5569`: whether `point` lies within `tolerance` of
    /// the satellite's facing.
    fn in_sights(&self, satellite: UnitId, point: (u16, u16), tolerance: u8) -> bool {
        let unit = self.unit(satellite);
        let facing = (unit.heading >> 8) as u8;
        let dx = u16::from(unit.x).wrapping_sub(point.0);
        let dy = u16::from(unit.y).wrapping_sub(point.1);
        let difference = ((atan16(dx as i16, dy as i16) >> 8) as u8).wrapping_sub(facing);
        difference <= tolerance || difference >= tolerance.wrapping_neg()
    }

    /// `$7F:550A`: every 60 ticks the satellite turns a sixteenth and
    /// publishes its facing.
    fn sweep(&mut self, satellite: UnitId) {
        let unit = self.unit_mut(satellite);
        unit.program_timer = unit.program_timer.wrapping_sub(1);
        if unit.program_timer != 0 {
            return;
        }
        unit.program_timer = 0x3C;
        unit.heading = unit.heading.wrapping_add(0x1000);
        let swapped = unit.heading.swap_bytes();
        self.globals.satellite_facing = (swapped.wrapping_add(8) & 0x00F0) >> 2;
    }

    // ---- units, second pass ($7F:5A8D) ----

    fn advance_units_second(
        &mut self,
        links: &mut SceneLinks,
        inputs: StrategicInputs<'_>,
        output: &mut TickOutput,
    ) -> Result<(), SimError> {
        if self.globals.pass_holds & 0x0002 != 0 {
            return Ok(());
        }
        let mut cursor = self.unit_head;
        while let Some(id) = cursor {
            let next = self.unit(id).next;
            if self.unit(id).flags & 0x8740 == 0 {
                self.behave(id, links, inputs, output)?;
            }
            cursor = next;
        }
        Ok(())
    }

    /// `$7F:66A6`: one unit's behavior (`$7F:5AB5` table).
    pub(crate) fn behave(
        &mut self,
        id: UnitId,
        links: &mut SceneLinks,
        inputs: StrategicInputs<'_>,
        output: &mut TickOutput,
    ) -> Result<(), SimError> {
        let unit = *self.unit(id);
        match unit.behavior & 0x00FF {
            0 => {}
            // $7F:5B67.
            1 => {
                if self.behavior_timer(id) {
                    if self.unit(id).flags & UNIT_FIGHTER == 0 {
                        self.launch(id, links, inputs)?;
                    } else {
                        self.unit_mut(id).turn_rate = 0x0200;
                        self.time_turn(id);
                        let unit = self.unit_mut(id);
                        unit.speed = 0x0200;
                        unit.motion = 7;
                    }
                }
            }
            // $7F:5B8A.
            2 => {
                if unit.timer == 0 {
                    self.launch(id, links, inputs)?;
                }
            }
            // $7F:5ADC.
            3 => {
                if self.reached_target(id) {
                    let unit = self.unit_mut(id);
                    unit.motion = 0;
                    unit.sprite = 0x23;
                    unit.frame = 0;
                    unit.behavior = 4;
                    unit.timer = 0x10;
                    unit.animation_timer = 0x10;
                }
            }
            // $7F:5B03.
            4 => {
                let unit = self.unit_mut(id);
                unit.timer = unit.timer.wrapping_sub(1);
                if unit.timer == 0 {
                    unit.flags &= !UNIT_HIDDEN;
                    unit.behavior = 7;
                    self.globals.launches_pending = self.globals.launches_pending.wrapping_sub(1);
                }
            }
            // $7F:5B1F.
            5 => {
                if self.reached_target(id) {
                    let unit = self.unit_mut(id);
                    unit.flags |= UNIT_HIDDEN;
                    unit.motion = 0;
                    unit.sprite = 0x26;
                    unit.frame = 0;
                    unit.behavior = 6;
                    unit.timer = 0x32;
                    unit.animation_timer = 0x32;
                    output.cues.push(0x008B);
                }
            }
            // $7F:5B56.
            6 => {
                let unit = self.unit_mut(id);
                unit.timer = unit.timer.wrapping_sub(1);
                if unit.timer == 0 {
                    self.destroy(id, links)?;
                    self.globals.launches_pending = self.globals.launches_pending.wrapping_sub(1);
                }
            }
            // $7F:5BC1.
            7 => {
                if unit.flags & UNIT_GROUNDED != 0 {
                    self.strike_planet(id, links)?;
                } else if unit.flags2 & 0x0100 != 0 {
                    self.run_program(id, inputs, links)?;
                } else if unit.flags & UNIT_MISSILE != 0 {
                    self.missile_flight(id, links, inputs)?;
                }
            }
            // $7F:5BE0.
            8 => self.strike_planet(id, links)?,
            // $7F:5CB3.
            9 => {
                if unit.timer == 0 {
                    self.burn_out(id);
                    if links.scene_events & 0x0010 == 0 {
                        self.unit_mut(id).behavior = 0x11;
                    } else {
                        self.globals.map_events |= 0x0008;
                        self.damage_planet(id, links);
                        self.destroy(id, links)?;
                    }
                }
            }
            // $7F:5C4E: the unit reaches the satellite and stays.
            0x0A => {
                if self.reached_target(id) {
                    self.dock_at_satellite(id, links)?;
                }
            }
            // $7F:5CB2.
            0x0B => {}
            // $7F:5CD6/$7F:5D02: an explosion starts.
            0x0C => self.explode(id, 0x1C, 0x3F),
            0x0D => self.explode(id, 0x22, 0x4B),
            // $7F:5D2E.
            0x0E => {
                if self.behavior_timer(id) {
                    self.free_unit(id);
                    self.globals.satellite_flags &= !0x0080;
                    self.globals.stored_target = 0;
                }
            }
            // $7F:5D45: the diving fighter.
            0x0F => {
                if links.scene_events & 0x0010 != 0 {
                    return self.crash(id, links);
                }
                if self.behavior_timer(id) {
                    let unit = self.unit_mut(id);
                    unit.timer = 0x0C;
                    unit.animation_timer = 0x0C;
                    unit.sprite = 0x15;
                    unit.frame = 0;
                    unit.flags |= UNIT_HIDDEN;
                }
            }
            // $7F:5D72.
            0x10 => {
                if links.scene_events & 0x0010 != 0 {
                    return self.crash(id, links);
                }
                if self.behavior_timer(id) {
                    self.damage_planet(id, links);
                    self.burn_out(id);
                }
            }
            // $7F:5D87.
            0x11 => {
                if links.scene_events & 0x0010 != 0 {
                    return self.destroy(id, links);
                }
                self.behavior_timer(id);
            }
            // $7F:5D9E.
            0x12 => self.destroy(id, links)?,
            other => return Err(SimError::Unported(0x7F5AB5 + u32::from(other) * 2)),
        }
        Ok(())
    }

    /// `$7F:5C53`: the unit joins the satellite; the satellite guards.
    fn dock_at_satellite(&mut self, id: UnitId, links: &mut SceneLinks) -> Result<(), SimError> {
        let satellite = self.globals.satellite.ok_or(SimError::MissingSatellite)?;
        let anchor = *self.unit(satellite);
        let unit = self.unit_mut(id);
        unit.x_fraction = (unit.x_fraction & 0x00FF) | (anchor.x_fraction & 0xFF00);
        unit.x = anchor.x;
        unit.y_fraction = (unit.y_fraction & 0x00FF) | (anchor.y_fraction & 0xFF00);
        unit.y = anchor.y;
        self.globals.satellite_guarding = 2;
        self.unit_mut(satellite).flags2 |= 0x2000;
        self.globals.satellite_flags |= 0x0400;
        let unit = self.unit_mut(id);
        unit.behavior = 0x0B;
        unit.motion = 0;
        self.globals.map_events |= 0x0100;
        if links.stage_results & 0x0010 == 0 {
            self.globals.satellite_flags |= 0x0200;
        }
        if self.globals.satellite_flags & 0x0008 != 0 {
            self.globals.satellite_flags &= !0x0008;
            let target = self.globals.satellite_target.ok_or(SimError::MissingUnit(0x7F5CA5))?;
            self.unit_mut(target).flags &= !UNIT_TARGETED;
        }
        Ok(())
    }

    /// `$7F:5CD6`/`$7F:5D02`.
    fn explode(&mut self, id: UnitId, sprite: u16, frames: u16) {
        let unit = self.unit_mut(id);
        unit.sprite = sprite;
        unit.frame = 0;
        unit.motion = 0;
        unit.flags |= 0x6000;
        unit.timer = frames;
        unit.animation_timer = frames;
        unit.behavior = 0x0E;
    }

    /// `$7F:5D93`: during a stage the diving fighter hits at once.
    fn crash(&mut self, id: UnitId, links: &mut SceneLinks) -> Result<(), SimError> {
        self.damage_planet(id, links);
        self.globals.map_events |= 0x0008;
        self.destroy(id, links)
    }

    /// `$7F:5B90`: leave on the origin's course and count the launch.
    fn launch(&mut self, id: UnitId, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        self.head_out(id, inputs)?;
        self.unit_mut(id).behavior = 7;
        if self.globals.interceptors != 0 {
            self.globals.interceptors -= 1;
        }
        if links.stage_results & 0x0010 != 0 {
            self.globals.map_events |= if self.unit(id).flags & UNIT_FIGHTER == 0 { 0x0020 } else { 0x0010 };
        }
        Ok(())
    }

    /// `$7F:607A`: take the origin's spawn target and a speed by variant;
    /// fighters steer straight at it, others first loop out (motion 8).
    fn head_out(&mut self, id: UnitId, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        let unit = *self.unit(id);
        if unit.flags & UNIT_SPECIAL != 0 {
            return Ok(());
        }
        let origin = self.place(unit.origin, 0x7F6085)?;
        let row = byte_at(&SPEED_ROWS, usize::from(inputs.difficulty), 0x7F60A3)?;
        let index = usize::from(((unit.variant as u8) << 1).wrapping_add(row));
        let speed = word_at(&SPEEDS, index, 0x7F60AA)?;
        let unit = self.unit_mut(id);
        unit.target_x = origin.spawn_target_x;
        unit.target_y = origin.spawn_target_y;
        unit.speed = speed;
        if unit.flags & 0x0060 != 0 {
            self.aim(id);
            self.globals.fighter_launches = self.globals.fighter_launches.wrapping_sub(1);
            self.unit_mut(id).motion = 1;
        } else {
            let (heading, bias) = if unit.flags2 & 0x0080 != 0 { (0x3000, 0x0040) } else { (0xB000, 0xFFC0) };
            unit.heading = heading;
            unit.turn_rate = 0x0080;
            unit.turn_bias = bias;
            unit.turn_countdown = 0x0240;
            unit.motion = 8;
            if unit.flags2 & 0x0100 != 0 {
                unit.program = 0;
                unit.program_timer = 0;
            }
        }
        let unit = self.unit_mut(id);
        unit.sprite = 0;
        unit.frame = 0;
        unit.flags &= 0x9FFF;
        unit.flags2 &= !0x0004;
        Ok(())
    }

    /// `$7F:679D`: frames needed to turn toward the target at the turn
    /// rate's whole part.
    fn time_turn(&mut self, id: UnitId) {
        let target = self.heading_to_target(id);
        let unit = self.unit(id);
        let difference = target.wrapping_sub(unit.heading);
        let magnitude = if difference & 0x8000 != 0 { difference.wrapping_neg() } else { difference };
        let dividend = magnitude >> 8;
        let divisor = unit.turn_rate >> 8;
        let frames = if divisor & 0x00FF == 0 { 0xFFFF } else { dividend / (divisor & 0x00FF) };
        self.unit_mut(id).timer = frames;
    }

    /// `$7F:5BE0`: a grounded unit damages the planet every 120 ticks.
    fn strike_planet(&mut self, id: UnitId, links: &mut SceneLinks) -> Result<(), SimError> {
        let unit = self.unit_mut(id);
        if unit.timer != 0 {
            unit.timer -= 1;
            if unit.timer != 0 {
                return Ok(());
            }
        }
        self.damage_planet(id, links);
        self.unit_mut(id).timer = 0x0078;
        Ok(())
    }

    /// `$7F:5F0D`: raise the planet-hit event and add the unit's damage.
    pub(crate) fn damage_planet(&mut self, id: UnitId, links: &mut SceneLinks) {
        let unit = *self.unit(id);
        links.event_word |= if unit.flags & UNIT_FIGHTER == 0 { 0x0004 } else { 0x0008 };
        let damage = u16::from(unit.count as u8) * u16::from(unit.strength as u8);
        links.planet_damage = links.planet_damage.wrapping_add(damage);
    }

    /// `$7F:5E13`.
    fn burn_out(&mut self, id: UnitId) {
        let unit = self.unit_mut(id);
        unit.sprite = 0x22;
        unit.timer = 0x4B;
        unit.animation_timer = 0x4B;
        unit.frame = 0;
        unit.motion = 0;
        unit.flags |= 0x6000;
    }

    /// `$7F:5BF5`: a missile closes on the player, or, during a stage, on
    /// the guarded place.
    fn missile_flight(&mut self, id: UnitId, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        if links.scene_events & 0x0060 == 0 {
            let unit = self.unit_mut(id);
            if unit.timer != 0 {
                unit.timer -= 1;
                return Ok(());
            }
            self.target_player(id, inputs);
            let unit = self.unit_mut(id);
            unit.motion = 3;
            unit.speed = 0x01F4;
            let index = usize::from(unit.kind.wrapping_sub(0x1C).wrapping_mul(2));
            let timer = word_at(&MISSILE_TIMERS, index, 0x7F5C1F)?;
            self.unit_mut(id).timer = timer;
            return Ok(());
        }
        // $7F:5C2F.
        if self.unit(id).flags & UNIT_ARRIVED != 0 {
            self.unit_mut(id).motion = 0;
            return Ok(());
        }
        let unit = self.unit_mut(id);
        unit.speed = 0x0096;
        unit.motion = 3;
        self.target_guarded(id)
    }

    /// `$7F:6A9D`: the target becomes the player's position.
    fn target_player(&mut self, id: UnitId, inputs: StrategicInputs<'_>) {
        let unit = self.unit_mut(id);
        unit.target_x = (unit.target_x & 0xFF00) | u16::from(inputs.player.0);
        unit.target_y = (unit.target_y & 0xFF00) | u16::from(inputs.player.1);
    }

    /// `$7F:6A87`: the target becomes the guarded place.
    fn target_guarded(&mut self, id: UnitId) -> Result<(), SimError> {
        let place = self.place(self.globals.guarded_place, 0x7F6A8B)?;
        let unit = self.unit_mut(id);
        unit.target_x = (unit.target_x & 0xFF00) | (place.x & 0x00FF);
        unit.target_y = (unit.target_y & 0xFF00) | (place.y & 0x00FF);
        Ok(())
    }

    /// `$7F:5E38`: a scripted unit's program.
    fn run_program(&mut self, id: UnitId, inputs: StrategicInputs<'_>, links: &mut SceneLinks) -> Result<(), SimError> {
        if self.globals.speed_flags & 0x0800 != 0 {
            return Ok(());
        }
        match self.unit(id).program {
            // $7F:5E71.
            0 => {
                self.target_player(id, inputs);
                let unit = self.unit_mut(id);
                unit.motion = 3;
                unit.program_timer = 0x78;
                unit.speed = 0x0226;
                if links.campaign_events & 0x0080 != 0 {
                    unit.speed = 0x04B0;
                    unit.program_timer = 0x012C;
                }
                unit.program = unit.program.wrapping_add(1);
            }
            // $7F:5E9F.
            1 => {
                self.target_player(id, inputs);
                self.program_wait(id);
            }
            // $7F:5EA6.
            2 => self.unit_mut(id).program = 0,
            // $7F:5E53.
            3 => {
                self.unit_mut(id).speed = 0x00C8;
                self.aim(id);
                let unit = self.unit_mut(id);
                unit.motion = 1;
                unit.program_timer = 0x00B4;
                unit.program = unit.program.wrapping_add(1);
            }
            // $7F:5E6D.
            4 => self.program_wait(id),
            other => return Err(SimError::Unported(0x7F5E49 + u32::from(other) * 2)),
        }
        Ok(())
    }

    /// `$7F:5EAD`.
    fn program_wait(&mut self, id: UnitId) {
        let unit = self.unit_mut(id);
        if unit.program_timer != 0 {
            unit.program_timer -= 1;
        } else {
            unit.program = unit.program.wrapping_add(1);
        }
    }

    // ---- meetings ($7F:649A) ----

    fn check_meetings(&mut self, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        links.scene_events &= !0x1000;
        let mut cursor = self.unit_head;
        while let Some(id) = cursor {
            self.check_meeting(id, links, inputs)?;
            cursor = self.unit(id).next;
        }
        Ok(())
    }

    /// `$7F:64B1`.
    pub(crate) fn check_meeting(&mut self, id: UnitId, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        let unit = *self.unit(id);
        if unit.flags & 0x3000 != 0 {
            return Ok(());
        }
        if links.scene_events & 0x0060 == 0 {
            if unit.proximity_delay != 0 {
                self.unit_mut(id).proximity_delay = unit.proximity_delay - 1;
            } else if self.near_player(id, inputs) {
                // $7F:64DA.
                self.globals.met_unit = Some(id);
                self.pause_for_meeting(id);
                self.unit_mut(id).flags |= 0x2001;
                links.scene_events |= 0x0064;
                self.globals.hold |= 0x001E;
                self.globals.encounter_request = 2;
                links.map_region = self.terrain(id, inputs)?;
                return Ok(());
            }
        }
        // $7F:6504.
        let unit = *self.unit(id);
        if links.campaign_events & 0x0010 != 0
            && links.campaign_events & 0x0020 == 0
            && unit.flags & UNIT_INTERCEPTOR != 0
            && unit.flags & UNIT_MET == 0
            && self.near_player(id, inputs)
        {
            links.campaign_events |= 0x0020;
            links.encounter_result = 1;
            links.node_variant = unit.node_variant as u8;
            self.globals.intercepted_unit = Some(id);
            self.globals.intercepted_motion = unit.motion;
            self.globals.intercepted_behavior = unit.behavior;
            let unit = self.unit_mut(id);
            unit.motion = 0;
            unit.behavior = 0;
        }
        // $7F:655C.
        let unit = self.unit_mut(id);
        unit.flags2 &= !0x0002;
        unit.flags &= !UNIT_ARRIVED;
        match self.terrain(id, inputs)? {
            0 => {
                let flags = self.unit(id).flags;
                if flags & UNIT_MET == 0 && flags & 0x00B0 != 0 {
                    self.reach_planet(id, links)?;
                }
            }
            9 => {
                let unit = self.unit_mut(id);
                if unit.flags & UNIT_ACTIVE != 0 {
                    unit.flags |= UNIT_ARRIVED;
                }
            }
            7 => self.unit_mut(id).flags2 |= 0x0002,
            _ => {}
        }
        Ok(())
    }

    /// `$7F:6653`: the met unit stops; its modes are kept for later.
    fn pause_for_meeting(&mut self, id: UnitId) {
        let unit = self.unit_mut(id);
        if unit.flags & 0x8040 == 0 {
            unit.sprite = 0;
            unit.frame = 0;
        }
        unit.saved_behavior = unit.behavior;
        unit.saved_motion = unit.motion;
        unit.motion = 0;
        unit.behavior = if unit.flags & UNIT_ACTIVE == 0 { 0 } else { 7 };
    }

    /// `$7F:6A5D`: whether the player is within the unit's radius.
    fn near_player(&self, id: UnitId, inputs: StrategicInputs<'_>) -> bool {
        let unit = self.unit(id);
        within(
            (u16::from(inputs.player.0), u16::from(inputs.player.1)),
            (u16::from(unit.x), u16::from(unit.y)),
            unit.radius & 0x00FF,
        )
    }

    /// `$7F:6692`/`$7F:6931`: the terrain cell under the unit.
    fn terrain(&self, id: UnitId, inputs: StrategicInputs<'_>) -> Result<u16, SimError> {
        let unit = self.unit(id);
        let index = usize::from(unit.y >> 3) * TERRAIN_ROW + usize::from(unit.x >> 3);
        inputs.terrain.get(index).map(|&cell| u16::from(cell)).ok_or(SimError::TableOverrun(0x7F695B))
    }

    /// `$7F:65AF`: a hostile unit reaches the home planet.
    fn reach_planet(&mut self, id: UnitId, links: &mut SceneLinks) -> Result<(), SimError> {
        let unit = *self.unit(id);
        if unit.flags & UNIT_MISSILE != 0 {
            // $7F:6623.
            if links.scene_events & 0x0010 == 0 {
                self.unit_mut(id).flags &= !UNIT_GROUNDED;
                return Ok(());
            }
            if unit.flags & UNIT_GROUNDED == 0 {
                self.globals.map_events |= 0x0004;
            }
        } else if unit.flags & UNIT_GROUNDED != 0 {
        } else if unit.flags & UNIT_FIGHTER != 0 {
            // $7F:6601.
            if links.scene_events & 0x0010 == 0 {
                self.dive(id);
            } else {
                self.globals.map_events |= 0x0008;
                links.scene_events |= 0x1000;
                self.damage_planet(id, links);
                return self.destroy(id, links);
            }
        } else {
            let unit = self.unit_mut(id);
            unit.timer = 1;
            unit.turn_rate = 0x0400;
            unit.speed = 0x0400;
            self.bounce(id);
            let unit = self.unit_mut(id);
            unit.target_x = u16::from(unit.x);
            unit.target_y = u16::from(unit.y);
            if links.scene_events & 0x0010 != 0 {
                self.globals.map_events |= 0x0004;
            }
        }
        // $7F:6639.
        self.unit_mut(id).flags |= 0x0006;
        links.scene_events |= 0x1000;
        Ok(())
    }

    /// `$7F:6723`: turn back, mirrored on the lower half of the map.
    fn bounce(&mut self, id: UnitId) {
        let unit = self.unit_mut(id);
        let word = (unit.y_fraction >> 8) | (u16::from(unit.y) << 8);
        if word.wrapping_sub(0x9800) & 0x8000 == 0 {
            unit.turn_rate = unit.turn_rate.wrapping_neg();
        }
        self.globals.speed_flags &= !FAST_FORWARD;
        self.unit_mut(id).motion = 4;
    }

    /// `$7F:5DA3`: a fighter dives at the planet.
    pub(crate) fn dive(&mut self, id: UnitId) {
        let unit = self.unit_mut(id);
        unit.flags |= UNIT_TARGETED;
        unit.flags2 |= 0x0004;
        unit.motion = 6;
        unit.behavior = 0x0F;
        unit.timer = 0x3C;
        unit.turn_rate = 0x0400;
        let band = match unit.x {
            0x00..=0x1F => 0,
            0x20..=0x2F => 1,
            0x30..=0x3F => 2,
            _ => 3,
        };
        let (vx_fraction, vx) = DIVE_VX[band];
        let (vy_fraction, vy) = DIVE_VY[band];
        unit.vx_fraction = vx_fraction;
        unit.vx = vx;
        unit.vy_fraction = vy_fraction;
        unit.vy = vy;
    }

    // ---- spawning ($7F:6027) and destruction ($7F:6181) ----

    /// `$7F:6027`: a place spawns a unit at the head of the unit list.
    fn spawn_unit(&mut self, place_id: PlaceId, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        let id = self.allocate_unit()?;
        self.globals.unit_count = self.globals.unit_count.wrapping_add(1);
        let place = self.places[usize::from(place_id.0)];
        self.places[usize::from(place_id.0)].spawned_unit = Some(id);
        let unit = self.unit_mut(id);
        unit.behavior = 1;
        unit.flags |= UNIT_TARGETED;
        unit.home = Some(place_id);
        unit.origin = Some(place_id);
        unit.x_fraction &= 0x00FF;
        unit.x = place.x as u8;
        unit.y_fraction &= 0x00FF;
        unit.y = place.y as u8;
        unit.target_x = place.spawn_target_x;
        unit.target_y = place.spawn_target_y;
        unit.motion = 1;
        unit.proximity_delay = 0x18;
        self.classify_unit(place_id, id, links, inputs)
    }

    /// `$7F:6141`: take the free list's first record and link it first.
    pub(crate) fn allocate_unit(&mut self) -> Result<UnitId, SimError> {
        let id = self.unit_free.ok_or(SimError::UnitPoolExhausted)?;
        self.unit_free = self.unit(id).next;
        let head = self.unit_head;
        let unit = self.unit_mut(id);
        unit.next = head;
        unit.prev = None;
        self.unit_head = Some(id);
        if let Some(next) = head {
            self.unit_mut(next).prev = Some(id);
        }
        Ok(id)
    }

    /// `$7F:625C`: the new unit's kind, from its place.
    fn classify_unit(&mut self, place_id: PlaceId, id: UnitId, links: &mut SceneLinks, inputs: StrategicInputs<'_>) -> Result<(), SimError> {
        let place = self.places[usize::from(place_id.0)];
        if place.flags & 0x0010 != 0 {
            // $7F:639F.
            let unit = self.unit_mut(id);
            unit.behavior = 0x0A;
            unit.category = 7;
            unit.flags = (unit.flags | UNIT_SPECIAL) & !UNIT_TARGETED;
            unit.variant = 5;
            unit.speed = 0x0190;
            unit.radius = 6;
            self.aim(id);
            self.assign_pattern(id, 0x30)?;
        } else if place.flags & 0x0004 == 0 {
            // $7F:633C.
            self.globals.escort_count = self.globals.escort_count.wrapping_add(1);
            let unit = self.unit_mut(id);
            unit.timer = 0x32;
            set_velocity_x_word(unit, 0x0000);
            set_velocity_y_word(unit, 0xFFC0);
            let base = word_at(&ESCORT_PATTERN_BASES, usize::from(inputs.difficulty) * 2, 0x7F6356)?;
            let index = base.wrapping_add(self.globals.escort_pattern);
            self.assign_pattern(id, index)?;
            self.globals.escort_pattern = self.globals.escort_pattern.wrapping_add(1) & 0x000F;
            let unit = self.unit_mut(id);
            unit.category = 7;
            unit.variant = 3;
            unit.count = 5;
            unit.heading = 0;
            unit.flags |= UNIT_FIGHTER;
            links.event_word |= 0x0002;
            unit.radius = 6;
        } else {
            // A cruiser base's carrier.
            self.globals.cruiser_count = self.globals.cruiser_count.wrapping_add(1);
            let unit = self.unit_mut(id);
            unit.flags |= UNIT_CARRIER;
            if place.flags & 0x0100 != 0 {
                unit.flags2 |= 0x1000;
            }
            unit.timer = 0x0F;
            unit.flags2 |= 0x0004;
            let lane = usize::from(place.spawn_remaining) * 2;
            set_velocity_x_word(unit, word_at(&CARRIER_VX, lane, 0x7F62A7)?);
            set_velocity_y_word(unit, word_at(&CARRIER_VY, lane, 0x7F62AE)?);
            let unit = self.unit_mut(id);
            if place.spawn_remaining & 0x0001 != 0 {
                unit.flags2 |= 0x0080;
            }
            unit.category = 6;
            let pattern = byte_at(&SPAWN_SEQUENCE, usize::from(self.globals.spawn_pattern), 0x7F62DF)?;
            let base = byte_at(&CARRIER_ROWS, usize::from(inputs.difficulty), 0x7F62EB)?;
            let kind = base.wrapping_add(pattern);
            let variant = byte_at(&KIND_VARIANTS, usize::from(kind), 0x7F62F6)?;
            let entry = PatternEntry {
                pattern: byte_at(&CARRIER_PATTERN, usize::from(kind), 0x7F6301)?,
                word: byte_at(&CARRIER_WORD, usize::from(kind), 0x7F6307)?,
                strength: byte_at(&CARRIER_STRENGTH, usize::from(kind), 0x7F630D)?,
            };
            let unit = self.unit_mut(id);
            unit.kind = (unit.kind & 0xFF00) | u16::from(kind);
            unit.variant = (unit.variant & 0xFF00) | u16::from(variant);
            self.place_in_slot(id, entry)?;
            self.globals.spawn_pattern = (self.globals.spawn_pattern & 0xFF00)
                | u16::from((self.globals.spawn_pattern as u8).wrapping_add(1) & 0x7F);
            let unit = self.unit_mut(id);
            unit.count = (unit.count & 0xFF00) | 2;
            links.event_word |= 0x0001;
            unit.radius = 8;
        }
        let variant = self.unit(id).variant;
        self.globals.spawn_variant_bit = word_at(&VARIANT_BITS, usize::from(variant).wrapping_mul(2), 0x7F63DF)?;
        Ok(())
    }

    /// `$7F:63E8`: kind and slot data from the pattern row `index`.
    pub(crate) fn assign_pattern(&mut self, id: UnitId, index: u16) -> Result<(), SimError> {
        let kind = byte_at(&PATTERN_KINDS, usize::from(index), 0x7F63FB)?;
        let unit = self.unit_mut(id);
        unit.kind = (unit.kind & 0xFF00) | u16::from(kind);
        let entry = PatternEntry {
            pattern: byte_at(&KIND_PATTERN, usize::from(kind), 0x7F6403)?,
            word: byte_at(&KIND_WORD, usize::from(kind), 0x7F6409)?,
            strength: byte_at(&KIND_STRENGTH, usize::from(kind), 0x7F640F)?,
        };
        self.place_in_slot(id, entry)
    }

    /// `$7F:6433`: claim an encounter slot for the unit.
    pub(crate) fn place_in_slot(&mut self, id: UnitId, entry: PatternEntry) -> Result<(), SimError> {
        let slot = self.claim_slot()?;
        let unit = self.unit_mut(id);
        unit.encounter_slot = (unit.encounter_slot & 0xFF00) | u16::from(slot);
        let index = usize::from(slot);
        self.globals.slot_kind[index] = entry.word;
        self.globals.slot_strength[index] = entry.strength;
        self.globals.slot_pattern[index] = entry.pattern;
        let unit = self.unit_mut(id);
        unit.strength = (unit.strength & 0xFF00) | u16::from((entry.pattern >> 4).wrapping_add(entry.pattern & 0x0F));
        self.globals.slot_word[index] = 0;
        Ok(())
    }

    /// `$7F:645E`: the next free slot from the cursor.
    fn claim_slot(&mut self) -> Result<u8, SimError> {
        let mut index = self.globals.slot_cursor;
        for _ in 0..ENCOUNTER_SLOTS {
            let slot = usize::from(index);
            if *self.globals.slot_used.get(slot).ok_or(SimError::TableOverrun(0x7F6468))? == 0 {
                self.globals.slot_used[slot] = 1;
                self.globals.slot_cursor = index;
                return Ok(index as u8);
            }
            index = (index + 1) & 0x000F;
        }
        Err(SimError::EncounterSlotsExhausted)
    }

    /// `$7F:6181`: remove a unit, releasing its slot and counters.
    pub(crate) fn destroy(&mut self, id: UnitId, links: &mut SceneLinks) -> Result<(), SimError> {
        let _ = links;
        let unit = *self.unit(id);
        if unit.flags & UNIT_FIGHTER != 0 {
            let origin = self.place_mut(unit.origin, 0x7F61BB)?;
            origin.flags &= !0x0800;
            origin.state = 1;
            self.globals.escort_count = self.globals.escort_count.wrapping_sub(1);
            if self.globals.campaign_flags & 0x0001 != 0 {
                self.globals.stored_counter = 0;
                self.globals.stored_target = 0;
            }
        } else if unit.flags & UNIT_MISSILE != 0 {
            self.globals.missile_count = self.globals.missile_count.wrapping_sub(1);
            self.globals.missile_salvo = self.globals.missile_salvo.wrapping_sub(1);
        } else if unit.flags & UNIT_SPECIAL != 0 {
        } else if unit.flags & UNIT_INTERCEPTOR != 0 {
            self.globals.interceptor_count = self.globals.interceptor_count.wrapping_sub(1);
        } else {
            if unit.flags2 & 0x1000 != 0 {
                let origin = self.place_mut(unit.origin, 0x7F61A8)?;
                origin.spawn_batch = origin.spawn_batch.wrapping_sub(1);
                if origin.spawn_batch == 0 {
                    origin.flags &= !0x0100;
                }
            }
            self.globals.cruiser_count = self.globals.cruiser_count.wrapping_sub(1);
        }
        let slot = usize::from(unit.encounter_slot);
        if slot >= ENCOUNTER_SLOTS {
            return Err(SimError::TableOverrun(0x7F61F7));
        }
        self.globals.slot_used[slot] = 0;
        self.globals.slot_pattern[slot] = 0;
        self.globals.slot_word[slot] = 0;
        self.free_unit(id);
        self.globals.unit_count = self.globals.unit_count.wrapping_sub(1);
        Ok(())
    }

    /// `$7F:6213`: unlink the unit and push it on the free list.
    pub(crate) fn free_unit(&mut self, id: UnitId) {
        let unit = self.unit_mut(id);
        unit.flags = 0;
        unit.flags2 = 0;
        unit.animation_timer = 0;
        unit.behavior = 0;
        let (prev, next) = (unit.prev, unit.next);
        match prev {
            None => {
                self.unit_head = next;
                if let Some(next) = next {
                    self.unit_mut(next).prev = None;
                }
            }
            Some(prev) => {
                self.unit_mut(prev).next = next;
                if let Some(next) = next {
                    self.unit_mut(next).prev = Some(prev);
                }
            }
        }
        self.unit_mut(id).next = self.unit_free;
        self.unit_free = Some(id);
    }

    // ---- motion ($7F:66E6) ----

    fn advance_motion(&mut self) -> Result<(), SimError> {
        let mut cursor = self.unit_head;
        while let Some(id) = cursor {
            self.move_unit(id)?;
            cursor = self.unit(id).next;
        }
        Ok(())
    }

    /// `$7F:66B2`: one unit's motion (`$7F:6700` table).
    pub(crate) fn move_unit(&mut self, id: UnitId) -> Result<(), SimError> {
        match self.unit(id).motion {
            0 => {}
            // $7F:6713.
            1 => self.integrate(id),
            // $7F:681E.
            2 => {
                self.update_velocity(id);
                self.integrate(id);
            }
            // $7F:6818.
            3 => {
                self.aim(id);
                self.integrate(id);
            }
            // $7F:6716.
            4 => {
                let unit = self.unit_mut(id);
                unit.heading = unit.heading.wrapping_add(unit.turn_rate);
                self.update_velocity(id);
                self.integrate(id);
            }
            // $7F:6742.
            5 => {
                let unit = self.unit_mut(id);
                if unit.flags2 & 0x0020 == 0 {
                    unit.heading = unit.heading.wrapping_add(unit.turn_rate);
                } else {
                    unit.heading = unit.heading.wrapping_sub(unit.turn_rate);
                }
                if unit.heading == 0 {
                    unit.flags2 ^= 0x0020;
                }
                self.update_velocity(id);
                self.integrate(id);
            }
            // $7F:6770.
            6 => {
                let unit = self.unit_mut(id);
                unit.turn_rate = unit.turn_rate.wrapping_add(0x0080);
                unit.heading = unit.heading.wrapping_add(unit.turn_rate);
                self.integrate_plain(id);
            }
            // $7F:6787.
            7 => {
                let unit = self.unit_mut(id);
                if unit.timer != 0 {
                    unit.timer -= 1;
                    unit.heading = unit.heading.wrapping_add(unit.turn_rate);
                }
                self.update_velocity(id);
                self.integrate(id);
            }
            // $7F:67F8.
            8 => {
                let unit = self.unit_mut(id);
                unit.heading = unit.heading.wrapping_add(unit.turn_bias);
                unit.turn_countdown = unit.turn_countdown.wrapping_sub(1);
                if unit.turn_countdown == 0 {
                    self.plan_turn(id);
                    self.unit_mut(id).motion = 7;
                } else {
                    self.update_velocity(id);
                    self.integrate(id);
                }
            }
            other => return Err(SimError::Unported(0x7F6700 + u32::from(other) * 2)),
        }
        Ok(())
    }

    /// `$7F:67C1`: the turn toward the target, as a rate and a frame count.
    fn plan_turn(&mut self, id: UnitId) {
        let target = self.heading_to_target(id);
        let unit = self.unit(id);
        let (difference, borrow) = target.overflowing_sub(unit.heading);
        let negative = difference & 0x8000 != 0;
        let magnitude = if negative { difference.wrapping_neg() } else { difference };
        let (frames, _) = serial_divide(u32::from(magnitude), unit.turn_rate, !borrow);
        let unit = self.unit_mut(id);
        if negative {
            unit.turn_rate = unit.turn_rate.wrapping_neg();
        }
        unit.timer = frames as u16;
    }

    /// `$7F:6830`: integrate the velocity, keeping the position on the map.
    pub(crate) fn integrate(&mut self, id: UnitId) {
        if self.globals.speed_flags & FAST_FORWARD == 0 {
            return self.integrate_plain(id);
        }
        // $7F:6877.
        let unit = *self.unit(id);
        let (vx_fraction, vx) = scale_fast(unit.vx_fraction, unit.vx);
        let (vy_fraction, vy) = scale_fast(unit.vy_fraction, unit.vy);
        // The scaled velocity also passes through D984..D98A, scratch words
        // the HUD reuses; nothing reads them back here.
        let unit = self.unit_mut(id);
        step_axis(&mut unit.x_fraction, &mut unit.x, vx_fraction, vx, 0x0C, 0xF4);
        step_axis(&mut unit.y_fraction, &mut unit.y, vy_fraction, vy, 0x10, 0xAC);
    }

    /// `$7F:6838`.
    fn integrate_plain(&mut self, id: UnitId) {
        let unit = self.unit_mut(id);
        let (vx_fraction, vx, vy_fraction, vy) = (unit.vx_fraction, unit.vx, unit.vy_fraction, unit.vy);
        step_axis(&mut unit.x_fraction, &mut unit.x, vx_fraction, vx, 0x0C, 0xF4);
        step_axis(&mut unit.y_fraction, &mut unit.y, vy_fraction, vy, 0x10, 0xAC);
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PatternEntry {
    pub(crate) pattern: u8,
    pub(crate) word: u8,
    pub(crate) strength: u8,
}

/// `$7F:6229`: the velocity's whole part and high fraction byte as a word.
fn set_velocity_x_word(unit: &mut MapUnit, word: u16) {
    unit.vx_fraction = (unit.vx_fraction & 0x00FF) | (word << 8);
    unit.vx = (word >> 8) as u8;
}

fn set_velocity_y_word(unit: &mut MapUnit, word: u16) {
    unit.vy_fraction = (unit.vy_fraction & 0x00FF) | (word << 8);
    unit.vy = (word >> 8) as u8;
}

/// `$7F:6AAE`: whether `point` lies within `radius` of `centre` on both
/// axes, by the source's sign-of-difference tests.
fn within(point: (u16, u16), centre: (u16, u16), radius: u16) -> bool {
    let inside = |p: u16, c: u16| {
        c.wrapping_add(radius).wrapping_sub(p) & 0x8000 == 0 && c.wrapping_sub(radius).wrapping_sub(p) & 0x8000 != 0
    };
    inside(point.0, centre.0) && inside(point.1, centre.1)
}

/// One axis of `$7F:6838`: the fraction carries into the coordinate, which
/// only moves while it stays inside `[low, high)`.
fn step_axis(fraction: &mut u16, coordinate: &mut u8, velocity_fraction: u16, velocity: u8, low: u8, high: u8) {
    let (sum, carry) = fraction.overflowing_add(velocity_fraction);
    *fraction = sum;
    let next = velocity.wrapping_add(*coordinate).wrapping_add(u8::from(carry));
    if next >= low && next < high {
        *coordinate = next;
    }
}

/// `$7F:6917`: four times the 8.16 velocity.
fn scale_fast(fraction: u16, whole: u8) -> (u16, u8) {
    let value = (u32::from(i16::from(whole as i8) as u16) << 16 | u32::from(fraction)) << 2;
    (value as u16, (value >> 16) as u8)
}

/// `$7F:69C7`: the signed product (`$7F:76D8`), shifted right twice as an
/// unsigned 32-bit value.
fn scaled_product(value: i16, factor: i16) -> u32 {
    ((i32::from(value) * i32::from(factor)) as u32) >> 2
}

/// `$7F:6DDD`: the bit-serial divide, which first rotates the incoming
/// carry into the dividend. Returns the result register and remainder.
fn serial_divide(dividend: u32, divisor: u16, carry_in: bool) -> (u32, u16) {
    let mut register = dividend;
    let out = register >> 31 != 0;
    register = (register << 1) | u32::from(carry_in);
    let mut carry = out;
    let mut accumulator: u16 = 0;
    for _ in 0..32 {
        accumulator = (accumulator << 1) | u16::from(carry);
        carry = accumulator >= divisor;
        if carry {
            accumulator = accumulator.wrapping_sub(divisor);
        }
        let out = register >> 31 != 0;
        register = (register << 1) | u32::from(carry);
        carry = out;
    }
    (register, accumulator)
}

pub(crate) fn byte_at(table: &[u8], index: usize, site: u32) -> Result<u8, SimError> {
    table.get(index).copied().ok_or(SimError::TableOverrun(site))
}

pub(crate) fn word_at(table: &[u8], index: usize, site: u32) -> Result<u16, SimError> {
    Ok(u16::from(byte_at(table, index, site)?) | (u16::from(byte_at(table, index + 1, site)?) << 8))
}

/// `$7F:6CDA`: each difficulty's row in the place timers.
const PLACE_TIMER_ROWS: [u8; 3] = [0x00, 0x06, 0x0C];
/// `$7F:6CDD`: the places' first timers by difficulty row and kind.
const PLACE_TIMERS: [u8; 0x2C] = [
    0xB0, 0x04, 0x80, 0x07, 0x54, 0x06, 0xA0, 0x05, 0xF8, 0x07, 0xAC, 0x08, 0x38, 0x13, 0x28, 0x05,
    0x80, 0x07, 0x18, 0x06, 0xA0, 0x05, 0x80, 0x07, 0x34, 0x08, 0x38, 0x13, 0xB0, 0x04, 0xCC, 0x06,
    0xA0, 0x05, 0x28, 0x05, 0x80, 0x07, 0xF8, 0x07, 0x38, 0x13, 0x04, 0x08,
];
/// `$7F:5504`: the satellite's rests, by 1BA3.
const SATELLITE_RESTS: [u8; 6] = [0xDC, 0x05, 0x64, 0x05, 0xB0, 0x04];
/// `$7F:5C27`: missile timers by kind (from 0x1C).
const MISSILE_TIMERS: [u8; 8] = [0x3C, 0x00, 0x78, 0x00, 0xB4, 0x00, 0xF0, 0x00];
/// `$7F:6491`: each difficulty's escort pattern base.
const ESCORT_PATTERN_BASES: [u8; 6] = [0x00, 0x00, 0x10, 0x00, 0x20, 0x00];
/// `$7F:6C9B`/`$7F:6CA3`: carriers' initial velocity words by lane.
const CARRIER_VX: [u8; 8] = [0x80, 0x00, 0x80, 0xFF, 0xE0, 0xFF, 0x60, 0x00];
const CARRIER_VY: [u8; 8] = [0x40, 0x00, 0xC0, 0xFF, 0xA0, 0xFF, 0xE0, 0xFF];
/// `$7F:6CC3`/`$7F:6CAB`: launch speeds by difficulty row and variant.
const SPEED_ROWS: [u8; 3] = [0x00, 0x08, 0x10];
const SPEEDS: [u8; 0x18] = [
    0x96, 0x00, 0xC8, 0x00, 0xFA, 0x00, 0x2C, 0x01, 0xAA, 0x00, 0xC8, 0x00, 0xFA, 0x00, 0x40, 0x01,
    0xDC, 0x00, 0xFA, 0x00, 0x2C, 0x01, 0x68, 0x01,
];
/// `$7F:6C10`: the places' first program offsets (words) and the rows.
pub(crate) const PROGRAMS: [u8; 0x6B] = [
    0x1A, 0x00, 0x1A, 0x00, 0x27, 0x00, 0x27, 0x00, 0x27, 0x00, 0x27, 0x00, 0x34, 0x00, 0x34, 0x00,
    0x34, 0x00, 0x34, 0x00, 0x34, 0x00, 0x34, 0x00, 0x34, 0x00, 0x02, 0x01, 0x00, 0x01, 0x01, 0x03,
    0x02, 0xD0, 0x02, 0x01, 0xFF, 0x03, 0x00, 0x02, 0x01, 0x00, 0x01, 0x01, 0x03, 0x02, 0x38, 0x04,
    0x01, 0xFF, 0x03, 0x00, 0x02, 0x01, 0x00, 0x01, 0x01, 0x03, 0x02, 0x84, 0x03, 0x01, 0xFF, 0x03,
    0x00, 0x02, 0x08, 0x20, 0x80, 0xAA, 0x00, 0xA8, 0x02, 0xA0, 0x0A, 0x80, 0x2A, 0x02, 0x03, 0x04,
    0x05, 0x06, 0x12, 0x13, 0x14, 0x15, 0x16, 0x2E, 0x2F, 0x3E, 0x3F, 0x00, 0x00, 0x60, 0xA0, 0x28,
    0xC8, 0xE0, 0xC0, 0x20, 0x38, 0x80, 0x58, 0xE0, 0x40, 0x80, 0xC0,
];
/// `$7F:6CC6`: variant bits.
const VARIANT_BITS: [u8; 0x14] = [
    0x01, 0x00, 0x02, 0x00, 0x04, 0x00, 0x08, 0x00, 0x10, 0x00, 0x20, 0x00, 0x40, 0x00, 0x80, 0x00,
    0x00, 0x01, 0x00, 0x02,
];
/// `$7F:6D0A`: each difficulty's carrier kind row.
const CARRIER_ROWS: [u8; 3] = [0x00, 0x06, 0x0C];
/// `$7F:6C7B`: each kind's variant.
const KIND_VARIANTS: [u8; 0x20] = [
    0x00, 0x01, 0x00, 0x01, 0x00, 0x02, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x02, 0x00, 0x02, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01, 0x00, 0x01,
];
/// `$7F:6D10`/`$7F:6D20`: a diving fighter's velocity by band.
const DIVE_VX: [(u16, u8); 4] = [(0x3000, 0x00), (0xF000, 0xFF), (0xD000, 0xFF), (0xC000, 0xFF)];
const DIVE_VY: [(u16, u8); 4] = [(0x4000, 0x00), (0x3000, 0x00), (0x3000, 0x00), (0xE000, 0xFF)];
/// `$00:B1FC`: the carriers' kind sequence.
const SPAWN_SEQUENCE: [u8; 0x80] = [
    0x00, 0x06, 0x0A, 0x02, 0x08, 0x0D, 0x01, 0x0F, 0x03, 0x0E, 0x09, 0x04, 0x0B, 0x02, 0x0D, 0x05,
    0x06, 0x03, 0x01, 0x08, 0x04, 0x09, 0x0C, 0x07, 0x0A, 0x0E, 0x00, 0x08, 0x0F, 0x05, 0x03, 0x0D,
    0x01, 0x04, 0x0C, 0x09, 0x08, 0x0F, 0x05, 0x0A, 0x0B, 0x06, 0x01, 0x09, 0x03, 0x07, 0x0C, 0x02,
    0x0E, 0x0B, 0x00, 0x09, 0x0F, 0x07, 0x01, 0x08, 0x0D, 0x04, 0x0C, 0x0A, 0x07, 0x09, 0x0A, 0x0B,
    0x05, 0x06, 0x0E, 0x02, 0x0C, 0x03, 0x0D, 0x00, 0x06, 0x0B, 0x01, 0x05, 0x07, 0x00, 0x0F, 0x03,
    0x08, 0x0D, 0x04, 0x0E, 0x0A, 0x0F, 0x05, 0x0C, 0x02, 0x0B, 0x04, 0x0D, 0x00, 0x07, 0x0A, 0x0E,
    0x0F, 0x00, 0x09, 0x01, 0x06, 0x0B, 0x02, 0x08, 0x0E, 0x04, 0x0C, 0x03, 0x0B, 0x09, 0x02, 0x05,
    0x07, 0x00, 0x0F, 0x0D, 0x0A, 0x04, 0x06, 0x0E, 0x07, 0x03, 0x0C, 0x06, 0x02, 0x05, 0x01, 0x08,
];
/// `$00:B0F8`/`$00:B118`/`$00:B138`: carriers' encounter data by kind.
pub(crate) const CARRIER_PATTERN: [u8; 0x20] = [
    0x03, 0x03, 0x03, 0x03, 0x21, 0x31, 0x04, 0x04, 0x21, 0x31, 0x21, 0x22, 0x05, 0x05, 0x31, 0x22,
    0x04, 0x23, 0x21, 0x21, 0x21, 0x41, 0x22, 0x21, 0x21, 0x41, 0x31, 0x21, 0x01, 0x01, 0x01, 0x01,
];
pub(crate) const CARRIER_WORD: [u8; 0x20] = [
    0x01, 0x02, 0x0A, 0x06, 0x03, 0x0A, 0x01, 0x02, 0x0A, 0x0A, 0x06, 0x01, 0x01, 0x02, 0x03, 0x0A,
    0x05, 0x01, 0x0C, 0x03, 0x04, 0x04, 0x0A, 0x0C, 0x09, 0x09, 0x0B, 0x0B, 0x12, 0x11, 0x13, 0x14,
];
pub(crate) const CARRIER_STRENGTH: [u8; 0x20] = [
    0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x00, 0x07, 0x06, 0x01, 0x01, 0x02, 0x00, 0x00, 0x02, 0x02,
    0x00, 0x05, 0x01, 0x06, 0x07, 0x07, 0x06, 0x02, 0x08, 0x08, 0x05, 0x06, 0x00, 0x00, 0x00, 0x00,
];
/// `$00:B27C`: escort and special kinds by pattern row.
const PATTERN_KINDS: [u8; 0x40] = [
    0x01, 0x03, 0x00, 0x02, 0x03, 0x00, 0x01, 0x02, 0x00, 0x01, 0x03, 0x02, 0x01, 0x00, 0x01, 0x02,
    0x03, 0x01, 0x05, 0x02, 0x04, 0x00, 0x05, 0x02, 0x03, 0x04, 0x00, 0x03, 0x02, 0x04, 0x01, 0x05,
    0x05, 0x07, 0x00, 0x04, 0x06, 0x07, 0x03, 0x04, 0x05, 0x06, 0x01, 0x05, 0x03, 0x06, 0x02, 0x07,
    0x08, 0x0A, 0x0B, 0x00, 0x06, 0x01, 0x07, 0x03, 0x05, 0x00, 0x04, 0x01, 0x02, 0x03, 0x00, 0x05,
];
/// `$00:B158`/`$00:B164`/`$00:B170`: escort and special encounter data.
const KIND_PATTERN: [u8; 0x0C] = [0x01, 0x02, 0x02, 0x03, 0x04, 0x01, 0x24, 0x01, 0x03, 0x03, 0x01, 0x01];
const KIND_WORD: [u8; 0x0C] = [0x0D, 0x0D, 0x0E, 0x0D, 0x0E, 0x10, 0x0E, 0x0F, 0x15, 0x15, 0x16, 0x16];
const KIND_STRENGTH: [u8; 0x0C] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0D, 0x00, 0x00, 0x00, 0x00, 0x00];

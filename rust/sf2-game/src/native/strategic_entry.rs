//! The map program's entry (`$04:DCBC`, then `$04:AE57..AE92`): the
//! campaign setup (`$04:DEA9`) that starts a new campaign, the map screen's
//! starting words, the ship's place, the terrain grid and palette the entry
//! decompresses, the sprite buffer and the radio's reset.
//!
//! The entry's code copies, video uploads, window effects and music are
//! presentation and are not modeled. A return from a stage (1B86 bit 2,
//! 1B88 bit 4) is not ported yet and faults.

use sf2_data::map_layers::{MAP_PALETTE, MAP_TERRAIN};

use super::strategic_hud::planet_colours;
use super::strategic_radio::{Radio, RadioInputs};
use super::strategic_screen::{Frame, MapPad, ScreenError, ScreenOutput, ScriptSubject};
use super::strategic_script::new_place;
use super::strategic_sim::{byte_at, word_at, MapPlace, MapUnit, PlaceId, UnitId, PLACE_CAPACITY, UNIT_CAPACITY};
use super::strategic_visit::{MapVisit, VisitError};

/// Campaign words from outside the map that the entry reads.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EntryInputs {
    /// 1C02: the campaign's seed word.
    pub seed: u16,
    /// D7E1..D7E3: the difficulties' tallies.
    pub tallies: [u8; 3],
    /// DP E4, the byte after the generator, which its sixteen-bit form
    /// reads and writes.
    pub generator_tail: u8,
}

/// 1B86 bits.
const RESULTS_SET_UP: u16 = 0x0040;
const RESULTS_FROM_STAGE: u16 = 0x0002;
const RESULTS_TUTORIAL: u16 = 0x0080;
const RESULTS_NEW: u16 = 0x0001;
/// 1B88 bit 4: back from a stage.
const EVENTS_FROM_STAGE: u16 = 0x0010;

/// `$04:E079`/`E07C`/`E07F`: by difficulty, the marks, fleets and enemies.
const MARKS: [u8; 3] = [0x02, 0x03, 0x06];
const FLEETS: [u8; 3] = [0x02, 0x04, 0x04];
const ENEMIES: [u8; 3] = [0x04, 0x07, 0x0A];
/// `$04:EE29`: by difficulty, the escorts' first number.
const ESCORT_NUMBERS: [u8; 3] = [0x00, 0x04, 0x0C];
/// `$04:E04C`: by the satellite timing offset, the schedule's first step.
const SCHEDULE_STARTS: [u8; 6] = [0x00, 0x00, 0x1A, 0x00, 0x50, 0x00];
/// `$04:E052`: by difficulty, the missiles.
const MISSILES: [u8; 3] = [0x01, 0x01, 0x02];
/// `$04:E3B6`: by difficulty, the tally that brings the planet.
const PLANET_TALLIES: [u8; 3] = [0x0D, 0x13, 0x14];
/// `$04:EF1E`: the six marked places (position, spawn target), last first.
const MARKED_PLACES: [u8; 0x18] = [
    0xC8, 0x50, 0x28, 0x90, 0x20, 0x20, 0x10, 0x78, 0x88, 0x20, 0x30, 0x90, 0xE0, 0x80, 0x38, 0x98, 0x80, 0x58,
    0x28, 0x90, 0x90, 0xA0, 0x40, 0x98,
];
/// `$04:E7C7`: by the satellite timing offset, the satellite's first wait.
const SATELLITE_WAITS: [u8; 6] = [0xB0, 0x04, 0xB0, 0x04, 0xB0, 0x04];
/// `$04:EE52`: the marker kinds' header (rows and base, by timing offset).
const MARKER_HEADER: [u8; 12] = [0x06, 0x00, 0x0C, 0x00, 0x14, 0x00, 0x18, 0x00, 0x14, 0x00, 0x54, 0x00];

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

/// The placement scratch (D9B4..D9B9): kind, menu row, position and spawn
/// target, each placement setting only some of them.
struct Placement {
    kind: u8,
    menu: u8,
    at: [u8; 4],
}

impl MapVisit {
    /// `$04:DCBC` and the program's head up to its loop (`$04:AE57..AE92`).
    pub fn enter(&mut self, mut inputs: EntryInputs, output: &mut ScreenOutput) -> Result<(), VisitError> {
        self.map.globals.speed_flags = 0;
        self.set_up_campaign(&mut inputs)?;
        self.links.hud_mode = 2;
        self.hud_inputs.layout = 5;
        self.links.scene.stage_results &= !0x0010;
        self.map.globals.hold |= 0x003F;
        self.links.display_flags = (self.links.display_flags & !0x0002) | 0x0100 | 0x0200 | 0x0400 | 0x0080 | 0x0004;
        self.links.presentation_flags |= 0x0008;
        self.map.globals.speed_flags |= 0x0040;
        self.links.menu_pad = [0, 0];
        self.place_ship(output);
        self.sprites.target_countdown = 8;
        self.upload = 0;
        self.links.message = 0;
        self.sprites.satellite_icon = (0x0008, 0x0000);
        self.map.globals.satellite_flags &= !0x0200;
        self.sprites.satellite_beam = (0x000A, 0x0000);
        self.hud.marker_cycle[0] = 0x0A;
        self.hud.marker_cycle[1] = 0x0A;
        self.screen.warning_flash[0] = 0x0A;
        self.screen.warning_flash[1] = 0x0A;
        self.hud.station_cycle = [0x04, 0x04, 0x00, 0x00];
        let warnings = self.screen.planet_warnings as u8;
        if warnings != 0 {
            if warnings != 1 {
                self.screen.warning_flash[0] = 0x04;
                self.screen.warning_flash[1] = 0x04;
            }
            self.links.display_flags |= 0x0400;
        }
        self.links.mode |= 0x0010;
        if self.links.scene.scene_events & EVENTS_FROM_STAGE != 0 {
            return Err(ScreenError::Unported(0x04DE00).into());
        }
        // $0B:8C17 and the presentation's countdown.
        self.links.presentation_countdown = 0x000A;
        self.links.presentation_flags |= 0x0002;
        self.load_layers();
        // $04:8001 (`$04:9D3C`, `$04:A109`): the sprite buffer's low table
        // hidden and its high table cleared. The fill stores E8E8 and copies
        // the first entry forward over the rest, so every entry keeps the
        // first entry's old tile and attribute bytes.
        let (low, high) = self.sprites.oam.split_at_mut(0x200);
        let pattern = [0xE8, 0xE8, low[2], low[3]];
        for entry in low.chunks_exact_mut(4) {
            entry.copy_from_slice(&pattern);
        }
        high.fill(0);
        // $0B:9F4C.
        Radio {
            radio: &mut self.radio,
            message_box: &mut self.message_box,
            sprites: &mut self.sprites,
            screen: &mut self.screen,
            director: &mut self.director,
            map: &mut self.map,
            links: &mut self.links,
            inputs: RadioInputs::default(),
            output,
        }
        .reset();
        // $03:D847.
        self.links.presentation_flags &= !0x0001;
        // $04:AE6B.
        self.links.mode |= 0x0004;
        if self.map.globals.encounter_request == 3 {
            return Err(ScreenError::Unported(0x04AF74).into());
        }
        self.begin_program_frame();
        Ok(())
    }

    /// `$04:DEA9`.
    pub fn set_up_campaign(&mut self, inputs: &mut EntryInputs) -> Result<(), VisitError> {
        let results = self.links.scene.stage_results;
        if results & RESULTS_SET_UP != 0 {
            return Ok(());
        }
        self.links.scene.stage_results |= RESULTS_SET_UP;
        if results & RESULTS_FROM_STAGE != 0 {
            return Err(ScreenError::Unported(0x04E05E).into());
        }
        self.links.scene.scene_events = 0;
        self.links.scene.campaign_events = 0;
        self.map.globals.encounter_request = 0;
        if results & (RESULTS_TUTORIAL | RESULTS_NEW) != 0 {
            self.new_campaign(inputs)?;
        } else {
            // $04:E055.
            self.director.aftermath = 0x000A;
        }
        // $04:E070.
        self.screen.ship.travel |= 0x0200;
        Ok(())
    }

    /// `$04:DEFB`: a new campaign.
    fn new_campaign(&mut self, inputs: &mut EntryInputs) -> Result<(), VisitError> {
        self.links.stage_flags |= 0x0002;
        self.clear_campaign();
        let globals = &mut self.map.globals;
        globals.slot_cursor = 0;
        globals.slot_used = [0; 16];
        globals.slot_pattern = [0; 16];
        globals.slot_word = [0; 16];
        globals.spawn_pattern = inputs.seed & 0x007F;
        globals.escort_pattern = inputs.seed & 0x000F;
        // $7F:058C with a sixteen-bit accumulator: the random word steps.
        let random = self.links.random.wrapping_add(1);
        let entropy = self.rng.next_word(&mut inputs.generator_tail);
        self.links.random = random.wrapping_add(self.screen.elapsed_steps).wrapping_add(entropy);
        set_low(&mut self.links.planet_health, 0x64);
        self.hud_inputs.score = 0;
        self.screen.elapsed_steps = 0;
        let difficulty = usize::from(self.links.difficulty);
        let marks = u16::from(table_byte(&MARKS, difficulty, 0x04DFA4)?);
        self.director.tally.marks_left = marks;
        let fleets = u16::from(table_byte(&FLEETS, difficulty, 0x04DFB4)?);
        self.director.tally.fleets = fleets;
        self.map.globals.launch_count = fleets;
        self.screen.campaign.start_cursor = fleets;
        self.director.tally.enemies_left = u16::from(table_byte(&ENEMIES, difficulty, 0x04DFC9)?);
        set_low(&mut self.screen.campaign.wave_escorts, table_byte(&ESCORT_NUMBERS, difficulty, 0x04DFD8)?);
        self.reset_pools();
        let mut placement = Placement { kind: 0, menu: 0, at: self.screen.campaign.placement };
        self.guarded_place(&mut placement)?;
        self.marked_places(&mut placement, inputs.seed)?;
        self.planet(&mut placement, inputs.tallies)?;
        self.screen.campaign.wave_cursor = super::strategic_script::wave_start(self.links.difficulty)?;
        self.base(&mut placement)?;
        self.station(&mut placement)?;
        self.warp(&mut placement)?;
        self.screen.campaign.placement = placement.at;
        self.screen.planet_warnings = 0;
        let timing = usize::from(self.links.satellite_timing);
        self.director.schedule_cursor = table_word(&SCHEDULE_STARTS, timing, 0x04E00E)?;
        self.map.globals.missile_count = u16::from(table_byte(&MISSILES, difficulty, 0x04E017)?);
        self.map.globals.pass_holds |= 0x0020;
        self.director.handshake |= 0x0001;
        self.links.timeline = 0x0002;
        if self.links.scene.stage_results & RESULTS_TUTORIAL == 0 {
            self.screen.planet_warning = 1;
        }
        Ok(())
    }

    /// The fixed-source transfer of `$03:EE9F` (zero) over D9FD..E82C.
    fn clear_campaign(&mut self) {
        let screen = &mut self.screen;
        screen.cursor_x = 0;
        screen.cursor_y = 0;
        screen.interface = 0;
        screen.hover = 0;
        screen.countdown = 0;
        screen.selected_x = 0;
        screen.selected_y = 0;
        screen.arrival_x = 0;
        screen.arrival_y = 0;
        screen.pulses = Default::default();
        screen.clock_frames = 0;
        screen.elapsed_steps = 0;
        screen.threat_cue_timer = 0;
        screen.select_state = 0;
        screen.script = 0;
        screen.menu_choice = 0;
        screen.menu_home = 0;
        screen.menu_confirm = 0;
        screen.menu_place = None;
        screen.ship = Default::default();
        screen.wingmate = Default::default();
        screen.info = Default::default();
        screen.hovered_place = None;
        screen.ordered_place = None;
        screen.hovered_unit = None;
        screen.script_offset = 0;
        screen.script_subject = ScriptSubject::Value(0);
        screen.script_countdown = 0;
        screen.saved_ship = (0, 0);
        screen.warp_target = [0; 3];
        screen.warp_spare = 0;
        screen.saved_motion = 0;
        let campaign = &mut screen.campaign;
        campaign.marker_cursor = 0;
        campaign.markers_placed = 0;
        campaign.wave_cursor = 0;
        campaign.wave_escorts = 0;
        campaign.start_cursor = 0;
        campaign.bases_left = 0;
        let links = &mut self.links;
        links.planet_health = 0;
        links.planet_damage_percent = 0;
        links.planet_place = None;
        links.station_place = None;
        links.pursuit_unit = None;
        links.sortie_escort = None;
        links.warp_place = None;
        links.warp_unit = None;
        links.arrival_word = 0;
        links.final_stage = 0;
        links.timeline = 0;
        links.batch_bonus = 0;
        links.scene.planet_damage = 0;
        links.scene.map_region = 0;
        let director = &mut self.director;
        let (handshake, saved_hold, choice, choosing, salvo_index) =
            (director.handshake, director.saved_hold, director.choice, director.choosing, director.salvo_index);
        *director = Default::default();
        director.handshake = handshake;
        director.saved_hold = saved_hold;
        director.choice = choice;
        director.choosing = choosing;
        director.salvo_index = salvo_index;
        let map = &mut self.map;
        map.places = [MapPlace::default(); PLACE_CAPACITY];
        map.units = [MapUnit::default(); UNIT_CAPACITY];
        map.place_head = None;
        map.place_free = None;
        map.unit_head = None;
        map.unit_free = None;
        let globals = &mut map.globals;
        globals.launch_count = 0;
        globals.unit_count = 0;
        globals.escort_count = 0;
        globals.fighter_launches = 0;
        globals.cruiser_count = 0;
        globals.missile_count = 0;
        globals.missile_salvo = 0;
        globals.interceptor_count = 0;
        globals.threat_kind = 0;
        globals.threat_place = None;
        globals.threat_countdown = 0;
        globals.threat_nearest = 0;
        globals.map_events = 0;
        globals.spawn_variant_bit = 0;
        globals.guarded_place = None;
        globals.base = None;
        globals.satellite = None;
        globals.satellite_phase = 0;
        globals.satellite_flags = 0;
        globals.satellite_hold = 0;
        globals.satellite_target = None;
        globals.satellite_guarding = 0;
        globals.satellite_facing = 0;
        globals.met_unit = None;
        globals.intercepted_unit = None;
        globals.intercepted_motion = 0;
        globals.intercepted_behavior = 0;
        globals.stored_counter = 0;
        globals.slot_kind = [0; 16];
        globals.slot_strength = [0; 16];
        self.sprites.speed_mark = 0;
        self.sprites.satellite_icon = (0, 0);
        self.sprites.satellite_beam = (0, 0);
        self.sprites.panel = [0; 8];
        self.radio.cleared_word = 0;
    }

    /// `$04:E0DC` and `$04:E0FE`: the place and unit pools, every record
    /// free and in order.
    fn reset_pools(&mut self) {
        let map = &mut self.map;
        map.place_head = None;
        map.globals.guarded_place = None;
        map.place_free = Some(PlaceId(0));
        for index in 0..PLACE_CAPACITY {
            map.places[index].next = (index + 1 < PLACE_CAPACITY).then(|| PlaceId(index as u8 + 1));
        }
        map.globals.unit_count = 0;
        map.unit_head = None;
        map.unit_free = Some(UnitId(0));
        for index in 0..UNIT_CAPACITY {
            map.units[index].next = (index + 1 < UNIT_CAPACITY).then(|| UnitId(index as u8 + 1));
        }
    }

    fn place(&mut self, placement: &Placement) -> Result<PlaceId, ScreenError> {
        new_place(&mut self.map, placement.at, placement.kind, placement.menu, self.links.difficulty)
    }

    fn unit(&mut self) -> Result<UnitId, ScreenError> {
        self.map.allocate_unit().map_err(ScreenError::Simulation)
    }

    /// `$04:E123`: the guarded place where the campaign begins.
    fn guarded_place(&mut self, placement: &mut Placement) -> Result<(), ScreenError> {
        placement.at[0] = 0x14;
        placement.at[1] = 0xA0;
        placement.kind = 0;
        placement.menu = 6;
        let id = self.place(placement)?;
        self.map.places[usize::from(id.0)].flags |= 0x1002;
        self.map.globals.guarded_place = Some(id);
        Ok(())
    }

    /// `$04:E1C3`: the six marked places, then the marker kinds' cursor.
    fn marked_places(&mut self, placement: &mut Placement, seed: u16) -> Result<(), ScreenError> {
        for row in (0..6u8).rev() {
            let at = usize::from(row) * 4;
            for k in 0..4 {
                placement.at[k] = table_byte(&MARKED_PLACES, at + k, 0x04E1CE)?;
            }
            placement.kind = row;
            placement.menu = row;
            let id = self.place(placement)?;
            self.map.places[usize::from(id.0)].flags |= 0x1201;
        }
        let offset = usize::from(self.links.satellite_timing.wrapping_mul(2));
        let base = table_word(&MARKER_HEADER, offset + 2, 0x04E20D)?;
        let rows = table_word(&MARKER_HEADER, offset, 0x04E213)? as u8;
        // $7F:6DBD: the hardware divider (a zero divisor leaves the
        // dividend as the remainder).
        let remainder = if rows == 0 { seed } else { seed % u16::from(rows) };
        let product = u16::from(self.director.tally.marks_left as u8) * (remainder & 0x00FF);
        self.screen.campaign.marker_cursor = product.wrapping_add(base);
        Ok(())
    }

    /// `$04:E30C`: the planet, once the difficulty's tally reaches its
    /// threshold.
    fn planet(&mut self, placement: &mut Placement, tallies: [u8; 3]) -> Result<(), ScreenError> {
        let difficulty = usize::from(self.links.difficulty);
        let tally = table_byte(&tallies, difficulty, 0x04E30F)?;
        let threshold = table_byte(&PLANET_TALLIES, difficulty, 0x04E312)?;
        if (tally.wrapping_sub(threshold) as i8) < 0 {
            return Ok(());
        }
        placement.at[0] = 0xA8;
        placement.at[1] = 0x30;
        placement.kind = 0;
        placement.menu = 0x0B;
        let place_id = self.place(placement)?;
        let place = &mut self.map.places[usize::from(place_id.0)];
        place.info = 0x0006;
        place.flags |= 0x2040;
        self.links.scene.campaign_events |= 0x0100;
        self.links.planet_place = Some(place_id);
        let id = self.unit()?;
        let unit = self.map.unit_mut(id);
        unit.home = Some(place_id);
        unit.behavior = 0;
        unit.motion = 0;
        unit.x = 0xA8;
        unit.y = 0x30;
        unit.flags |= 0x8000;
        unit.variant = 0x000B;
        unit.sprite = 0x0027;
        unit.frame = 0;
        unit.animation_timer = 0x0030;
        unit.flags |= 0x4000;
        unit.flags2 |= 0x0010;
        unit.radius = 3;
        self.map.places[usize::from(place_id.0)].held_unit = Some(id);
        Ok(())
    }

    /// `$04:E88E`: the enemy base and the fleet that pursues from it.
    fn base(&mut self, placement: &mut Placement) -> Result<(), ScreenError> {
        placement.at = [0xEC, 0x18, 0x50, 0x70];
        placement.kind = 0x0A;
        placement.menu = 0x0C;
        let place_id = self.place(placement)?;
        self.map.places[usize::from(place_id.0)].flags |= 0x0010;
        self.map.globals.base = Some(place_id);
        let id = self.unit()?;
        self.links.pursuit_unit = Some(id);
        let unit = self.map.unit_mut(id);
        unit.x = 0xEC;
        unit.y = 0x18;
        unit.target_x = 0x0050;
        unit.target_y = 0x0070;
        unit.category = 0x000B;
        unit.sprite = 0x0016;
        unit.frame = 0;
        unit.animation_timer = 0x0050;
        unit.motion = 0;
        unit.radius = 6;
        unit.flags |= 0x6200;
        unit.variant = 0x000A;
        unit.flags2 |= 0x0010;
        Ok(())
    }

    /// `$04:E73B`: the defence station and its satellite.
    fn station(&mut self, placement: &mut Placement) -> Result<(), ScreenError> {
        placement.at[0] = 0x50;
        placement.at[1] = 0x70;
        placement.menu = 9;
        let place_id = self.place(placement)?;
        self.map.places[usize::from(place_id.0)].flags |= 0x2020;
        self.links.station_place = Some(place_id);
        let id = self.unit()?;
        self.map.globals.satellite = Some(id);
        self.map.places[usize::from(place_id.0)].held_unit = Some(id);
        let wait = table_word(&SATELLITE_WAITS, usize::from(self.links.satellite_timing), 0x04E7BE)?;
        let unit = self.map.unit_mut(id);
        unit.home = Some(place_id);
        unit.behavior = 0;
        unit.motion = 0;
        unit.x = 0x50;
        unit.y = 0x70;
        unit.flags |= 0x2400;
        unit.variant = 9;
        unit.heading = 0;
        unit.program_timer = 0x003C;
        unit.count = 0x0014;
        unit.strength = 1;
        unit.radius = 8;
        unit.timer = wait;
        self.map.globals.satellite_flags |= 0x0002;
        Ok(())
    }

    /// `$04:E14B`: the warp point and the ship that waits at it.
    fn warp(&mut self, placement: &mut Placement) -> Result<(), ScreenError> {
        self.screen.warp_target[0] = 0x14;
        self.screen.warp_target[1] = 0xA0;
        placement.at[0] = 0x14;
        placement.at[1] = 0xA0;
        placement.kind = 8;
        placement.menu = 8;
        let place_id = self.place(placement)?;
        self.map.places[usize::from(place_id.0)].flags |= 0x2008;
        self.screen.menu_place = self.map.globals.guarded_place;
        self.screen.menu_choice = 6;
        self.screen.menu_confirm = 6;
        self.links.warp_place = Some(place_id);
        let id = self.unit()?;
        self.links.warp_unit = Some(id);
        let unit = self.map.unit_mut(id);
        unit.x = 0x14;
        unit.y = 0xA0;
        unit.speed = 0x2710;
        unit.motion = 0;
        unit.flags |= 0x2100;
        unit.variant = 8;
        unit.radius = 6;
        Ok(())
    }

    /// `$04:DE71`: the ship sets out toward the campaign's start and faces
    /// it; its wingmate takes its place.
    fn place_ship(&mut self, output: &mut ScreenOutput) {
        let ship = &mut self.screen.ship;
        ship.target_x = 0x14;
        ship.target_y = 0xA0;
        let (x, y) = ((ship.x >> 8) as u8, (ship.y >> 8) as u8);
        set_high(&mut self.screen.wingmate.x, x);
        set_high(&mut self.screen.wingmate.y, y);
        self.screen.ship.travel |= 0x0001;
        let mut frame = Frame {
            screen: &mut self.screen,
            map: &mut self.map,
            links: &mut self.links,
            terrain: &self.terrain,
            pad: MapPad::default(),
            output,
        };
        frame.face_target();
        frame.screen.ship.travel &= !0x0001;
        frame.show_ship();
    }

    /// `$04:EA8C`'s state: the window layers, the terrain grid (`$04:EB80`)
    /// and the palette shadow (`$04:EC76`) with the planet's colours.
    fn load_layers(&mut self) {
        self.radio.layers = 0x17;
        self.terrain = MAP_TERRAIN.to_vec();
        // $04:EBC4: the HUD's numbers are redrawn.
        self.map.globals.speed_flags |= 0x2000;
        self.hud.palette = MAP_PALETTE.to_vec();
        let percent = self.links.planet_damage_percent as u8;
        if percent >= 0x32 {
            planet_colours(&mut self.hud, if percent >= 0x50 { 3 } else { 0 });
        }
    }
}

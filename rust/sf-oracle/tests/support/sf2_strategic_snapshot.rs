//! Decoding of the strategic map simulation's retail state (`$7F:537D`'s
//! pools, shared words and scene links) for the oracles.

use sf2_game::strategic_sim::{
    MapPlace, MapUnit, PlaceId, SceneLinks, StrategicGlobals, StrategicMap, UnitId, PLACE_CAPACITY, UNIT_CAPACITY,
};
use sf_oracle::RetailMachine;

pub(crate) const PLACES: u16 = 0xDB69;
pub(crate) const PLACE_SIZE: u16 = 0x56;
pub(crate) const UNITS: u16 = 0xE0A7;
pub(crate) const UNIT_SIZE: u16 = 0x52;
pub(crate) const TERRAIN: u32 = 0x7FD400;
pub(crate) const TERRAIN_BYTES: u32 = 0x400;
pub(crate) const GSU_BYTES: usize = 0x400;

/// Bytes of retail work RAM, captured at one instant.
pub(crate) struct Snapshot {
    pub(crate) low: Vec<u8>,
    pub(crate) terrain: Vec<u8>,
    /// GSU RAM `$70:0000..0400` (the message box's words).
    pub(crate) gsu: Vec<u8>,
}

impl Snapshot {
    pub(crate) fn take(m: &RetailMachine) -> Self {
        Self {
            low: (0..0x10000u32).map(|a| m.peek8(0x7E0000 + a)).collect(),
            terrain: (0..TERRAIN_BYTES).map(|a| m.peek8(TERRAIN + a)).collect(),
            gsu: (0..GSU_BYTES).map(|a| m.peek_gsu_ram(a)).collect(),
        }
    }
    pub(crate) fn byte(&self, address: u16) -> u8 {
        self.low[usize::from(address)]
    }
    pub(crate) fn gsu_word(&self, address: usize) -> u16 {
        u16::from(self.gsu[address]) | (u16::from(self.gsu[address + 1]) << 8)
    }
    pub(crate) fn word(&self, address: u16) -> u16 {
        u16::from(self.byte(address)) | (u16::from(self.byte(address.wrapping_add(1))) << 8)
    }
    pub(crate) fn place_id(&self, pointer: u16) -> Option<PlaceId> {
        if pointer == 0 {
            return None;
        }
        let offset = pointer.wrapping_sub(PLACES);
        assert_eq!(offset % PLACE_SIZE, 0, "place pointer {pointer:04X}");
        let index = offset / PLACE_SIZE;
        assert!(usize::from(index) < PLACE_CAPACITY, "place pointer {pointer:04X}");
        Some(PlaceId(index as u8))
    }
    pub(crate) fn unit_id(&self, pointer: u16) -> Option<UnitId> {
        if pointer == 0 {
            return None;
        }
        let offset = pointer.wrapping_sub(UNITS);
        assert_eq!(offset % UNIT_SIZE, 0, "unit pointer {pointer:04X}");
        let index = offset / UNIT_SIZE;
        assert!(usize::from(index) < UNIT_CAPACITY, "unit pointer {pointer:04X}");
        Some(UnitId(index as u8))
    }

    pub(crate) fn place(&self, index: usize) -> MapPlace {
        let base = PLACES + index as u16 * PLACE_SIZE;
        let w = |offset: u16| self.word(base + offset);
        MapPlace {
            next: self.place_id(w(0x00)),
            prev: self.place_id(w(0x02)),
            kind: w(0x04),
            menu_index: w(0x06),
            info: w(0x08),
            stage: w(0x0A),
            x: w(0x0C),
            y: w(0x0E),
            spawn_target_x: w(0x10),
            spawn_target_y: w(0x12),
            cell_x: w(0x14),
            cell_y: w(0x16),
            program: w(0x18),
            program_repeats: w(0x1A),
            flags: w(0x1C),
            status: w(0x1E),
            marker: w(0x20),
            arrivals: w(0x22),
            heading: w(0x24),
            spawned_unit: self.unit_id(w(0x26)),
            held_unit: self.unit_id(w(0x28)),
            target_x: w(0x2A),
            target_y: w(0x2C),
            route_x: w(0x2E),
            route_y: w(0x30),
            warning: w(0x32),
            warning_countdown: w(0x34),
            state: w(0x36),
            spawn_remaining: w(0x3A),
            spawn_batch: w(0x3C),
            timer: w(0x3E),
            warning_rate: w(0x4A),
            warning_bias: w(0x4C),
        }
    }

    pub(crate) fn unit(&self, index: usize) -> MapUnit {
        let base = UNITS + index as u16 * UNIT_SIZE;
        let w = |offset: u16| self.word(base + offset);
        let b = |offset: u16| self.byte(base + offset);
        MapUnit {
            next: self.unit_id(w(0x00)),
            prev: self.unit_id(w(0x02)),
            behavior: w(0x04),
            saved_behavior: w(0x06),
            timer: w(0x08),
            program: w(0x0A),
            program_timer: w(0x0C),
            motion: w(0x0E),
            saved_motion: w(0x10),
            home: self.place_id(w(0x12)),
            origin: self.place_id(w(0x14)),
            x_fraction: w(0x1A),
            x: b(0x1C),
            y_fraction: w(0x1D),
            y: b(0x1F),
            target_x: w(0x20),
            target_y: w(0x22),
            heading: w(0x24),
            speed: w(0x26),
            vx_fraction: w(0x28),
            vx: b(0x2A),
            vy_fraction: w(0x2B),
            vy: b(0x2D),
            flags: w(0x2E),
            flags2: w(0x30),
            category: w(0x32),
            kind: w(0x34),
            variant: w(0x36),
            strength: w(0x38),
            encounter_slot: w(0x3A),
            count: w(0x3C),
            node_variant: w(0x3E),
            radius: w(0x40),
            proximity_delay: w(0x42),
            sprite: w(0x44),
            frame: w(0x46),
            animation_timer: w(0x48),
            turn_rate: w(0x4A),
            turn_bias: w(0x4C),
            turn_countdown: w(0x4E),
        }
    }

    pub(crate) fn map(&self) -> StrategicMap {
        StrategicMap {
            places: std::array::from_fn(|index| self.place(index)),
            units: std::array::from_fn(|index| self.unit(index)),
            place_head: self.place_id(self.word(0xDB67)),
            place_free: self.place_id(self.word(0xDB65)),
            unit_head: self.unit_id(self.word(0xE0A3)),
            unit_free: self.unit_id(self.word(0xE0A5)),
            globals: self.globals(),
        }
    }

    pub(crate) fn globals(&self) -> StrategicGlobals {
        StrategicGlobals {
            hold: self.word(0x1B8C),
            pass_holds: self.word(0x1B90),
            speed_flags: self.word(0x1B92),
            campaign_flags: self.word(0x1B94),
            encounter_request: self.word(0x1B74),
            launch_count: self.word(0xDA39),
            unit_count: self.word(0xDA43),
            escort_count: self.word(0xDA45),
            fighter_launches: self.word(0xDA47),
            cruiser_count: self.word(0xDA4D),
            missile_count: self.word(0xDA53),
            missile_salvo: self.word(0xDA55),
            interceptor_count: self.word(0xDA57),
            threat_kind: self.word(0xDA69),
            threat_place: self.place_id(self.word(0xDA6B)),
            threat_countdown: self.word(0xDA71),
            threat_nearest: self.word(0xDA73),
            interceptors: self.word(0xD998),
            launches_pending: self.word(0xD9B0),
            spawn_pattern: self.word(0xD9F9),
            escort_pattern: self.word(0xD9FB),
            map_events: self.word(0xDB33),
            spawn_variant_bit: self.word(0xDB3B),
            guarded_place: self.place_id(self.word(0xDB45)),
            base: self.place_id(self.word(0xDB61)),
            satellite: self.unit_id(self.word(0xE079)),
            satellite_phase: self.word(0xE085),
            satellite_flags: self.word(0xE087),
            satellite_hold: self.word(0xE089),
            satellite_target: self.unit_id(self.word(0xE091)),
            satellite_guarding: self.word(0xE093),
            satellite_facing: self.word(0xE095),
            met_unit: self.unit_id(self.word(0xE097)),
            intercepted_unit: self.unit_id(self.word(0xE099)),
            intercepted_motion: self.word(0xE09B),
            intercepted_behavior: self.word(0xE09D),
            alerts: self.word(0xF55C),
            threat_cue: self.byte(0x1CD3) & 0x10 != 0,
            stored_target: self.word(0xD7FC),
            stored_counter: self.word(0xDA11),
            slot_cursor: self.word(0xD7FE),
            slot_used: std::array::from_fn(|i| self.byte(0xD800 + i as u16)),
            slot_pattern: std::array::from_fn(|i| self.byte(0xD7C2 + i as u16)),
            slot_word: std::array::from_fn(|i| self.word(0xD7A2 + 2 * i as u16)),
            slot_kind: std::array::from_fn(|i| self.byte(0xE805 + i as u16)),
            slot_strength: std::array::from_fn(|i| self.byte(0xE815 + i as u16)),
        }
    }

    pub(crate) fn links(&self) -> SceneLinks {
        SceneLinks {
            stage_results: self.word(0x1B86),
            scene_events: self.word(0x1B88),
            campaign_events: self.word(0x1B8A),
            event_word: self.word(0x1C0E),
            planet_damage: self.word(0xDB4B),
            map_region: self.word(0xDB5B),
            encounter_result: self.word(0xD79D),
            node_variant: self.byte(0x1E09),
        }
    }
}


/// The records and words where two maps differ, for failure messages.
#[allow(dead_code)]
pub(crate) fn map_differences(native: &StrategicMap, retail: &StrategicMap) -> Vec<String> {
    let mut out = Vec::new();
    for index in 0..PLACE_CAPACITY {
        if native.places[index] != retail.places[index] {
            out.push(format!("place {index}: native {:?}\n  retail {:?}", native.places[index], retail.places[index]));
        }
    }
    for index in 0..UNIT_CAPACITY {
        if native.units[index] != retail.units[index] {
            out.push(format!("unit {index}: native {:?}\n  retail {:?}", native.units[index], retail.units[index]));
        }
    }
    if native.globals != retail.globals {
        out.push(format!("globals: native {:?}\n  retail {:?}", native.globals, retail.globals));
    }
    let heads = |m: &StrategicMap| (m.place_head, m.place_free, m.unit_head, m.unit_free);
    if heads(native) != heads(retail) {
        out.push(format!("heads: native {:?} retail {:?}", heads(native), heads(retail)));
    }
    out
}

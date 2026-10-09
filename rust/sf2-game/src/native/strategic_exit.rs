//! The map program's exits (`$04:AED1..AEE4` and the exits it dispatches
//! on 1B74): an encounter launches a stage, the tutorial ends, the planet
//! falls or the campaign's end is reached. Each sets the next mode and the
//! stage's launch words, then the map fades out and its epilogue
//! (`$04:B154..B1BF`) clears the map's frame words.
//!
//! The fade's window shapes and brightness are presentation. Each of its
//! frames runs the map's sprite pass, then the frame's display work
//! (`$7F:0412`). The fade ends when its countdown runs out or the render
//! interrupt's brightness service has darkened the screen, so the caller
//! passes the frames after which the screen is dark.

use super::strategic_screen::{MapPad, ScreenError, ScreenOutput};
use super::strategic_sim::{byte_at, word_at};
use super::strategic_sprites::{self, SpriteInputs};
use super::strategic_visit::{MapVisit, VisitError};

/// The stage-launch and mode words the exits write.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapLaunch {
    /// 1B68: the top-level mode; 1B6A the mode to enter next.
    pub mode: u16,
    pub next_mode: u16,
    /// 1B76: the scene the next mode starts with.
    pub scene: u8,
    /// 1B96: the stage's start options.
    pub options: u16,
    /// 1BA7: the met unit's encounter slot.
    pub encounter_slot: u16,
    /// 1BA9: the met unit's heading (its high byte).
    pub encounter_heading: u8,
    /// D79F: the slot's word; D7A1 the stage's rank; D7F6 the place's stage
    /// word (+0A).
    pub slot_word: u16,
    pub rank: u8,
    pub place_stage: u8,
    /// D7D3: the encounter's bonus word.
    pub bonus: u16,
    /// D79A, D79B, D7D5 and D79C: encounter flags.
    pub encounter_flags: [u8; 4],
    /// D786..D791 and CF33: the encounter's tallies.
    pub tallies: [u16; 6],
    pub hits: u16,
    /// 1DE4, 1DE8 (the ship's position, y negated), 1DEA (its heading),
    /// 1DF3, 1DF5 (the met unit's position, y negated).
    pub ship_view: (u16, u16, u8),
    pub unit_view: (u16, u16),
    /// D794..D799: by location, a rank that overrides the table.
    pub location_ranks: [u8; 6],
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExitInputs {
    /// The fade frames after which the screen is dark (the brightness
    /// service's count); the countdown ends the fade after 59 regardless.
    pub dark_after: u16,
    /// The pads the frame interrupts read while the map leaves, OR-ed.
    pub pad: MapPad,
    /// DP E4, which the sixteen-bit generator reads and writes.
    pub generator_tail: u8,
}

/// `$00:B0E6`: by location (three difficulties each), the stage's rank.
const LOCATION_RANKS: [u8; 33] = [
    0x03, 0x04, 0x02, 0x03, 0x03, 0x03, 0x00, 0x03, 0x02, 0x03, 0x02, 0x05, 0x02, 0x04, 0x03, 0x00, 0x03, 0x02,
    0x03, 0x03, 0x03, 0x03, 0x21, 0x31, 0x04, 0x04, 0x21, 0x31, 0x21, 0x22, 0x05, 0x05, 0x31,
];
/// `$06:FCF1`: the encounter's bonus words, picked at random.
const BONUSES: [u8; 16] = [0x80, 0xF3, 0x80, 0x0C, 0x74, 0xF5, 0x8C, 0x0A, 0x68, 0xF7, 0x98, 0x08, 0x5C, 0xF9, 0xA4, 0x06];

/// The cue `$7F:6E09` queues as the map leaves for a stage, and the one an
/// interception queues.
const CUE_LEAVE: u16 = 0x0060;
const CUE_INTERCEPT: u16 = 0x006E;
/// 1C6E: the fade's frames (`$0B:8C57`'s parameter, `$04:B14D`'s count).
const FADE_FRAMES: u16 = 0x003C;

fn set_low(word: &mut u16, byte: u8) {
    *word = (*word & 0xFF00) | u16::from(byte);
}

/// How the map leaves after its exit's words: `$04:B121`, the closing
/// iris (director state 0x10) with the sprite pass each frame, or
/// `$04:B14D`, a plain wait (`$03:E017`).
enum Leaving {
    Iris,
    Wait,
}

impl MapVisit {
    /// Whether the program frame's wait ends in an exit (1B74 set).
    pub fn exit_due(&self) -> bool {
        self.map.globals.encounter_request & 0x00FF != 0
    }

    /// `$04:AED1`, the exit 1B74 selects, the fade and the epilogue.
    pub fn leave(&mut self, mut inputs: ExitInputs, output: &mut ScreenOutput) -> Result<(), VisitError> {
        self.map.globals.speed_flags |= 0x0040;
        if self.exit_words(&mut inputs, output)? {
            self.close_iris(inputs, output)?;
        } else {
            self.wait_out(inputs);
        }
        self.epilogue();
        Ok(())
    }

    /// The exit 1B74 selects (`$04:AEE4`), up to its fade: the next mode
    /// and the launch words. Returns whether the exit fades by the iris.
    pub fn exit_words(&mut self, inputs: &mut ExitInputs, output: &mut ScreenOutput) -> Result<bool, VisitError> {
        let leaving = match self.map.globals.encounter_request & 0x00FF {
            1 => self.leave_tutorial(),
            2 => self.leave_for_encounter(inputs, output)?,
            3 => self.leave_campaign(output),
            4 => self.leave_for_final(output),
            5 => self.leave_for_threat(output)?,
            other => return Err(ScreenError::Unported(0x04AEE4 + 2 * u32::from(other)).into()),
        };
        Ok(matches!(leaving, Leaving::Iris))
    }

    /// `$04:AEF0`: a threat's place.
    fn leave_for_threat(&mut self, output: &mut ScreenOutput) -> Result<Leaving, VisitError> {
        set_low(&mut self.launch.next_mode, 4);
        self.launch.scene = 0x28;
        self.links.scene.scene_events |= 0x0010;
        let id = self.map.globals.threat_place.ok_or(ScreenError::MissingPlace(0x04AF09))?;
        let place = self.map.places[usize::from(id.0)];
        self.screen.arrival_x = place.target_x as u8;
        self.screen.arrival_y = place.target_y as u8;
        let _ = output;
        Ok(Leaving::Iris)
    }

    /// `$04:AF1B`: the campaign's final stage.
    fn leave_for_final(&mut self, output: &mut ScreenOutput) -> Leaving {
        set_low(&mut self.launch.next_mode, 4);
        self.launch.scene = 0x20;
        self.links.scene.scene_events |= 0x0010 | 0x0800;
        self.screen.arrival_x = 0xE8;
        self.screen.arrival_y = 0x20;
        output.cues.push(CUE_LEAVE);
        Leaving::Iris
    }

    /// `$04:AF56`: the tutorial map ends.
    fn leave_tutorial(&mut self) -> Leaving {
        set_low(&mut self.launch.next_mode, 6);
        self.links.scene.stage_results &= !0x0040 & !0x0080;
        self.map.globals.hold &= !0x003F;
        Leaving::Wait
    }

    /// `$04:AF74`: the planet has fallen.
    fn leave_campaign(&mut self, output: &mut ScreenOutput) -> Leaving {
        self.links.scene.stage_results |= 0x0008;
        set_low(&mut self.launch.next_mode, 4);
        self.launch.scene = 0x14;
        self.screen.arrival_x = 0x20;
        self.screen.arrival_y = 0xC0;
        let _ = output;
        Leaving::Iris
    }

    /// `$04:AF9A`: the ship met a unit or reached a place.
    fn leave_for_encounter(&mut self, inputs: &mut ExitInputs, output: &mut ScreenOutput) -> Result<Leaving, VisitError> {
        self.links.scene.scene_events |= 0x0010;
        self.reset_encounter(&mut inputs.generator_tail)?;
        if self.links.scene.stage_results & 0x0020 != 0 {
            // $04:B026.
            self.launch.next_mode = 1;
            self.launch.options |= 0x0080;
            output.cues.push(CUE_LEAVE);
            return Ok(Leaving::Iris);
        }
        if self.links.scene.scene_events & 0x0004 == 0 {
            // $04:B042: a place.
            output.cues.push(CUE_LEAVE);
            set_low(&mut self.launch.next_mode, 2);
            self.launch_place()?;
            return Ok(Leaving::Iris);
        }
        let id = self.map.globals.met_unit.ok_or(ScreenError::MissingUnit(0x04AFB9))?;
        let unit = *self.map.unit(id);
        self.links.scene.scene_events &= !0x2000;
        if unit.flags2 & 0x0002 != 0 {
            self.links.scene.scene_events |= 0x2000;
        }
        self.screen.arrival_x = (self.screen.ship.x >> 8) as u8;
        self.screen.arrival_y = (self.screen.ship.y >> 8) as u8;
        if unit.flags & 0x8000 != 0 {
            // $04:AFFE.
            self.links.launch_layout = 6;
            self.links.launch_location = 0;
            self.links.mission_rank = 1;
            self.links.arrival_word = 0x000A;
            self.launch.next_mode = 1;
            output.cues.push(CUE_LEAVE);
            return Ok(Leaving::Iris);
        }
        if unit.flags & 0x0040 != 0 {
            // $04:B05A: the unit's home place is entered.
            self.links.arrival_word = 2;
            let home = unit.home.ok_or(ScreenError::MissingPlace(0x04B060))?;
            let place = &mut self.map.places[usize::from(home.0)];
            place.flags |= 0x0080;
            if place.status & 0x0002 != 0 {
                self.launch.encounter_flags[1] = 1;
            }
            set_low(&mut self.launch.next_mode, 2);
            output.cues.push(CUE_LEAVE);
        } else {
            if unit.flags & 0x0080 != 0 {
                self.links.arrival_word = 6;
            } else {
                // $04:B098.
                self.links.arrival_word = 4;
                if unit.flags & 0x0800 != 0 {
                    if self.map.globals.satellite_flags & 0x0400 != 0 {
                        self.map.globals.satellite_flags |= 0x1000;
                    }
                } else if unit.flags & 0x0008 == 0 {
                    self.links.scene.campaign_events |= 0x0010;
                }
            }
            set_low(&mut self.launch.next_mode, 1);
            output.cues.push(CUE_INTERCEPT);
        }
        // $04:B0D3.
        self.launch_unit()?;
        self.launch.ship_view = (
            self.screen.ship.x >> 8,
            (self.screen.ship.y >> 8).wrapping_neg() & 0x00FF,
            (self.screen.ship.heading >> 8) as u8,
        );
        self.launch.unit_view = (u16::from(unit.x), u16::from(unit.y).wrapping_neg() & 0x00FF);
        self.launch.encounter_heading = (unit.heading >> 8) as u8;
        Ok(Leaving::Iris)
    }

    /// `$04:B1C0`: the encounter's words start over; its bonus is drawn.
    fn reset_encounter(&mut self, tail: &mut u8) -> Result<(), ScreenError> {
        self.launch.tallies = [0; 6];
        self.launch.hits = 0;
        self.links.mission_result = 0;
        self.links.scene.encounter_result = 0;
        // $7F:058C with a sixteen-bit accumulator.
        let random = self.links.random.wrapping_add(1);
        let entropy = self.rng.next_word(tail);
        let random = random.wrapping_add(self.screen.elapsed_steps).wrapping_add(entropy);
        self.links.random = random;
        let at = usize::from(random & 0x000E);
        self.launch.bonus = word_at(&BONUSES, at, 0x04B1EA).map_err(|_| ScreenError::TableOverrun(0x06FCF1))?;
        self.launch.encounter_flags[0] = 0;
        self.launch.encounter_flags[2] = 0;
        Ok(())
    }

    /// `$04:B1FC`: the destination place's stage.
    fn launch_place(&mut self) -> Result<(), ScreenError> {
        let id = self.screen.ship.destination.ok_or(ScreenError::MissingPlace(0x04B1FC))?;
        let place = self.map.places[usize::from(id.0)];
        set_low(&mut self.links.launch_location, place.kind as u8);
        set_low(&mut self.links.launch_layout, place.info as u8);
        self.launch.place_stage = place.stage as u8;
        let mut rank = self.location_rank(self.links.launch_location)?;
        if rank == 0 {
            let at = (self.links.launch_location as u8)
                .wrapping_mul(3)
                .wrapping_add(self.links.difficulty as u8);
            rank = byte_at(&LOCATION_RANKS, usize::from(at), 0x04B230)
                .map_err(|_| ScreenError::TableOverrun(0x00B0E6))?;
        }
        set_low(&mut self.links.mission_rank, rank);
        self.launch.rank = rank;
        Ok(())
    }

    /// `$04:B21E`: D794 indexed by the location; past the six ranks it
    /// reads the encounter words that follow.
    fn location_rank(&self, location: u16) -> Result<u8, ScreenError> {
        Ok(match location {
            0..=5 => self.launch.location_ranks[usize::from(location)],
            6 => self.launch.encounter_flags[0],
            7 => self.launch.encounter_flags[1],
            8 => self.launch.encounter_flags[3],
            9 => self.links.mission_result as u8,
            10 => (self.links.mission_result >> 8) as u8,
            _ => return Err(ScreenError::TableOverrun(0x04B21E)),
        })
    }

    /// `$04:B259`: the met unit's stage.
    fn launch_unit(&mut self) -> Result<(), ScreenError> {
        let id = self.map.globals.met_unit.ok_or(ScreenError::MissingUnit(0x04B259))?;
        let unit = *self.map.unit(id);
        set_low(&mut self.links.launch_location, unit.category as u8);
        set_low(&mut self.links.launch_layout, unit.kind as u8);
        set_low(&mut self.launch.encounter_slot, unit.encounter_slot as u8);
        self.links.scene.node_variant = unit.node_variant as u8;
        if self.links.arrival_word as u8 == 2 {
            set_low(&mut self.links.mission_rank, 1);
            return Ok(());
        }
        let slot = usize::from(self.launch.encounter_slot);
        let globals = &self.map.globals;
        let pattern = *globals.slot_pattern.get(slot).ok_or(ScreenError::TableOverrun(0x04B28E))?;
        self.launch.rank = pattern;
        set_low(&mut self.links.mission_rank, (pattern & 0x0F).wrapping_add(pattern >> 4));
        self.launch.slot_word = *globals.slot_word.get(slot).ok_or(ScreenError::TableOverrun(0x04B2A8))?;
        Ok(())
    }

    /// `$04:B121` and `$03:DFA4`: the closing iris; each frame the
    /// director dims the layers and the sprite pass runs, until the
    /// countdown ends or the screen is dark.
    fn close_iris(&mut self, inputs: ExitInputs, output: &mut ScreenOutput) -> Result<(), VisitError> {
        self.links.presentation_countdown = FADE_FRAMES;
        self.links.presentation_flags |= 0x0004;
        let mut frames = 0;
        loop {
            self.links.presentation_countdown -= 1;
            if self.links.presentation_countdown == 0 {
                break;
            }
            // $0B:9626: director state 0x10.
            if self.links.scene.scene_events & 0x0400 == 0 {
                self.radio.layers = 0x16;
            }
            strategic_sprites::pass(
                &mut self.sprites,
                &mut self.screen,
                &self.director,
                &mut self.map,
                &mut self.links,
                SpriteInputs { panel_enabled: self.radio.panel, ..self.sprite_inputs },
                output,
            )?;
            // The frame's display work (`$7F:0412`) runs while the loop
            // waits for the next frame.
            self.display_frame();
            frames += 1;
            if frames >= inputs.dark_after {
                break;
            }
        }
        self.links.menu_pad[0] |= inputs.pad.held;
        self.links.menu_pad[1] |= inputs.pad.pressed;
        self.blank();
        Ok(())
    }

    /// `$04:B14D` (`$03:E017`): the screen fades while the countdown runs
    /// out.
    fn wait_out(&mut self, inputs: ExitInputs) {
        for _ in 1..FADE_FRAMES {
            self.display_frame();
        }
        self.links.presentation_countdown = 0;
        self.links.menu_pad[0] |= inputs.pad.held;
        self.links.menu_pad[1] |= inputs.pad.pressed;
        self.blank();
    }

    /// `$03:DFCE`: the screen is blanked and the presentation released.
    /// Both fades start a music command (1CD3 bit 0); once the sound port
    /// acknowledges it, the frame's sound service (`$7F:0F23`) silences the
    /// travel sound.
    fn blank(&mut self) {
        self.screen.travel_sound = 0;
        self.links.mode &= !0x0004;
        self.links.presentation_flags &= !0x0004;
    }

    /// `$04:B158`.
    fn epilogue(&mut self) {
        self.launch.mode = self.launch.next_mode;
        self.map.globals.encounter_request = 0;
        self.links.final_stage = 0;
        self.links.mode &= !0x0030;
        self.map.globals.hold &= !0x003F;
        self.map.globals.speed_flags = 0;
        self.director.handshake &= !0x0555;
        self.links.display_flags = 0;
        self.links.presentation_flags = 0;
        self.screen.ship.travel = 0;
        self.screen.wingmate.flags = 0;
        self.screen.interface = 0;
        self.screen.hover = 0;
        self.screen.info.shown &= !0x0001;
    }
}

//! A visit to the strategic map: the map program's loop (`$04:AE92`) on
//! the main thread and the map's frame interrupt (`$7F:0249`).
//!
//! The interrupt uploads the GSU's map picture in eight slices; after the
//! eighth it signals the main loop, which runs one program frame
//! (`$04:B9A3`) and starts the next picture. Between program frames the
//! interrupt runs eight service frames, each with the screen frame
//! (`$04:B5DA`) and the simulation tick (`$04:F002` -> `$7F:535E`). While a
//! picture is complete and the next not yet started, interrupts skip their
//! services. The picture's drawing, the HUD services and the window
//! effects are presentation and are not modeled here.

use super::strategic_director::{self, MapDirector};
use super::strategic_screen::{self, MapPad, MapScreen, ScreenError, ScreenLinks, ScreenOutput};
use super::strategic_sim::{StrategicInputs, StrategicMap, TickOutput};
use super::strategic_hud::{self, HudInputs, HudOutput, MapHud};
use super::strategic_sprites::{self, MapSprites, SpriteInputs};
use super::RandomState;

/// 1B92 bits the loop and the interrupt exchange.
const PICTURE_DONE: u16 = 0x0004;
const NEW_PROGRAM_FRAME: u16 = 0x0020;
const PICTURE_HOLD: u16 = 0x0040;
/// DA77: the upload offset; eight slices of 0x400.
const SLICE: u16 = 0x0400;
const PICTURE: u16 = 0x2000;
const SERVICE_ENTRIES: u16 = 8;
/// E83F..EA3E and the byte `$7F:0A2E` fills them with.
const OAM_ENTRIES: usize = 0x200;
const OAM_FILL: u8 = 0xE8;
/// `$00:DA5B`.
const INTERRUPT_DA5B: u8 = 0xDA;

/// The display frame's timers (`$7F:0516`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FrameTimers {
    /// EFD0: counts frames (a byte).
    pub ticks: u8,
    /// EFCD..EFCF and EFD1: countdowns held at zero.
    pub countdowns: [u8; 3],
    pub long_countdown: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapVisit {
    pub screen: MapScreen,
    pub director: MapDirector,
    pub map: StrategicMap,
    pub links: ScreenLinks,
    pub sprites: MapSprites,
    pub sprite_inputs: SpriteInputs,
    pub hud: MapHud,
    pub hud_inputs: HudInputs,
    pub timers: FrameTimers,
    /// The shared generator (`$7F:7BD4`).
    pub rng: RandomState,
    /// DA77.
    pub upload: u16,
    /// The terrain grid (`$7F:D400`).
    pub terrain: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisitError {
    Screen(ScreenError),
    InvalidService(u16),
}

impl From<ScreenError> for VisitError {
    fn from(error: ScreenError) -> Self {
        Self::Screen(error)
    }
}

impl MapVisit {
    /// Whether the interrupt has signalled a finished picture (the main
    /// loop's wait at `$04:AEC9`).
    pub fn program_frame_due(&self) -> bool {
        self.map.globals.speed_flags & PICTURE_DONE != 0
    }

    /// `$04:AED1` and the loop head (`$04:AE92`): the wait is over and
    /// the next program frame begins.
    pub fn begin_program_frame(&mut self) {
        let flags = &mut self.map.globals.speed_flags;
        *flags = ((*flags | PICTURE_HOLD) & !PICTURE_DONE) | NEW_PROGRAM_FRAME;
    }

    /// `$04:AEA8..AEAE`: one program frame; the next picture starts
    /// (`$7F:532A` releases the interrupt's services).
    pub fn program_frame(&mut self, output: &mut ScreenOutput) -> Result<(), VisitError> {
        strategic_director::step(
            &mut self.director,
            &mut self.screen,
            &mut self.map,
            &mut self.links,
            &self.terrain,
            output,
        )?;
        self.map.globals.speed_flags &= !PICTURE_HOLD;
        Ok(())
    }

    /// `$7F:0249`: one frame interrupt. `pad` is the frame's pad (1292
    /// held, 1296 newly pressed).
    pub fn interrupt(&mut self, pad: MapPad, output: &mut ScreenOutput) -> Result<(), VisitError> {
        if self.map.globals.speed_flags & PICTURE_HOLD == 0 {
            // $7F:02A1: the next slice of the picture.
            if self.map.globals.speed_flags & PICTURE_DONE == 0 {
                self.upload = self.upload.wrapping_add(SLICE);
                if self.upload == PICTURE {
                    self.map.globals.speed_flags |= PICTURE_DONE | PICTURE_HOLD;
                    self.upload = 0;
                }
            }
            self.display_frame();
            self.services(pad, output)?;
        }
        self.links.menu_pad[1] |= pad.pressed;
        self.links.menu_pad[0] |= pad.held;
        Ok(())
    }

    /// `$7F:0412`'s tail (`$7F:0516`): the frame timers, then the random
    /// word (`$7F:058C` with an eight-bit accumulator).
    fn display_frame(&mut self) {
        // $7F:0424..0444: after the OAM upload the map refills the entries
        // with E8 (`$7F:0A2E`, a fixed-source transfer of `$04:9C65`).
        if self.links.display_flags & 0x0001 == 0 {
            for byte in &mut self.sprites.oam[..OAM_ENTRIES] {
                *byte = OAM_FILL;
            }
        }
        let timers = &mut self.timers;
        timers.ticks = timers.ticks.wrapping_add(1);
        self.sprites.counter = self.sprites.counter.wrapping_add(1);
        for countdown in timers.countdowns.iter_mut() {
            *countdown = countdown.saturating_sub(1);
        }
        let next = timers.long_countdown.wrapping_sub(1);
        timers.long_countdown = if (next as i16) < 0 { 0 } else { next };
        let random = (self.links.random as u8).wrapping_add(1);
        let entropy = self.rng.next_byte();
        // The interrupt runs with data bank 00, so `ADC $DA5B` adds the ROM
        // byte there rather than the map clock.
        let random = random.wrapping_add(INTERRUPT_DA5B).wrapping_add(entropy);
        self.links.random = (self.links.random & 0xFF00) | u16::from(random);
    }

    /// `$7F:03C2`'s modeled services: the sprite pass, the screen frame,
    /// then the tick.
    fn services(&mut self, pad: MapPad, output: &mut ScreenOutput) -> Result<(), VisitError> {
        strategic_sprites::pass(
            &mut self.sprites,
            &mut self.screen,
            &self.director,
            &mut self.map,
            &mut self.links,
            self.sprite_inputs,
            output,
        )?;
        let phase = self.sprites.phases[0];
        strategic_hud::service(
            &mut self.hud,
            &mut self.screen,
            &mut self.director,
            &mut self.map,
            &mut self.links,
            phase,
            self.hud_inputs,
            &mut HudOutput::default(),
        )?;
        strategic_screen::frame(&mut self.screen, &mut self.map, &mut self.links, &self.terrain, pad, output)?;
        // $04:F002 -> $7F:535E: every service entry is the tick.
        let service = self.links.service;
        if service % 2 != 0 || service / 2 >= SERVICE_ENTRIES {
            return Err(VisitError::InvalidService(service));
        }
        let inputs = StrategicInputs {
            difficulty: self.links.difficulty,
            player: ((self.screen.ship.x >> 8) as u8, (self.screen.ship.y >> 8) as u8),
            batch_bonus: self.links.batch_bonus,
            satellite_timing: self.links.satellite_timing,
            satellite_busy: self.map.globals.satellite_hold,
            terrain: &self.terrain,
        };
        let mut tick = TickOutput::default();
        self.map
            .tick(&mut self.links.scene, inputs, &mut tick)
            .map_err(|error| VisitError::Screen(ScreenError::Simulation(error)))?;
        output.cues.extend(tick.cues);
        Ok(())
    }
}

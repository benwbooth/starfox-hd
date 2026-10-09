//! The map's radio and message box (`$0B:9F87`, from the main loop each
//! program frame), and the message box's side of the GSU picture
//! (`$0B:8234`).
//!
//! The CPU side turns the map's alerts (F55C) into radio messages, shows
//! the place and unit info boxes, and runs the message scripts (F582, a
//! bytecode at `$0B:A609..A81E` with a few native predicates). The GSU side
//! advances an open message once per picture and reports when it has been
//! shown for its duration; it also measures the message's height with the
//! text engine's word wrap (`$01:ED91`). The box's drawing is presentation.

use sf2_data::messages::{FONT, FONT_BASE, MESSAGES, MESSAGE_BASE, MESSAGE_POINTERS};

use super::stage_announcer::{self, StageMessage};
use super::strategic_director::MapDirector;
use super::strategic_screen::{MapScreen, ScreenError, ScreenLinks, ScreenOutput};
use super::strategic_sim::StrategicMap;
use super::strategic_sprites::MapSprites;

/// The message box's words in GSU RAM.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MessageBox {
    /// 37C: nonzero while the radio is held off.
    pub busy: u16,
    /// 36C: the opening size; 378 the style; 37A the portrait state.
    pub size: u16,
    pub style: u16,
    pub open: u16,
    /// 382/386: the pilots shown.
    pub left_pilot: u16,
    pub right_pilot: u16,
    /// 388: the message (0 none, FFFF shown in full, FF the info box).
    pub message: u16,
    /// 38A: pictures shown; 390 the duration; 380 the GSU's remaining mark.
    pub progress: u16,
    pub duration: u16,
    pub remaining: u16,
    /// 38E: the box's row; 392 the portrait frame; 356 the text height.
    pub row: u16,
    pub portrait: u16,
    pub height: u16,
}

/// The CPU side's own words.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapRadio {
    /// F594: the script a message opened from (the running script, F582,
    /// is `ScreenLinks::message`); F584 the page's frames.
    pub opened_from: u16,
    pub page_frames: u16,
    /// F56A: the buttons a wait accepts (8000 A, 4000 B).
    pub accept: u16,
    /// F59A: the place whose info box shows.
    pub shown_place: u16,
    /// F572: the box's base tile word; F574 the choice's result.
    pub box_base: u16,
    pub choice: u16,
    /// F556: the encounter panel shows.
    pub panel: u16,
    /// 1E84: the radio message; F55E/F560/F566 its words (progress, timer,
    /// style).
    pub radio_event: u16,
    pub radio: StageMessage,
    /// 1E60: cleared each frame.
    pub tick: u16,
    /// DB39: cleared with the radio.
    pub cleared_word: u16,
    /// 1C51: the main screen's layers, set when the planet falls.
    pub layers: u8,
}

/// Words the service reads that other owners keep.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RadioInputs {
    /// 1E70: the second portrait.
    pub portrait: u16,
    /// 1292: the pad held.
    pub held: u16,
}

/// `$0B:A609..A81E`.
const SCRIPT_BASE: u16 = 0xA609;
const SCRIPTS: [u8; 0x215] = [
    0x01, 0x1B, 0x10, 0x6C, 0x03, 0x7F, 0x00, 0x16, 0x06, 0x04, 0x06, 0x01, 0x15, 0x1F, 0xA6, 0x06, 0x01, 0x18,
    0x05, 0x06, 0x03, 0x14, 0xAF, 0x88, 0x03, 0x70, 0xC9, 0xFF, 0x00, 0xD0, 0x06, 0xA9, 0xFF, 0xFF, 0x8D, 0x56,
    0xF5, 0x38, 0x60, 0x01, 0x1B, 0x16, 0x06, 0x06, 0x01, 0x15, 0x1F, 0xA6, 0x06, 0x01, 0x18, 0x0D, 0x56, 0xF5,
    0x00, 0x00, 0x14, 0x01, 0x15, 0x87, 0xA6, 0x10, 0x6C, 0x03, 0x7F, 0x00, 0x10, 0x92, 0x03, 0x00, 0x00, 0x04,
    0x06, 0x02, 0x19, 0x15, 0x6A, 0xA6, 0x15, 0x87, 0xA6, 0x19, 0x16, 0x47, 0x05, 0x06, 0x03, 0x15, 0x9E, 0xA6,
    0x17, 0x0D, 0x84, 0x1E, 0x00, 0x00, 0x14, 0xAD, 0x5E, 0xF5, 0xF0, 0x13, 0x8D, 0x84, 0x1E, 0x8F, 0x88, 0x03,
    0x70, 0xA9, 0x00, 0x00, 0x8F, 0x8A, 0x03, 0x70, 0x9C, 0x5E, 0xF5, 0x38, 0x60, 0xA2, 0x59, 0xA6, 0x38, 0x60,
    0xDA, 0xAF, 0x78, 0x03, 0x70, 0xAA, 0xBF, 0xCA, 0x85, 0x0B, 0x29, 0xFF, 0x00, 0x29, 0xFF, 0x00, 0x22, 0x09,
    0x6E, 0x7F, 0xFA, 0x38, 0x60, 0xAD, 0x84, 0x1E, 0xC9, 0x6E, 0x00, 0xD0, 0x13, 0xA9, 0x02, 0x00, 0x1C, 0x1D,
    0xDA, 0xE2, 0x20, 0xA9, 0x16, 0x8F, 0x51, 0x1C, 0x00, 0xC2, 0x20, 0x20, 0x29, 0xA0, 0x38, 0x60, 0x0C, 0x10,
    0x88, 0x03, 0x00, 0x00, 0x0D, 0x96, 0xF5, 0xFF, 0xFF, 0x06, 0x02, 0x17, 0x13, 0x02, 0x1C, 0x16, 0x06, 0x10,
    0x7A, 0x03, 0xFF, 0xFF, 0x1B, 0x08, 0x16, 0x08, 0x14, 0x08, 0xD4, 0x0D, 0x96, 0xF5, 0xFF, 0xFF, 0x08, 0x15,
    0x17, 0x13, 0x15, 0x06, 0xA7, 0x06, 0x02, 0x17, 0x14, 0x15, 0x06, 0xA7, 0x06, 0x02, 0x02, 0x1B, 0x10, 0x78,
    0x03, 0x00, 0x00, 0x10, 0x7A, 0x03, 0xFF, 0xFF, 0x0D, 0x96, 0xF5, 0xFF, 0xFF, 0x16, 0x06, 0x08, 0x6D, 0x17,
    0x13, 0xAD, 0x0D, 0xDA, 0xC9, 0x62, 0x00, 0xD0, 0x02, 0x38, 0x60, 0x18, 0x60, 0x01, 0x15, 0x2D, 0xA7, 0x1B,
    0x10, 0x78, 0x03, 0x00, 0x00, 0x10, 0x7A, 0x03, 0xFF, 0xFF, 0x10, 0x6C, 0x03, 0x7F, 0x00, 0x09, 0x0D, 0x74,
    0xF5, 0xFF, 0xFF, 0x14, 0x20, 0x29, 0xA0, 0x38, 0x60, 0x01, 0x06, 0x32, 0x1B, 0x16, 0x40, 0x04, 0x07, 0x01,
    0x07, 0x02, 0x0B, 0x30, 0x80, 0x0D, 0x07, 0x03, 0x0C, 0x0D, 0x5A, 0xF5, 0x02, 0x00, 0x07, 0x04, 0x07, 0x05,
    0x0D, 0x5A, 0xF5, 0x01, 0x00, 0x07, 0x06, 0x0D, 0x5A, 0xF5, 0x03, 0x00, 0x07, 0x07, 0x0D, 0x5A, 0xF5, 0x00,
    0x00, 0x0B, 0x30, 0x80, 0x0D, 0x07, 0x08, 0x07, 0x09, 0x0C, 0x0B, 0x20, 0xC0, 0x0D, 0x07, 0x0A, 0x07, 0x0B,
    0x0C, 0x0D, 0x96, 0xF5, 0xFF, 0xFF, 0x07, 0x0C, 0x06, 0x04, 0x16, 0x47, 0x05, 0x06, 0x03, 0x0D, 0xFD, 0xD9,
    0x16, 0x00, 0x12, 0x02, 0x04, 0x07, 0x01, 0x07, 0x02, 0x07, 0x03, 0x07, 0x04, 0x07, 0x05, 0x07, 0x06, 0x07,
    0x07, 0x07, 0x08, 0x07, 0x09, 0x07, 0x0A, 0x07, 0x0B, 0x07, 0x0C, 0x07, 0x0D, 0x07, 0x0E, 0x07, 0x0F, 0x07,
    0x10, 0x07, 0x11, 0x07, 0x12, 0x07, 0x13, 0x07, 0x14, 0x07, 0x15, 0x07, 0x16, 0x07, 0x17, 0x07, 0x18, 0x07,
    0x19, 0x07, 0x1A, 0x07, 0x1B, 0x07, 0x1C, 0x07, 0x1D, 0x07, 0x1E, 0x07, 0x1F, 0x07, 0x20, 0x07, 0x21, 0x07,
    0x22, 0x07, 0x23, 0x07, 0x24, 0x07, 0x25, 0x07, 0x26, 0x07, 0x27, 0x07, 0x28, 0x07, 0x29, 0x07, 0x2A, 0x07,
    0x2B, 0x07, 0x2C, 0x07, 0x2D, 0x07, 0x2E, 0x07, 0x2F, 0x07, 0x30, 0x07, 0x31, 0x07, 0x62, 0x07, 0x63, 0x07,
    0x64, 0x07, 0x65, 0x07, 0x66, 0x07, 0x67, 0x07, 0x68, 0x07, 0x69, 0x07, 0x6A, 0x07, 0x6B, 0x07, 0x6C, 0x07,
    0x6D, 0x07, 0x6E, 0x07, 0x6F, 0x07, 0x70, 0x07, 0x71, 0x07, 0x72, 0x07, 0x73, 0x07, 0x74, 0x07, 0x75, 0x07,
    0x7C, 0x07, 0x7D, 0x07, 0x7E, 0x05, 0x06, 0x05, 0x03, 0x86, 0xA7,
];
/// The scripts' entries the CPU starts.
const SCRIPT_PLACE_INFO: u16 = 0xA609;
const SCRIPT_UNIT_INFO: u16 = 0xA630;
const SCRIPT_RADIO: u16 = 0xA642;
const SCRIPT_SKIP: u16 = 0xA6BB;
const SCRIPT_TUTORIAL: u16 = 0xA732;
/// `$0B:A07B`: the message box's base tile word.
const BOX_BASE: u16 = 0x00D7;
/// `$0B:FB8D`: alert bits and their message, progress, timer and style.
const ALERTS: [(u16, u8, u8, u8, u8); 14] = [
    (0x0001, 0x6F, 0x00, 0x00, 0x04),
    (0x0002, 0x76, 0x00, 0x00, 0x04),
    (0x0004, 0x86, 0x00, 0x00, 0x06),
    (0x0008, 0x73, 0x00, 0x00, 0x05),
    (0x0010, 0x74, 0x00, 0x00, 0x05),
    (0x0020, 0x75, 0x00, 0x00, 0x04),
    (0x0040, 0x78, 0x00, 0x00, 0x05),
    (0x0080, 0x79, 0x00, 0x00, 0x04),
    (0x0100, 0x71, 0x72, 0x00, 0x05),
    (0x0200, 0x57, 0x00, 0x00, 0x06),
    (0x0400, 0xB6, 0x00, 0x00, 0x07),
    (0x0800, 0x58, 0x00, 0x00, 0x04),
    (0x1000, 0xC4, 0x00, 0x00, 0x02),
    (0x2000, 0xD2, 0xD3, 0x00, 0x00),
];
/// `$0B:FBE3`: the place info message by place number (DB2F).
const PLACE_MESSAGES: [u8; 12] = [0x0D, 0x0D, 0x0D, 0x0D, 0x0D, 0x0D, 0x0D, 0x0E, 0xC3, 0x5D, 0x5E, 0xD8];
/// `$0B:85D2`/`$0B:85CA`: by style, the box's placement and the opening cue.
const STYLE_TOP: [u8; 8] = [0, 0, 0, 0, 1, 1, 1, 1];
const STYLE_CUES: [u8; 8] = [0x4B, 0x4B, 0x4B, 0x4B, 0x5D, 0x5E, 0x5F, 0x4C];

const MESSAGE_INFO: u16 = 0x00FF;
const MESSAGE_SHOWN: u16 = 0xFFFF;
/// 1B88 bit 0100: the planet has fallen.
const PLANET_FALLEN: u16 = 0x0100;

// ---- the GSU side ($0B:8234) ----

/// `$0B:8234`: the message box's step in a GSU picture.
pub fn picture(message_box: &mut MessageBox) -> Result<(), ScreenError> {
    let message = message_box.message;
    if message == 0 || message & 0x8000 != 0 {
        message_box.message = 0;
        message_box.progress = 0;
        return Ok(());
    }
    if message == MESSAGE_INFO {
        // $0B:8347: the info box stays until the CPU closes it.
        message_box.remaining = 1;
        return Ok(());
    }
    // $0B:83CC: the message's first byte and height.
    let text = message_text(message)?;
    message_box.remaining = u16::from(*text.first().ok_or(ScreenError::TableOverrun(0x0083E7))?);
    message_box.height = text_height(text)?;
    // $0B:84B9.
    message_box.progress = message_box.progress.wrapping_add(1);
    message_box.remaining = if message_box.progress < message_box.duration { message_box.duration } else { 0 };
    if message_box.remaining == 0 {
        message_box.message = MESSAGE_SHOWN;
    }
    Ok(())
}

/// The message's text after its speaker and style bytes.
fn message_text(message: u16) -> Result<&'static [u8], ScreenError> {
    let pointer = *MESSAGE_POINTERS
        .get(usize::from(message).wrapping_sub(1))
        .ok_or(ScreenError::TableOverrun(0x00AEB3 + u32::from(message) * 2))?;
    let start = usize::from(pointer - MESSAGE_BASE) + 2;
    MESSAGES.get(start..).ok_or(ScreenError::TableOverrun(u32::from(pointer)))
}

fn font(address: u16) -> Result<u8, ScreenError> {
    FONT.get(usize::from(address.wrapping_sub(FONT_BASE)))
        .copied()
        .ok_or(ScreenError::TableOverrun(0x0D0000 | u32::from(address)))
}

/// `$01:EF10`: the wrapped text's height, two pixels between lines.
pub fn text_height(text: &[u8]) -> Result<u16, ScreenError> {
    let line_height = u16::from(font(FONT_BASE + 1)?);
    let mut height: u16 = 0;
    let mut start = 0usize;
    loop {
        let (length, next) = wrap_line(text, start)?;
        if length == 0 {
            return Ok(height);
        }
        start = next;
        height = height.wrapping_add(line_height + 2);
    }
}

/// `$01:ED91`: one display line from `start`: its length in bytes and the
/// next line's start, breaking at the last space once the width runs past
/// 0x7F.
pub fn wrap_line(text: &[u8], start: usize) -> Result<(usize, usize), ScreenError> {
    const MAX_WIDTH: u16 = 0x7F;
    let line_height = u16::from(font(FONT_BASE + 1)?);
    let first = font(FONT_BASE + 2)?;
    let last = font(FONT_BASE + 3)?.wrapping_add(1);
    let spacing = u16::from(font(FONT_BASE + 4)?);
    let glyphs = FONT_BASE + 7;
    let mut last_space: Option<usize> = None;
    let mut width: u16 = 0;
    let mut at = start;
    loop {
        let byte = *text.get(at).ok_or(ScreenError::TableOverrun(0x00EDC6))?;
        at += 1;
        if byte == 0 {
            return Ok((at - 1 - start, at - 1));
        }
        let glyph_width = if byte < first || byte >= last {
            None
        } else {
            let index = font(glyphs + u16::from(byte - first))?;
            if index == 0xFF {
                None
            } else {
                let at = glyphs + 2 * u16::from(index) + u16::from(last - first) + 1;
                Some(u16::from(font(at)? >> 3))
            }
        };
        let glyph_width = glyph_width.unwrap_or(if byte == 0x23 { 0 } else { (line_height >> 1).wrapping_sub(spacing) });
        width = width.wrapping_add(glyph_width).wrapping_add(spacing);
        if width > MAX_WIDTH {
            // The branch's delay slot restores the space's address; without
            // a space the overflowing glyph stays on the line.
            return Ok(match last_space {
                Some(space) => (space - start, space + 1),
                None => (at - start, at),
            });
        }
        if byte == 0x20 {
            last_space = Some(at - 1);
        }
    }
}

// ---- the CPU side ($0B:9F87) ----

/// The service's context.
pub struct Radio<'a> {
    pub radio: &'a mut MapRadio,
    pub message_box: &'a mut MessageBox,
    pub sprites: &'a mut MapSprites,
    pub screen: &'a mut MapScreen,
    pub director: &'a mut MapDirector,
    pub map: &'a mut StrategicMap,
    pub links: &'a mut ScreenLinks,
    pub inputs: RadioInputs,
    pub output: &'a mut ScreenOutput,
}

/// What a script step did.
enum Flow {
    Next(u16),
    /// Stop this frame, staying on the step.
    Wait,
    /// Stop this frame at `u16`.
    WaitAt(u16),
    /// The script restarts at `u16` within this frame.
    Jump(u16),
}

fn script_byte(address: u16) -> Result<u8, ScreenError> {
    SCRIPTS
        .get(usize::from(address.wrapping_sub(SCRIPT_BASE)))
        .copied()
        .ok_or(ScreenError::TableOverrun(0x0B0000 | u32::from(address)))
}

fn script_word(address: u16) -> Result<u16, ScreenError> {
    Ok(u16::from(script_byte(address)?) | (u16::from(script_byte(address.wrapping_add(1))?) << 8))
}

impl Radio<'_> {
    /// `$0B:9F87`.
    pub fn run(&mut self) -> Result<(), ScreenError> {
        // $0B:A0E5.
        self.message_box.left_pilot = u16::from(self.links.pilots[0]);
        self.message_box.right_pilot = self.inputs.portrait;
        let fallen = self.links.scene.scene_events & PLANET_FALLEN != 0;
        if fallen || (self.message_box.busy == 0 && self.radio.opened_from == 0) {
            self.radio_frame()?;
        }
        self.scripts()?;
        self.radio.tick = 0;
        Ok(())
    }

    /// `$0B:A13F`.
    fn radio_frame(&mut self) -> Result<(), ScreenError> {
        if self.links.scene.scene_events & PLANET_FALLEN != 0 {
            if self.radio.radio_event != 0x006E {
                self.radio.radio_event = 0;
                self.links.message = 0;
            }
            self.map.globals.map_events &= 0x0002;
            self.map.globals.alerts = 0;
        } else {
            self.claim_alert();
        }
        // $7F:7763 (the map's 1B68 is 7, so only the message gate applies).
        if self.radio.radio_event == 0 {
            if let Some((message, words)) =
                stage_announcer::announce(&mut self.map.globals.map_events, self.links.planet_health)
            {
                self.radio.radio_event = message;
                self.radio.radio = words;
            }
        }
        if self.radio.radio_event == 0 {
            if self.links.scene.scene_events & PLANET_FALLEN != 0 {
                self.links.message = 0;
                self.radio.radio_event = 0;
            }
            return self.info_box();
        }
        if self.links.message == 0 {
            // $0B:A182: open the radio.
            self.message_box.size = 0x0064;
            self.message_box.progress = 0;
            self.links.message = SCRIPT_RADIO;
            self.message_box.message = self.radio.radio_event;
            self.message_box.style = self.radio.radio.voice;
            self.place_box()?;
        }
        // $0B:A1B7.
        let message = self.message_box.message;
        self.radio.accept = if (message == 0x73 || message == 0xD1)
            && (self.map.globals.map_events & 0x03FF != 0 || self.map.globals.alerts & 0x3FFF != 0)
        {
            0xC000
        } else {
            0x4000
        };
        Ok(())
    }

    /// `$0B:A0F8`: the first raised alert becomes the radio message.
    fn claim_alert(&mut self) {
        if self.radio.radio_event != 0 {
            return;
        }
        for (bit, message, progress, timer, style) in ALERTS {
            let raised = self.map.globals.alerts & bit != 0;
            self.map.globals.alerts &= !bit;
            if !raised {
                continue;
            }
            if message == 0 {
                return;
            }
            self.radio.radio_event = u16::from(message);
            self.radio.radio.progress = u16::from(progress);
            self.radio.radio.timer = u16::from(timer);
            self.radio.radio.voice = u16::from(style);
            return;
        }
    }

    /// `$0B:A2A6`: the box's row and base tile by style (the callers'
    /// accumulator is replaced before it is used).
    fn place_box(&mut self) -> Result<(), ScreenError> {
        let top = *STYLE_TOP.get(usize::from(self.message_box.style)).ok_or(ScreenError::TableOverrun(0x0BA2B2))?;
        let (row, base) = if top != 0 { (0x0094, 0x2700) } else { (0x00F4, 0xC700) };
        self.message_box.row = row;
        self.sprites.effect_origin = base | self.radio.box_base;
        Ok(())
    }

    /// `$0B:A1E5`: the place or unit info boxes.
    fn info_box(&mut self) -> Result<(), ScreenError> {
        if self.screen.info.shown != 0 {
            // $0B:A23C.
            if self.screen_pad_held() || self.screen.info.number == self.radio.shown_place {
                return Ok(());
            }
            if self.links.message != 0 {
                self.radio.accept = 0x4000;
                return Ok(());
            }
            self.place_box()?;
            self.radio.shown_place = self.screen.info.number;
            let message = *PLACE_MESSAGES
                .get(usize::from(self.screen.info.number))
                .ok_or(ScreenError::TableOverrun(0x0BFBE3))?;
            self.message_box.message = u16::from(message);
            self.message_box.style = u16::from(self.screen.info.detail == 0x000F);
            self.radio.accept = 0;
            self.message_box.progress = 0;
            self.links.message = SCRIPT_PLACE_INFO;
            return Ok(());
        }
        if self.screen.hover & 0x0040 == 0 {
            self.radio.accept = 0x4000;
            self.radio.shown_place = 0xFFFF;
            return Ok(());
        }
        if self.links.message != 0 {
            if self.message_box.message != MESSAGE_INFO {
                self.radio.accept = 0x4000;
            }
            return Ok(());
        }
        self.place_box()?;
        self.message_box.message = MESSAGE_INFO;
        self.radio.shown_place = 0xFFFF;
        self.message_box.style = 0;
        self.radio.accept = 0;
        self.message_box.progress = 0;
        self.links.message = SCRIPT_UNIT_INFO;
        Ok(())
    }

    fn screen_pad_held(&self) -> bool {
        self.inputs.held & 0x0F00 != 0
    }

    /// `$0B:A2DE`: run the script until a step waits.
    fn scripts(&mut self) -> Result<(), ScreenError> {
        let mut at = self.links.message;
        loop {
            if at == 0 {
                return Ok(());
            }
            match self.step(at)? {
                Flow::Next(next) => {
                    at = next;
                    self.links.message = next;
                }
                Flow::Wait => return Ok(()),
                Flow::WaitAt(next) => {
                    self.links.message = next;
                    return Ok(());
                }
                Flow::Jump(next) => {
                    at = next;
                    self.links.message = next;
                }
            }
        }
    }

    fn step(&mut self, at: u16) -> Result<Flow, ScreenError> {
        let op = script_byte(at)?;
        let arg = at.wrapping_add(1);
        Ok(match op {
            0x00 | 0x12 => Flow::Wait,
            0x01 => {
                self.reset_effects();
                Flow::Next(arg)
            }
            0x02 => {
                self.message_box.message = 0;
                self.message_box.duration = 0x007F;
                self.radio.opened_from = self.links.message;
                self.reset_effects();
                Flow::Next(arg)
            }
            0x03 => Flow::Jump(script_word(arg)?),
            0x04 => {
                self.sprites.burst_frames.0 = 1;
                self.message_box.open = 1;
                Flow::Next(arg)
            }
            0x05 => {
                self.sprites.burst_frames.1 = 1;
                self.message_box.message = 0;
                self.message_box.open = 0;
                self.sprites.ring_animation = 0;
                self.radio.panel = 0;
                Flow::Next(arg)
            }
            0x06 => {
                let frames = u16::from(script_byte(arg)?);
                if frames >= self.radio.page_frames {
                    self.radio.page_frames += 1;
                    Flow::Wait
                } else {
                    self.radio.tick = 0;
                    self.radio.page_frames = 0;
                    Flow::WaitAt(arg + 1)
                }
            }
            0x07 => self.page(arg, 0)?,
            0x08 => {
                if self.links.menu_pad[1] & 0x1000 != 0 {
                    self.output.cues.push(0x0005);
                    Flow::Jump(SCRIPT_SKIP)
                } else {
                    self.page(arg, 0)?
                }
            }
            0x09 => {
                self.message_box.message = self.links.text_state[1];
                if self.links.text_state[0] != 0 {
                    self.links.text_state[0] = 0;
                    self.message_box.message = 0;
                    Flow::Next(arg)
                } else {
                    Flow::Wait
                }
            }
            0x0A => self.page(arg, 0x0010)?,
            0x0B => {
                self.sprites.ring_position = script_word(arg)?;
                self.sprites.ring_animation = u16::from(script_byte(arg + 2)?);
                self.ring_by_row();
                Flow::Next(arg + 3)
            }
            0x0C => {
                self.sprites.ring_position = 0;
                self.sprites.ring_animation = 0;
                Flow::Next(arg)
            }
            0x0D..=0x10 => {
                let target = script_word(arg)?;
                let value = script_word(arg + 2)?;
                self.store(op, target, value)?;
                Flow::Next(arg + 4)
            }
            0x11 => {
                let low = (self.sprites.ring_position >> 8) as u8;
                let position = low.wrapping_add(script_byte(arg)?);
                self.sprites.ring_position = (self.sprites.ring_position & 0x00FF) | (u16::from(position) << 8);
                Flow::Next(arg + 1)
            }
            0x13 => {
                self.links.message = 0;
                self.radio.opened_from = 0;
                self.message_box.open = 0;
                self.message_box.size = 0x007F;
                self.message_box.message = 0;
                self.message_box.duration = 0x0021;
                // $0B:A046.
                self.radio.accept = 0;
                self.sprites.burst_frames = (0, 0);
                self.radio.radio_event = 0;
                self.message_box.message = 0;
                return Ok(Flow::Wait);
            }
            0x14 => {
                self.links.message = 0;
                return Ok(Flow::Wait);
            }
            0x15 => match self.snippet(script_word(arg)?)? {
                Some(resume) => Flow::Next(resume.unwrap_or(arg) + 2),
                None => Flow::Wait,
            },
            0x16 => {
                self.output.cues.push(u16::from(script_byte(arg)?));
                Flow::Next(arg + 1)
            }
            0x17 => {
                self.director.message_wait = 0;
                Flow::Next(arg)
            }
            0x18 => self.await_button(0x4000, arg),
            0x19 => {
                let portrait = self.message_box.portrait.wrapping_add(1);
                self.message_box.portrait = if portrait < 6 { portrait } else { 0 };
                self.await_button(0x8000, arg)
            }
            0x1A => {
                self.message_box.row = 0x0094;
                self.sprites.effect_origin = 0x2700 | self.radio.box_base;
                Flow::Next(arg)
            }
            0x1B => {
                self.message_box.row = 0x00F4;
                self.sprites.effect_origin = 0xC700 | self.radio.box_base;
                Flow::Next(arg)
            }
            0x1C => {
                let aftermath = self.director.aftermath;
                if aftermath == 0x60 || aftermath == 0x62 {
                    Flow::WaitAt(arg)
                } else {
                    Flow::Wait
                }
            }
            other => return Err(ScreenError::InvalidScriptStep(0x0B00 | u16::from(other))),
        })
    }
}

impl Radio<'_> {
    /// `$0B:A3E1`.
    fn reset_effects(&mut self) {
        self.sprites.ring_animation = 0;
        self.radio.page_frames = 0;
        self.sprites.flare_frame = 0;
    }

    /// `$0B:A5E5`: the ring's animation by its row.
    fn ring_by_row(&mut self) {
        let row = (self.sprites.ring_position >> 8) as u8;
        if row != 0 {
            let animation = if row < 0x60 { 0x0C } else { 0x0D };
            self.sprites.ring_animation = (self.sprites.ring_animation & 0xFF00) | animation;
        }
    }

    /// `$0B:A377`/`A389`/`A39E`/`A3B3`: the scripts' stores, by target.
    fn store(&mut self, op: u8, target: u16, value: u16) -> Result<(), ScreenError> {
        let apply = |old: u16| match op {
            0x0E => old | value,
            0x0F => old & value,
            _ => value,
        };
        let site = 0x0B0000 | u32::from(target);
        if op == 0x10 {
            let word = match target {
                0x036C => &mut self.message_box.size,
                0x0378 => &mut self.message_box.style,
                0x037A => &mut self.message_box.open,
                0x0388 => &mut self.message_box.message,
                0x0392 => &mut self.message_box.portrait,
                _ => return Err(ScreenError::Unported(0x700000 | u32::from(target))),
            };
            *word = value;
            let _ = site;
            return Ok(());
        }
        let word = match target {
            0x1E84 => &mut self.radio.radio_event,
            0xF556 => &mut self.radio.panel,
            0xF574 => &mut self.radio.choice,
            0xF596 => &mut self.sprites.flare_frame,
            0xF55A => &mut self.sprites.highlight_kind,
            0xD9FD => &mut self.links.timeline,
            _ => return Err(ScreenError::Unported(site)),
        };
        *word = apply(*word);
        Ok(())
    }

    /// `$0B:A49B`: wait for the accepted button or the message's end.
    fn await_button(&mut self, button: u16, arg: u16) -> Flow {
        if button & self.radio.accept == 0 && self.message_box.message & 0x8000 == 0 {
            return Flow::Wait;
        }
        self.radio.accept = 0;
        self.message_box.message = 0;
        Flow::Next(arg)
    }

    /// `$0B:A50D`: one page of a message; Up to the page's end the box
    /// opens, and a press (B, Y, Select, Start or the pad) after the
    /// minimum frames turns it.
    fn page(&mut self, arg: u16, progress: u16) -> Result<Flow, ScreenError> {
        const PRESSES: u16 = 0xCF00;
        if self.radio.page_frames == 0 {
            self.message_box.progress = progress & 0x00FF;
            self.message_box.message = u16::from(script_byte(arg)?);
            return Ok(self.page_frame());
        }
        if self.radio.page_frames >= 0x0064 {
            return Ok(self.finish_page(arg));
        }
        let short = self.message_box.height < 0x0024;
        let (style, hold, finish, minimum) = if short { (1, 6, 7, 4) } else { (0, 10, 11, 8) };
        self.sprites.flare_style = style;
        let frames = self.radio.page_frames;
        if frames >= hold {
            self.sprites.flare_frame |= 0x4000;
        }
        if self.links.menu_pad[1] & PRESSES == 0 {
            return Ok(self.page_frame());
        }
        if frames >= finish {
            self.output.cues.push(0x0005);
            return Ok(self.finish_page(arg));
        }
        if frames < minimum {
            self.radio.page_frames = minimum;
            self.message_box.progress = minimum;
        }
        Ok(self.page_frame())
    }

    /// `$0B:A5A2`.
    fn page_frame(&mut self) -> Flow {
        self.radio.page_frames += 1;
        self.message_box.size = self.message_box.progress >> 2;
        Flow::Wait
    }

    /// `$0B:A5B8`.
    fn finish_page(&mut self, arg: u16) -> Flow {
        self.radio.page_frames = 0;
        self.sprites.flare_frame = 0;
        Flow::WaitAt(arg + 1)
    }

    /// `$0B:9F4C`: the map entry's reset of the radio and the message box
    /// (the box's tiles are presentation).
    pub fn reset(&mut self) {
        self.radio.panel = 0;
        self.links.text_state[0] = 0;
        self.radio.choice = 0;
        self.message_box.row = 0;
        self.sprites.highlight_kind = 0;
        // $0B:9FC1: the tutorial map and a held box keep the box busy.
        let held = self.links.scene.stage_results & 0x0080 != 0 || self.map.globals.speed_flags & 0x0100 != 0;
        self.message_box.busy = if held { 0xFFFF } else { 0 };
        // $0B:A05A.
        self.radio.box_base = BOX_BASE;
        // $0B:9FDD.
        self.sprites.flare_style = 0;
        self.radio.opened_from = 0;
        self.radio.tick = 0;
        self.sprites.flare_frame = 0;
        self.message_box.open = 0;
        self.message_box.row = 0;
        self.message_box.progress = 0;
        self.message_box.style = 0;
        self.sprites.ring_animation = 0;
        self.sprites.effect_origin = self.radio.box_base | 0xC700;
        self.message_box.duration = 0x0021;
        self.links.message = 0;
        if self.message_box.busy != 0 {
            self.message_box.duration = 0x007F;
        }
        // $0B:A046.
        self.radio.accept = 0;
        self.sprites.burst_frames = (0, 0);
        self.radio.radio_event = 0;
        self.message_box.message = 0;
        self.clear_radio();
        // $0B:9FB2: the tutorial map's script.
        if self.links.scene.stage_results & 0x0080 != 0 {
            self.links.message = SCRIPT_TUTORIAL;
        }
    }

    /// `$0B:A029`: the radio and the map's pending events are cleared.
    fn clear_radio(&mut self) {
        self.radio.radio.progress = 0;
        self.radio.radio.timer = 0;
        self.map.globals.alerts = 0;
        self.map.globals.map_events = 0;
        self.radio.cleared_word = 0;
        self.map.globals.spawn_variant_bit = 0;
        self.radio.radio_event = 0;
        self.message_box.message = 0;
    }

    /// The scripts' native predicates: `Some(resume)` continues (from the
    /// step after `resume` when given), `None` waits.
    fn snippet(&mut self, address: u16) -> Result<Option<Option<u16>>, ScreenError> {
        Ok(match address {
            0xA61F => {
                if self.message_box.message == MESSAGE_INFO {
                    self.radio.panel = 0xFFFF;
                }
                Some(None)
            }
            0xA66A => {
                let next = self.radio.radio.progress;
                if next == 0 {
                    // Skip the second message's step.
                    Some(Some(0xA659))
                } else {
                    self.radio.radio_event = next;
                    self.message_box.message = next;
                    self.message_box.progress = 0;
                    self.radio.radio.progress = 0;
                    Some(None)
                }
            }
            0xA687 => {
                let cue = *STYLE_CUES.get(usize::from(self.message_box.style)).ok_or(ScreenError::TableOverrun(0x0BA68D))?;
                self.output.cues.push(u16::from(cue));
                Some(None)
            }
            0xA69E => {
                if self.radio.radio_event == 0x006E {
                    self.director.dialog_hold &= !0x0002;
                    self.radio.layers = 0x16;
                    self.clear_radio();
                }
                Some(None)
            }
            0xA706 => (self.director.aftermath == 0x62).then_some(None),
            0xA72D => {
                self.clear_radio();
                Some(None)
            }
            other => return Err(ScreenError::Unported(0x0B0000 | u32::from(other))),
        })
    }
}

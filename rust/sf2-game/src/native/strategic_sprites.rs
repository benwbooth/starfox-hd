//! The strategic map's sprite pass: the HUD service (`$04:8301`) in the
//! map's mode (`$04:87A9`), run in each service frame. It composes the
//! frame's sprites (the cursor, the ship and wingmate, the units and places,
//! the encounter panel, the shield gauge and the map's effects) into the
//! OAM buffer, and it advances the animations the campaign waits on: the
//! units' frames, the places' warning and marker animations, the planet's
//! explosion and the satellite's.
//!
//! Sprite pieces come from the bank-18 catalog (`sf2_data::map_sprites`).
//! The double-buffered path (1B9C bit 0002) is not used on the map and
//! faults.

use sf2_data::map_sprites::{MAP_SPRITES, MAP_SPRITE_BASE};

use super::strategic_director::MapDirector;
use super::strategic_screen::{MapScreen, ScreenError, ScreenLinks, ScreenOutput};
use super::strategic_sim::{PlaceId, SceneLinks, StrategicMap, UnitId};

/// E83F..EA3F: 128 four-byte entries, then EA3F..EA5F the size bits.
pub const OAM_BYTES: usize = 0x220;
const OAM_HIGH: usize = 0x200;
const OFFSCREEN: u16 = 0xE0E0;

/// `$04:8B14`/`$04:8B1C`/`$04:8B24`: the second pilot's portrait tiles and
/// the radar corner tiles by D991.
const PORTRAIT_ROTATION: [u16; 4] = [0x0000, 0xC000, 0x8000, 0x4000];
const CORNER_TOP: [u16; 4] = [0x316F, 0x317F, 0x319F, 0xB18F];
const CORNER_BOTTOM: [u16; 4] = [0xB16F, 0x318F, 0xB19F, 0xB17F];
/// `$04:8BA5`: the marked places' metasprites by kind (six kinds).
const PLACE_ICONS: [u16; 6] = [0x000C, 0x0008, 0x000C, 0x000A, 0x000C, 0x0008];
/// `$04:8E0A`: the launch icons' positions (x, y).
const LAUNCH_ICONS: [u16; 4] = [0x1FDA, 0x19D4, 0x21E4, 0x0FD2];
/// `$04:8FB7`: the special unit's tiles by D996.
const SPECIAL_TILES: [u16; 4] = [0x28D1, 0x28D2, 0x28D3, 0x28D2];
/// `$04:9630..9647`: the gauge segments: per pilot pair, the count and the
/// first offset, then (x, y) offsets.
const GAUGE_COUNTS: [u16; 3] = [8, 10, 6];
const GAUGE_STARTS: [u16; 3] = [0x00, 0x10, 0x24];
const GAUGE_OFFSETS: [u16; 0x18] = [
    0x0000, 0x0008, 0x0010, 0x0018, 0x0800, 0x0808, 0x0810, 0x0818, 0x0000, 0x0008, 0x0010, 0x0018,
    0x0020, 0x0800, 0x0808, 0x0810, 0x0818, 0x0820, 0x0000, 0x0008, 0x0010, 0x0800, 0x0808, 0x0810,
];
/// `$04:9FCD`: the four-piece metasprites' piece offsets.
const QUAD_OFFSETS: [(u8, u8); 4] = [(0xF0, 0xEF), (0x00, 0xEF), (0xF0, 0xFF), (0x00, 0xFF)];
/// `$04:A3BB`/`$04:A3D3`/`$04:A3DB`: size bits.
const LARGE_BIT: [u16; 4] = [0x0002, 0x0008, 0x0020, 0x0080];
const QUAD_BITS: [u16; 4] = [0x00AA, 0x02A8, 0x0AA0, 0x2A80];
const RUN_BITS: [u16; 9] = [0x0000, 0x0002, 0x000A, 0x002A, 0x00AA, 0x02AA, 0x0AAA, 0x2AAA, 0xAAAA];
/// `$04:F4A0`/`$04:F4B0`: the fighter target marker's wobble.
const MARKER_DX: [u16; 8] = [0x0000, 0x0004, 0x0001, 0xFFFD, 0xFFFC, 0x0002, 0xFFFE, 0x0001];
const MARKER_DY: [u16; 8] = [0x0000, 0x0003, 0xFFFD, 0x0002, 0xFFFE, 0x0003, 0x0001, 0xFFFC];
/// `$04:F91E`/`$04:F928`: the units' badge tiles by `$A7`.
const BADGE_A: [u16; 5] = [0x2580, 0x2560, 0x25C0, 0x25A0, 0x0000];
const BADGE_B: [u16; 5] = [0x2180, 0x2160, 0x21C0, 0x21A0, 0x0000];

/// The sprite pass's own words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapSprites {
    /// EFD3: the frame counter; EFD5/EFD7/EFD9/EFDB its low bits (1, 3, 7,
    /// F) and EFDD/EFDF/EFE1/EFE3 the times each wrapped.
    pub counter: u16,
    pub phases: [u16; 4],
    pub wraps: [u16; 4],
    /// D994, D996, D991, 1BEE, D97F: blink phases; D981/D982 the target
    /// marker's blink; 1BF0 the low-shield blink.
    pub blink: u16,
    pub flash: u16,
    pub corner: u16,
    pub choice_blink: u16,
    pub cursor_frame: u16,
    pub target_countdown: u8,
    pub target_phase: u16,
    pub shield_blink: u16,
    /// DA7B: 8 with fast-forward, else 1.
    pub speed_mark: u16,
    /// EC7F/EC81: the scripted view's icon animation.
    pub view_icon: (u16, u16),
    /// E07D/E07F and E081/E083: the satellite's animations.
    pub satellite_icon: (u16, u16),
    pub satellite_beam: (u16, u16),
    /// D9D0/D9D2 and D9D4..D9E7: the planet explosion's count, place and
    /// four (x, y, animation, frame, delay) records.
    pub explosion_count: u16,
    pub explosion_place: Option<PlaceId>,
    pub explosions: [[u8; 5]; 4],
    /// E825..E82C: the encounter panel's kinds and counts.
    pub panel: [u8; 8],
    /// F558, F56C, F56E, F58A, F596, F5A0, F5A2, F5A4, F5A6: the effects'
    /// frames and the fighter marker's target.
    pub highlight_frame: u16,
    pub burst_frames: (u16, u16),
    pub ring_frame: u16,
    pub flare_frame: u16,
    pub marker_tick: u16,
    pub marker_phase: u16,
    pub marker_target: (u16, u16),
    /// F55A, F570, F588, F58C, F598: the effects' requests.
    pub highlight_kind: u16,
    pub effect_origin: u16,
    pub ring_position: u16,
    pub ring_animation: u16,
    pub flare_style: u16,
    /// 1C49 and E834: the OAM cursor and the frame's entry count.
    pub cursor: u16,
    pub count: u16,
    /// E83F..EA5F.
    pub oam: Vec<u8>,
}

impl Default for MapSprites {
    fn default() -> Self {
        Self {
            counter: 0,
            phases: [0; 4],
            wraps: [0; 4],
            blink: 0,
            flash: 0,
            corner: 0,
            choice_blink: 0,
            cursor_frame: 0,
            target_countdown: 0,
            target_phase: 0,
            shield_blink: 0,
            speed_mark: 0,
            view_icon: (0, 0),
            satellite_icon: (0, 0),
            satellite_beam: (0, 0),
            explosion_count: 0,
            explosion_place: None,
            explosions: [[0; 5]; 4],
            panel: [0; 8],
            highlight_frame: 0,
            burst_frames: (0, 0),
            ring_frame: 0,
            flare_frame: 0,
            marker_tick: 0,
            marker_phase: 0,
            marker_target: (0, 0),
            highlight_kind: 0,
            effect_origin: 0,
            ring_position: 0,
            ring_animation: 0,
            flare_style: 0,
            cursor: 0,
            count: 0,
            oam: vec![0; OAM_BYTES],
        }
    }
}

/// Words the pass reads that other owners keep.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SpriteInputs {
    /// 1AA6 bit 80 turns the HUD off.
    pub hud_flags: u8,
    /// 1DD3: the shield gauge's style; D812 its tile row.
    pub gauge_style: u16,
    pub gauge_row: u16,
    /// F556: the encounter panel is enabled.
    pub panel_enabled: u16,
}

/// `$04:8301` in the map's mode.
pub fn pass(
    sprites: &mut MapSprites,
    screen: &mut MapScreen,
    director: &MapDirector,
    map: &mut StrategicMap,
    links: &mut ScreenLinks,
    inputs: SpriteInputs,
    output: &mut ScreenOutput,
) -> Result<(), ScreenError> {
    let mut pass = Pass {
        s: sprites,
        screen,
        director,
        map,
        links,
        inputs,
        output,
        y: 0,
        a: Regs::default(),
        panel_label: 0,
        panel_badge: 0,
    };
    pass.run()
}

/// The composer's direct-page arguments.
#[derive(Debug, Default, Clone, Copy)]
struct Regs {
    x: u16,
    y: u16,
    tile: u16,
    /// 1E78/1E7A/1E7C/1E7E: the animated sprite's position, animation and
    /// frame; `badge` is `$A7`.
    ax: u16,
    ay: u16,
    anim: u16,
    frame: u16,
    badge: u16,
    /// `$97`: the selected frame record.
    record: usize,
}

struct Pass<'a> {
    s: &'a mut MapSprites,
    screen: &'a mut MapScreen,
    director: &'a MapDirector,
    map: &'a mut StrategicMap,
    links: &'a mut ScreenLinks,
    inputs: SpriteInputs,
    output: &'a mut ScreenOutput,
    /// The OAM byte cursor (the source's Y).
    y: usize,
    a: Regs,
    /// 1BCE/1BD0: the panel's label and badge, kept for its second column.
    panel_label: u16,
    panel_badge: u16,
}

fn catalog_byte(address: u32) -> Result<u8, ScreenError> {
    let offset = address.checked_sub(u32::from(MAP_SPRITE_BASE)).ok_or(ScreenError::TableOverrun(0x180000 | address))?;
    MAP_SPRITES.get(offset as usize).copied().ok_or(ScreenError::TableOverrun(0x180000 | address))
}

fn catalog_word(address: u32) -> Result<u16, ScreenError> {
    Ok(u16::from(catalog_byte(address)?) | (u16::from(catalog_byte(address + 1)?) << 8))
}

fn table<T: Copy>(values: &[T], index: usize, site: u32) -> Result<T, ScreenError> {
    values.get(index).copied().ok_or(ScreenError::TableOverrun(site))
}

impl Pass<'_> {
    fn run(&mut self) -> Result<(), ScreenError> {
        // $04:830C: the counters.
        let s = &mut *self.s;
        let counter = s.counter;
        s.phases = [counter & 0x0001, counter & 0x0003, counter & 0x0007, counter & 0x000F];
        for k in (0..4).rev() {
            if s.phases[k] == 0 {
                s.wraps[k] = s.wraps[k].wrapping_add(1);
            }
        }
        if self.inputs.hud_flags & 0x80 != 0 {
            return Ok(());
        }
        if self.links.display_flags & 0x0002 != 0 {
            return Err(ScreenError::Unported(0x048374));
        }
        // $04:9C66: on the map only the size bits are cleared.
        for byte in &mut self.s.oam[OAM_HIGH..OAM_BYTES] {
            *byte = 0;
        }
        self.y = 0;
        self.s.cursor = 0;
        self.links.display_flags &= !0x0001;
        if self.links.hud_mode != 2 {
            return Err(ScreenError::Unported(0x0483B3 + u32::from(self.links.hud_mode) * 2));
        }
        self.map_mode()
    }

    fn write8(&mut self, offset: usize, value: u8) -> Result<(), ScreenError> {
        *self.s.oam.get_mut(offset).ok_or(ScreenError::TableOverrun(0x7EE83F))? = value;
        Ok(())
    }

    fn write16(&mut self, offset: usize, value: u16) -> Result<(), ScreenError> {
        self.write8(offset, value as u8)?;
        self.write8(offset + 1, (value >> 8) as u8)
    }

    fn or16(&mut self, offset: usize, value: u16) -> Result<(), ScreenError> {
        let low = *self.s.oam.get(offset).ok_or(ScreenError::TableOverrun(0x7EEA3F))?;
        let high = *self.s.oam.get(offset + 1).ok_or(ScreenError::TableOverrun(0x7EEA3F))?;
        self.write16(offset, (u16::from(low) | (u16::from(high) << 8)) | value)
    }

    /// One entry: x and y bytes, then the tile word.
    fn entry(&mut self, x: u8, y: u8, tile: u16) -> Result<(), ScreenError> {
        self.write8(self.y, x)?;
        self.write8(self.y + 1, y)?;
        self.write16(self.y + 2, tile)?;
        self.y += 4;
        Ok(())
    }

    /// `$04:A12E`: the current entry is large.
    fn large(&mut self) -> Result<(), ScreenError> {
        let bit = LARGE_BIT[(self.y & 0x000C) >> 2];
        self.or16(OAM_HIGH + (self.y >> 4), bit)
    }

    /// `$04:A246` with a bit table.
    fn bits(&mut self, table: &[u16; 4]) -> Result<(), ScreenError> {
        let half = self.y >> 1;
        let bit = table[(half & 0x0006) >> 1];
        self.or16(OAM_HIGH + (half >> 3), bit)
    }

    /// `$04:9EFE`: one large sprite centred on (x, y).
    fn sprite(&mut self) -> Result<(), ScreenError> {
        self.large()?;
        let Regs { x, y, tile, .. } = self.a;
        self.entry((x as u8).wrapping_sub(8), (y as u8).wrapping_sub(9), tile)
    }

    /// `$04:9EDE`: one small sprite.
    fn small(&mut self) -> Result<(), ScreenError> {
        let Regs { x, y, tile, .. } = self.a;
        self.entry((x as u8).wrapping_sub(4), (y as u8).wrapping_sub(5), tile)
    }

    /// `$04:9F6F`: a four-piece metasprite from `$18:BF15`.
    fn quad(&mut self) -> Result<(), ScreenError> {
        self.bits(&QUAD_BITS)?;
        let mut pieces = u32::from(catalog_word(0xBF15 + u32::from(self.a.tile))?);
        for (dx, dy) in QUAD_OFFSETS {
            let x = (self.a.x as u8).wrapping_add(dx);
            let y = (self.a.y as u8).wrapping_add(dy);
            let tile = catalog_word(0xBF15 + pieces)?;
            self.entry(x, y, tile)?;
            pieces += 2;
        }
        Ok(())
    }

    /// `$04:A085`: a metasprite from `$18:C0FD`.
    fn metasprite(&mut self, index: u16) -> Result<(), ScreenError> {
        let mut at = u32::from(catalog_word(0xC0FD + u32::from(index))?);
        let header = catalog_word(0xC0FD + at)?;
        if header & 0x8000 != 0 {
            self.run_bits(header & 0x7FFF)?;
        }
        let count = (header & 0x7FFF) >> 2;
        for _ in 0..count {
            let x = catalog_byte(0xC0FF + at)?.wrapping_add(self.a.x as u8);
            let y = catalog_byte(0xC0FF + at + 1)?.wrapping_add(self.a.y as u8);
            let tile = catalog_word(0xC0FF + at + 2)?;
            self.entry(x, y, tile)?;
            at += 4;
        }
        Ok(())
    }

    /// `$04:A28D`: size bits for a run of `length` bytes of entries.
    fn run_bits(&mut self, length: u16) -> Result<(), ScreenError> {
        let mut remaining = length >> 2;
        let mut row = self.y >> 4;
        let mut slot = (self.y & 0x000C) >> 1;
        loop {
            if slot == 0 {
                if remaining >= 8 && remaining != 8 {
                    remaining -= 8;
                    self.or16(OAM_HIGH + row, RUN_BITS[8])?;
                    row += 2;
                    continue;
                }
                let bits = table(&RUN_BITS, usize::from(remaining), 0x04A2C7)?;
                return self.or16(OAM_HIGH + row, bits);
            }
            loop {
                self.or16(OAM_HIGH + row, LARGE_BIT[slot >> 1])?;
                remaining = remaining.wrapping_sub(1);
                if remaining == 0 {
                    return Ok(());
                }
                slot += 2;
                if slot >= 8 {
                    break;
                }
            }
            row += 1;
            slot = 0;
        }
    }

    /// `$04:9FD5`: one step of an icon animation (`$18:C037` scripts);
    /// returns the updated (cursor, state).
    fn animate(&mut self, (cursor, state): (u16, u16)) -> Result<(u16, u16), ScreenError> {
        let (mut cursor, mut state) = (cursor, state);
        if state == 0xFFFF {
            return Ok((cursor, state));
        }
        let start = |cursor: u16| -> Result<(u16, u16), ScreenError> {
            Ok((cursor, catalog_word(0xC037 + u32::from(cursor))? | 0x8000))
        };
        if state != 0xFFFD {
            if state & 0x8000 == 0 {
                let record = catalog_word(0xC037 + u32::from(cursor))?;
                (cursor, state) = start(record)?;
            } else {
                let remaining = (state & 0x7FFF).wrapping_sub(1);
                if remaining != 0 {
                    state = remaining | 0x8000;
                } else {
                    state = catalog_word(0xC03B + u32::from(cursor))?;
                    match state {
                        0xFFFF => return Ok((cursor, state)),
                        0xFFFD => {}
                        0xFFFC => (cursor, state) = start(catalog_word(0xC03D + u32::from(cursor))?)?,
                        _ => (cursor, state) = start(cursor.wrapping_add(4))?,
                    }
                }
            }
        }
        let index = catalog_word(0xC039 + u32::from(cursor))?;
        self.metasprite(index)?;
        Ok((cursor, state))
    }

    // ---- the animated sprite composer ($04:F726) ----

    /// `$04:F688`: animate at (x, y) bytes.
    fn animated(&mut self, x: u8, y: u8, anim: u16, frame: u8) -> Result<(), ScreenError> {
        self.a.ax = u16::from(x);
        self.a.ay = u16::from(y);
        self.a.anim = anim & 0x00FF;
        self.a.frame = u16::from(frame);
        self.compose()
    }

    /// `$04:F726`.
    fn compose(&mut self) -> Result<(), ScreenError> {
        self.select_frame()?;
        if self.a.anim == 0 {
            return Ok(());
        }
        self.align()?;
        self.draw_frame()?;
        self.align()
    }

    /// `$04:F902`: offscreen entries up to the next group of four.
    fn align(&mut self) -> Result<(), ScreenError> {
        while self.y & 0x000F != 0 {
            self.write16(self.y, OFFSCREEN)?;
            self.y += 4;
        }
        Ok(())
    }

    /// `$04:F738`: the animation's frame record for `frame`.
    fn select_frame(&mut self) -> Result<(), ScreenError> {
        let base = catalog_word(0xAF24)?;
        if self.a.anim == 0 {
            return Ok(());
        }
        let mut at = u32::from(catalog_word(0xAF24 + u32::from(self.a.anim) * 2)?);
        let mut total: u8 = 0;
        loop {
            let length = catalog_byte(0xAF24 + at)?;
            if length & 0x80 != 0 {
                at -= 2;
                break;
            }
            total = total.wrapping_add(length);
            if total >= self.a.frame as u8 {
                break;
            }
            at += 2;
        }
        let record = u16::from(catalog_byte(0xAF25 + at)?) * 2 + base;
        self.a.record = usize::from(catalog_word(0xAF24 + u32::from(record))?);
        Ok(())
    }

    /// `$04:F778`.
    fn draw_frame(&mut self) -> Result<(), ScreenError> {
        let mut at = self.a.record as u32;
        let header = catalog_byte(0xAF24 + at)?;
        let size = if header & 0x80 != 0 { 0 } else { 0xAA };
        let count = header & 0x7F;
        if count == 0 {
            return Ok(());
        }
        at += 1;
        let groups = (count - 1) >> 2;
        let row = self.y >> 4;
        for k in 0..=usize::from(groups) {
            self.write8(OAM_HIGH + row + k, size)?;
        }
        self.pieces(at, count)
    }

    /// `$04:F7BB`.
    fn pieces(&mut self, mut at: u32, mut count: u8) -> Result<(), ScreenError> {
        while count != 0 {
            let dy = catalog_byte(0xAF25 + at)? as i8 as i16 as u16;
            let mut y = dy.wrapping_add(self.a.ay);
            if (y as i16) < 0 && y < 0xFFE0 {
                y = OFFSCREEN;
            }
            let dx = catalog_byte(0xAF24 + at)?;
            if dx & 0x80 != 0 && dx & 0x78 == 0 {
                match dx & 0x07 {
                    0 | 1 => {
                        self.corner_entry()?;
                        let badge = usize::from(self.a.badge) / 2;
                        let base = if dx & 0x07 == 0 {
                            table(&BADGE_B, badge, 0x04F821)?
                        } else {
                            table(&BADGE_A, badge, 0x04F82C)?
                        };
                        if base == 0 {
                            self.write16(self.y, OFFSCREEN)?;
                        }
                        let tile = base.wrapping_add(catalog_word(0xAF25 + at)?);
                        self.write16(self.y + 2, tile)?;
                        at += 3;
                        self.y += 4;
                        count -= 1;
                        continue;
                    }
                    2 => {
                        at = (at + u32::from(catalog_word(0xAF25 + at)?)) & 0xFFFF;
                        continue;
                    }
                    _ => return self.block(at + 1),
                }
            }
            let x = dx.wrapping_add(self.a.ax as u8);
            self.write16(self.y, (u16::from(y as u8) << 8) | u16::from(x))?;
            let tile = catalog_word(0xAF26 + at)?;
            self.write16(self.y + 2, tile)?;
            at += 4;
            self.y += 4;
            count -= 1;
        }
        Ok(())
    }

    /// `$04:F847`.
    fn corner_entry(&mut self) -> Result<(), ScreenError> {
        let y = (self.a.ay as u8).wrapping_sub(8);
        let x = (self.a.ax as u8).wrapping_sub(8);
        self.write16(self.y, (u16::from(y) << 8) | u16::from(x))
    }

    /// `$04:F85C`: a 32x32 block of four 16x16 pieces; it ends the frame.
    fn block(&mut self, at: u32) -> Result<(), ScreenError> {
        let (x, y) = (self.a.ax, self.a.ay);
        let corners = [(0x00F0, 0x00F0), (0x0000, 0x00F0), (0x00F0, 0x0000), (0x0000, 0x0000)];
        for (k, (dx, dy)) in corners.into_iter().enumerate() {
            let tile = catalog_word(0xAF24 + at + 2 * k as u32)?;
            self.entry(x.wrapping_add(dx) as u8, y.wrapping_add(dy) as u8, tile)?;
        }
        Ok(())
    }

    // ---- the map mode ($04:87A9) ----

    fn map_mode(&mut self) -> Result<(), ScreenError> {
        self.y = usize::from(self.s.cursor);
        let s = &mut *self.s;
        if s.phases[0] as u8 == 0 {
            s.blink = if (s.blink as u8).wrapping_add(2) == 6 { s.blink & 0xFF00 } else { (s.blink & 0xFF00) | u16::from((s.blink as u8).wrapping_add(2)) };
            s.flash = (s.flash & 0xFF00) | u16::from((s.flash as u8).wrapping_add(2) & 7);
        }
        if s.phases[1] as u8 == 0 {
            let next = (s.choice_blink as u8).wrapping_add(1);
            s.choice_blink = (s.choice_blink & 0xFF00) | u16::from(if next == 3 { 0 } else { next });
        }
        if self.map.globals.hold & 0x0001 == 0 && s.phases[2] as u8 == 0 {
            s.corner = (s.corner & 0xFF00) | u16::from((s.corner as u8).wrapping_add(2) & 7);
        }
        s.speed_mark = if self.map.globals.speed_flags & 0x0008 == 0 { 1 } else { 8 };
        if self.links.mode & 0x0040 != 0 {
            self.a.x = (self.a.x & 0xFF00) | 0x70;
            self.a.y = (self.a.y & 0xFF00) | 0x60;
            self.s.view_icon = self.animate(self.s.view_icon)?;
        }
        self.effects()?;
        if self.director.choosing as u8 != 0 && self.s.choice_blink as u8 != 0 {
            let position = if self.director.choice == 0 { 0xC498 } else { 0xD098 };
            self.write16(self.y, position)?;
            let tile = catalog_word(0xBDFF + u32::from(self.s.choice_blink) * 2)?;
            self.write16(self.y + 2, tile)?;
            self.y += 4;
        }
        self.encounter_panel()?;
        if self.links.scene.stage_results & 0x0080 == 0 {
            self.pilots()?;
            self.gauge()?;
            self.a.x = 0x00A5;
            self.a.y = 0x00CE;
            self.a.tile = catalog_word(0xBDDF + (self.inputs.gauge_style & 0x00FF) as u32 * 2)?;
            self.sprite()?;
        }
        let corner = usize::from(self.s.corner) / 2;
        self.write16(self.y, 0xB7F0)?;
        self.write16(self.y + 2, table(&CORNER_TOP, corner, 0x04B1C)?)?;
        self.write16(self.y + 4, 0xBFF0)?;
        self.write16(self.y + 6, table(&CORNER_BOTTOM, corner, 0x04B24)?)?;
        self.y += 8;
        if self.links.scene.scene_events & 0x0020 == 0 {
            self.cursor()?;
        }
        self.ships()?;
        self.units()?;
        if self.links.scene.scene_events & 0x0020 == 0 {
            self.places()?;
            self.launch_icons()?;
        }
        self.s.count = (self.y >> 2) as u16;
        self.s.cursor = self.y as u16;
        Ok(())
    }

    /// `$04:88DF..89DE`: the cursor, or the chosen place with its label,
    /// or the order pulses; then the destination marker.
    fn cursor(&mut self) -> Result<(), ScreenError> {
        let screen = *self.screen;
        if screen.interface & 0x0400 != 0 {
            self.chosen_place()?;
        } else if screen.interface & 0x0100 != 0 {
            // $04:899B.
            for pulse in screen.pulses {
                let x = ((pulse.x >> 8) & 0xFFFF) as u16;
                let y = ((pulse.y >> 8) & 0xFFFF) as u16;
                let inside = |v: u16| (v.wrapping_sub(8) as i16) >= 0 && (v.wrapping_sub(0xF8) as i16) < 0;
                if inside(x) && inside(y) {
                    self.a.x = x;
                    self.a.y = y;
                    self.a.tile = pulse.flags;
                    self.sprite()?;
                }
            }
        } else if self.map.globals.hold & 0x0008 != 0 {
            return self.target_marker();
        } else if screen.hover & 0x0001 != 0 {
            self.chosen_place()?;
        } else {
            self.animated(screen.cursor_x, screen.cursor_y.wrapping_sub(1), 6, self.s.cursor_frame as u8)?;
        }
        // $04:89D7.
        let frame = &mut self.s.cursor_frame;
        *frame = (*frame & 0xFF00) | u16::from((*frame as u8).wrapping_add(1) & 0x0F);
        self.target_marker()
    }

    /// `$04:892C`: the cursor on the chosen place, with its name.
    fn chosen_place(&mut self) -> Result<(), ScreenError> {
        let screen = *self.screen;
        self.animated(screen.selected_x, screen.selected_y.wrapping_sub(1), 5, self.s.cursor_frame as u8)?;
        let mut tile = screen.info.number.wrapping_mul(16).wrapping_add(0x016A);
        let y = if screen.selected_y < 0x80 { screen.selected_y.wrapping_add(0x10) } else { screen.selected_y.wrapping_sub(0x19) };
        let mut position = (u16::from(y) << 8) | u16::from(screen.selected_x.wrapping_add(0xE4));
        for _ in 0..5 {
            position = position.wrapping_add(8);
            self.write16(self.y, position)?;
            self.write16(self.y + 2, tile.wrapping_add(0x2000))?;
            self.y += 4;
            tile = tile.wrapping_add(1);
        }
        Ok(())
    }

    /// `$04:89E0`: the ordered destination's marker.
    fn target_marker(&mut self) -> Result<(), ScreenError> {
        if self.screen.interface & 0x0004 == 0 {
            return Ok(());
        }
        self.s.target_countdown = self.s.target_countdown.wrapping_sub(1);
        if self.s.target_countdown == 0 {
            self.s.target_phase ^= 0x0001;
            self.s.target_countdown = 4;
        }
        self.large()?;
        let x = self.screen.ship.target_x.wrapping_sub(8);
        let y = self.screen.ship.target_y.wrapping_sub(9);
        self.write8(self.y, x)?;
        self.write8(self.y + 1, y)?;
        let tile = self.s.target_phase.swap_bytes().wrapping_shl(1).wrapping_add(0x204C);
        self.write16(self.y + 2, tile)?;
        self.y += 4;
        Ok(())
    }

    /// `$04:8A2E`: the ship and wingmate with their exhausts, nearer last.
    fn ships(&mut self) -> Result<(), ScreenError> {
        if self.screen.ship.travel & 0x0200 != 0 {
            return Ok(());
        }
        let ship = self.screen.ship;
        if (ship.sector as u8).wrapping_sub(5) < 7 {
            self.craft(ship.x, ship.y, ship.sector, self.links.pilots[0], 0)?;
            self.exhaust(ship.travel & 0x0002 != 0, ship.x, ship.y, ship.sector)?;
        } else {
            self.exhaust(ship.travel & 0x0002 != 0, ship.x, ship.y, ship.sector)?;
            self.craft(ship.x, ship.y, ship.sector, self.links.pilots[0], 0)?;
        }
        if self.links.scene.stage_results & 0x0002 != 0 || self.screen.wingmate.flags & 0x0200 != 0 {
            return Ok(());
        }
        let wing = self.screen.wingmate;
        let following = wing.flags & 0x0002 != 0;
        if (wing.sector as u8).wrapping_sub(5) < 7 {
            self.craft(wing.x, wing.y, wing.sector, self.links.pilots[1], 2)?;
            self.exhaust(following, wing.x, wing.y, wing.sector)?;
        } else {
            self.exhaust(following, wing.x, wing.y, wing.sector)?;
            self.craft(wing.x, wing.y, wing.sector, self.links.pilots[1], 2)?;
        }
        Ok(())
    }

    /// `$04:8E12`/`$04:8E76`; the wingmate's tile word starts from 2.
    fn craft(&mut self, x: u16, y: u16, sector: u16, pilot: u8, low: u8) -> Result<(), ScreenError> {
        self.a.x = (self.a.x & 0xFF00) | (x >> 8);
        self.a.y = (self.a.y & 0xFF00) | (y >> 8);
        let mut high = (pilot & 0x01) << 1;
        if sector as u8 & 0x08 == 0 {
            high = high.wrapping_add(0x40);
        }
        high = high.wrapping_add(0x20);
        self.a.tile = (u16::from(high) << 8) | u16::from(low);
        self.sprite()
    }

    /// `$04:8E40`/`$04:8EA4`.
    fn exhaust(&mut self, moving: bool, x: u16, y: u16, sector: u16) -> Result<(), ScreenError> {
        if !moving || self.s.blink as u8 == 0 {
            return Ok(());
        }
        self.a.tile = catalog_word(0xBDD1 + u32::from(self.s.blink))?;
        let x = catalog_byte(0xBDB1 + u32::from(sector))?.wrapping_add((x >> 8) as u8);
        let y = catalog_byte(0xBDC1 + u32::from(sector))?.wrapping_add((y >> 8) as u8);
        self.a.x = (self.a.x & 0xFF00) | u16::from(x);
        self.a.y = (self.a.y & 0xFF00) | u16::from(y);
        self.sprite()
    }

    /// `$04:8AA3`: the two pilots' portraits and the low-shield warning.
    fn pilots(&mut self) -> Result<(), ScreenError> {
        self.a.x = 0x0052;
        self.a.y = 0x00CE;
        if self.links.pilot_shields[0] < 0x0D {
            let blink = self.s.shield_blink.wrapping_add(1);
            self.s.shield_blink = blink;
            if blink >= 0x10 {
                if blink != 0x10 {
                    self.s.shield_blink = 0;
                }
                self.a.tile = 0x2664;
                self.sprite()?;
            }
        }
        self.a.tile = u16::from(self.links.pilots[0] & 0x7F) * 2 + 0x2020;
        self.sprite()?;
        self.a.x = 0x0065;
        self.a.y = 0x00CE;
        self.a.tile = if self.links.scene.stage_results & 0x0002 != 0 {
            table(&PORTRAIT_ROTATION, usize::from(self.s.phases[2] >> 1), 0x048AF9)?.wrapping_add(0x2080)
        } else {
            u16::from(self.links.pilots[1] & 0x7F) * 2 + 0x2020
        };
        self.sprite()
    }

    /// `$04:95DA`: the shield gauge.
    fn gauge(&mut self) -> Result<(), ScreenError> {
        let mut shield = u16::from(self.links.pilot_shields[0]) & 0x003F;
        let row = catalog_word(0xBDA5 + u32::from(self.inputs.gauge_row))?;
        let pair = usize::from(self.links.pilots[0] >> 1);
        let count = table(&GAUGE_COUNTS, pair, 0x0495E6)?;
        let mut at = usize::from(table(&GAUGE_STARTS, pair, 0x0495EC)?) / 2;
        for _ in 0..count {
            let position = table(&GAUGE_OFFSETS, at, 0x0495F1)?.wrapping_add(row);
            self.write16(self.y, position)?;
            let step = if shield == 0 {
                2
            } else if shield < 4 {
                shield = 0;
                1
            } else {
                shield -= 4;
                0
            };
            self.write16(self.y + 2, 0x2645 + step)?;
            self.y += 4;
            at += 1;
        }
        Ok(())
    }

    /// `$04:8BEF`: the encounter panel for the unit under the cursor.
    fn encounter_panel(&mut self) -> Result<(), ScreenError> {
        if self.inputs.panel_enabled == 0
            || self.map.globals.hold & 0x0008 != 0
            || self.screen.interface & 0x0100 != 0
        {
            return Ok(());
        }
        let id = self.screen.hovered_unit.ok_or(ScreenError::MissingUnit(0x048C09))?;
        let unit = *self.map.unit(id);
        let slot = usize::from(unit.encounter_slot);
        let globals = &self.map.globals;
        let pattern = *globals.slot_pattern.get(slot).ok_or(ScreenError::TableOverrun(0x048C12))?;
        let panel = &mut self.s.panel;
        panel[2] = pattern & 0x0F;
        panel[6] = pattern >> 4;
        panel[0] = globals.slot_kind[slot];
        panel[4] = globals.slot_strength[slot];
        if panel[2] == 0 {
            panel[2] = panel[6];
            panel[0] = panel[4];
            panel[6] = 0;
        }
        self.a.y = (self.a.y & 0xFF00) | u16::from(unit.y);
        let (tile, offset, mark) = if unit.x >= 0x80 {
            (unit.strength.wrapping_add(0x26E0), 0x0004u16, 0x6082)
        } else {
            (unit.strength.wrapping_add(0x26E0), 0xFFFC, 0x2082)
        };
        let x = if unit.x >= 0x80 { unit.x.wrapping_sub(0x10) } else { unit.x.wrapping_add(0x10) };
        self.a.x = (self.a.x & 0xFF00) | u16::from(x);
        self.a.tile = tile;
        self.small()?;
        self.a.x = self.a.x.wrapping_add(offset);
        self.a.tile = mark;
        self.sprite()?;
        let panel = self.s.panel;
        let first = u16::from_le_bytes([panel[2], panel[3]]);
        if first != 0 {
            let kind = u16::from_le_bytes([panel[0], panel[1]]);
            self.panel_column(kind, first, unit.variant, 0x5C, 0x4C, 0xCE6C, true)?;
        }
        let second = u16::from_le_bytes([panel[6], panel[7]]);
        if second != 0 {
            let kind = u16::from_le_bytes([panel[4], panel[5]]);
            self.panel_column(kind, second, unit.variant, 0x8C, 0x7C, 0xCE9C, false)?;
        }
        Ok(())
    }

    /// `$04:8CA2..8D3C` and `$04:8D42..8DBC`: one column of the panel.
    #[allow(clippy::too_many_arguments)]
    fn panel_column(
        &mut self,
        kind: u16,
        count: u16,
        variant: u16,
        x: u16,
        label_x: u16,
        count_position: u16,
        first: bool,
    ) -> Result<(), ScreenError> {
        let icon = catalog_word(0xBE05 + u32::from(kind) * 2)?;
        self.a.tile = (icon & 0x7FFF).wrapping_add(0x3000);
        self.a.x = x;
        self.a.y = 0x00CB;
        self.sprite()?;
        if icon & 0x8000 != 0 && self.s.flash != 0 {
            self.a.tile = catalog_word(0xBE35 + u32::from(self.s.flash))?;
            self.a.x = x + 4;
            self.a.y = 0x00C7;
            self.sprite()?;
        }
        self.a.x = x;
        self.a.y = 0x00CB;
        if first {
            let row = variant.wrapping_mul(2);
            let index = ((self.s.wraps[2] & 0x0001) + row) as u32 * 2;
            self.a.tile = catalog_word(0xBE4B + index)?;
            self.panel_badge = self.a.tile;
            self.quad()?;
            self.a.tile = catalog_word(0xBE3D + u32::from(row))?;
            self.panel_label = self.a.tile;
        } else {
            self.a.tile = self.panel_badge;
            self.quad()?;
            self.a.tile = self.panel_label;
        }
        self.a.x = label_x;
        self.a.y = 0x00B3;
        for _ in 0..4 {
            self.entry(self.a.x as u8, self.a.y as u8, self.a.tile)?;
            self.a.x = (self.a.x & 0xFF00) | u16::from((self.a.x as u8).wrapping_add(8));
            self.a.tile = self.a.tile.wrapping_add(1);
        }
        self.write16(self.y, count_position)?;
        self.write16(self.y + 2, count.wrapping_add(0x30F0))?;
        self.y += 4;
        Ok(())
    }

    /// `$04:8EDA`: the units.
    fn units(&mut self) -> Result<(), ScreenError> {
        self.s.marker_phase = 0;
        self.s.marker_tick = self.s.marker_tick.wrapping_add(1);
        let mut next = self.map.unit_head;
        while let Some(id) = next {
            let unit = *self.map.unit(id);
            next = unit.next;
            let encounter = self.links.scene.scene_events & 0x0020 != 0;
            if encounter && unit.flags & 0x8400 == 0 && unit.flags & 0x0001 == 0 {
                continue;
            }
            if unit.flags & 0x1000 != 0 {
                continue;
            }
            self.a.x = (self.a.x & 0xFF00) | u16::from(unit.x);
            self.a.y = (self.a.y & 0xFF00) | u16::from(unit.y);
            if unit.flags & 0x4000 != 0 {
                if unit.sprite == 0 {
                    continue;
                }
                if unit.sprite == 0xFFFF {
                    let tile = table(&SPECIAL_TILES, usize::from(self.s.flash) / 2, 0x048F9E)?;
                    self.entry(unit.x.wrapping_sub(4), unit.y.wrapping_sub(5), tile)?;
                    continue;
                }
                let mut frame = unit.frame.wrapping_add(1);
                if frame >= unit.animation_timer {
                    if unit.flags2 & 0x0010 == 0 {
                        continue;
                    }
                    frame = frame.wrapping_sub(unit.animation_timer);
                }
                self.map.unit_mut(id).frame = frame;
                // $04:F675.
                let unit = *self.map.unit(id);
                self.a.anim = unit.sprite;
                self.a.badge = (unit.variant << 1) & 0x00FF;
                self.a.ax = u16::from(unit.x);
                self.a.ay = u16::from(unit.y);
                self.a.frame = unit.frame & 0x00FF;
                self.compose()?;
                if unit.flags & 0x0040 != 0 {
                    self.unit_mark(&unit)?;
                }
                continue;
            }
            if unit.flags2 & 0x0040 != 0 {
                if unit.sprite != 0 {
                    self.a.tile = unit.sprite;
                    self.sprite()?;
                }
                continue;
            }
            self.unit_icon(id)?;
        }
        Ok(())
    }

    /// `$04:9352`.
    fn unit_mark(&mut self, unit: &super::strategic_sim::MapUnit) -> Result<(), ScreenError> {
        if unit.flags2 & 0x0004 != 0 || self.s.blink as u8 == 0 {
            return Ok(());
        }
        self.bits(&LARGE_BIT)?;
        let tile = catalog_word(0xBF0F + u32::from(self.s.blink))?;
        self.write16(self.y + 2, tile)?;
        let y = (self.a.ay as u8).wrapping_sub(0x0F);
        let x = (self.a.ax as u8).wrapping_sub(0x02);
        self.write16(self.y, (u16::from(y) << 8) | u16::from(x))?;
        self.y += 8;
        Ok(())
    }

    /// `$04:8FBF`: a unit's icon by its variant.
    fn unit_icon(&mut self, id: UnitId) -> Result<(), ScreenError> {
        let unit = *self.map.unit(id);
        match unit.variant {
            0..=3 | 10 | 11 => self.fleet(id),
            4 => {
                self.a.tile = catalog_word(0xBEE1 + u32::from(self.s.flash))?;
                self.sprite()
            }
            5 => {
                self.a.tile = catalog_word(0xBEE9 + u32::from(self.s.wraps[1] & 0x0003) * 2)?;
                self.small()
            }
            6 => {
                self.a.tile = catalog_word(0xBEF1 + u32::from(self.s.flash))?;
                self.small()
            }
            7 => {
                self.home_icon(&unit)?;
                self.home_warning(&unit)
            }
            8 => {
                self.a.tile = 0x212C;
                self.sprite()
            }
            9 => self.satellite(&unit),
            other => Err(ScreenError::Unported(0x048FCA + u32::from(other) * 2)),
        }
    }

    /// `$04:90F7`: a fleet with its heading and exhaust.
    fn fleet(&mut self, id: UnitId) -> Result<(), ScreenError> {
        let unit = *self.map.unit(id);
        let x = unit.x.wrapping_sub(4);
        let y = unit.y.wrapping_sub(5);
        self.a.x = (self.a.x & 0xFF00) | u16::from(x);
        self.a.y = (self.a.y & 0xFF00) | u16::from(y);
        let octant = (((unit.heading >> 8) as u8).wrapping_add(8) >> 5) as u16;
        let tile = if unit.flags & 0x0020 == 0 { 0x20B3 } else { catalog_word(0xBE67 + u32::from(octant) * 2)? };
        self.entry(x, y, tile)?;
        if unit.flags2 & 0x0004 == 0 && self.s.blink as u8 != 0 {
            let ex = x.wrapping_add(catalog_byte(0xBEF9 + u32::from(octant))?);
            let ey = y.wrapping_add(catalog_byte(0xBF01 + u32::from(octant))?);
            let tile = catalog_word(0xBF09 + u32::from(self.s.blink))?;
            self.entry(ex, ey, tile)?;
        }
        if unit.flags & 0x0010 != 0 && unit.flags & 0x0002 != 0 {
            self.s.marker_target = (unit.target_x, unit.target_y);
            self.fighter_marker()?;
        }
        Ok(())
    }

    /// `$04:F44A`.
    fn fighter_marker(&mut self) -> Result<(), ScreenError> {
        let phase = self.s.marker_tick.wrapping_add(self.s.marker_phase);
        let index = usize::from((phase >> 5) & 0x0007);
        self.a.ax = self.s.marker_target.0.wrapping_add(MARKER_DX[index]);
        self.a.ay = self.s.marker_target.1.wrapping_add(MARKER_DY[index]);
        self.a.frame = phase & 0x001F;
        self.a.anim = 0x000F;
        self.compose()?;
        self.s.marker_phase = self.s.marker_phase.wrapping_add(0x004B);
        Ok(())
    }

    /// `$04:91A0`: the home place's icon.
    fn home_icon(&mut self, unit: &super::strategic_sim::MapUnit) -> Result<(), ScreenError> {
        let home = unit.home.ok_or(ScreenError::MissingPlace(0x0491A1))?;
        let place = self.map.places[usize::from(home.0)];
        let ticks = if (place.warning_countdown.wrapping_sub(0x14) as i16) < 0 { self.s.wraps[1] } else { self.s.wraps[2] };
        self.a.tile = if ticks & 0x0001 == 0 { 0x232E } else { 0x212E };
        self.sprite()
    }

    /// `$04:91C9`: the home place's warning animation (`$04:91E3`).
    fn home_warning(&mut self, unit: &super::strategic_sim::MapUnit) -> Result<(), ScreenError> {
        let home = unit.home.ok_or(ScreenError::MissingPlace(0x0491CA))?;
        self.s.explosion_place = Some(home);
        let index = usize::from(home.0);
        let place = self.map.places[index];
        if place.status & 0x0002 == 0 {
            return Ok(());
        }
        match place.warning {
            0 => {}
            1 => {
                // $04:9302.
                self.a.x = place.x.wrapping_sub(7);
                self.a.y = place.y.wrapping_add(8);
                let countdown = place.warning_countdown;
                let mut tile = 0x2E68u16;
                for limit in [0x01E0u16, 0x0168, 0x0078] {
                    if (countdown.wrapping_sub(limit) as i16) >= 0 {
                        break;
                    }
                    tile += 2;
                }
                self.a.tile = tile;
                self.sprite()?;
                let state = self.animate((place.warning_rate, place.warning_bias))?;
                let place = &mut self.map.places[index];
                place.warning_rate = state.0;
                place.warning_bias = state.1;
            }
            2 => {
                // $04:92CE.
                let x = (place.x as u8).wrapping_sub(0x0A);
                let y = (place.y as u8).wrapping_add(0x0C);
                let frame = place.warning_bias as u8;
                let next = frame.wrapping_add(1);
                let record = &mut self.map.places[index];
                record.warning_bias = (record.warning_bias & 0xFF00) | u16::from(next);
                if next == 0x24 {
                    // An eight-bit increment.
                    record.warning = (record.warning & 0xFF00) | u16::from((record.warning as u8).wrapping_add(1));
                }
                self.animated(x, y, place.warning_rate & 0x00FF, frame)?;
            }
            3 => {
                // $04:9206.
                self.output.cues.push(0x000F);
                self.map.globals.speed_flags |= 0x1000;
                self.s.explosions = [
                    [0x28, 0x88, 0x21, 0x00, 0x00],
                    [0x18, 0x80, 0x21, 0x00, 0x0F],
                    [0x30, 0xA0, 0x21, 0x00, 0x1E],
                    [0x18, 0x98, 0x21, 0x00, 0x2D],
                ];
                let held = place.held_unit.ok_or(ScreenError::MissingUnit(0x049256))?;
                let mut scene: SceneLinks = self.links.scene;
                self.map.damage_planet(held, &mut scene);
                self.links.scene = scene;
                self.map.places[index].warning = place.warning.wrapping_add(1);
            }
            4 => {
                // $04:9263.
                self.s.explosion_count = 0;
                for k in 0..4 {
                    let record = self.s.explosions[k];
                    if record[2] == 0xFF {
                        self.s.explosion_count += 1;
                        continue;
                    }
                    if record[4] != 0 {
                        self.s.explosions[k][4] = record[4] - 1;
                        continue;
                    }
                    let frame = record[3];
                    let next = frame.wrapping_add(1);
                    self.s.explosions[k][3] = next;
                    if next == 0x2A {
                        self.s.explosions[k][2] = 0xFF;
                    }
                    self.animated(record[0], record[1], u16::from(record[2]), frame)?;
                }
                if self.s.explosion_count == 4 {
                    self.map.places[index].warning = place.warning.wrapping_add(1);
                    self.s.explosion_count = 5;
                }
            }
            5 => {
                self.s.explosion_count = self.s.explosion_count.wrapping_sub(1);
                if self.s.explosion_count == 0 {
                    self.map.places[index].warning = place.warning.wrapping_add(1);
                }
            }
            6 => self.map.globals.speed_flags &= !0x1000,
            other => return Err(ScreenError::Unported(0x0491E3 + u32::from(other) * 2)),
        }
        Ok(())
    }

    /// `$04:8FFA`: the satellite, its beam and its guard.
    fn satellite(&mut self, unit: &super::strategic_sim::MapUnit) -> Result<(), ScreenError> {
        let timer = unit.timer;
        let globals = self.map.globals;
        let facing = globals.satellite_facing;
        if globals.satellite_flags & 0x0004 != 0 && self.s.blink as u8 != 0 {
            let half = u32::from(facing >> 1);
            let x = catalog_byte(0xBEBB + half)?.wrapping_add(self.a.x as u8);
            let y = catalog_byte(0xBEBC + half)?.wrapping_add(self.a.y as u8).wrapping_sub(1);
            let tile = catalog_word(0xBEDB + u32::from(self.s.blink))?;
            self.entry(x, y, tile)?;
        }
        let x = catalog_byte(0xBE7B + u32::from(facing))?.wrapping_add(self.a.x as u8);
        let y = catalog_byte(0xBE7C + u32::from(facing))?.wrapping_add(self.a.y as u8).wrapping_sub(1);
        let tile = catalog_word(0xBE7D + u32::from(facing))?;
        self.entry(x, y, tile)?;
        if globals.satellite_flags & 0x0002 != 0 && timer < 0x012C {
            self.s.satellite_icon = self.animate(self.s.satellite_icon)?;
        }
        if globals.satellite_flags & 0x0200 != 0 {
            self.s.satellite_beam = self.animate(self.s.satellite_beam)?;
            if self.s.satellite_beam.1 != 0xFFFF {
                return Ok(());
            }
            self.map.globals.satellite_flags &= !0x0200;
        }
        self.a.tile = catalog_word(0xBE77 + u32::from(globals.satellite_guarding))?;
        self.sprite()
    }

    /// `$04:8B2C`: the marked places.
    fn places(&mut self) -> Result<(), ScreenError> {
        if self.links.display_flags & 0x0080 == 0 {
            return Ok(());
        }
        let mut next = self.map.place_head;
        while let Some(id) = next {
            let index = usize::from(id.0);
            let place = self.map.places[index];
            next = place.next;
            if place.flags & 0x0001 == 0 {
                continue;
            }
            if place.flags & 0x4000 != 0 {
                let frame = place.arrivals as u8;
                self.map.places[index].arrivals = place.arrivals.wrapping_add(1);
                self.animated(place.x as u8, place.y as u8, place.marker & 0x00FF, frame)?;
                continue;
            }
            if place.flags & 0x1000 != 0 || place.flags & 0x2000 == 0 {
                continue;
            }
            self.a.x = place.x;
            self.a.y = place.y;
            self.place_timer(&place)?;
            self.a.tile = table(&PLACE_ICONS, usize::from(place.kind), 0x048B6D)?;
            self.quad()?;
        }
        Ok(())
    }

    /// `$04:8BB1`: the attack countdown's seconds.
    fn place_timer(&mut self, place: &super::strategic_sim::MapPlace) -> Result<(), ScreenError> {
        if place.state != 2 || place.timer >= 0x0168 {
            return Ok(());
        }
        if place.timer >= 0x000A {
            self.a.tile = (place.timer / 0x3C).wrapping_add(0x20E0);
            let y = self.a.y;
            self.a.y = y.wrapping_add(8);
            self.small()?;
            self.a.y = y;
        }
        self.a.tile = 0x20B0;
        self.small()
    }

    /// `$04:8DE1`: one icon per pending launch.
    fn launch_icons(&mut self) -> Result<(), ScreenError> {
        for k in 0..usize::from(self.map.globals.launch_count) {
            self.large()?;
            let position = table(&LAUNCH_ICONS, k, 0x048DF0)?;
            self.write16(self.y, position)?;
            self.write16(self.y + 2, 0x2640)?;
            self.y += 4;
        }
        Ok(())
    }

    /// `$04:F431`: the effects.
    fn effects(&mut self) -> Result<(), ScreenError> {
        // $04:F53D.
        if self.s.ring_animation != 0 {
            self.s.ring_frame = self.s.ring_frame.wrapping_add(1);
            let position = self.s.ring_position;
            self.at_word(position, self.s.ring_animation, self.s.ring_frame & 0x000F)?;
        }
        // $04:F603.
        for (k, anim) in [(0usize, 0x000Au16), (1, 0x000B)] {
            let frame = if k == 0 { self.s.burst_frames.0 } else { self.s.burst_frames.1 };
            if frame == 0 {
                continue;
            }
            let next = (frame + 1).min(0x0040);
            if k == 0 {
                self.s.burst_frames.0 = next;
            } else {
                self.s.burst_frames.1 = next;
            }
            self.at_word(self.s.effect_origin, anim, frame)?;
        }
        // $04:F644.
        if self.s.flare_frame != 0 && (self.s.flare_frame as i16) > 0 {
            let frame = self.s.flare_frame as u8;
            self.s.flare_frame = (self.s.flare_frame & 0xFF00) | u16::from(frame.wrapping_add(1));
            let base: u16 = if self.s.flare_style != 0 { 0x06EA } else { 0x0CEA };
            self.at_word(base.wrapping_add(self.s.effect_origin), 0x0014, u16::from(frame & 0x0F))?;
        }
        // $04:F5A3.
        let kind = self.s.highlight_kind;
        if kind == 0 {
            return Ok(());
        }
        let frame = self.s.highlight_frame + 1;
        self.s.highlight_frame = if frame >= 0x10 { 0 } else { frame };
        let frame = self.s.highlight_frame;
        match kind {
            2 | 3 => {
                let mask = if kind == 2 { 0x8728 } else { 0x0020 };
                let mut next = self.map.unit_head;
                while let Some(id) = next {
                    let unit = *self.map.unit(id);
                    next = unit.next;
                    let chosen = if kind == 2 { unit.flags & mask == 0 } else { unit.flags & mask != 0 };
                    if chosen {
                        let anim = if unit.y >= 0x60 { 0x000D } else { 0x000C };
                        self.at_word((u16::from(unit.y) << 8) | u16::from(unit.x), anim, frame)?;
                    }
                }
            }
            _ => {
                let mut next = self.map.place_head;
                while let Some(id) = next {
                    let place = self.map.places[usize::from(id.0)];
                    next = place.next;
                    if place.flags & 0x0002 == 0 && place.flags & 0x0001 != 0 && place.flags & 0x2000 != 0 {
                        let anim = if place.y as u8 >= 0x60 { 0x000D } else { 0x000C };
                        self.at_word((u16::from(place.y as u8) << 8) | u16::from(place.x as u8), anim, frame)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// `$04:F6A6`: animate at a packed (y, x) word.
    fn at_word(&mut self, position: u16, anim: u16, frame: u16) -> Result<(), ScreenError> {
        self.a.ax = position & 0x00FF;
        self.a.ay = position >> 8;
        self.a.anim = anim;
        self.a.frame = frame;
        self.compose()
    }
}

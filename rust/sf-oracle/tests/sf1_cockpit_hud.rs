//! Execute the unchanged SF1 `mdrawhud` and `mdodrawline` instructions.
use sf_core::cockpit_hud::{CockpitHudLine, CockpitHudState};
use sf_oracle::{load_built_rom, SnesBus};

const DRAW_HUD: u16 = 0xD77B;
const DRAW_LINE: u16 = 0xD8C3;
// Original `mdo_3d_display` tail: RPIX; STOP. No patched return program.
const FLUSH_AND_STOP: u16 = 0xAC96;
const STACK: u16 = 0x04A6;
const HUD_ROTATION: usize = 0x3510;
const HUD_COLOR: usize = 0x3512;
const HUD_DAMAGE: usize = 0x3514;

fn state(roll: u8, damage: u8, color: u8) -> CockpitHudState {
    CockpitHudState {
        enabled: true,
        roll,
        palette_index: color,
        left_wing_broken: damage & 1 != 0,
        right_wing_broken: damage & 2 != 0,
    }
}

#[test]
fn all_roll_damage_and_palette_choices_match_original_line_endpoints_and_order() {
    let mut source = drawing_source();
    source.watch_gsu_execution_capture(1, DRAW_LINE, 0, 0);
    for roll in 0..=u8::MAX {
        for damage in 0..4 {
            for color in 0..16 {
                for (address, value) in [
                    (HUD_ROTATION, 0xFF00 | u16::from(roll)),
                    (HUD_COLOR, color as u16),
                    (HUD_DAMAGE, damage as u16),
                ] {
                    word(&mut source, 0x700000 + address as u32, value);
                }
                run_hud(&mut source);
                assert!(
                    !source.gsu_ref().unwrap().is_running(),
                    "roll={roll} damage={damage} color={color}"
                );
                let lines: Vec<_> = source
                    .take_gsu_execution_captures()
                    .into_iter()
                    .map(|capture| CockpitHudLine {
                        start: [capture.values[1] as u8, capture.values[2] as u8],
                        end: [capture.values[3] as u8, capture.values[9] as u8],
                        palette_index: capture.color,
                    })
                    .collect();
                assert_eq!(
                    lines,
                    state(roll, damage, color).lines().unwrap(),
                    "roll={roll} damage={damage} color={color}"
                );
            }
        }
        for enable in [0, 1, 127] {
            word(
                &mut source,
                0x700000 + HUD_ROTATION as u32,
                u16::from_le_bytes([roll, enable]),
            );
            run_hud(&mut source);
            assert!(source.take_gsu_execution_captures().is_empty());
        }
    }
}

fn word(bus: &mut SnesBus, address: u32, value: u16) {
    let [lo, hi] = value.to_le_bytes();
    bus.write8(address, lo);
    bus.write8(address + 1, hi);
}

fn drawing_source() -> SnesBus {
    let mut source = SnesBus::new(load_built_rom().expect("source-built SF1 ROM"));
    source.enable_gsu();
    // Do not use the default zero-based bitmap: repeated draws there can
    // overwrite the original routine's scratch variables and stack.
    source.write8(0x3038, 0x20);
    source.write8(0x303A, 0x39);
    source.write8(0x3034, 1);
    source
}

fn run_hud(source: &mut SnesBus) {
    word(source, 0x3014, STACK);
    word(source, 0x3016, FLUSH_AND_STOP);
    word(source, 0x301E, DRAW_HUD);
    source.tick_gsu(1_000_000);
    assert!(!source.gsu_ref().unwrap().is_running());
}

#[test]
fn raster_pixels_match_original_four_bit_bitmap_for_every_roll_and_damage_state() {
    let mut source = drawing_source();
    // Four-bit, 192-line column-major output, away from source variables.
    source.write8(0x3038, 0x20);
    source.write8(0x303A, 0x39);
    source.write8(0x3034, 1);
    const BITMAP: u32 = 0x708000;
    const WIDTH: usize = 224;
    const HEIGHT: usize = 192;
    for roll in 0..=u8::MAX {
        for damage in 0..4 {
            // Include transparent and every wire-frame color as well as white.
            let color = [0, 5, 6, 7, 8, 15][usize::from(roll) % 6];
            // Existing scene pixels are color one. A transparent HUD line
            // must preserve them, not clear them or recolor them as black.
            for offset in 0..(WIDTH * HEIGHT / 2) as u32 {
                let value = if offset % 32 < 16 && offset % 2 == 0 {
                    255
                } else {
                    0
                };
                source.write8(BITMAP + offset, value);
            }
            word(
                &mut source,
                0x700000 + HUD_ROTATION as u32,
                0xFF00 | u16::from(roll),
            );
            word(&mut source, 0x700000 + HUD_COLOR as u32, color.into());
            word(&mut source, 0x700000 + HUD_DAMAGE as u32, damage.into());
            word(&mut source, 0x3014, STACK);
            word(&mut source, 0x3016, FLUSH_AND_STOP);
            let prior = source.gsu_plot_count();
            word(&mut source, 0x301E, DRAW_HUD);
            source.tick_gsu(1_000_000);
            assert!(!source.gsu_ref().unwrap().is_running());
            let native = state(roll, damage, color);
            let mut samples = 0;
            for line in native.lines().unwrap() {
                line.visit_pixels(|_, _| samples += 1);
            }
            assert_eq!(source.gsu_plot_count() - prior, samples);
            let mut bitmap = vec![1; WIDTH * HEIGHT];
            for pixel in native.pixels() {
                bitmap[usize::from(pixel.y) * WIDTH + usize::from(pixel.x)] = pixel.palette_index;
            }
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let tile = (x / 8) * (HEIGHT / 8) + y / 8;
                    let row = BITMAP + (tile * 32 + (y % 8) * 2) as u32;
                    let mut color = 0;
                    for bit in 0..4 {
                        color |= ((source.read8(row + (bit / 2 * 16 + bit % 2) as u32)
                            >> (7 - x % 8))
                            & 1)
                            << bit;
                    }
                    assert_eq!(
                        bitmap[y * WIDTH + x],
                        color,
                        "roll={roll} damage={damage} pixel=({x},{y})"
                    );
                }
            }
        }
    }
}

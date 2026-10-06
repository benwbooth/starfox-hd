//! Independent launch scanouts selected only by original BG1 bitmap data.

use std::{collections::BTreeMap, path::Path};

const WIDTH: usize = 256;
const HEIGHT: usize = 224;
const MESEN_HEIGHT: usize = 239;
// Mesen SnesPpu::ApplyHiResMode places non-overscan hardware line N at N+6.
// Our diagnostic coordinates are hardware lines 0..223, not a best-fit crop.
const HARDWARE_LINE_ZERO: usize = 6;
const BITMAP_WIDTH: usize = 224;
const BITMAP_HEIGHT: usize = 192;
const BITMAP_ORIGIN: usize = 16;

pub struct Scanout {
    pub video_frame: u64,
    bitmap: Vec<u8>,
    pub rgb: Vec<u8>,
}

pub struct LaunchVideo {
    frames: Vec<Scanout>,
}

impl LaunchVideo {
    pub fn read(directory: &Path) -> Self {
        let manifest = std::fs::read_to_string(directory.join("launch_display.txt"))
            .expect("Mesen launch manifest");
        assert_eq!(
            manifest.lines().next(),
            Some("rom_sha1=cf08148cd8f26d51f8c67c956179dfc594e7a4f1"),
            "Mesen must use the same original USA Rev 2 ROM"
        );
        let mut frames: Vec<Scanout> = Vec::new();
        for line in manifest.lines().filter(|line| line.starts_with("video=")) {
            let mut fields = BTreeMap::new();
            for field in line.split_whitespace() {
                let (name, value) = field.split_once('=').expect("Mesen manifest field");
                assert!(
                    fields.insert(name, value).is_none(),
                    "duplicate Mesen field"
                );
            }
            let number = |name| fields[name].parse::<usize>().expect("Mesen decimal field");
            let read = |name| {
                let filename = fields[name];
                assert_eq!(
                    Path::new(filename).file_name().and_then(|s| s.to_str()),
                    Some(filename)
                );
                std::fs::read(directory.join(filename)).expect("Mesen capture file")
            };
            let video_frame = number("video") as u64;
            if let Some(previous) = frames.last() {
                assert_eq!(
                    video_frame,
                    previous.video_frame + 1,
                    "missing Mesen scanout"
                );
            }
            let vram = read("vram");
            assert_eq!(vram.len(), 65536);
            let bitmap = decode_bitmap(
                &vram,
                number("bg1_map"),
                number("bg1_chr"),
                number("bg1_hscroll"),
                number("bg1_vscroll"),
            );
            let ppm = read("image");
            let header = format!("P6\n{WIDTH} {MESEN_HEIGHT}\n255\n");
            assert!(ppm.starts_with(header.as_bytes()), "Mesen PPM geometry");
            let full_rgb = &ppm[header.len()..];
            assert_eq!(full_rgb.len(), WIDTH * MESEN_HEIGHT * 3);
            let rgb = full_rgb
                [HARDWARE_LINE_ZERO * WIDTH * 3..(HARDWARE_LINE_ZERO + HEIGHT) * WIDTH * 3]
                .to_vec();
            frames.push(Scanout {
                video_frame,
                bitmap,
                rgb,
            });
        }
        assert!(!frames.is_empty(), "no Mesen scanouts");
        Self { frames }
    }

    /// Require two consecutive original captures with the complete target
    /// bitmap and an unchanged original scanout. This excludes partially
    /// transferred VRAM and the first capture at a display-buffer boundary.
    /// Native pixels are deliberately not an input to this selector.
    pub fn settled_original_bitmap(&self, bitmap: &[u8]) -> Option<&Scanout> {
        assert_eq!(bitmap.len(), BITMAP_WIDTH * BITMAP_HEIGHT);
        self.frames.windows(2).find_map(|pair| {
            (pair[0].bitmap == bitmap && pair[1].bitmap == bitmap && pair[0].rgb == pair[1].rgb)
                .then_some(&pair[1])
        })
    }
}

fn decode_bitmap(
    vram: &[u8],
    map: usize,
    characters: usize,
    hscroll: usize,
    vscroll: usize,
) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(BITMAP_WIDTH * BITMAP_HEIGHT);
    for y in BITMAP_ORIGIN..BITMAP_ORIGIN + BITMAP_HEIGHT {
        for x in BITMAP_ORIGIN..BITMAP_ORIGIN + BITMAP_WIDTH {
            let sx = (x + hscroll) & 255;
            let sy = (y + vscroll) & 255;
            let address = (map + ((sy / 8) * 32 + sx / 8) * 2) & 65535;
            let entry = u16::from_le_bytes([vram[address], vram[(address + 1) & 65535]]);
            let column = if entry & 0x4000 != 0 {
                7 - sx % 8
            } else {
                sx % 8
            };
            let row = if entry & 0x8000 != 0 {
                7 - sy % 8
            } else {
                sy % 8
            };
            let base = characters + usize::from(entry & 0x03FF) * 32 + row * 2;
            let color = (0..4).fold(0, |value, plane| {
                let byte = vram[(base + (plane / 2) * 16 + plane % 2) & 65535];
                value | (((byte >> (7 - column)) & 1) << plane)
            });
            pixels.push(color);
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_identity_decodes_tiles_flips_and_scroll_without_palette_or_native_pixels() {
        let mut vram = vec![0; 65536];
        // Source row/column 16 selects tilemap (2,2). Tile one has a unique
        // color at its upper-left corner and a different color below/right.
        let map = 8192;
        let address = map + (2 * 32 + 2) * 2;
        vram[address] = 1;
        vram[32] = 128;
        vram[32 + 2 + 1] = 64;
        let plain = decode_bitmap(&vram, map, 0, 0, 0);
        assert_eq!(plain[0], 1);
        assert_eq!(plain[BITMAP_WIDTH + 1], 2);
        assert_eq!(decode_bitmap(&vram, map, 0, 1, 1)[0], 2);
        vram[address + 1] = 0xFC; // Both flips and palette/priority bits.
        let flipped = decode_bitmap(&vram, map, 0, 0, 0);
        assert_eq!(flipped[7 * BITMAP_WIDTH + 7], 1);
        assert_eq!(flipped[6 * BITMAP_WIDTH + 6], 2);
    }

    #[test]
    fn selection_rejects_a_partial_bitmap_and_unsettled_original_scanout() {
        let bitmap = vec![3; BITMAP_WIDTH * BITMAP_HEIGHT];
        let frame = |video_frame, bitmap, rgb| Scanout {
            video_frame,
            bitmap,
            rgb,
        };
        let mut partial = bitmap.clone();
        partial[BITMAP_WIDTH * (BITMAP_HEIGHT - 1)] = 0;
        let mut captures = LaunchVideo {
            frames: vec![
                frame(1, partial, vec![4]),
                frame(2, bitmap.clone(), vec![4]),
                frame(3, bitmap.clone(), vec![5]),
            ],
        };
        assert!(captures.settled_original_bitmap(&bitmap).is_none());
        captures.frames.push(frame(4, bitmap.clone(), vec![5]));
        assert_eq!(
            captures
                .settled_original_bitmap(&bitmap)
                .unwrap()
                .video_frame,
            4
        );
    }
}

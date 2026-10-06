//! Associate original rendered bitmaps with original completed scanouts.
//!
//! No native pixels participate in selection. A gameplay boundary can occur
//! before the preceding bitmap is displayed, particularly during a wipe.

use sf_difftest::{SOURCE_FRAME_HEIGHT, SOURCE_FRAME_WIDTH};
use sf_oracle::{CompletedRaster, RetailMachine};

const BITMAP_WIDTH: usize = 224;
const BITMAP_HEIGHT: usize = 192;
const BITMAP_LEFT: usize = 16;
const BITMAP_TOP: usize = 16;
const COLOR_PLANES: usize = 4;
const TILE_BYTES: usize = 32;
const SCREEN_BASE_UNIT: usize = 1_024;
const INDEX_MASK: u8 = 15;

/// Read the completed original 224-by-192 planar bitmap, before a later
/// transfer can reuse its drawing buffer. Call after `do_3d_display_l` and
/// before the next scene has been rendered.
pub fn original_bitmap(machine: &RetailMachine) -> Vec<u8> {
    let (screen_base, screen_mode, ..) = machine.gsu_screen_state().expect("original GSU state");
    assert_eq!(screen_mode & 3, 1, "original bitmap must be four-plane");
    let layout = ((screen_mode >> 5) & 1) * 2 + ((screen_mode >> 2) & 1);
    assert_eq!(layout, 2, "original bitmap must be 192 rows high");
    (0..BITMAP_HEIGHT)
        .flat_map(|y| {
            (0..BITMAP_WIDTH).map(move |x| {
                let character = (x & !7) * 3 + ((y & !7) >> 3);
                let base = character * TILE_BYTES
                    + usize::from(screen_base) * SCREEN_BASE_UNIT
                    + (y & 7) * 2;
                let bit = 7 - (x & 7);
                (0..COLOR_PLANES).fold(0, |color, plane| {
                    let offset = (plane >> 1) * 16 + (plane & 1);
                    color | (((machine.peek_gsu_ram((base + offset) & 0xFFFF) >> bit) & 1) << plane)
                })
            })
        })
        .collect()
}

/// Require the complete original bitmap, including pixels outside the
/// aperture, to have reached BG1. Partial transfers are not accepted.
pub fn displays_original_bitmap(raster: &CompletedRaster, bitmap: &[u8]) -> bool {
    if raster.bg1_unwindowed_indices.len() != SOURCE_FRAME_WIDTH * SOURCE_FRAME_HEIGHT
        || bitmap.len() != BITMAP_WIDTH * BITMAP_HEIGHT
    {
        return false;
    }
    (0..BITMAP_HEIGHT).all(|y| {
        (0..BITMAP_WIDTH).all(|x| {
            let raw = raster.bg1_unwindowed_indices
                [(y + BITMAP_TOP) * SOURCE_FRAME_WIDTH + x + BITMAP_LEFT];
            let displayed = if raw == u8::MAX { 0 } else { raw & INDEX_MASK };
            bitmap[y * BITMAP_WIDTH + x] == displayed
        })
    })
}

pub fn completed_rgb(raster: &CompletedRaster) -> Vec<u8> {
    assert_eq!(
        raster.rgba.len(),
        SOURCE_FRAME_WIDTH * SOURCE_FRAME_HEIGHT * 4
    );
    raster
        .rgba
        .chunks_exact(4)
        .flat_map(|pixel| pixel[..3].iter().copied())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn association_requires_every_original_pixel_even_outside_the_aperture() {
        let mut raster = CompletedRaster {
            video_frame: 7,
            rgba: vec![0; SOURCE_FRAME_WIDTH * SOURCE_FRAME_HEIGHT * 4],
            bg1_indices: vec![u8::MAX; SOURCE_FRAME_WIDTH * SOURCE_FRAME_HEIGHT],
            bg1_unwindowed_indices: vec![u8::MAX; SOURCE_FRAME_WIDTH * SOURCE_FRAME_HEIGHT],
        };
        let mut bitmap = vec![0; BITMAP_WIDTH * BITMAP_HEIGHT];
        assert!(displays_original_bitmap(&raster, &bitmap));
        for (x, y) in [(0, 0), (223, 0), (0, 191), (223, 191), (112, 96)] {
            let source = y * BITMAP_WIDTH + x;
            let display = (y + BITMAP_TOP) * SOURCE_FRAME_WIDTH + x + BITMAP_LEFT;
            bitmap[source] = 3;
            assert!(!displays_original_bitmap(&raster, &bitmap));
            raster.bg1_unwindowed_indices[display] = 7 * 16 + 3;
            assert!(displays_original_bitmap(&raster, &bitmap));
            // Masked display samples and RGB cannot select a different scene.
            assert_eq!(raster.bg1_indices[display], u8::MAX);
            bitmap[source] = 0;
            raster.bg1_unwindowed_indices[display] = u8::MAX;
        }
        bitmap.pop();
        assert!(!displays_original_bitmap(&raster, &bitmap));
        bitmap.push(0);
        raster.bg1_unwindowed_indices.pop();
        assert!(!displays_original_bitmap(&raster, &bitmap));
    }
}

//! Retail GSU differential for the typed Star Fox 1 aperture representation.

use sf_core::screen_wipe::{ScreenWipeKind, ScreenWipeState, SOURCE_HEIGHT};
use sf_oracle::{gsu::Gsu, load_retail_rom};

const LINE_WALK_BANK: u8 = 1;
const LINE_WALK_ENTRY: u16 = 0xD6E8;
const STOP_RETURN: u16 = 0xD74B;
const EDGE_BUFFER: u16 = 0x1000;
const SOURCE_TO_SCREEN_X: i16 = 16;
const SCREEN_MIDDLE_X: i16 = 112;
const SCREEN_MIDDLE_Y: i16 = 95;
const SCREEN_BOTTOM_Y: i16 = 191;
const SCREEN_RIGHT_X: i16 = 223;
const STAR_X_STEP: i16 = 7;
const STAR_Y_STEP: i16 = 6;

fn draw_retail_line(
    oracle: &mut Gsu,
    start_x: i16,
    start_y: i16,
    end_x: i16,
    end_y: i16,
) {
    oracle.r[1] = start_x as u16;
    oracle.r[2] = start_y as u16;
    oracle.r[3] = end_x as u16;
    oracle.r[4] = end_y as u16;
    oracle.r[7] = EDGE_BUFFER;
    oracle.r[11] = STOP_RETURN;
    oracle.run(LINE_WALK_BANK, LINE_WALK_ENTRY);
    assert!(!oracle.last_run_hit_limit, "retail aperture line walker");
}

fn retail_edge(oracle: &Gsu, row: usize) -> i16 {
    let address = usize::from(EDGE_BUFFER) + row * 2;
    i16::from(oracle.ram[address]) - SOURCE_TO_SCREEN_X
}

#[test]
fn star_aperture_spans_match_retail_line_walker() {
    let Some(rom) = load_retail_rom() else {
        eprintln!("skip: no retail Star Fox 1 ROM");
        return;
    };

    for frame in 0..ScreenWipeKind::StarReveal.frame_count() {
        let distance_x = i16::from(frame) * STAR_X_STEP;
        let distance_y = i16::from(frame) * STAR_Y_STEP;
        let left_x = SCREEN_MIDDLE_X - distance_x;
        let right_x = SCREEN_MIDDLE_X + distance_x;
        let top_y = SCREEN_MIDDLE_Y - distance_y;
        let bottom_y = SCREEN_MIDDLE_Y + distance_y;

        let mut left = Gsu::new(rom.clone());
        let mut right = Gsu::new(rom.clone());
        draw_retail_line(&mut left, SCREEN_MIDDLE_X, 0, left_x, top_y);
        draw_retail_line(&mut right, SCREEN_MIDDLE_X, 0, right_x, top_y);
        draw_retail_line(&mut left, 0, SCREEN_MIDDLE_Y, left_x, top_y);
        draw_retail_line(&mut right, SCREEN_RIGHT_X, SCREEN_MIDDLE_Y, right_x, top_y);
        draw_retail_line(&mut left, 0, SCREEN_MIDDLE_Y, left_x, bottom_y);
        draw_retail_line(
            &mut right,
            SCREEN_RIGHT_X,
            SCREEN_MIDDLE_Y,
            right_x,
            bottom_y,
        );
        draw_retail_line(
            &mut left,
            SCREEN_MIDDLE_X,
            SCREEN_BOTTOM_Y,
            left_x,
            bottom_y,
        );
        draw_retail_line(
            &mut right,
            SCREEN_MIDDLE_X,
            SCREEN_BOTTOM_Y,
            right_x,
            bottom_y,
        );

        let state = ScreenWipeState {
            kind: ScreenWipeKind::StarReveal,
            frame,
            active: true,
        };
        let spans = state.aperture_spans();
        for (row, span) in spans.iter().enumerate().take(SOURCE_HEIGHT) {
            let retail_left = retail_edge(&left, row);
            let retail_right = retail_edge(&right, row);
            let expected = if retail_left < retail_right {
                Some((retail_left as u16, (retail_right + 1) as u16))
            } else {
                None
            };
            assert_eq!(
                span.map(|span| (span.left, span.right_exclusive)),
                expected,
                "star aperture frame {frame}, source row {row}"
            );
        }
    }
}

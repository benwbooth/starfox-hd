//! Tick 115: PRINT* + CLIP_PLOT + PROJECTLOG/PROJLOG + COPY* + PALGOTO +
//! planet DMA256 / draw / moveship.

use sf_game::clip::GameClipWindow;
use sf_game::debug_draw::{
    clip_plot, pal_goto_step, proj_log, project_log, BootCopy, DebugPrint, PlanetScreenDma,
};

#[test]
fn print_family_hex_dec_ab() {
    let mut p = DebugPrint::new();
    p.print_b(0x3C);
    assert_eq!(p.glyphs, vec![0x3, 0xC]);

    p.glyphs.clear();
    p.print_w(0xABCD);
    assert_eq!(p.glyphs, vec![0xA, 0xB, 0xC, 0xD]);

    p.glyphs.clear();
    p.print_bd(105);
    assert_eq!(p.glyphs, vec![1, 0, 5]);

    p.glyphs.clear();
    p.print_bd(7);
    assert_eq!(p.glyphs, vec![7]); // no hundreds/tens

    p.glyphs.clear();
    p.print_bsd(-3);
    assert_eq!(p.glyphs[0], 36); // 26+10 minus
    assert_eq!(p.glyphs[1], 3);

    p.glyphs.clear();
    p.print_ab(3, 0xF0);
    assert_eq!(p.ab_col, 6);
    assert_eq!(p.glyphs, vec![0xF, 0x0]);

    p.glyphs.clear();
    p.print_t("A_B");
    assert_eq!(p.glyphs, vec![b'A', 42, b'B']);
}

#[test]
fn clip_plot_inside_outside() {
    let c = GameClipWindow::game();
    assert!(clip_plot(0, 0, &c));
    assert!(clip_plot(112, 96, &c));
    assert!(!clip_plot(-1, 10, &c));
    assert!(!clip_plot(10, 200, &c));
}

#[test]
fn projectlog_vanish_and_sign() {
    let c = GameClipWindow::game();
    assert_eq!(project_log(0, 0, 200, &c), (112, 96));
    let (xs, _) = project_log(100, 0, 200, &c);
    assert!(xs > 112);
    let (xs2, _) = project_log(-100, 0, 200, &c);
    assert!(xs2 < 112);
    assert_eq!(proj_log(0, 50, 112), 112);
}

#[test]
fn boot_copy_and_palgoto_and_planet_dma() {
    let mut b = BootCopy::default();
    b.copy_chars();
    b.copy_to_0101();
    assert_eq!((b.chars, b.nmi_handler), (1, 1));

    let mut dst = [0u16; 2];
    let src = [0x7FFF, 0x001F];
    let mut fade = 2u8;
    assert!(pal_goto_step(&mut dst, &src, &mut fade, false));
    assert_eq!(fade, 1);
    assert_eq!(dst[0] & 0x1F, 1); // R stepped toward 31
    assert!(!pal_goto_step(&mut dst, &src, &mut fade, true)); // HP0 blocks

    let mut d = PlanetScreenDma::default();
    d.dma256_screen();
    d.dma256_screen_fast();
    d.dma_pepper_screen();
    d.draw_selected_planet();
    d.draw_planet_in_centre();
    d.move_ship_along_path();
    assert_eq!(d.dma256, 1);
    assert_eq!(d.dma256_fast, 1);
    assert_eq!(d.pepper, 1);
    assert_eq!(d.draw_selected, 1);
    assert_eq!(d.draw_centre, 1);
    assert_eq!(d.move_ship, 1);
}

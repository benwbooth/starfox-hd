//! Tick 114: ALLOC/SFREE/SALLOC/SALLFREE/MPUSH/MPULL/AVAIL + DEC_* +
//! FADELINES/FADETAB*/FADECOL0 + P_INIT_SPRITES.

use sf_game::heap::{
    fade_col0, fade_tab0, p_init_sprites, DecRun, DecTarget, FadeLines, StratHeap, FADE_TABLE,
    SP_SIZEOF,
};

#[test]
fn alloc_avail_free_roundtrip() {
    let mut h = StratHeap::new(256);
    assert_eq!(h.avail(), 256);
    assert!(h.alloc(0).is_none()); // ROM rejects zero after round
    let a = h.alloc(10).expect("alloc");
    let left = h.avail();
    assert!(left < 256);
    h.free(a);
    assert_eq!(h.avail(), 256);
    h.free(0); // no-op
}

#[test]
fn salloc_sfree_sallfree_chain() {
    let mut h = StratHeap::new(1024);
    let mut mp = 0u16;
    let a = h.salloc(&mut mp, 8).unwrap();
    let b = h.salloc(&mut mp, 8).unwrap();
    assert_eq!(mp as u32, b);
    // Unlink older block a (not head).
    h.sfree(&mut mp, a);
    assert_eq!(mp as u32, b);
    h.sallfree(&mut mp);
    assert_eq!(mp, 0);
    assert_eq!(h.avail(), 1024);
}

#[test]
fn mpush_mpull_and_smpush() {
    let mut h = StratHeap::new(1024);
    let s0 = h.mpush(None, 0xAABB_CCDD).unwrap();
    let s1 = h.mpush(Some(s0), 0x1122_3344).unwrap();
    let (sp, d) = h.mpull(s1).unwrap();
    assert_eq!(d, 0x1122_3344);
    assert_eq!(sp, Some(s0));

    let mut mp = 0u16;
    let t0 = h.smpush(&mut mp, None, 0x55).unwrap();
    assert_ne!(mp, 0);
    let (sp2, d2) = h.smpull(&mut mp, t0).unwrap();
    assert_eq!(d2, 0x55);
    assert_eq!(sp2, None);
    assert_eq!(mp, 0);
    let _ = SP_SIZEOF;
}

#[test]
fn fadetable_and_fadetab0() {
    assert_eq!(FADE_TABLE.len(), 17);
    assert_eq!(FADE_TABLE[0], 0xFF);
    assert_eq!(FADE_TABLE[16], 0xE1);
    let tab = fade_tab0();
    assert_eq!(tab.len(), 16);
    // First step: scale = 100 - 6 = 94; rgbws(6,8,10,94)
    let r = 6u16 * 94 / 100;
    let g = 8u16 * 94 / 100;
    let b = 10u16 * 94 / 100;
    assert_eq!(tab[0], r | (g << 5) | (b << 10));
    assert_eq!(tab[15], 0); // black
    assert_eq!(fade_col0(0), tab[0]);
    assert_eq!(fade_col0(99), tab[15]); // clamp
}

#[test]
fn dec_and_fadelines_and_p_init() {
    let mut d = DecRun::default();
    d.run(DecTarget::Chr);
    d.run(DecTarget::Bg);
    d.run(DecTarget::Bg3);
    assert_eq!((d.chr, d.bg, d.bg3), (1, 1, 1));

    let mut fl = FadeLines::default();
    fl.fade_lines();
    fl.fade_lines2();
    assert_eq!(fl.calls, 2);

    let mut cleared = 0u32;
    p_init_sprites(&mut || cleared += 1);
    assert_eq!(cleared, 1);
}

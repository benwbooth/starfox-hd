//! ROM `mcalc_circle` Bresenham core (MCIRCLE.MC:67).

use sf_strat::snes_trig::mcalc_circle_edges;

#[test]
fn circle_radius_zero_empty() {
    let mut edges = [None; 224];
    mcalc_circle_edges(100, 100, 0, &mut edges);
    assert!(edges.iter().all(|e| e.is_none()));
}

#[test]
fn circle_radius_one_covers_center_row() {
    let mut edges = [None; 224];
    mcalc_circle_edges(50, 50, 1, &mut edges);
    // Midpoint circle of r=1 touches y=49,50,51.
    assert!(edges[50].is_some());
    let (l, r) = edges[50].unwrap();
    assert!(l <= 50 && r >= 50);
}

#[test]
fn circle_is_symmetric_about_center() {
    let mut edges = [None; 224];
    let (cx, cy, r) = (112i16, 112, 40);
    mcalc_circle_edges(cx, cy, r, &mut edges);
    let mut touched = 0;
    for y in 0..224i16 {
        if let Some((l, r)) = edges[y as usize] {
            touched += 1;
            assert_eq!(
                l.wrapping_add(r),
                cx.wrapping_mul(2),
                "y={y}: left/right not mirrored about cx"
            );
            assert!(r >= l);
        }
    }
    assert!(
        touched > 20,
        "expected a solid band of scanlines, got {touched}"
    );
}

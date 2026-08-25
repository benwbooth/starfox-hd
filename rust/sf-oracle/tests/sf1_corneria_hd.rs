#[path = "../examples/sf1_corneria_hd_probe.rs"]
mod corridor_probe;

#[test]
fn corneria_corridor_has_no_fractional_depth_color_flash() {
    corridor_probe::verify_corneria_depth_transition();
}

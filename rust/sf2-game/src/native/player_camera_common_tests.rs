use super::*;

#[test]
fn orientation_approach_crosses_wrapped_zero_without_overshoot() {
    assert_eq!(approach_angle(65500, 100), 100);
    assert_eq!(approach_angle(100, 65500), 65500);
    assert_eq!(approach_angle(4096, 0), 3840);
    assert_eq!(approach_angle(32768, 1), 32512);
}

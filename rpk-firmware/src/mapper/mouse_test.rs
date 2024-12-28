use super::*;

use crate::time_driver_test_stub;

#[test]
fn delta() {
    let mut now = 1_234;

    let mut m = Mouse::default();

    let kc = key_range::MOUSE_DELTA;

    assert_eq!(m.action(kc, true, now), None);
    now += 50;
    assert_eq!(m.action(kc + 1, true, now), None);
    assert_eq!(m.action(kc, false, now), None);
    assert_eq!(m.first_down_time, 1_233);

    assert_eq!(m.action(kc + 1, false, now), None);
    assert_eq!(m.first_down_time, 0);

    assert_eq!(m.action(kc + 1, false, now), None);
    assert_eq!(m.first_down_time, 0);

    assert_eq!(m.action(kc + 3, true, now), None);
    assert_eq!(m.first_down_time, 1_283);

    time_driver_test_stub::set_time(now + 50_000);
}

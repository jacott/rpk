use rpk_common::mouse::MouseAnalogSetting;

use super::*;

#[test]
fn mouse_to_from_binary() {
    let config = MouseAnalogSetting {
        curve: (0.2, 0.9),
        max_time: 2.1,
        min_ticks_per_ms: 1.5,
        max_ticks_per_ms: 5.4,
    };

    let ans: Vec<u16> = spec::mouse_to_binary(config).collect();
    let c2 = MouseAnalogSetting::deserialize(&mut ans.iter().copied()).unwrap();
    assert_eq!(config.curve, c2.curve);
    assert_eq!(config.max_time, c2.max_time);
    assert_eq!(config.min_ticks_per_ms, c2.min_ticks_per_ms);
    assert_eq!(config.max_ticks_per_ms, c2.max_ticks_per_ms);

    assert_eq!(
        &ans,
        &vec![52429, 15948, 26214, 16230, 26214, 16390, 0, 16320, 52429, 16556]
    );
}

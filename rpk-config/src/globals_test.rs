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
        &vec![
            52429, 15948, 26214, 16230, 26214, 16390, 0, 16320, 52429, 16556
        ]
    );
}

#[test]
fn parse_debounce() {
    let err = spec::parse_key_settle_time("-0.01").err().unwrap();
    assert_eq!(
        err,
        "Invalid duration; only 0 to 25000 milliseconds are valid"
    );

    assert_eq!(spec::parse_key_settle_time("0.1").ok().unwrap(), 3);
    assert_eq!(spec::parse_key_settle_time("20").ok().unwrap(), 525);
    assert_eq!(spec::parse_key_settle_time("25000").ok().unwrap(), 65528);
    assert_eq!(spec::parse_key_settle_time("100").ok().unwrap(), 2622);

    use rpk_common::globals::key_settle_time_uncompress;

    assert_eq!(key_settle_time_uncompress(0), 0);
    assert_eq!(key_settle_time_uncompress(3), 114);
    assert_eq!(key_settle_time_uncompress(525), 20027);
    assert_eq!(key_settle_time_uncompress(65528), 2499726);
    assert_eq!(key_settle_time_uncompress(2622), 100022);
}

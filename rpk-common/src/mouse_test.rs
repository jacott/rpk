use super::*;

#[test]
fn speed_test() {
    let config = MouseConfig {
        movement: MouseAnalogSetting {
            curve: (0.08, 0.5),
            max_time: 2_000.0,
            min_ticks_per_ms: 0.02,
            max_ticks_per_ms: 3.0,
        },
        scroll: MouseAnalogSetting {
            curve: (0.0, 0.0),
            max_time: 5_000.0,
            min_ticks_per_ms: 0.01,
            max_ticks_per_ms: 0.5,
        },
    };

    let time_since_last_report = 32.0;
    let mm = MouseMove {
        start: 5,
        fraction: RefCell::new(0.0),
        delta: -1,
    };
    let ms = MouseMove {
        start: 5,
        fraction: RefCell::new(0.0),
        delta: 1,
    };

    let now = 21;

    assert_eq!(
        config
            .movement
            .mouse_delta(time_since_last_report, now, &mm),
        0
    );

    {
        let fr = mm.fraction.borrow();
        assert!((*fr - 0.8306).abs() < 1e-4, "wrong fraction: {fr:.4}");
    }

    assert_eq!(
        config.scroll.mouse_delta(time_since_last_report, now, &ms),
        0
    );

    let now = 37;

    assert_eq!(
        config
            .movement
            .mouse_delta(time_since_last_report, now, &mm),
        -1
    );

    let now = 100;

    assert_eq!(
        config
            .movement
            .mouse_delta(time_since_last_report, now, &mm),
        -2
    );

    let now = 5000;
    assert_eq!(
        config
            .movement
            .mouse_delta(time_since_last_report, now, &mm),
        -97
    );

    {
        let fr = mm.fraction.borrow();
        assert!((*fr - 0.4568).abs() < 1e-4, "{fr:.4}");
    }
}

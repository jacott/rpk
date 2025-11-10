extern crate std;

use embassy_futures::{
    block_on, join,
    select::{self},
};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use std::vec::Vec;

use super::*;

use crate::switch_test_stub::{KeyMatrix, Pin};
use crate::time_driver_test_stub::{self, set_time, set_wait_lag};

const TIME_PER_OUTPUT_PIN: u64 = 32;

fn from_ms(ms: u64) -> u16 {
    ((ms * 1000) << 10).div_ceil(39063) as u16
}

macro_rules! setup {
    ($scan:ident, $km:ident, $channel:ident, $scanner:ident: $debounce_ms:literal $b:block) => {
        block_on(async move {
            let mut p1 = Pin::new(1);
            let p2 = Pin::new(2);
            let p3 = Pin::new(3);
            let p4 = Pin::new(4);
            p1.set_high().ok();

            let inputs = [p1.clone()];
            let outputs = [p2, p3, p4];
            let $km = KeyMatrix::new(Vec::from(&inputs), Vec::from(&outputs));

            let $channel = KeyScannerChannel::<NoopRawMutex, 16>::default();
            let debounce_ms_atomic = atomic::AtomicU16::new(from_ms($debounce_ms));
            let mut $scanner = KeyScanner::new(inputs, outputs, &$channel, &debounce_ms_atomic);
            #[allow(unused_mut)]
            let mut now = 1000;
            time_driver_test_stub::set_time(now);

            assert_eq!(p1.num(), 1);

            macro_rules! $scan {
                () => {
                    scan!(1)
                };
                ($c:expr, $exp:expr) => {
                    assert_eq!(
                        scan!($c),
                        $exp,
                        "\nwe got:  {:#010b}\nwanted:  {:#010b}",
                        $scanner.state[1][0],
                        $exp
                    )
                };
                ($c:expr) => {{
                    for _ in 0..$c {
                        $scanner.scan::<true>().await;
                    }
                    $scanner.state[1][0]
                }};
            }

            $b
        })
    };
}

#[test]
fn calc_debounce_cycle() {
    setup!(scan, _km, channel, scanner: 4 {
        scanner.calc_debounce_cycle();
        assert_eq!(scanner.scan_free_time, i32::MIN);
        assert_eq!(scanner.debounce_count_max, 0);
        assert_eq!(scanner.scan_count_max, 0);

        set_wait_lag(20);
        scan!(1);
        assert_eq!(scanner.scan_free_time, -4);
        assert_eq!(scanner.time_per_output_pin.as_ticks(), 32);

        scanner.calc_debounce_cycle();
        assert_eq!(scanner.time_per_output_pin.as_ticks(), 64);
        assert_eq!(scanner.debounce_count_max, 15);
        assert_eq!(scanner.scan_count_max, 0);

        scan!(1);
        assert_eq!(scanner.scan_free_time, 12);
        assert_eq!(scanner.time_per_output_pin.as_ticks(), 64);
        scanner.calc_debounce_cycle();
        assert_eq!(scanner.time_per_output_pin.as_ticks(), 64);
        assert_eq!(scanner.debounce_count_max, 15);
        assert_eq!(scanner.scan_count_max, 0);

        scanner.scan_free_time = 33;
        scanner.calc_debounce_cycle();
        assert_eq!(scanner.time_per_output_pin.as_ticks(), 32);
        assert_eq!(scanner.debounce_count_max, 31);
        assert_eq!(scanner.scan_count_max, 0);

        scanner.scan_free_time = 10;
        scanner.debounce_ms_atomic.store(from_ms(100), atomic::Ordering::Relaxed);
        scanner.calc_debounce_cycle();
        assert_eq!(scanner.time_per_output_pin.as_ticks(), 32);
        assert_eq!(scanner.debounce_count_max, 31);
        assert_eq!(scanner.scan_count_max, 31);

        scanner.scan_free_time = -1025;
        scanner.calc_debounce_cycle();
        assert_eq!(scanner.time_per_output_pin.as_ticks(), 2048);
        assert_eq!(scanner.debounce_count_max, 15);
        assert_eq!(scanner.scan_count_max, 0);
    });
}

#[test]
fn calc_debounce_short() {
    setup!(scan, _km, channel, scanner: 4 {
        set_wait_lag(20);
        scan!(3);
        assert_eq!(scanner.time_per_output_pin.as_ticks(), TIME_PER_OUTPUT_PIN);
        assert_eq!(Instant::now().as_ticks(), 1000 + (20 + TIME_PER_OUTPUT_PIN*5/2)*3);

        scanner.calc_debounce_cycle();
        assert_eq!(scanner.scan_free_time, -4);
        assert_eq!(scanner.debounce_count_max, 15);
        assert_eq!(scanner.scan_count_max, 0);

        assert_eq!(scanner.time_per_output_pin.as_ticks(), TIME_PER_OUTPUT_PIN*2);
        scan!(20);
        scanner.calc_debounce_cycle();
        assert_eq!(scanner.time_per_output_pin.as_ticks(), TIME_PER_OUTPUT_PIN*2);
        assert_eq!(scanner.debounce_count_max, 15);
        assert_eq!(scanner.scan_count_max, 0);
    });
}

#[test]
fn calc_debounce_mid() {
    setup!(scan, _km, channel, scanner: 39 {
        set_wait_lag(500);
        assert_eq!(scanner.time_per_output_pin.as_ticks(), TIME_PER_OUTPUT_PIN);

        scan!(3);
        assert_eq!(Instant::now().as_ticks(), 2548);
        assert_eq!(scanner.scan_free_time, -420);

        scanner.calc_debounce_cycle();
        assert_eq!(scanner.debounce_count_max, 15);
        assert_eq!(scanner.scan_count_max, 0);
        assert_eq!(scanner.time_per_output_pin.as_ticks(), TIME_PER_OUTPUT_PIN*16);

        scan!(10);
        scanner.calc_debounce_cycle();
        assert_eq!(scanner.time_per_output_pin.as_ticks(), TIME_PER_OUTPUT_PIN*32);
        scan!(10);
        scanner.calc_debounce_cycle();
        assert_eq!(scanner.time_per_output_pin.as_ticks(), TIME_PER_OUTPUT_PIN*32);
        assert_eq!(scanner.debounce_count_max, 7);
        assert_eq!(scanner.scan_count_max, 0);
    });
}

#[test]
fn calc_debounce_high() {
    setup!(scan, _km, channel, scanner: 382 {
        set_wait_lag(100);
        assert_eq!(scanner.time_per_output_pin.as_ticks(), TIME_PER_OUTPUT_PIN);

        scan!(3);
        assert_eq!(Instant::now().as_ticks(), 1348);

        scanner.calc_debounce_cycle();
        assert_eq!(scanner.scan_free_time, -20);
        assert_eq!(scanner.debounce_count_max, 31);
        assert_eq!(scanner.scan_count_max, 61);

        scan!(10);
        scanner.calc_debounce_cycle();
        assert_eq!(scanner.time_per_output_pin.as_ticks(), TIME_PER_OUTPUT_PIN*4);
        scan!(10);
        scanner.calc_debounce_cycle();
        assert_eq!(scanner.time_per_output_pin.as_ticks(), TIME_PER_OUTPUT_PIN*8);
        scan!(10);
        scanner.calc_debounce_cycle();
        assert_eq!(scanner.time_per_output_pin.as_ticks(), TIME_PER_OUTPUT_PIN*8);
        assert_eq!(scanner.debounce_count_max, 31);
        assert_eq!(scanner.scan_count_max, 14);
    });
}

#[test]
fn debounce() {
    setup!(scan, km, channel, scanner: 25 {
        scanner.debounce_count_max = 31;
        scanner.scan_count_max = 3;
        scan!(1, 0);
        assert_eq!(scanner.scan_count, 1);
        km.down(0, 1);
        scan!(1, 255);
        assert_eq!(channel.0.try_receive().unwrap(), ScanKey::new(1, 0, true));

        scan!(1, 255);
        assert!(channel.0.try_receive().is_err());
        assert_eq!(scanner.scan_count, 3);

        scan!(10, 255);
        assert_eq!(scanner.scan_count, 1);

        km.up(0, 1);
        scan!(100, 138);
        km.down(0, 1);
        km.down(0, 2);
        scan!(3, 239);
        assert_eq!(channel.0.try_receive().unwrap(), ScanKey::new(2, 0, true));

        km.up(0, 1);
        scan!(123, 242);
        assert!(channel.0.try_receive().is_err());

        scan!(1, 2);
        assert_eq!(channel.0.try_receive().unwrap(), ScanKey::new(1, 0, false));
    });
}

#[test]
fn wait_for_key() {
    setup!(_pscan, km, channel, scanner: 5 {
        set_time(0);
        scanner.scan_count = 3;

        let ordering = Channel::<NoopRawMutex, &'static str, 40>::new();
        let o1 = &ordering;
        let o2 = &ordering;
        let wf = async move {
            o1.try_send("wf0").unwrap();
            scanner.wait_for_key().await;
            o1.try_send("wf1").unwrap();
            scanner
        };
        let sf = async move {
            o2.receive().await;
            Timer::after_micros(10).await;
            km.down(0, 1);
            o2.receive().await;
            true
        };
        let ans = select::select(join::join(wf,sf), Timer::after_millis(20)).await;
        let select::Either::First((mut scanner, true)) = ans else {
            panic!("Expected scanner");
        };

        assert!(scanner.output_pins.iter_mut().all(|p| p.is_high().unwrap()));
        assert!(!scanner.all_up);
    });
}

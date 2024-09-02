extern crate std;
use core::{cell::RefCell, task::Waker};
use embassy_time_driver::{AlarmHandle, Driver};
use embassy_time_queue_driver::TimerQueue;
use std::time::SystemTime;

pub mod usb_test_stub;

#[cfg(feature = "defmt")]
#[defmt::global_logger]
struct Logger;

//struct MyDecoder {}

//static DECODER: Mutex<Option<MyDecoder>> = Mutex::new(None);

#[cfg(feature = "defmt")]
unsafe impl defmt::Logger for Logger {
    fn acquire() {}

    unsafe fn release() {}

    unsafe fn write(_bytes: &[u8]) {}

    unsafe fn flush() {}
}

pub(crate) struct TestTimeDriver;

impl Driver for TestTimeDriver {
    fn now(&self) -> u64 {
        NOW.with_borrow(|now| {
            if *now == 0 {
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_micros() as u64
            } else {
                *now
            }
        })
    }
    unsafe fn allocate_alarm(&self) -> Option<AlarmHandle> {
        std::unimplemented!()
    }
    fn set_alarm_callback(&self, alarm: AlarmHandle, callback: fn(*mut ()), ctx: *mut ()) {
        let _ = (alarm, callback, ctx);
        std::unimplemented!()
    }
    fn set_alarm(&self, alarm: AlarmHandle, timestamp: u64) -> bool {
        let _ = (alarm, timestamp);
        std::unimplemented!()
    }
}

impl TestTimeDriver {
    pub(crate) fn set_time(&self, t: u64) {
        NOW.with_borrow_mut(|now| *now = t);
    }
}

std::thread_local! {
    static NOW: RefCell<u64> = const {RefCell::new(0)};
}

// TODO this needs to be thread local
embassy_time_driver::time_driver_impl!(static TIME_DRIVER: TestTimeDriver = TestTimeDriver);

pub fn set_time(t: u64) {
    TIME_DRIVER.set_time(t);
}

struct MyTimerQueue; // not public!

impl TimerQueue for MyTimerQueue {
    fn schedule_wake(&'static self, _at: u64, waker: &Waker) {
        let waker = waker.clone();
        waker.wake();
    }
}

embassy_time_queue_driver::timer_queue_impl!(static QUEUE: MyTimerQueue = MyTimerQueue);

#[cfg(all(not(test), feature = "defmt"))]
#[defmt::panic_handler] // defmt's attribute
fn defmt_panic() -> ! {
    // leave out the printing part here
    std::unimplemented!()
}

#[test]
fn key_bits() {
    let mut bits = [0; crate::KEY_BITS_SIZE];
    assert!(crate::add_key_bit(&mut bits, 0x06));
    assert!(crate::add_key_bit(&mut bits, 0x07));
    assert!(!crate::add_key_bit(&mut bits, 0x07));
    assert!(crate::add_key_bit(&mut bits, 0x08));
    assert!(crate::add_key_bit(&mut bits, 0xe7));
    assert_eq!(&bits[..3], &[0b1100_0000, 0x01, 0]);
    assert_eq!(bits[crate::KEY_BITS_SIZE - 4], 128);

    assert!(crate::del_key_bit(&mut bits, 0x07));
    assert_eq!(&bits[..3], &[0b0100_0000, 0x01, 0]);
    assert!(!crate::del_key_bit(&mut bits, 0x07));
}

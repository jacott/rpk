extern crate std;

use core::{cell::RefCell, task::Waker};
use embassy_time_driver::Driver;
use std::time::SystemTime;

struct TestTimeDriver;

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

    fn schedule_wake(&self, at: u64, waker: &Waker) {
        set_time(at);

        let waker = waker.clone();
        waker.wake();
    }
}

impl TestTimeDriver {
    pub fn set_time(&self, t: u64) {
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

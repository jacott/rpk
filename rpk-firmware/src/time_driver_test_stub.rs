extern crate std;

use core::{cell::RefCell, task::Waker};
use embassy_time_driver::Driver;
use std::time::SystemTime;

struct TestTimeDriver;

impl Driver for TestTimeDriver {
    fn now(&self) -> u64 {
        NOW.with_borrow(|now| {
            if now.0 == 0 {
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_micros() as u64
            } else {
                now.0
            }
        })
    }

    fn schedule_wake(&self, at: u64, waker: &Waker) {
        NOW.with_borrow_mut(|now| {
            if now.0 != 0 && at > now.0 {
                now.0 = at + now.1;
            }
        });

        waker.wake_by_ref();
    }
}

std::thread_local! {
    static NOW: RefCell<(u64,u64)> = const {RefCell::new((0,0))};
}

// TODO this needs to be thread local
embassy_time_driver::time_driver_impl!(static TIME_DRIVER: TestTimeDriver = TestTimeDriver);

pub fn set_time(t: u64) {
    NOW.with_borrow_mut(|now| now.0 = t);
}

pub fn set_wait_lag(t: u64) {
    NOW.with_borrow_mut(|now| now.1 = t);
}

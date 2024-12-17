//! Functions specific to the firmware.

use core::cell::RefCell;

use embassy_sync::blocking_mutex::CriticalSectionMutex;

pub type ResetFn = &'static (dyn Fn() + Sync);

struct Functions {
    reset: Option<ResetFn>,
    reset_to_usb_boot: Option<ResetFn>,
}

const fn default_functions() -> Functions {
    Functions {
        reset: None,
        reset_to_usb_boot: None,
    }
}

static FUNCTIONS: CriticalSectionMutex<RefCell<Functions>> =
    CriticalSectionMutex::new(RefCell::new(default_functions()));

pub fn reset() {
    FUNCTIONS.lock(|r| {
        let mut guard = r.borrow_mut();
        if let Some(f) = guard.reset.take() {
            f();
        }
    });
}

pub fn reset_to_usb_boot() {
    FUNCTIONS.lock(|r| {
        let mut guard = r.borrow_mut();
        if let Some(f) = guard.reset_to_usb_boot.take() {
            f();
        }
    });
}

/// Register a function that will reset the MCU when requested [reset] is called.
///
/// ```
/// use rpk_firmware::firmware_functions::handle_reset;
/// # pub mod cortex_m { pub mod peripheral {pub mod SCB {pub fn sys_reset() {}}}}
///
/// fn myreset() {
///     cortex_m::peripheral::SCB::sys_reset();
/// }
///
/// handle_reset(Some(&myreset));
/// ```
pub fn handle_reset(value: Option<ResetFn>) {
    FUNCTIONS.lock(|r| {
        let mut guard = r.borrow_mut();
        guard.reset = value;
    });
}

pub fn handle_reset_to_usb_boot(value: Option<ResetFn>) {
    FUNCTIONS.lock(|r| {
        let mut guard = r.borrow_mut();
        guard.reset_to_usb_boot = value;
    });
}

#[cfg(all(not(test), feature = "reset-on-panic", target_os = "none"))]
mod panic {
    #[panic_handler]
    fn panic(_info: &core::panic::PanicInfo) -> ! {
        super::reset();

        loop {}
    }
}

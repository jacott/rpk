#![no_std]

#[cfg(feature = "rp")]
pub mod rp;

#[cfg(feature = "defmt")]
use defmt_rtt as _;

pub mod usb;

pub use embassy_sync::blocking_mutex::raw::NoopRawMutex;
pub use rpk_firmware::{
    config, debug, firmware_functions, fixme, info, key_reporter, key_scanner, mapper,
    norflash_ring_fs, ring_fs, usb::Configurator as UsbConfigurator, usb::State as UsbState,
    usb::UsbBuffers,
};
pub use rpk_macros::configure_keyboard;
pub use static_cell::StaticCell;

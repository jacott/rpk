#![no_std]
pub mod config;
pub mod firmware_functions;
pub mod hid;
pub mod key_reporter;
pub mod key_scanner;
pub mod layout;
pub mod mapper;
pub mod norflash_ring_fs;
pub mod ring_fs;
pub mod usb;

#[cfg(feature = "test-utils")]
pub mod flash_test_stub;
#[cfg(feature = "test-utils")]
pub mod switch_test_stub;
#[cfg(feature = "test-utils")]
pub mod time_driver_test_stub;
#[cfg(feature = "test-utils")]
pub mod usb_test_stub;

#[macro_use]
mod macros;

pub(crate) const KEY_BITS_SIZE: usize = 32;

fn add_bit<const SIZE: usize>(keys_down: &mut [u8], kc: u8) -> bool {
    let i = (kc >> 3) as usize;
    if i > SIZE {
        crate::error!("invalid key! {}", kc);
        return false;
    }
    let bp = 1 << (kc & 7);
    let old = keys_down[i];
    keys_down[i] |= bp;
    old & bp == 0
}

fn del_bit<const SIZE: usize>(keys_down: &mut [u8], kc: u8) -> bool {
    let i = (kc >> 3) as usize;
    if i > SIZE {
        crate::error!("invalid key! {}", kc);
        return false;
    }
    let bp = !(1 << (kc & 7));
    let old = keys_down[i];
    keys_down[i] &= bp;
    old | bp == 0xff
}

fn add_key_bit(keys_down: &mut [u8], kc: u8) -> bool {
    add_bit::<KEY_BITS_SIZE>(keys_down, kc)
}

fn del_key_bit(keys_down: &mut [u8], kc: u8) -> bool {
    del_bit::<KEY_BITS_SIZE>(keys_down, kc)
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod test;

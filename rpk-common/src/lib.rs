//! Common functions and values shared between [`rpk-config`](../rpk-config) and
//! [`rpk-firmware`](../rpk-firmware) crates.

#![no_std]
pub mod globals;
pub mod keycodes;
pub mod math;
pub mod mouse;
pub mod usb_vendor_message;

/// The protocol version of a keyboard configuration object.
///
/// This constant defines the protocol version used by keyboard configuration objects generated by the
/// [`rpk-config`](../rpk-config) tool.
/// It's essential for compatibility between different versions of the tool and its consumers.
pub const PROTOCOL_VERSION: u16 = 1;

///
/// This function takes two 16-bit unsigned integers, `n1` and `n2`, and combines them into a 32-bit floating-point number.
/// The bytes of `n1` and `n2` are interleaved and interpreted as a little-endian 32-bit floating-point number.
///
/// # Example
///
/// ```rust
/// let f = f32_from_u16(0x4120, 0x0000);
/// assert_eq!(f, 10.0);
/// ```
pub fn f32_from_u16(n1: u16, n2: u16) -> f32 {
    f32::from_le_bytes([n1 as u8, (n1 >> 8) as u8, n2 as u8, (n2 >> 8) as u8])
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod test;
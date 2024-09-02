#![no_std]

#[cfg(feature = "rp")]
pub mod rp {
    pub use embassy_rp::*;
}

#[cfg(feature = "defmt")]
use defmt_rtt as _;

pub use rpk_firmware::*;
pub use rpk_macros::*;
pub use static_cell::StaticCell;

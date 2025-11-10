use crate::mouse::MouseConfig;

pub enum GlobalProps {
    Usize(usize),
    MouseConfig(MouseConfig),
}

pub const MOUSE_PROFILE1: u16 = 0;
pub const MOUSE_PROFILE2: u16 = 1;
pub const MOUSE_PROFILE3: u16 = 2;

pub const DUAL_ACTION_TIMEOUT: u16 = 3;
pub const DUAL_ACTION_TIMEOUT2: u16 = 4;
pub const DEBOUNCE_SETTLE_TIME: u16 = 5;
pub const TAPDANCE_TAP_TIMEOUT: u16 = 6;
pub const LAST_TIMEOUT: u16 = 6;

pub const DUAL_ACTION_TIMEOUT_DEFAULT: u16 = 180; // 180ms
pub const DUAL_ACTION_TIMEOUT2_DEFAULT: u16 = 20; // 20ms
pub const DEBOUNCE_SETTLE_TIME_DEFAULT: u16 = (20.0 * 65535.0 / 2500.0) as u16; // 20.0 ms
pub const TAPDANCE_TAP_TIMEOUT_DEFAULT: u16 = 180; // 180ms

pub const COMPOSITE_BIT: u16 = 0x0100;
pub const COMPOSITE_PART_BIT: u16 = 0x0200;

/// Uncompress a key settle time which was compressed using [`rpk_config::globals::parse_key_settle_time`].
/// See [`rpk_config::globals::test`] for tests.
#[inline]
pub fn key_settle_time_uncompress(m16: u32) -> u32 {
    (m16 * 39063) >> 10
}

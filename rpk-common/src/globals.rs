use crate::mouse::MouseConfig;

pub enum GlobalProps {
    Usize(usize),
    MouseConfig(MouseConfig),
}

pub const DUAL_ACTION_TIMEOUT: u16 = 0;
pub const DUAL_ACTION_TIMEOUT2: u16 = 1;
pub const DUAL_ACTION_TIMEOUT_DEFAULT: u16 = 180; // 180ms
pub const DUAL_ACTION_TIMEOUT2_DEFAULT: u16 = 20; // 20ms

pub const DEBOUNCE_SETTLE_TIME: u16 = 2;
pub const DEBOUNCE_SETTLE_TIME_DEFAULT: u16 = 200; // 20.0 ms

pub const MOUSE_PROFILE1: u16 = 3;
pub const MOUSE_PROFILE2: u16 = 4;
pub const MOUSE_PROFILE3: u16 = 5;

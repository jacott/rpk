pub mod key_range {
    pub const MAX_LAYER_N: u16 = 0xff;

    pub const LAYER: u16 = 0x600;
    pub const TOGGLE: u16 = 0x700;
    pub const SET_LAYOUT: u16 = 0x800;
    pub const ONESHOT: u16 = 0x900;
    pub const REPLACE_LAYERS: u16 = 0xa00;
    pub const LAYERS_LAST: u16 = REPLACE_LAYERS + MAX_LAYER_N;

    pub const BASIC_MIN: u16 = 0x4;
    pub const BASIC_A: u16 = 0x4;
    pub const BASIC_1: u16 = 0x1e;
    pub const BASIC_0: u16 = 0x27;
    pub const BASIC_MAX: u16 = 0xfe;
    pub const MODIFIER_MIN: u16 = 0xe0;
    pub const MODIFIER_MAX: u16 = 0xe7;
    pub const CONSUMER_MIN: u16 = 0x100;
    pub const CONSUMER_MAX: u16 = 0x3a0;
    pub const SYS_CTL_MIN: u16 = 0x3a1;
    pub const SYS_CTL_MAX: u16 = 0x3d5;
    pub const MOUSE_MIN: u16 = 0x400;
    pub const MOUSE_MAX: u16 = MOUSE_MIN + 0xff;

    pub const LAYER_MIN: u16 = LAYER;
    pub const LAYER_MAX: u16 = LAYER_MIN + MAX_LAYER_N;
    pub const TOGGLE_MIN: u16 = TOGGLE;
    pub const TOGGLE_MAX: u16 = TOGGLE_MIN + MAX_LAYER_N;
    pub const SET_LAYOUT_MIN: u16 = SET_LAYOUT;
    pub const SET_LAYOUT_MAX: u16 = SET_LAYOUT_MIN + MAX_LAYER_N;
    pub const ONESHOT_MIN: u16 = ONESHOT;
    pub const ONESHOT_MAX: u16 = ONESHOT_MIN + MAX_LAYER_N;
    pub const REPLACE_LAYERS_MIN: u16 = REPLACE_LAYERS;
    pub const REPLACE_LAXERS_MAX: u16 = REPLACE_LAYERS_MIN + MAX_LAYER_N;

    pub const MACROS_MIN: u16 = 0x1000;
    pub const MACROS_MAX: u16 = 0x1fff;

    pub const FIRMWARE_MIN: u16 = MACROS_MAX + 1;
    pub const FIRMWARE_MAX: u16 = FIRMWARE_MIN + 0xff;

    pub const FW_RESET_KEYBOARD: u16 = FIRMWARE_MIN;
    pub const FW_RESET_TO_USB_BOOT: u16 = FIRMWARE_MIN + 1;
    pub const FW_CLEAR_ALL: u16 = FIRMWARE_MIN + 2;
    pub const FW_CLEAR_LAYERS: u16 = FIRMWARE_MIN + 3;
    pub const FW_STOP_ACTIVE: u16 = FIRMWARE_MIN + 4;

    pub const MOUSE_BUTTON: u16 = 0;
    pub const MOUSE_BUTTON_END: u16 = 7;
    pub const MOUSE_DELTA: u16 = MOUSE_BUTTON_END + 1;
    pub const MOUSE_DELTA_END: u16 = MOUSE_DELTA + 7;
    pub const MOUSE_ACCEL: u16 = MOUSE_DELTA_END + 1;
    pub const MOUSE_ACCEL_END: u16 = MOUSE_ACCEL + 2;

    pub const SYS_CTL_BASE: u16 = 0x81;

    pub fn base_code(code: u16) -> u16 {
        code & 0xff00
    }
}

pub mod macro_types {
    pub const MODIFIER: u16 = 0;
    pub const DUAL_ACTION: u16 = 1;
    pub const TAP: u16 = 2;
    pub const HOLD_RELEASE: u16 = 3;
    pub const HOLD: u16 = 4;
    pub const RELEASE: u16 = 5;
    pub const DELAY: u16 = 6;
}

pub const OPEN_SAVE_CONFIG: u8 = 1;
pub const CLOSE_SAVE_CONFIG: u8 = 2;
pub const RESET_KEYBOARD: u8 = 3;
pub const RESET_TO_USB_BOOT: u8 = 4;
pub const READ_FILE_BY_INDEX: u8 = 5;
pub const FETCH_STATS: u8 = 6;
pub const SCAN_KEYS: u8 = 7;

/// the maximum allowed size of a usb bulk message.
pub const MAX_BULK_LEN: u16 = 64;

pub mod host_recv {
    pub const FILE_INFO: u8 = 0;
    pub const STATS: u8 = 1;
    pub const KEY_SCAN: u8 = 2;
}

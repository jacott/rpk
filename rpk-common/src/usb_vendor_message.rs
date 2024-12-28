pub const OPEN_SAVE_CONFIG: u8 = 1;
pub const CLOSE_SAVE_CONFIG: u8 = 2;
pub const RESET_KEYBOARD: u8 = 3;
pub const RESET_TO_USB_BOOT: u8 = 4;
pub const READ_FILE_BY_INDEX: u8 = 5;

/// the maximum allowed size of a usb bulk message.
pub const MAX_BULK_LEN: u16 = 64;

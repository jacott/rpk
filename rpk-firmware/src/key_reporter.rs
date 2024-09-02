use embassy_time::Timer;
use embassy_usb::driver::Driver;
use rpk_common::keycodes::key_range::{MODIFIER_MAX, MODIFIER_MIN};

use crate::{add_key_bit, del_key_bit, hid::HidWriter, transformer::KeyEvent, warn};

pub struct Reporter<'d, D: Driver<'d>, const DESC_SIZE: usize> {
    hid_writer: HidWriter<'d, D, DESC_SIZE>,
    keyboard_report: [u8; crate::KEY_BITS_SIZE + 2],
}

impl<'d, D: Driver<'d>, const DESC_SIZE: usize> Reporter<'d, D, DESC_SIZE> {
    pub fn new(hid_writer: HidWriter<'d, D, DESC_SIZE>) -> Self {
        let mut keyboard_report = [0; crate::KEY_BITS_SIZE + 2];
        keyboard_report[0] = 6;
        Self {
            hid_writer,
            keyboard_report,
        }
    }

    async fn write_report(&mut self, report: &[u8]) {
        if let Err(e) = self.hid_writer.write(report).await {
            warn!("Failed to send report: {:?}", e);
        }
    }

    async fn write_keyboard_report(&mut self) {
        if let Err(e) = self.hid_writer.write(&self.keyboard_report).await {
            warn!("Failed to send report: {:?}", e);
        }
    }

    fn add_modifiers(&mut self, modifiers: u8) {
        self.keyboard_report[1] |= modifiers;
    }

    fn remove_modifiers(&mut self, modifiers: u8) {
        self.keyboard_report[1] &= !modifiers;
    }

    pub async fn report(&mut self, msg: KeyEvent) {
        match msg {
            KeyEvent::Basic(key, is_down) => {
                if is_down {
                    if !self.add_key(key) {
                        self.remove_key(key);
                        self.write_keyboard_report().await;
                        self.add_key(key);
                    }
                } else {
                    self.remove_key(key)
                };

                self.write_keyboard_report().await;
            }
            KeyEvent::Modifiers(modifiers, is_down) => {
                if is_down {
                    self.add_modifiers(modifiers);
                } else {
                    self.remove_modifiers(modifiers);
                }
                self.write_keyboard_report().await;
            }
            KeyEvent::PendingModifiers(modifiers, is_down) => {
                if is_down {
                    self.add_modifiers(modifiers);
                } else {
                    self.remove_modifiers(modifiers);
                }
            }
            KeyEvent::Consumer(key) => {
                self.write_report(&[4, (key & 0xff) as u8, (key >> 8) as u8])
                    .await;
            }
            KeyEvent::SysCtl(key) => {
                self.write_report(&[3, (key & 0xff) as u8, (key >> 8) as u8])
                    .await;
            }
            KeyEvent::MouseButton(byte) => {
                self.write_report(&[2, byte]).await;
            }
            KeyEvent::MouseMove(key, value, keys) => {
                let mut mouse_report = [2, keys, 0, 0, 0, 0];
                mouse_report[2 + key as usize] = value;
                self.write_report(&mouse_report).await;
            }
            KeyEvent::Pending => {
                self.write_keyboard_report().await;
            }
            KeyEvent::Clear => {
                self.keyboard_report.iter_mut().skip(1).for_each(|b| *b = 0);
                self.write_report(&[2, 0, 0, 0, 0, 0]).await;
                self.write_report(&[3, 0, 0]).await;
                self.write_report(&[4, 0, 0]).await;
                self.write_keyboard_report().await;
            }
            KeyEvent::Delay(n) => Timer::after_millis(n.into()).await,
        }
    }

    fn add_key(&mut self, key: u8) -> bool {
        if key >= MODIFIER_MIN as u8 && key <= MODIFIER_MAX as u8 {
            let bit = 1 << (key - 0xe0);
            self.keyboard_report[1] |= bit;
            return true;
        }
        if key > 3 {
            add_key_bit(&mut self.keyboard_report[2..], key)
        } else {
            true
        }
    }

    fn remove_key(&mut self, key: u8) {
        if key >= MODIFIER_MIN as u8 && key <= MODIFIER_MAX as u8 {
            let bit = !(1 << (key - 0xe0));
            self.keyboard_report[1] &= bit;
            return;
        }
        if key > 3 {
            del_key_bit(&mut self.keyboard_report[2..], key);
        }
    }
}

#[cfg(test)]
#[path = "key_reporter_test.rs"]
mod test;

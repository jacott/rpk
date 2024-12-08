use crate::{
    firmware_functions,
    ring_fs::{RingFs, RingFsReader, RingFsWriter},
    mapper,
};
use rpk_common::usb_vendor_message::{
    CLOSE_SAVE_CONFIG, OPEN_SAVE_CONFIG, RESET_KEYBOARD, RESET_TO_USB_BOOT,
};

enum ReceiveState {
    Idle,
    ConfigData,
}

pub struct ConfigInterface<'f, 'c> {
    fs: &'f dyn RingFs<'f>,
    mapper_ctl: &'c mapper::ControlSignal,
    fw: Option<RingFsWriter<'f>>,
    rcv_state: ReceiveState,
}

impl<'f, 'c> ConfigInterface<'f, 'c> {
    pub fn new(fs: &'f dyn RingFs<'f>, mapper_ctl: &'c mapper::ControlSignal) -> Self {
        Self {
            fs,
            mapper_ctl,
            fw: None,
            rcv_state: ReceiveState::Idle,
        }
    }

    pub async fn receive(&mut self, data: &[u8]) {
        match self.rcv_state {
            ReceiveState::Idle => match *data.first().unwrap_or(&0) {
                OPEN_SAVE_CONFIG if data.len() == 1 => {
                    self.rcv_state = ReceiveState::ConfigData;
                }
                RESET_KEYBOARD if data.len() == 1 => {
                    firmware_functions::reset();
                }
                RESET_TO_USB_BOOT if data.len() == 1 => {
                    firmware_functions::reset_to_usb_boot();
                }
                n => {
                    crate::error!("Unexpected msg [{}; {}]", n, data.len())
                }
            },
            ReceiveState::ConfigData => match *data.first().unwrap_or(&0) {
                _ if data.len() == 64 => {
                    self.file_write(data);
                }
                OPEN_SAVE_CONFIG if data.len() == 1 => {
                    self.rcv_state = ReceiveState::ConfigData;
                }
                CLOSE_SAVE_CONFIG => {
                    let data = data.split_at(1).1;
                    self.file_write(data);
                    if let Some(fw) = self.fw.take() {
                        self.mapper_ctl.load_layout(fw.location());
                    }
                    self.rcv_state = ReceiveState::Idle;
                }
                n => {
                    crate::error!("Unexpected msg [{}; {}]", n, data.len());
                    self.rcv_state = ReceiveState::Idle;
                }
            },
        }
    }

    fn file_write(&mut self, data: &[u8]) {
        if self.fw.is_none() {
            match self.fs.create_file() {
                Ok(fw) => self.fw = Some(fw),
                Err(_) => {
                    crate::info!("can't create file");
                    return;
                }
            }
        }
        if let Some(ref mut fw) = &mut self.fw {
            if let Err(err) = fw.write(data) {
                crate::info!("write failed {:?}", err);
            }
        }
    }
}

pub struct ConfigFileIter<'f>(RingFsReader<'f>);

impl Iterator for ConfigFileIter<'_> {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        let mut bytes = [0; 2];
        match self.0.read(&mut bytes) {
            Ok(2) => Some(u16::from_le_bytes(bytes)),
            _ => None,
        }
    }
}

impl<'f> ConfigFileIter<'f> {
    pub fn new(reader: RingFsReader<'f>) -> Self {
        Self(reader)
    }
}

#[cfg(test)]
#[path = "config_test.rs"]
mod test;

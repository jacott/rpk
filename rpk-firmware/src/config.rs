use crate::{
    firmware_functions, mapper,
    ring_fs::{RingFs, RingFsReader, RingFsWriter},
};
use rpk_common::usb_vendor_message::{self as msg, MAX_BULK_LEN};

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

    pub async fn receive(&mut self, data: &[u8], write_buf: &mut [u8]) -> usize {
        match self.rcv_state {
            ReceiveState::Idle => match *data.first().unwrap_or(&0) {
                msg::OPEN_SAVE_CONFIG if data.len() == 1 => {
                    self.rcv_state = ReceiveState::ConfigData;
                }
                msg::RESET_KEYBOARD if data.len() == 1 => {
                    firmware_functions::reset();
                }
                msg::RESET_TO_USB_BOOT if data.len() == 1 => {
                    firmware_functions::reset_to_usb_boot();
                }
                msg::READ_FILE_BY_INDEX if data.len() == 5 => {
                    if let Ok(mut fr) = self
                        .fs
                        .file_reader_by_index(u32::from_le_bytes(data[1..].try_into().unwrap()))
                    {
                        write_buf[..4].copy_from_slice(&fr.location().to_le_bytes());

                        if let Ok(n) = fr.read(&mut write_buf[4..]) {
                            return n as usize + 4;
                        }
                    }

                    write_buf[0] = 0;
                    return 1;
                }
                n => {
                    crate::error!("Unexpected msg [{}; {}]", n, data.len())
                }
            },
            ReceiveState::ConfigData => match *data.first().unwrap_or(&0) {
                _ if data.len() == MAX_BULK_LEN as usize => {
                    self.file_write(data);
                }
                msg::OPEN_SAVE_CONFIG if data.len() == 1 => {
                    self.rcv_state = ReceiveState::ConfigData;
                }
                msg::CLOSE_SAVE_CONFIG => {
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
        0
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
    pub fn new(mut reader: RingFsReader<'f>) -> Self {
        reader.seek(13);
        let mut buf = [0];
        reader.read(&mut buf).ok(); // filename length
        reader.seek(14 + buf[0] as u32);
        Self(reader)
    }
}

#[cfg(test)]
#[path = "config_test.rs"]
mod test;

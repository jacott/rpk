use crate::{
    firmware_functions, mapper,
    ring_fs::{RingFs, RingFsReader, RingFsWriter},
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel};
use embassy_time::Instant;
use rpk_common::usb_vendor_message::{self as msg, MAX_BULK_LEN, host_recv};

enum ReceiveState {
    Idle,
    ConfigData,
}

const MSG_LEN: usize = MAX_BULK_LEN as usize;

pub struct HostMessage {
    len: usize,
    data: [u8; MSG_LEN],
}
impl HostMessage {
    pub fn file_info() -> Self {
        Self {
            len: 0,
            data: [host_recv::FILE_INFO; MSG_LEN],
        }
    }

    pub fn stats(time: u32) -> Self {
        let time = time.to_le_bytes();
        let mut time = time.iter();
        let data = core::array::from_fn(|i| match i {
            0 => host_recv::STATS,
            1..5 => *time.next().unwrap(),
            _ => 0,
        });
        Self { len: 4, data }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data[..(self.len + 1)]
    }

    pub fn key_scan() -> Self {
        let mut data = [0; MSG_LEN];
        data[0] = host_recv::KEY_SCAN;
        Self { len: 3, data }
    }

    pub fn set_key(&mut self, memo_bytes: (u8, u8)) {
        self.data[1] = memo_bytes.0;
        self.data[2] = memo_bytes.1;
    }
}

pub struct HostChannel<const N: usize>(Channel<NoopRawMutex, HostMessage, N>);
impl<const N: usize> Default for HostChannel<N> {
    fn default() -> Self {
        Self(Channel::new())
    }
}

impl<const N: usize> HostChannel<N> {
    pub async fn receive(&self) -> HostMessage {
        self.0.receive().await
    }
}

pub struct ConfigInterface<'f, 'c, const N: usize> {
    fs: &'f dyn RingFs<'f>,
    mapper_ctl: &'c mapper::ControlSignal,
    fw: Option<RingFsWriter<'f>>,
    rcv_state: ReceiveState,
    pub host_channel: &'c HostChannel<N>,
}

impl<'f, 'c, const N: usize> ConfigInterface<'f, 'c, N> {
    pub fn new(
        fs: &'f dyn RingFs<'f>,
        mapper_ctl: &'c mapper::ControlSignal,
        host_channel: &'c HostChannel<N>,
    ) -> Self {
        Self {
            fs,
            mapper_ctl,
            fw: None,
            rcv_state: ReceiveState::Idle,
            host_channel,
        }
    }

    pub async fn receive(&mut self, data: &[u8]) {
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
                        let mut info = HostMessage::file_info();
                        info.data[1..5].copy_from_slice(&fr.location().to_le_bytes());

                        if let Ok(n) = fr.read(&mut info.data[5..]) {
                            info.len = n as usize + 4;
                            self.host_channel.0.send(info).await;
                            return;
                        }
                    }
                    self.host_channel.0.send(HostMessage::file_info()).await;
                }
                msg::FETCH_STATS if data.len() == 1 => {
                    let now = Instant::now().as_millis() as u32;
                    self.host_channel.0.send(HostMessage::stats(now)).await;
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
        if let Some(fw) = &mut self.fw
            && let Err(err) = fw.write(data)
        {
            crate::info!("write failed {:?}", err);
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

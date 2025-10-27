use std::{
    cmp::min,
    collections::HashMap,
    ffi::OsStr,
    fmt::Display,
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError},
    },
    time::Duration,
};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Local, Utc};
use futures_lite::future::block_on;
use nusb::transfer::{Direction, RequestBuffer};
use rpk_common::usb_vendor_message::{self as msg, MAX_BULK_LEN, READ_FILE_BY_INDEX, host_recv};

fn u16tou8(words: &[u16]) -> impl Iterator<Item = u8> + use<'_> {
    words.iter().flat_map(|a| a.to_le_bytes())
}

pub trait KeyboardInterface {
    fn bulk_out(&self, endpoint: u8, buf: Vec<u8>) -> Result<()>;
    fn bulk_in(&self, endpoint: u8, max_len: u16) -> Result<Vec<u8>>;
}

#[derive(Debug, Default)]
pub enum FileType {
    #[default]
    Config,
}
impl FileType {
    pub fn as_u8(&self) -> u8 {
        use FileType::*;
        match self {
            Config => 0,
        }
    }
}
impl From<u8> for FileType {
    fn from(_value: u8) -> Self {
        Self::Config
    }
}

#[derive(Default, Debug)]
pub struct FileInfo {
    pub timestamp: DateTime<Utc>,
    pub length: u32,
    pub location: u32,
    pub index: u32,
    pub file_type: FileType,
    pub filename: String,
}
impl FileInfo {
    fn is_none(&self) -> bool {
        self.location == 0
    }
}
impl From<&[u8]> for FileInfo {
    fn from(value: &[u8]) -> Self {
        if value.len() < 18 {
            return Default::default();
        }
        let name_end = min(value[17] as usize + 18, value.len());

        let mut filename = &value[18..name_end];
        if !filename.is_empty() && filename[0] == 0 {
            filename = &filename[1..];
        }

        Self {
            index: 0,
            location: u32::from_le_bytes(value[..4].try_into().unwrap()),
            length: u32::from_le_bytes(value[4..8].try_into().unwrap()),
            timestamp: DateTime::from_timestamp_millis(i64::from_le_bytes(
                value[8..16].try_into().unwrap(),
            ))
            .unwrap_or(DateTime::UNIX_EPOCH),
            file_type: FileType::from(value[16]),
            filename: String::from_utf8_lossy(filename).to_string(),
        }
    }
}
impl Display for FileInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dt = DateTime::<Local>::from(self.timestamp);

        f.write_fmt(format_args!(
            "{} {:5} {} {}",
            dt, self.length, self.filename, self.index
        ))
    }
}

pub struct FileListIterator<'a, I: KeyboardInterface> {
    keyboard: &'a KeyboardCtl<I>,
    index: u32,
    receiver: HostRecvReceiver,
}
impl<'a, I: KeyboardInterface> FileListIterator<'a, I> {
    fn new(keyboard: &'a KeyboardCtl<I>) -> Self {
        let receiver = keyboard.handle_incomming(host_recv::FILE_INFO).unwrap();
        Self {
            keyboard,
            index: 0,
            receiver,
        }
    }
}
impl<I: KeyboardInterface> Iterator for FileListIterator<'_, I> {
    type Item = FileInfo;

    fn next(&mut self) -> Option<Self::Item> {
        let mut msg = vec![READ_FILE_BY_INDEX];
        msg.extend_from_slice(&self.index.to_le_bytes());
        if self
            .keyboard
            .intf
            .bulk_out(self.keyboard.epout, msg)
            .is_err()
        {
            return None;
        }

        let data = match self.receiver.recv() {
            Ok(data) => data,
            Err(_) => return None,
        };

        let mut info = FileInfo::from(&data[1..]);
        info.index = self.index;

        if info.is_none() {
            None
        } else {
            self.index += 1;
            Some(info)
        }
    }
}

pub type HostRecvSender = mpsc::Sender<Vec<u8>>;

pub struct HostRecvReceiver(mpsc::Receiver<Vec<u8>>);
impl HostRecvReceiver {
    fn recv(&mut self) -> std::result::Result<Vec<u8>, RecvTimeoutError> {
        self.0.recv_timeout(Duration::from_millis(200))
    }
}

pub struct KeyboardStats {
    pub uptime: Duration,
}
impl From<&[u8]> for KeyboardStats {
    fn from(value: &[u8]) -> Self {
        Self {
            uptime: Duration::from_millis(u32::from_le_bytes(value[..4].try_into().unwrap()) as u64),
        }
    }
}

pub struct KeyboardCtl<I: KeyboardInterface> {
    intf: I,
    epout: u8,
    epin: u8,
    handlers: Arc<Mutex<HashMap<u8, HostRecvSender>>>,
}
impl KeyboardInterface for nusb::Interface {
    fn bulk_out(&self, endpoint: u8, buf: Vec<u8>) -> Result<()> {
        block_on((self as &nusb::Interface).bulk_out(endpoint, buf))
            .into_result()
            .map_err(|err| anyhow!("USB comms error: {}", err))?;
        Ok(())
    }
    fn bulk_in(&self, endpoint: u8, max_len: u16) -> Result<Vec<u8>> {
        block_on((self as &nusb::Interface).bulk_in(endpoint, RequestBuffer::new(max_len as usize)))
            .into_result()
            .map_err(|err| anyhow!("USB comms error: {}", err))
    }
}
impl<I: KeyboardInterface> KeyboardCtl<I> {
    pub fn find_vendor_interface(dev: &nusb::Device) -> Result<KeyboardCtl<nusb::Interface>> {
        if let Some((i, epout, epin)) = dev.configurations().find_map(|c| {
            c.interfaces().find_map(|i| {
                i.alt_settings().find(|a| a.class() == 255).map(|i| {
                    let mut epout = 0;
                    let mut epin = 0;
                    for ep in i.endpoints() {
                        match ep.direction() {
                            Direction::Out => epout = ep.address(),
                            Direction::In => epin = ep.address(),
                        }
                    }
                    (i.interface_number(), epout, epin)
                })
            })
        }) {
            let intf = dev.claim_interface(i)?;
            Ok(KeyboardCtl::<nusb::Interface> {
                intf,
                epout,
                epin,
                handlers: Default::default(),
            })
        } else {
            Err(anyhow!("Keyboard interface not found"))
        }
    }

    pub fn save_config(&self, data: &[u16], file_name: Option<&OsStr>) -> Result<()> {
        self.out(vec![msg::OPEN_SAVE_CONFIG])?;
        let (file_name, file_name_len) = file_name_iter(file_name);

        let iter = (18 + file_name_len as u32 + ((data.len() as u32) << 1))
            .to_le_bytes()
            .into_iter()
            .chain(chrono::Local::now().timestamp_millis().to_le_bytes())
            .chain([FileType::Config.as_u8(), file_name_len as u8])
            .chain(file_name.copied())
            .chain(u16tou8(data));

        for chunk in chunked(iter, MAX_BULK_LEN as usize) {
            if chunk.len() < MAX_BULK_LEN as usize {
                return self.out(
                    [msg::CLOSE_SAVE_CONFIG]
                        .iter()
                        .copied()
                        .chain(chunk)
                        .collect(),
                );
            } else {
                self.out(chunk)?;
            }
        }

        self.out(vec![msg::CLOSE_SAVE_CONFIG])
    }

    pub fn reset_keyboard(&self) -> Result<()> {
        self.out(vec![msg::RESET_KEYBOARD])
    }

    pub fn reset_to_usb_boot_from_usb(&self) -> Result<()> {
        self.out(vec![msg::RESET_TO_USB_BOOT])
    }

    fn out(&self, data: Vec<u8>) -> Result<()> {
        self.intf
            .bulk_out(self.epout, data)
            .map(|_| ())
            .map_err(|err| anyhow!("USB comms error: {}", err))
    }

    pub fn list_files(&self) -> FileListIterator<'_, I> {
        FileListIterator::new(self)
    }

    pub fn fetch_stats(&self) -> Result<KeyboardStats> {
        let msg = vec![msg::FETCH_STATS];

        let mut receiver = self.handle_incomming(host_recv::STATS).unwrap();

        self.intf
            .bulk_out(self.epout, msg)
            .map_err(|e| anyhow!(e))?;

        let data = match receiver.recv() {
            Ok(data) => data,
            Err(err) => return Err(anyhow!(err)),
        };

        Ok(KeyboardStats::from(&data[1..]))
    }

    pub fn listen(&self) {
        loop {
            match self.intf.bulk_in(self.epin, MAX_BULK_LEN) {
                Ok(msg) if !msg.is_empty() => {
                    let handler = {
                        let guard = self.handlers.lock().unwrap();
                        match guard.get(&msg[0]) {
                            Some(handler) => handler.clone(),
                            None => continue,
                        }
                    };
                    if let Err(err) = handler.send(msg) {
                        eprintln!("{err:?}");
                    }
                }
                Ok(_) => {}
                Err(err) => {
                    eprintln!("{err:?}");
                }
            }
        }
    }

    fn handle_incomming(&self, id: u8) -> Result<HostRecvReceiver> {
        let mut guard = self.handlers.lock().unwrap();

        if let std::collections::hash_map::Entry::Vacant(e) = guard.entry(id) {
            let (sender, receiver) = mpsc::channel();
            e.insert(sender);
            Ok(HostRecvReceiver(receiver))
        } else {
            Err(anyhow!("id {id} in use"))
        }
    }
}

pub fn file_name_iter(file_name: Option<&OsStr>) -> (impl Iterator<Item = &u8>, usize) {
    let file_name = file_name.unwrap_or(OsStr::new("")).as_encoded_bytes();
    let file_name = &file_name[..min(50, file_name.len())];
    let mut v = vec![];
    let mut len = file_name.len();
    if len & 1 == 1 {
        let null: &[u8] = &[0];
        len += 1;
        v.push(null);
    }
    v.push(file_name);

    (v.into_iter().flatten(), len)
}

struct Chunked<I> {
    iterator: I,
    chunk_size: usize,
}

fn chunked<Collection>(a: Collection, chunk_size: usize) -> Chunked<Collection::IntoIter>
where
    Collection: IntoIterator,
{
    let iterator = a.into_iter();
    Chunked {
        iterator,
        chunk_size,
    }
}

impl<I: Iterator> Iterator for Chunked<I> {
    type Item = Vec<I::Item>;
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.iterator.by_ref().take(self.chunk_size).collect())
            .filter(|chunk: &Vec<_>| !chunk.is_empty())
    }
}

#[cfg(test)]
#[path = "vendor_coms_test.rs"]
mod test;

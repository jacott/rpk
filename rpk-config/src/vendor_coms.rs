use std::{cmp::min, ffi::OsStr, fmt::Display};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, Utc};
use futures_lite::future::block_on;
use nusb::transfer::{Direction, RequestBuffer};
use rpk_common::usb_vendor_message::{self as msg, MAX_BULK_LEN, READ_FILE_BY_INDEX};

fn u16tou8(words: &[u16]) -> impl Iterator<Item = u8> + use<'_> {
    words.iter().flat_map(|a| a.to_le_bytes())
}

pub trait KeyboardInterface {
    fn bulk_out(&self, endpoint: u8, buf: Vec<u8>) -> Result<()>;
    fn bulk_in(&self, endpoint: u8, max_len: u16) -> Result<Vec<u8>>;
}

#[derive(Debug)]
pub enum FileType {
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
impl Default for FileType {
    fn default() -> Self {
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
}
impl<'a, I: KeyboardInterface> FileListIterator<'a, I> {
    fn new(keyboard: &'a KeyboardCtl<I>) -> Self {
        Self { keyboard, index: 0 }
    }
}
impl<I: KeyboardInterface> Iterator for FileListIterator<'_, I> {
    type Item = Result<FileInfo>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut msg = vec![READ_FILE_BY_INDEX];
        msg.extend_from_slice(&self.index.to_le_bytes());
        if let Err(err) = self.keyboard.intf.bulk_out(self.keyboard.epout, msg) {
            return Some(Err(anyhow!("USB comms error: {}", err)));
        }
        let data = match self.keyboard.intf.bulk_in(self.keyboard.epin, MAX_BULK_LEN) {
            Ok(data) => data,
            Err(err) => return Some(Err(anyhow!("USB comms error: {}", err))),
        };

        let mut info = FileInfo::from(data.as_slice());
        info.index = self.index;

        if info.is_none() {
            None
        } else {
            self.index += 1;
            Some(Ok(info))
        }
    }
}

pub struct KeyboardCtl<I: KeyboardInterface> {
    intf: I,
    epout: u8,
    epin: u8,
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
            Ok(KeyboardCtl::<nusb::Interface> { intf, epout, epin })
        } else {
            Err(anyhow!("Keyboard interface not found"))
        }
    }

    pub fn save_config(&self, data: &[u16], file_name: Option<&OsStr>) -> Result<()> {
        self.out(vec![msg::OPEN_SAVE_CONFIG])?;
        let file_name = file_name.unwrap_or(OsStr::new("")).as_encoded_bytes();
        let file_name = &file_name[..min(50, file_name.len())];

        let mut type_fnlen = vec![FileType::Config.as_u8(), file_name.len() as u8];
        if file_name.len() % 2 == 1 {
            type_fnlen[1] += 1;
            type_fnlen.push(0);
        }

        let iter = (18 + type_fnlen[1] as u32 + ((data.len() as u32) << 1))
            .to_le_bytes()
            .into_iter()
            .chain(chrono::Local::now().timestamp_millis().to_le_bytes())
            .chain(type_fnlen)
            .chain(file_name.iter().copied())
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

    pub fn list_files(&self) -> FileListIterator<I> {
        FileListIterator::new(self)
    }
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

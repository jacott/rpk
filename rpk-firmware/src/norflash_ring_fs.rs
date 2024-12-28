use core::cell::RefCell;

use embedded_storage::nor_flash::{self, NorFlash};

use crate::ring_fs::{FileDescriptor, RingFs, RingFsError, RingFsReader, RingFsWriter};

const FORMAT_MAGIC_NUMBER: [u8; 4] = 0x6e0fac0bu32.to_be_bytes();
const FORMAT_VERSION: u8 = 2;

const END_PAGE_PATTERN: [u8; 4] = 0x00ffff00u32.to_le_bytes();

const PREAMBLE_LEN: u32 = 4 * 2 + FORMAT_MAGIC_NUMBER.len() as u32;
const RING_END_MARKER: u32 = u32::MAX >> 1;

pub struct NorflashRingFs<
    'd,
    F: NorFlash,
    const BASE: usize,
    const SIZE: usize,
    const DIR_SIZE: u32,
    const PAGE_SIZE: usize,
    const MAX_FILES: u32,
> {
    inner: RefCell<NorflashRingFsInner<'d, F, BASE, SIZE, DIR_SIZE, PAGE_SIZE, MAX_FILES>>,
}

impl<
        'd,
        F: NorFlash,
        const BASE: usize,
        const SIZE: usize,
        const DIR_SIZE: u32,
        const PAGE_SIZE: usize,
        const MAX_FILES: u32,
    > RingFs<'d> for NorflashRingFs<'d, F, BASE, SIZE, DIR_SIZE, PAGE_SIZE, MAX_FILES>
{
    fn create_file(&'d self) -> Result<RingFsWriter<'d>, RingFsError> {
        let mut inner = self.inner.borrow_mut();
        Ok(RingFsWriter::new(self, inner.create_file()?))
    }

    fn file_reader_by_index(&'d self, index: u32) -> Result<RingFsReader<'d>, RingFsError> {
        let mut inner = self.inner.borrow_mut();
        Ok(RingFsReader::new(self, inner.file_reader(index)?))
    }

    fn file_reader_by_location(&'d self, start: u32) -> Result<RingFsReader<'d>, RingFsError> {
        let mut inner = self.inner.borrow_mut();
        Ok(RingFsReader::new(self, inner.file_reader_by_offset(start)?))
    }

    fn write_file(&self, desc: &mut FileDescriptor, data: &[u8]) -> Result<(), RingFsError> {
        let mut inner = self.inner.borrow_mut();
        inner.write_file(desc, data)
    }

    fn read_file(&self, desc: &mut FileDescriptor, data: &mut [u8]) -> Result<u32, RingFsError> {
        let mut inner = self.inner.borrow_mut();
        inner.read_file(desc, data)
    }

    fn close_file(&self, desc: &mut FileDescriptor) {
        let mut inner = self.inner.borrow_mut();
        inner.close_file(desc);
    }
}

struct NorflashRingFsInner<
    'd,
    F: NorFlash,
    const BASE: usize,
    const SIZE: usize,
    const DIR_SIZE: u32,
    const PAGE_SIZE: usize,
    const MAX_FILES: u32,
> {
    flash: &'d mut F,
    next_file_index: u32,
    oldest_file_index: u32,
    free_index: u32,
    write_cache: [u8; PAGE_SIZE],
    cache_offset: u32,
    writer: bool,
    read_counter: usize,
}

impl<
        'd,
        F: NorFlash,
        const BASE: usize,
        const SIZE: usize,
        const DIR_SIZE: u32,
        const PAGE_SIZE: usize,
        const MAX_FILES: u32,
    > NorflashRingFs<'d, F, BASE, SIZE, DIR_SIZE, PAGE_SIZE, MAX_FILES>
{
    pub fn new(flash: &'d mut F) -> Result<Self, RingFsError> {
        Ok(Self {
            inner: RefCell::new(NorflashRingFsInner::new(flash)?),
        })
    }
}

const fn max32(a: u32, b: u32) -> u32 {
    if a < b {
        b
    } else {
        a
    }
}

const fn assert_fs_params<
    const BASE: usize,
    const SIZE: usize,
    const DIR_SIZE: u32,
    const PAGE_SIZE: usize,
    const MAX_FILES: u32,
>(
    erase_size: u32,
) -> u32 {
    assert!(PAGE_SIZE >= 4);
    assert!(PAGE_SIZE % 4 == 0);
    let erase_size = max32(4, (erase_size >> 2) << 2);
    let erase_size = max32(
        erase_size,
        (max32(MAX_FILES * 4 + PREAMBLE_LEN + 8, erase_size) / erase_size) * erase_size,
    );

    assert!(BASE % DIR_SIZE as usize == 0);
    assert!(SIZE % erase_size as usize == 0);
    assert!(DIR_SIZE % erase_size == 0);
    assert!(PAGE_SIZE as u32 <= erase_size);
    assert!(erase_size % PAGE_SIZE as u32 == 0);
    assert!(DIR_SIZE % erase_size == 0);
    assert!(DIR_SIZE >= 20);
    erase_size
}

fn map_flash_error(err: impl nor_flash::NorFlashError) -> RingFsError {
    match err.kind() {
        nor_flash::NorFlashErrorKind::NotAligned => RingFsError::NotAligned,
        nor_flash::NorFlashErrorKind::OutOfBounds => RingFsError::OutOfBounds,
        _ => RingFsError::Unknown,
    }
}

impl<
        'd,
        F: NorFlash,
        const BASE: usize,
        const SIZE: usize,
        const DIR_SIZE: u32,
        const PAGE_SIZE: usize,
        const MAX_FILES: u32,
    > NorflashRingFsInner<'d, F, BASE, SIZE, DIR_SIZE, PAGE_SIZE, MAX_FILES>
{
    const ERASE_SIZE: u32 =
        assert_fs_params::<BASE, SIZE, DIR_SIZE, PAGE_SIZE, MAX_FILES>(F::ERASE_SIZE as u32);
    const DISK_SIZE_BYTES: [u8; 4] = (SIZE as u32).to_le_bytes();
    const FIRST_FILE_OFFSET: u32 = Self::align_next_page(DIR_SIZE + PREAMBLE_LEN);

    fn new(flash: &'d mut F) -> Result<Self, RingFsError> {
        let mut fs = Self {
            flash,
            next_file_index: PREAMBLE_LEN,
            oldest_file_index: PREAMBLE_LEN,
            free_index: Self::FIRST_FILE_OFFSET,
            write_cache: [0xff; PAGE_SIZE],
            cache_offset: u32::MAX,
            writer: false,
            read_counter: 0,
        };

        if fs.check_formatted(0)? {
            fs.init_dir_indices()?;
            if !fs.check_formatted(DIR_SIZE)? {
                fs.recover_store_from_dir()?;
            } else {
                //  everything is Okay
                fs.find_free_index()?;
            }
        } else if fs.check_formatted(DIR_SIZE)? {
            fs.recover_dir_from_store()?;
        } else {
            // unformatted disk
            fs.create_header_page(0)?;
            fs.create_header_page(DIR_SIZE)?;
        }

        Ok(fs)
    }

    fn create_file(&mut self) -> Result<FileDescriptor, RingFsError> {
        if self.writer || self.read_counter > 0 {
            return Err(RingFsError::InUse);
        }
        self.writer = true;
        Ok(FileDescriptor::new_writer())
    }

    fn close_file(&mut self, desc: &mut FileDescriptor) {
        match desc.state {
            crate::ring_fs::FileState::Closed => return,

            crate::ring_fs::FileState::Reader => {
                self.read_counter -= 1;
            }
            crate::ring_fs::FileState::Writer => {
                self.writer = false;
            }
        }
        desc.close();
    }

    fn file_reader(&mut self, index: u32) -> Result<FileDescriptor, RingFsError> {
        if index * 4 + self.oldest_file_index >= self.next_file_index {
            return Err(RingFsError::FileNotFound);
        }
        let dir_location = self.next_file_index - index * 4 - 4;
        let start = self.read_u32(dir_location)?;

        self.file_reader_by_offset(start)
    }

    fn file_reader_by_offset(&mut self, start: u32) -> Result<FileDescriptor, RingFsError> {
        if self.writer {
            return Err(RingFsError::InUse);
        }
        let len = self.read_u32(start)?;

        self.read_counter += 1;
        Ok(FileDescriptor::new_reader(start, len))
    }

    fn write_file(&mut self, desc: &mut FileDescriptor, data: &[u8]) -> Result<(), RingFsError> {
        if desc.is_closed() {
            return Err(RingFsError::FileClosed);
        };

        let result = self.guarded_write_file(desc, data);

        if result.is_err() || desc.offset >= desc.len {
            self.close_file(desc);
        }

        result
    }

    fn guarded_write_file(
        &mut self,
        desc: &mut FileDescriptor,
        data: &[u8],
    ) -> Result<(), RingFsError> {
        if desc.location == 0 {
            if data.len() < 4 {
                return Err(RingFsError::MissingFileLength);
            }
            let len = u32::from_le_bytes(data[..4].try_into().unwrap());
            if len > SIZE as u32 - 4 - Self::FIRST_FILE_OFFSET {
                return Err(RingFsError::FileTooLarge);
            }

            let index = self.next_file_index()?; // do before free_space incase we recycle_dir_page

            let start = self.free_space(len)?;

            self.write_u32(start, len)?;
            self.write_u32(index, start)?;
            self.commit_write_cache()?;

            desc.len = len;
            desc.location = start;
        }

        let len = data.len() as u32;

        if len > desc.len - desc.offset {
            return Err(RingFsError::FileOverrun);
        }

        let offset = desc.location + desc.offset;
        let next_offset = offset + len;

        if next_offset > SIZE as u32 {
            if offset >= SIZE as u32 {
                let offset = offset - SIZE as u32 + self.first_file_offset() + 4;
                self.write(offset, data)?;
            } else {
                let split = SIZE - offset as usize;
                let (d1, d2) = data.split_at(split);
                self.write(offset, d1)?;
                self.write(self.first_file_offset() + 4, d2)?;
            }
        } else {
            self.write(offset, data)?;
        }
        desc.offset = next_offset - desc.location;
        Ok(())
    }

    fn read_file(
        &mut self,
        desc: &mut FileDescriptor,
        data: &mut [u8],
    ) -> Result<u32, RingFsError> {
        if desc.is_closed() {
            return Err(RingFsError::FileClosed);
        };

        let result = self.guarded_file_read(desc, data);

        if result.is_err() || desc.offset >= desc.len {
            self.close_file(desc);
        }
        result
    }

    fn guarded_file_read(
        &mut self,
        desc: &mut FileDescriptor,
        data: &mut [u8],
    ) -> Result<u32, RingFsError> {
        if desc.offset > desc.len {
            return Err(RingFsError::OutOfBounds);
        }
        let rem = (desc.len - desc.offset) as usize;

        let data = if rem < data.len() {
            data.split_at_mut(rem).0
        } else {
            data
        };

        let len = data.len() as u32;

        let offset = desc.location + desc.offset;
        let next_offset = offset + len;

        if next_offset > SIZE as u32 {
            if offset >= SIZE as u32 {
                let offset = offset - SIZE as u32 + self.first_file_offset() + 4;
                self.read(offset, data)?;
            } else {
                let split = SIZE - offset as usize;
                let (d1, d2) = data.split_at_mut(split);
                self.read(offset, d1)?;
                self.read(self.first_file_offset() + 4, d2)?;
            }
        } else {
            self.read(offset, data)?;
        }
        desc.offset = next_offset - desc.location;

        Ok(len)
    }

    fn first_file_offset(&self) -> u32 {
        Self::FIRST_FILE_OFFSET
    }

    fn init_dir_indices(&mut self) -> Result<(), RingFsError> {
        let mut start = 0;
        let mut end = (DIR_SIZE - PREAMBLE_LEN - 4) >> 2;
        let mut live_start = 0;

        while start < end {
            let mid = (start + end) >> 1;
            let v = self.read_u32(PREAMBLE_LEN + mid * 4)?;

            if v == u32::MAX {
                end = mid;
            } else {
                start = mid + 1;
                if v == 0 {
                    live_start = start;
                }
            }
        }
        while live_start < end {
            let mid = (live_start + end) >> 1;
            let v = self.read_u32(PREAMBLE_LEN + mid * 4)?;

            if v != 0 {
                end = mid;
            } else {
                live_start = mid + 1;
            }
        }
        self.next_file_index = PREAMBLE_LEN + start * 4;
        self.oldest_file_index = PREAMBLE_LEN + live_start * 4;

        Ok(())
    }

    fn find_free_index(&mut self) -> Result<(), RingFsError> {
        if self.next_file_index == PREAMBLE_LEN {
            self.free_index = Self::FIRST_FILE_OFFSET;
        } else {
            let offset = self.read_u32(self.next_file_index - 4)?;
            let len = self.read_u32(offset)?;
            self.free_index = Self::align_next_page(offset + len);
            if self.free_index > SIZE as u32 {
                self.free_index = self.free_index - SIZE as u32 + Self::FIRST_FILE_OFFSET;
            }
        }

        Ok(())
    }

    const fn align_start_erase(offset: u32) -> u32 {
        offset - (offset % Self::ERASE_SIZE)
    }

    const fn align_start_page(offset: u32) -> u32 {
        offset - (offset % PAGE_SIZE as u32)
    }

    const fn align_next_page(offset: u32) -> u32 {
        Self::align_start_page(offset + PAGE_SIZE as u32 - 1)
    }

    fn next_file_index(&mut self) -> Result<u32, RingFsError> {
        if self.next_file_index >= DIR_SIZE - 4 {
            self.recycle_dir_page()?;
        }
        let index = self.next_file_index;
        while ((index - self.oldest_file_index + 4) >> 2) >= MAX_FILES {
            self.delete_oldest()?;
        }

        self.next_file_index += 4;
        Ok(index)
    }

    fn free_space(&mut self, len: u32) -> Result<u32, RingFsError> {
        if len == 0 {
            return Ok(self.free_index);
        }
        let start = self.free_index;

        let start_eend = Self::align_start_erase(start);
        let mut pend = Self::align_next_page(start + len);

        if Self::align_start_erase(pend) > start_eend {
            let estart = start_eend + Self::ERASE_SIZE;
            if pend < SIZE as u32 {
                self.clear_space(estart, Self::align_start_erase(pend) + Self::ERASE_SIZE)?;
            } else {
                pend =
                    Self::align_next_page(start + len + 4 + Self::FIRST_FILE_OFFSET - SIZE as u32);

                if estart < SIZE as u32 {
                    self.clear_space(estart, SIZE as u32)?;
                }

                self.clear_space(DIR_SIZE, Self::align_start_erase(pend) + Self::ERASE_SIZE)?;
                if pend > Self::FIRST_FILE_OFFSET {
                    self.write_u32(Self::FIRST_FILE_OFFSET, pend - Self::FIRST_FILE_OFFSET)?;
                    self.commit_write_cache()?;
                }
            }
        }

        self.free_index = pend;
        Ok(start)
    }

    fn clear_space(&mut self, start: u32, end: u32) -> Result<(), RingFsError> {
        while self.oldest_file_index < self.next_file_index {
            let offset = self.read_u32(self.oldest_file_index)?;
            if offset < start || offset >= end {
                break;
            }
            self.delete_oldest()?;
        }

        self.erase(start, end)?;

        if start == DIR_SIZE {
            self.write(DIR_SIZE, &Self::header_sequence())?;
        }

        Ok(())
    }

    fn delete_oldest(&mut self) -> Result<(), RingFsError> {
        if self.oldest_file_index < self.next_file_index {
            self.write_u32(self.oldest_file_index, 0)?;
            self.oldest_file_index += 4;
        }
        Ok(())
    }

    fn write_u32(&mut self, offset: u32, value: u32) -> Result<(), RingFsError> {
        if offset < self.cache_offset || offset + 4 > self.cache_offset + PAGE_SIZE as u32 {
            if self.cache_offset != u32::MAX {
                self.commit_write_cache()?;
            }
            self.cache_offset = Self::align_start_page(offset);
        }
        let offset = offset as usize % PAGE_SIZE;
        let data = value.to_le_bytes();
        self.write_cache[offset..offset + 4].clone_from_slice(&data);
        Ok(())
    }

    fn commit_write_cache(&mut self) -> Result<(), RingFsError> {
        if self.cache_offset != u32::MAX {
            let result = self
                .flash
                .write(BASE as u32 + self.cache_offset, &self.write_cache)
                .map_err(map_flash_error);
            self.write_cache.fill(0xff);
            self.cache_offset = u32::MAX;
            result?;
        }
        Ok(())
    }

    fn write(&mut self, offset: u32, data: &[u8]) -> Result<(), RingFsError> {
        if self.cache_offset != u32::MAX {
            self.commit_write_cache()?;
        }
        self.flash
            .write(BASE as u32 + offset, data)
            .map_err(map_flash_error)
    }

    fn erase(&mut self, from: u32, to: u32) -> Result<(), RingFsError> {
        if self.cache_offset != u32::MAX {
            self.commit_write_cache()?;
        }
        self.flash
            .erase(BASE as u32 + from, BASE as u32 + to)
            .map_err(map_flash_error)
    }

    fn read_u32(&mut self, offset: u32) -> Result<u32, RingFsError> {
        let mut data = [0; 4];
        self.read(offset, &mut data)?;
        if offset >= self.cache_offset && offset + 4 <= self.cache_offset + PAGE_SIZE as u32 {
            let offset = offset as usize % PAGE_SIZE;
            for (t, f) in data
                .iter_mut()
                .zip(self.write_cache[offset..offset + 4].iter())
            {
                *t &= *f;
            }
        }

        Ok(u32::from_le_bytes(data))
    }

    fn read(&mut self, offset: u32, data: &mut [u8]) -> Result<(), RingFsError> {
        self.flash
            .read(BASE as u32 + offset, data)
            .map_err(map_flash_error)
    }

    fn check_formatted(&mut self, offset: u32) -> Result<bool, RingFsError> {
        let mut check_bytes = [0; 12];

        {
            let end_page = &mut check_bytes[..4];
            self.read(DIR_SIZE - 4, end_page)?;
            if end_page != END_PAGE_PATTERN {
                return Ok(false);
            }
        }
        self.read(offset, &mut check_bytes)?;

        let seq = Self::header_sequence();

        Ok(check_bytes == seq)
    }

    fn header_sequence() -> [u8; 12] {
        let format_version = (((FORMAT_VERSION as u32) << 24) | DIR_SIZE).to_le_bytes();

        let words = [FORMAT_MAGIC_NUMBER, format_version, Self::DISK_SIZE_BYTES];
        let mut iter = words.iter().flatten();

        core::array::from_fn(|_| *iter.next().unwrap())
    }

    fn create_header_page(&mut self, offset: u32) -> Result<(), RingFsError> {
        self.erase(
            offset,
            if offset == 0 {
                DIR_SIZE
            } else {
                offset + Self::ERASE_SIZE
            },
        )?;
        self.write(offset, &Self::header_sequence())?;
        if offset == 0 {
            self.write(DIR_SIZE - 4, &END_PAGE_PATTERN)?;
        }

        Ok(())
    }

    fn rebuild_dir(&mut self, oldest: u32) -> Result<(), RingFsError> {
        self.erase(0, DIR_SIZE)?;
        self.oldest_file_index = PREAMBLE_LEN;
        let mut index = PREAMBLE_LEN;
        let mut offset = oldest;
        loop {
            let len = self.read_u32(offset)?;

            if len == RING_END_MARKER {
                break;
            }

            self.write_u32(index, offset)?;
            index += 4;
            offset = self.next_file_offset(offset, len)?;
        }
        self.next_file_index = index;
        self.find_free_index()?;
        self.write(0, &Self::header_sequence())?;
        self.write(DIR_SIZE - 4, &END_PAGE_PATTERN)?;
        Ok(())
    }

    fn recycle_dir_page(&mut self) -> Result<(), RingFsError> {
        let free_index = self.free_index;
        let mut estart = Self::align_start_erase(free_index) + Self::ERASE_SIZE;
        if estart + Self::ERASE_SIZE > SIZE as u32 {
            estart = Self::align_start_erase(Self::FIRST_FILE_OFFSET) + Self::ERASE_SIZE;
            self.clear_space(DIR_SIZE, estart + Self::ERASE_SIZE)?;
            self.write_u32(Self::FIRST_FILE_OFFSET, RING_END_MARKER)?;
        } else {
            self.clear_space(estart, estart + Self::ERASE_SIZE)?;
        }

        let oldest = self.read_u32(self.oldest_file_index)?;

        self.write_u32(free_index, RING_END_MARKER)?;
        self.write_u32(estart, oldest)?;

        self.rebuild_dir(oldest)
    }

    fn next_file_offset(&mut self, offset: u32, len: u32) -> Result<u32, RingFsError> {
        Ok(match len {
            u32::MAX => self.next_page(offset),
            RING_END_MARKER => {
                let mut mark = Self::align_start_erase(offset) + Self::ERASE_SIZE;
                if mark >= SIZE as u32 {
                    mark = Self::align_start_erase(Self::FIRST_FILE_OFFSET) + Self::ERASE_SIZE;
                }

                self.read_u32(mark)?
            }
            len => {
                let pend = Self::align_next_page(offset + len);

                if pend < SIZE as u32 {
                    pend
                } else {
                    Self::align_next_page(offset + len + 4 + Self::FIRST_FILE_OFFSET - SIZE as u32)
                }
            }
        })
    }

    fn next_page(&self, offset: u32) -> u32 {
        let next = Self::align_next_page(offset + PAGE_SIZE as u32);
        if next > SIZE as u32 {
            DIR_SIZE + PREAMBLE_LEN
        } else {
            next
        }
    }

    fn recover_dir_from_store(&mut self) -> Result<(), RingFsError> {
        let mut offset = Self::FIRST_FILE_OFFSET;
        let mut len = self.read_u32(offset)?;
        if len == u32::MAX {
            self.create_header_page(0)?;
            return Ok(());
        }
        loop {
            if len == RING_END_MARKER {
                let oldest = self.next_file_offset(offset, len)?;
                return self.rebuild_dir(oldest);
            }

            offset = if len == u32::MAX {
                return Err(RingFsError::UnrecoverableDisk);
            } else {
                Self::align_next_page(offset + len)
            };
            len = self.read_u32(offset)?;
        }
    }

    fn recover_store_from_dir(&mut self) -> Result<(), RingFsError> {
        self.erase(DIR_SIZE, DIR_SIZE + Self::ERASE_SIZE)?;
        self.write(DIR_SIZE, &Self::header_sequence())
    }
}

#[cfg(test)]
#[path = "norflash_ring_fs_test.rs"]
pub(crate) mod test;

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum RingFsError {
    OutOfSpace,
    FileOverrun,
    MissingFileLength,
    InUse,
    UnrecoverableDisk,
    FileTooLarge,
    FileNotFound,
    FileClosed,
    NotAligned,
    OutOfBounds,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub enum FileState {
    Closed,
    Reader,
    Writer,
}

#[derive(Debug, Clone, Copy)]
pub struct FileDescriptor {
    pub state: FileState,
    /// Address of file contents on disk.
    pub location: u32,
    /// Length of file.
    pub len: u32,
    /// Offset from location to the current read/write location.
    pub offset: u32,
}

pub trait RingFs<'f> {
    fn create_file(&'f self) -> Result<RingFsWriter<'f>, RingFsError>;
    fn file_reader_by_index(&'f self, index: u32) -> Result<RingFsReader<'f>, RingFsError>;
    fn file_reader_by_location(&'f self, location: u32) -> Result<RingFsReader<'f>, RingFsError>;
    fn close_file(&self, desc: &mut FileDescriptor);
    fn write_file(&self, desc: &mut FileDescriptor, data: &[u8]) -> Result<(), RingFsError>;
    fn read_file(&self, desc: &mut FileDescriptor, data: &mut [u8]) -> Result<u32, RingFsError>;
}

pub struct RingFsWriter<'f> {
    fs: &'f dyn RingFs<'f>,
    desc: FileDescriptor,
}
impl Drop for RingFsWriter<'_> {
    fn drop(&mut self) {
        self.close();
    }
}
impl<'f> RingFsWriter<'f> {
    pub fn new(fs: &'f dyn RingFs<'f>, desc: FileDescriptor) -> Self {
        Self { fs, desc }
    }

    pub fn close(&mut self) {
        self.fs.close_file(&mut self.desc);
    }

    pub fn is_closed(&self) -> bool {
        self.desc.is_closed()
    }

    pub fn write(&mut self, data: &[u8]) -> Result<(), RingFsError> {
        self.fs.write_file(&mut self.desc, data)
    }

    pub fn location(&self) -> u32 {
        self.desc.location
    }
}

pub struct RingFsReader<'f> {
    fs: &'f dyn RingFs<'f>,
    desc: FileDescriptor,
}
impl Drop for RingFsReader<'_> {
    fn drop(&mut self) {
        self.close();
    }
}
impl<'f> RingFsReader<'f> {
    pub fn new(fs: &'f dyn RingFs<'f>, desc: FileDescriptor) -> Self {
        Self { fs, desc }
    }

    pub fn close(&mut self) {
        self.fs.close_file(&mut self.desc);
    }

    pub fn is_closed(&self) -> bool {
        self.desc.is_closed()
    }

    pub fn read(&mut self, data: &mut [u8]) -> Result<u32, RingFsError> {
        self.fs.read_file(&mut self.desc, data)
    }

    pub fn location(&self) -> u32 {
        self.desc.location
    }

    pub fn seek(&mut self, offset: u32) {
        self.desc.offset = offset;
    }
}

impl FileDescriptor {
    pub fn new_writer() -> Self {
        Self {
            state: FileState::Writer,
            location: 0,
            len: 0,
            offset: 0,
        }
    }

    pub fn new_reader(location: u32, len: u32) -> Self {
        Self {
            state: FileState::Reader,
            location,
            len,
            offset: 0,
        }
    }

    pub fn is_closed(&self) -> bool {
        matches!(self.state, FileState::Closed)
    }

    pub(crate) fn close(&mut self) {
        if !self.is_closed() {
            self.state = FileState::Closed;
        }
    }
}

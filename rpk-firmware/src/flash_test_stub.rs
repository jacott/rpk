use embedded_storage::nor_flash::{
    ErrorType, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash,
};

extern crate std;

#[derive(Debug)]
pub enum FlashStubError {
    Unknown,
}

#[derive(Debug)]
pub enum Action {
    Erase(u32, u32),
    Write(u32, std::vec::Vec<u8>),
}

pub struct NorFlashStub<'f, const FLASH_SIZE: usize> {
    pub buf: [u8; FLASH_SIZE],
    #[allow(clippy::type_complexity)]
    pub observer: Option<&'f dyn Fn(Action, &mut [u8]) -> Result<(), FlashStubError>>,
}
impl NorFlashError for FlashStubError {
    fn kind(&self) -> NorFlashErrorKind {
        match self {
            FlashStubError::Unknown => NorFlashErrorKind::Other,
        }
    }
}
impl<const FLASH_SIZE: usize> ErrorType for NorFlashStub<'_, FLASH_SIZE> {
    type Error = FlashStubError;
}
impl<const FLASH_SIZE: usize> ReadNorFlash for NorFlashStub<'_, FLASH_SIZE> {
    const READ_SIZE: usize = 1;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        let offset = offset as usize;

        for (f, t) in self.buf[offset..offset + bytes.len()]
            .iter()
            .zip(bytes.iter_mut())
        {
            *t = *f;
        }

        Ok(())
    }

    fn capacity(&self) -> usize {
        self.buf.len()
    }
}
impl<const FLASH_SIZE: usize> NorFlash for NorFlashStub<'_, FLASH_SIZE> {
    const WRITE_SIZE: usize = 1;

    const ERASE_SIZE: usize = 64;

    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        if let Some(observer) = self.observer {
            observer(Action::Erase(from, to), &mut self.buf)?;
        }
        let from = from as usize;
        let to = to as usize;
        for b in self.buf[from..to].iter_mut() {
            *b = 0xff;
        }
        Ok(())
    }

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        if let Some(observer) = self.observer {
            observer(Action::Write(offset, bytes.into()), &mut self.buf)?;
        }
        let offset = offset as usize;

        for (t, f) in self.buf[offset..offset + bytes.len()]
            .iter_mut()
            .zip(bytes.iter())
        {
            *t &= *f;
        }

        Ok(())
    }
}
impl<const FLASH_SIZE: usize> Default for NorFlashStub<'_, FLASH_SIZE> {
    fn default() -> Self {
        Self {
            buf: [0; FLASH_SIZE],
            observer: None,
        }
    }
}

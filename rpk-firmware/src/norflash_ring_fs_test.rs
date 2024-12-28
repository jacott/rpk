use embedded_storage::nor_flash::ReadNorFlash;

use crate::{
    flash_test_stub::{Action, FlashStubError, NorFlashStub},
    ring_fs::RingFs,
};

use super::*;

extern crate std;
use std::format;
use std::string::String;

const DEFAULT_DSIZE: usize = 512;
const DIR_SIZE: u32 = 64;
const PAGE_SIZE: usize = 16;
const MAX_FILES: u32 = 20;

pub(crate) type DefaultNorFlashStub<'f> = NorFlashStub<'f, DEFAULT_DSIZE>;
pub(crate) type TestMaxFilesFs<'d, 'f, const K: usize, const M: u32> =
    NorflashRingFs<'d, NorFlashStub<'f, K>, 0, K, DIR_SIZE, PAGE_SIZE, M>;
pub(crate) type TestFs<'d, 'f, const K: usize> = TestMaxFilesFs<'d, 'f, K, MAX_FILES>;

#[test]
fn ring_fs() {
    let mut stub = DefaultNorFlashStub::default();

    let fs = TestFs::new(&mut stub).unwrap();
    let fs: &dyn RingFs = &fs;

    let data: [u8; 30] = core::array::from_fn(|i| i as u8);
    {
        let mut fw = fs.create_file().unwrap();
        fw.write(&34u32.to_le_bytes()).unwrap();
        fw.write(&data).unwrap();
        assert!(fw.is_closed());
    }
    let mut fr = fs.file_reader_by_index(0).unwrap();
    let mut rdata = [0; 34];
    assert_eq!(fr.read(&mut rdata).unwrap(), 34);

    assert_eq!(&34u32.to_le_bytes(), &rdata[..4]);
    assert_eq!(&data, &rdata[4..]);
    assert!(fr.is_closed());
    fr.close();
    assert!(fr.is_closed());
}

#[test]
fn limit_max_files() {
    let mut stub = NorFlashStub::<DEFAULT_DSIZE>::default();
    {
        let fs = TestMaxFilesFs::<'_, '_, DEFAULT_DSIZE, 6>::new(&mut stub).unwrap();
        for i in 0..8 {
            let mut fw = fs.create_file().unwrap();
            fw.write(&5_u32.to_le_bytes()).unwrap();
            fw.write(&[i as u8]).unwrap();
        }
        let mut i = 0;
        loop {
            let Ok(_fr) = fs.file_reader_by_index(i) else {
                break;
            };
            i += 1;
        }
        assert_eq!(i, 5);
    }
}

#[test]
fn recycle_dir_page() {
    let mut stub = NorFlashStub::<DEFAULT_DSIZE>::default();
    let buf: [u8; 100] = core::array::from_fn(|i| i as u8);
    {
        let fs = TestFs::new(&mut stub).unwrap();

        for i in 0..12 {
            let mut fw = fs.create_file().unwrap();
            fw.write(&(i as u32 + 34).to_le_bytes()).unwrap();

            fw.write(&buf[i..i * 2 + 30]).unwrap();
        }

        let i = 12;
        let mut fw = fs.create_file().unwrap();
        fw.write(&(i as u32 * 2 + 34).to_le_bytes()).unwrap();
        fw.write(&buf[i..i * 3 + 30]).unwrap();

        {
            let mut inner = fs.inner.borrow_mut();
            assert_eq!(inner.oldest_file_index, 12);
            assert_eq!(inner.next_file_index, 44);
            assert_eq!(inner.read_u32(12).unwrap(), 0x140);
            assert_eq!(inner.read_u32(40).unwrap(), 0xe0);
            inner.recycle_dir_page().unwrap();
        }
        {
            let mut inner = fs.inner.borrow_mut();
            assert_eq!(inner.oldest_file_index, 12);
            assert_eq!(inner.next_file_index, 36);
            assert_eq!(inner.read_u32(12).unwrap(), 0x1a0);
            assert_eq!(inner.read_u32(32).unwrap(), 0xe0);
        }

        let mut fw = fs.create_file().unwrap();
        fw.write(&6u32.to_le_bytes()).unwrap();
        fw.write(&buf[..2]).unwrap();

        {
            let mut inner = fs.inner.borrow_mut();
            inner.recycle_dir_page().unwrap();
        }
        {
            let mut inner = fs.inner.borrow_mut();
            assert_eq!(inner.oldest_file_index, 12);
            assert_eq!(inner.next_file_index, 40);
            assert_eq!(inner.read_u32(12).unwrap(), 0x1a0);
            assert_eq!(inner.read_u32(36).unwrap(), 0x120);
            let index = inner.free_index;
            assert_eq!(index, 0x130);
            let len = inner.read_u32(index).unwrap();
            assert_eq!(len, u32::MAX >> 1);
        }
    }

    {
        let fs = TestFs::new(&mut stub).unwrap();
        {
            let mut fw = fs.create_file().unwrap();
            fw.write(&7u32.to_le_bytes()).unwrap();
            fw.write(&buf[..3]).unwrap();
            let inner = fs.inner.borrow();
            let index = inner.free_index;
            assert_eq!(index, 0x140);
        }
    }
}

#[test]
fn marker_spans_end_recycle_dir_page() {
    let mut stub = NorFlashStub::<DEFAULT_DSIZE>::default();
    let buf: [u8; 200] = core::array::from_fn(|i| i as u8);

    let active = RefCell::new(0);
    let a1 = &active;
    let oldest_offset = 272u32.to_le_bytes();
    let observer = move |a: Action, buf: &mut [u8]| {
        if *a1.borrow() == 1 {
            match a {
                Action::Write(128, data) if data[..4] == oldest_offset => {
                    buf[128..132].copy_from_slice(&oldest_offset);
                    return Err(FlashStubError::Unknown);
                }
                _ => {}
            }
        }

        Ok(())
    };
    stub.observer = Some(&observer);

    {
        let fs = TestFs::new(&mut stub).unwrap();

        for i in 0..4 {
            let mut fw = fs.create_file().unwrap();
            let i = i * 9 + 81;
            fw.write(&((i + 4) as u32).to_le_bytes()).unwrap();
            fw.write(&buf[..i]).unwrap();
        }

        {
            {
                let mut a2 = active.borrow_mut();
                *a2 = 1;
            }
            let mut inner = fs.inner.borrow_mut();
            assert_eq!(inner.free_index, 496);
            assert!(inner.recycle_dir_page().is_err());
            assert_eq!(inner.next_file_offset(0x1f0, u32::MAX >> 1).unwrap(), 272);
        }
    }

    {
        let mut a2 = active.borrow_mut();
        *a2 = 0;
    }

    stub.buf[0] = 0; // corrupt the dir

    {
        // recover_dir_from_store
        let fs = TestFs::new(&mut stub).unwrap();
        {
            let mut inner = fs.inner.borrow_mut();
            assert_eq!(inner.oldest_file_index, 12);
            assert_eq!(inner.next_file_index, 20);
            assert_eq!(inner.free_index, 496);
            assert_eq!(inner.read_u32(80).unwrap(), u32::MAX >> 1);
            assert_eq!(inner.read_u32(12).unwrap(), 0x110);
            assert_eq!(inner.read_u32(16).unwrap(), 0x180);
        }
    }
}

#[test]
fn nor_flash_stub() {
    let _err = FlashStubError::Unknown;

    let mut stub: NorFlashStub<256> = NorFlashStub::default();

    const BASE: u32 = 10;

    let mut buf = [0, 1, 2, 3];
    stub.read(BASE, &mut buf).unwrap();
    assert_eq!(&buf, &[0, 0, 0, 0]);

    let buf2 = [0xff, 0xf2, 0xf4, 0xf8];

    stub.write(BASE, &buf2).unwrap();

    stub.read(BASE, &mut buf).unwrap();
    assert_eq!(&buf, &[0, 0, 0, 0]);

    stub.erase(BASE, BASE + 16).unwrap();
    stub.read(BASE, &mut buf).unwrap();
    assert_eq!(&buf, &[0xff, 0xff, 0xff, 0xff]);

    stub.write(BASE, &buf2).unwrap();

    stub.read(BASE, &mut buf).unwrap();
    assert_eq!(&buf, &buf2);

    let buf2 = [0x12, 0x4f, 0x8f, 0x4f];

    stub.write(BASE, &buf2).unwrap();

    stub.read(BASE, &mut buf).unwrap();
    assert_eq!(&buf, &[18, 66, 132, 72]);
}

const FORMATTED: [u8; 16] = [
    110, 15, 172, 11, 64, 0, 0, 2, 0, 2, 0, 0, 255, 255, 255, 255,
];

fn header_pattern(suffix: &[u8]) -> [u8; FORMATTED.len()] {
    let mut pattern = FORMATTED;
    pattern[FORMATTED.len() - suffix.len()..].copy_from_slice(suffix);
    pattern
}

#[test]
fn uninitialized_disk() {
    let mut stub = NorFlashStub::<DEFAULT_DSIZE>::default();

    {
        let _ = TestFs::new(&mut stub).unwrap();
    }
    assert_eq!(&stub.buf[..16], &FORMATTED);
    assert_eq!(&stub.buf[64..(64 + 16)], &FORMATTED);

    stub.buf[12] = 0;
    stub.buf[13] = 0x01;
    stub.buf[14] = 0;
    stub.buf[15] = 0;

    {
        let fs = TestFs::new(&mut stub).unwrap();

        {
            let inner = fs.inner.borrow();
            assert_eq!(inner.oldest_file_index, 12);
            assert_eq!(inner.next_file_index, 16);
            assert_eq!(inner.free_index, 256);
        }
        assert_eq!(&stub.buf[..16], &header_pattern(&[0, 1, 0, 0]));
    }

    // format disk
    stub.buf[63] = 0xf0;

    {
        let _ = TestFs::new(&mut stub).unwrap();
        assert_eq!(&stub.buf[..16], &FORMATTED);
    }

    // test corrupt empty dir
    stub.buf[1] = 0;
    stub.buf[15] = 0;

    {
        let _ = TestFs::new(&mut stub).unwrap();
        assert_eq!(&stub.buf[..16], &FORMATTED);
    }

    assert_eq!(&stub.buf[64..80], &FORMATTED);

    stub.buf[64 + 15] = 0;

    {
        let _ = TestFs::new(&mut stub).unwrap();
        assert_eq!(&stub.buf[64..80], &header_pattern(&[255, 255, 255, 0]));
    }
}

#[test]
fn align() {
    type FS<'a> = NorflashRingFsInner<'a, NorFlashStub<'a, 256>, 0, 256, 64, 16, 6>;

    assert_eq!(FS::align_start_erase(63), 0);
    assert_eq!(FS::align_start_erase(64), 64);
    assert_eq!(FS::align_start_erase(65), 64);
    assert_eq!(FS::align_start_erase(129), 128);
    assert_eq!(FS::align_start_erase(0), 0);

    assert_eq!(FS::align_next_page(15), 16);
    assert_eq!(FS::align_next_page(16), 16);
    assert_eq!(FS::align_next_page(17), 32);
    assert_eq!(FS::align_next_page(0), 0);
}

#[test]
fn reader_writer_conflicts() {
    let mut stub = NorFlashStub::<DEFAULT_DSIZE>::default();

    let fs = TestFs::new(&mut stub).unwrap();
    {
        // only one writer; no readers
        let mut fw = fs.create_file().unwrap();
        fw.write(&5u32.to_le_bytes()).unwrap();
        assert!(!fw.is_closed());
        assert!(matches!(fs.create_file(), Err(RingFsError::InUse)));
        assert!(matches!(
            fs.file_reader_by_index(0),
            Err(RingFsError::InUse)
        ));

        fw.write(&[60]).unwrap();
        assert!(fw.is_closed());

        // multi readers; no writers
        let mut fr1 = fs.file_reader_by_index(0).unwrap();
        let mut fr2 = fs.file_reader_by_index(0).unwrap();

        let mut buf1 = [0; 5];
        let mut buf2 = [0; 5];

        assert_eq!(fr1.read(&mut buf1).unwrap(), 5);
        assert!(matches!(fs.create_file(), Err(RingFsError::InUse)));
        assert_eq!(fr2.read(&mut buf2).unwrap(), 5);

        assert_eq!(&buf2, &[5, 0, 0, 0, 60]);
        assert_eq!(&buf1, &buf2);
        {
            let _fw = fs.create_file().unwrap();
        } // drop closes
        let mut fw = fs.create_file().unwrap();
        fw.close();

        let mut fr2 = fs.file_reader_by_index(0).unwrap();
        assert_eq!(fr2.read(&mut buf2).unwrap(), 5);

        assert_eq!(&buf2, &[5, 0, 0, 0, 60]);
    }
}

#[test]
fn seek() {
    let mut stub = NorFlashStub::<DEFAULT_DSIZE>::default();

    let fs = TestFs::new(&mut stub).unwrap();
    {
        let mut fw = fs.create_file().unwrap();
        fw.write(&8u32.to_le_bytes()).unwrap();
        fw.write(&[6, 5, 4, 3]).unwrap();

        let mut fr = fs.file_reader_by_index(0).unwrap();
        let mut buf = [0];

        fr.read(&mut buf).unwrap();
        assert_eq!(buf[0], 8);
        fr.seek(4);
        fr.read(&mut buf).unwrap();
        assert_eq!(buf[0], 6);

        fr.seek(0);
        fr.read(&mut buf).unwrap();
        assert_eq!(buf[0], 8);

        fr.seek(40);
        let err = fr.read(&mut buf).err().unwrap();
        assert!(matches!(err, RingFsError::OutOfBounds));
        assert!(fr.is_closed());
    }
}

#[test]
fn create_file() {
    let mut stub = NorFlashStub::<DEFAULT_DSIZE>::default();
    {
        let fs = TestFs::new(&mut stub).unwrap();

        let start = {
            let mut fw = fs.create_file().unwrap();

            fw.write(&134u32.to_le_bytes()).unwrap();

            fw.write(b"0123456789abcdefghijklmnopqrstuvwxyz!@#$")
                .unwrap();
            let buf: [u8; 90] = core::array::from_fn(|i| i as u8);
            assert!(!fw.is_closed());
            fw.write(&buf).unwrap();
            assert!(fw.is_closed(), "should auto close once full");

            fw.location()
        };
        {
            let mut inner = fs.inner.borrow_mut();
            assert_eq!(inner.read_u32(12).unwrap(), 80);
        }

        {
            let mut buf = [0; 140];

            let mut fr = fs.file_reader_by_index(0).unwrap();

            {
                let n = fr.read(&mut buf).unwrap();

                assert_eq!(n, 134);
                let (len, data) = buf.split_at(4);
                assert_eq!(len, 134u32.to_le_bytes());
                assert_eq!(&data[..10], b"0123456789");

                assert_eq!(data[..130].iter().map(|n| *n as usize).sum::<usize>(), 7545);

                assert!(fr.is_closed());
            }
            let mut fr = fs.file_reader_by_location(start).unwrap();
            fr.read(&mut buf).unwrap();
        }
    }

    assert_eq!(
        stub.buf[80..(80 + 134)]
            .iter()
            .map(|n| *n as usize)
            .sum::<usize>(),
        7545 + 134
    );
}

#[test]
fn fill_disk() {
    let mut stub = NorFlashStub::<DEFAULT_DSIZE>::default();
    {
        let fs = TestFs::new(&mut stub).unwrap();
        let buf: [u8; 154] = core::array::from_fn(|i| {
            if i < 4 {
                154u32.to_le_bytes()[i]
            } else {
                i as u8
            }
        });

        for i in 0..3 {
            let mut fw = fs.create_file().unwrap();
            fw.write(&54u32.to_le_bytes()).unwrap();
            fw.write(&buf[(i * 50)..(50 + i * 50)]).unwrap();
        }

        let mut fw = fs.create_file().unwrap();
        fw.write(&buf).unwrap();
        {
            let mut inner = fs.inner.borrow_mut();
            assert_eq!(inner.read_u32(80).unwrap(), 54); // lenght of first file
            assert_eq!(inner.free_index, 432);
            assert_eq!(inner.next_file_index, 28);
        }

        // should be too big to fit at end; need to split in two
        let mut fw = fs.create_file().unwrap();
        fw.write(&104u32.to_le_bytes()).unwrap();

        fw.write(&buf[..100]).unwrap();
        assert!(fw.is_closed());

        // read_file should work with split file
        let mut fr = fs.file_reader_by_index(0).unwrap();
        let mut rbuf = [0; 104];
        assert_eq!(fr.read(&mut rbuf).unwrap(), 104);
        assert_eq!(&buf[..100], &rbuf[4..]);

        let mut fw = fs.create_file().unwrap();
        fw.write(&24u32.to_le_bytes()).unwrap();
        fw.write(&buf[130..150]).unwrap();

        {
            let mut inner = fs.inner.borrow_mut();
            assert_eq!(inner.free_index, 144);
            assert_eq!(inner.next_file_index, 36);
            assert_eq!(inner.read_u32(12).unwrap(), 0); // deleted
            assert_eq!(inner.read_u32(16).unwrap(), 0); // deleted
            assert_eq!(inner.read_u32(20).unwrap(), 208);
            assert_eq!(inner.read_u32(24).unwrap(), 272);
            assert_eq!(inner.read_u32(28).unwrap(), 432);
            assert_eq!(inner.read_u32(32).unwrap(), 0x70);

            assert_eq!(inner.read_u32(0x50).unwrap(), 32);
            assert_eq!(inner.read_u32(0x70).unwrap(), 24);
        }
    }
}

#[test]
fn recover_store_from_dir() {
    let mut stub = NorFlashStub::<DEFAULT_DSIZE>::default();
    {
        let fs = TestFs::new(&mut stub).unwrap();
        let buf: [u8; 154] = core::array::from_fn(|i| {
            if i < 4 {
                154u32.to_le_bytes()[i]
            } else {
                i as u8
            }
        });

        for i in 0..3 {
            let mut fw = fs.create_file().unwrap();
            fw.write(&54u32.to_le_bytes()).unwrap();
            fw.write(&buf[(i * 50)..(50 + i * 50)]).unwrap();
        }

        {
            let mut inner = fs.inner.borrow_mut();
            inner.delete_oldest().unwrap();
            inner.delete_oldest().unwrap();
            inner.erase(64, 128).unwrap();
        }
    }

    {
        // recover_store_from_dir
        let fs = TestFs::new(&mut stub).unwrap();

        let mut inner = fs.inner.borrow_mut();
        assert_eq!(inner.free_index, 80);
        assert_eq!(inner.next_file_index, 24);
        assert_eq!(inner.read_u32(12).unwrap(), 0); // deleted
        assert_eq!(inner.read_u32(16).unwrap(), 0); // deleted
        assert_eq!(inner.read_u32(20).unwrap(), 208); // deleted
        assert_eq!(inner.read_u32(24).unwrap(), u32::MAX);

        assert_eq!(inner.read_u32(208).unwrap(), 54);
    }
    assert_eq!(&stub.buf[64..80], &FORMATTED);
}

#[test]
fn recover_dir_from_store() {
    let mut stub = NorFlashStub::<DEFAULT_DSIZE>::default();

    let active = RefCell::new(0);
    let a1 = &active;
    let observer = move |a: Action, _buf: &mut [u8]| {
        match *a1.borrow() {
            1 => {
                if let Action::Erase(128, 192) = a {
                    //                _buf[0] = 0;
                    return Err(FlashStubError::Unknown);
                }
            }
            2 => {
                if let Action::Erase(0, 64) = a {
                    _buf[0] = 0;
                    return Err(FlashStubError::Unknown);
                }
            }
            _ => {}
        }

        Ok(())
    };
    stub.observer = Some(&observer);

    {
        let fs = TestFs::new(&mut stub).unwrap();
        let buf: [u8; 154] = core::array::from_fn(|i| {
            if i < 4 {
                154u32.to_le_bytes()[i]
            } else {
                i as u8
            }
        });

        for i in 0..7 {
            let mut fw = fs.create_file().unwrap();
            fw.write(&54u32.to_le_bytes()).unwrap();
            let i = i % 3;
            fw.write(&buf[(i * 50)..(50 + i * 50)]).unwrap();
        }
        let inner = fs.inner.borrow();
        assert_eq!(inner.oldest_file_index, 16);
        assert_eq!(inner.next_file_index, 40);
        assert_eq!(inner.free_index, 96);
    }

    {
        // no recover needed here
        let fs = TestFs::new(&mut stub).unwrap();

        {
            let inner = fs.inner.borrow();
            assert_eq!(inner.oldest_file_index, 16);
            assert_eq!(inner.next_file_index, 40);
            assert_eq!(inner.free_index, 96);
        }

        {
            {
                let mut a2 = active.borrow_mut();
                *a2 = 1;
            }
            let mut inner = fs.inner.borrow_mut();
            assert!(inner.recycle_dir_page().is_err());
        }
        let mut a2 = active.borrow_mut();
        *a2 = 0;
    }

    {
        // no recover needed here either
        let fs = TestFs::new(&mut stub).unwrap();

        let mut inner = fs.inner.borrow_mut();
        assert_eq!(inner.free_index, 96);
        assert_eq!(inner.next_file_index, 40);
        assert_eq!(inner.read_u32(12).unwrap(), 0); // deleted
        assert_eq!(inner.read_u32(16).unwrap(), 0); // deleted
        assert_eq!(inner.read_u32(20).unwrap(), 208); // deleted
        assert_eq!(inner.read_u32(36).unwrap(), 464);

        assert_eq!(inner.read_u32(208).unwrap(), 54);

        {
            {
                let mut a2 = active.borrow_mut();
                *a2 = 2;
            }
            assert!(inner.recycle_dir_page().is_err());
        }
        let mut a2 = active.borrow_mut();
        *a2 = 0;
    }

    {
        // recover_dir_from_store
        let fs = TestFs::new(&mut stub).unwrap();

        let mut fr = fs.file_reader_by_index(0).unwrap();
        let mut buf = [0; 54];
        assert_eq!(fr.read(&mut buf).unwrap(), 54);
        assert_eq!(buf.iter().map(|i| *i as usize).sum::<usize>(), 1427);

        let mut inner = fs.inner.borrow_mut();
        assert_eq!(inner.free_index, 96);
        assert_eq!(inner.next_file_index, 32);
        assert_eq!(inner.read_u32(12).unwrap(), 0xd0);
        assert_eq!(inner.read_u32(16).unwrap(), 0x110);
        assert_eq!(inner.read_u32(28).unwrap(), 0x1d0);

        assert_eq!(inner.read_u32(80).unwrap(), 16);
    }

    assert_eq!(&stub.buf[..12], &FORMATTED[..12]);
}

const fn align_next(offset: u32, align: u32) -> u32 {
    let offset = offset + align - 1;
    offset - (offset % align)
}

/// run like `std::eprintln!("{}", disk_stats(&fs));`
#[allow(dead_code)]
fn disk_stats<
    const FSIZE: usize,
    const BASE: usize,
    const SIZE: usize,
    const DIR_SIZE: u32,
    const PAGE_SIZE: usize,
    const MAX_FILES: u32,
>(
    fs: &NorflashRingFs<NorFlashStub<FSIZE>, BASE, SIZE, DIR_SIZE, PAGE_SIZE, MAX_FILES>,
) -> String {
    let mut inner = fs.inner.borrow_mut();
    let mut buf = [0; SIZE];
    inner.read(0, &mut buf).unwrap();
    let mut out = String::new();

    out += "======================= FS Info ============================\n";
    out += format!(
        "Size {}({:04x}), directory_indices {}..{}\nFiles: (oldest..newest)\n",
        SIZE, SIZE, inner.oldest_file_index, inner.next_file_index,
    )
    .as_str();

    let range = inner.oldest_file_index as usize..inner.next_file_index as usize;
    for (i, (desc, index)) in buf[range.clone()]
        .chunks(4)
        .zip(range.step_by(4))
        .enumerate()
    {
        let offset = u32::from_le_bytes(desc.try_into().unwrap()) as usize;
        let len = u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());

        out += format!(
            "id:{} @{} from:{:04x} len:{:04x}  (next: {:04x})\n",
            i,
            index,
            offset,
            len,
            align_next(offset as u32 + len, PAGE_SIZE as u32),
        )
        .as_str();
    }

    out
}

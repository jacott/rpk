use core::cell::RefCell;

use embassy_futures::block_on;
use mapper::{ControlMessage, ControlSignal};

use crate::norflash_ring_fs::test::{DefaultNorFlashStub, TestFs};

use super::*;

extern crate std;

macro_rules! setup {
    ($ci:ident, $x:tt) => {
        setup!($ci, _ctl_sig, _fs, $x);
    };
    ($ci:ident, $ctl_sig:ident, $fs:ident, $x:tt) => {
        let mut stub = DefaultNorFlashStub::default();
        let $fs = TestFs::new(&mut stub).unwrap();
        let $ctl_sig = ControlSignal::default();

        let mut $ci = ConfigInterface::new(&$fs, &$ctl_sig);
        block_on(async { $x })
    };
}

#[test]
fn reset_from_usb() {
    setup!(ci, {
        std::thread_local! {
            static CALL_COUNT: RefCell<usize> = const {RefCell::new(0)};
        }
        fn myreset() {
            CALL_COUNT.with_borrow_mut(|c| *c += 1);
        }
        firmware_functions::handle_reset(Some(&myreset));

        assert_eq!(CALL_COUNT.with_borrow(|c| *c), 0);

        ci.receive(&[msg::RESET_KEYBOARD], &mut [0; 1]).await;
        assert_eq!(CALL_COUNT.with_borrow(|c| *c), 1);
    });
}

#[test]
fn reset_to_usb_boot_from_usb() {
    setup!(ci, {
        std::thread_local! {
            static CALL_COUNT: RefCell<usize> = const {RefCell::new(0)};
        }
        fn myreset() {
            CALL_COUNT.with_borrow_mut(|c| *c += 1);
        }
        firmware_functions::handle_reset_to_usb_boot(Some(&myreset));

        assert_eq!(CALL_COUNT.with_borrow(|c| *c), 0);

        ci.receive(&[msg::RESET_TO_USB_BOOT], &mut [0; 1]).await;
        assert_eq!(CALL_COUNT.with_borrow(|c| *c), 1);
    });
}

#[test]
fn read_file() {
    setup!(ci, ctl_sig, fs, {
        let mut fw = fs.create_file().unwrap();
        let mut data: [u8; 20] = core::array::from_fn(|i| i as u8);
        data[0..4].copy_from_slice(&20u32.to_le_bytes());
        fw.write(&data).unwrap();
        let mut wb = [0; 30];
        let ans = ci
            .receive(&[msg::READ_FILE_BY_INDEX, 0, 0, 0, 0], &mut wb)
            .await;
        assert_eq!(ans, 24);
        assert_eq!(&wb[4..24], &data);
        assert_eq!(
            fw.location(),
            u32::from_le_bytes((&wb[..4]).try_into().unwrap())
        );
    });
}

#[test]
fn save_config() {
    let mut write_buf = [0; 10];
    setup!(ci, ctl_sig, fs, {
        {
            // load small layout file
            assert_eq!(
                ci.receive(&[msg::OPEN_SAVE_CONFIG], &mut write_buf).await,
                0
            );

            let mut data = [msg::CLOSE_SAVE_CONFIG, 6, 0, 0, 0, 1, 2];
            ci.receive(&data, &mut write_buf).await;

            let Some(ControlMessage::LoadLayout {
                file_location: location,
            }) = ctl_sig.try_take()
            else {
                panic!("expected LoadLayout()")
            };

            let mut fr = fs.file_reader_by_location(location).unwrap();
            assert_eq!(fr.read(&mut data).unwrap(), 6);
            assert_eq!(data[..6], [6, 0, 0, 0, 1, 2]);
            assert!(matches!(ci.rcv_state, ReceiveState::Idle));
        }

        {
            // load larger layout file
            ci.receive(&[msg::OPEN_SAVE_CONFIG], &mut write_buf).await;

            let mut data: [u8; 190] = core::array::from_fn(|i| i as u8);
            data[..4].copy_from_slice(&[190, 0, 0, 0]);
            ci.receive(&data[..64], &mut write_buf).await;
            ci.receive(&data[64..128], &mut write_buf).await;

            assert!(ctl_sig.try_take().is_none());

            data[127] = msg::CLOSE_SAVE_CONFIG;
            ci.receive(&data[127..], &mut write_buf).await;

            let Some(ControlMessage::LoadLayout {
                file_location: location,
            }) = ctl_sig.try_take()
            else {
                panic!("expected LoadLayout()")
            };

            let mut fr = fs.file_reader_by_location(location).unwrap();
            assert_eq!(fr.read(&mut data).unwrap(), 190);
            assert_eq!(data[..6], [190, 0, 0, 0, 4, 5]);
            assert_eq!(data[185..], [185, 186, 187, 188, 189]);
        }
    });
}

#[test]
fn config_file_iter() {
    let mut stub = DefaultNorFlashStub::default();
    let fs = TestFs::new(&mut stub).unwrap();
    let mut fw = fs.create_file().unwrap();
    let mut data: [u8; 31] = core::array::from_fn(|i| i as u8);
    data[0..4].copy_from_slice(&31u32.to_le_bytes());
    data[13] = 3;
    fw.write(&data).unwrap();
    let fr = fs.file_reader_by_index(0).unwrap();

    let iter = ConfigFileIter::new(fr);
    let ans: std::vec::Vec<u16> = iter.collect();
    assert_eq!(&ans, &[4625, 5139, 5653, 6167, 6681, 7195, 7709]);
}

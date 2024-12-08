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
        let mut stub = DefaultNorFlashStub::new();
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

        ci.receive(&[RESET_KEYBOARD]).await;
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

        ci.receive(&[RESET_TO_USB_BOOT]).await;
        assert_eq!(CALL_COUNT.with_borrow(|c| *c), 1);
    });
}

#[test]
fn save_config() {
    setup!(ci, ctl_sig, fs, {
        {
            // load small layout file
            ci.receive(&[OPEN_SAVE_CONFIG]).await;

            let mut data = [CLOSE_SAVE_CONFIG, 6, 0, 0, 0, 1, 2];
            ci.receive(&data).await;

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
            ci.receive(&[OPEN_SAVE_CONFIG]).await;

            let mut data: [u8; 190] = core::array::from_fn(|i| i as u8);
            data[..4].copy_from_slice(&[190, 0, 0, 0]);
            ci.receive(&data[..64]).await;
            ci.receive(&data[64..128]).await;

            assert!(ctl_sig.try_take().is_none());

            data[127] = CLOSE_SAVE_CONFIG;
            ci.receive(&data[127..]).await;

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

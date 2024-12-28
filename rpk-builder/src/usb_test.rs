extern crate std;
use super::*;
use embassy_futures::{
    block_on,
    select::{select3, Either3},
};
use embassy_time::Timer;
use rpk_common::usb_vendor_message as msg;
use rpk_firmware::{
    flash_test_stub::NorFlashStub,
    mapper::ControlSignal,
    norflash_ring_fs::NorflashRingFs,
    ring_fs::RingFs,
    usb_test_stub::{MyDriver, MyEndpointIn, MyEndpointOut},
};

const DEFAULT_DSIZE: usize = 512;
const DIR_SIZE: u32 = 64;
const PAGE_SIZE: usize = 16;
const MAX_FILES: u32 = 20;

pub(crate) type DefaultNorFlashStub<'f> = NorFlashStub<'f, DEFAULT_DSIZE>;
pub(crate) type TestMaxFilesFs<'d, 'f, const K: usize, const M: u32> =
    NorflashRingFs<'d, NorFlashStub<'f, K>, 0, K, DIR_SIZE, PAGE_SIZE, M>;
pub(crate) type TestFs<'d, 'f, const K: usize> = TestMaxFilesFs<'d, 'f, K, MAX_FILES>;

macro_rules! setup {
    ($fs:ident, $messages:ident, $cfg_ep:ident $x:tt) => {
        block_on(async {
            let ep_in = MyEndpointIn::default();
            let ep_out = MyEndpointOut::default();
            let mut stub = DefaultNorFlashStub::default();
            let $fs = TestFs::new(&mut stub).unwrap();
            let ctl_sig = ControlSignal::default();

            let $messages = ep_in.messages.clone();
            let mut $cfg_ep = ConfigEndPoint::<'_, MyDriver> {
                write_ep: ep_in,
                read_ep: ep_out,
                config_interface: ConfigInterface::new(&$fs, &ctl_sig),
            };
            $x
        });
    };
}

#[test]
fn config_builder() {
    let mut b = ConfigBuilder {
        manufacturer: "Jacott",
        product: "Macropad",
        vendor_id: 0xba5e,
        product_id: 0xfade,
        serial_number: "rpk:123",
        max_power: 150,
    };

    b.manufacturer = "Jacott";

    assert_eq!(b.manufacturer, "Jacott");
}

#[test]
fn config_end_point() {
    setup!(
        fs,
        messages,
        cfg_ep {
            let mut fw = fs.create_file().unwrap();
            fw.write(&[8,0,0,0,6,7,8,9]).unwrap();
            cfg_ep.read_ep.messages.send(std::vec![msg::READ_FILE_BY_INDEX,0,0,0,0]).await;
            match select3(cfg_ep.run(), messages.receive(), Timer::after_millis(200)).await {
                Either3::First(_) => panic!("Unexpected run end"),
                Either3::Second(msg) => {
                    assert_eq!(msg.len(), 12);
                    assert_eq!(msg, &[80, 0, 0, 0, 8, 0, 0, 0, 6, 7, 8, 9])
                },
                Either3::Third(_) => panic!("Timed out"),

        }
    }
    );
}

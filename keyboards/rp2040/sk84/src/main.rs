#![no_std]
#![no_main]

#[cfg(feature = "defmt")]
use defmt_rtt as _;

use embassy_executor::Spawner;
use embassy_rp::peripherals::{FLASH, USB};
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::{bind_interrupts, flash};
use embassy_rp::{
    flash::Async,
    gpio::{AnyPin, Input, Output},
};
use rpk_firmware::norflash_ring_fs::NorflashRingFs;
use rpk_macros::layout_config;
use static_cell::StaticCell;

// ---------------- User Config ------------------
const LAYOUT_MAPPING: &[u16] = layout_config!(
    r#"
[matrix:6x14]

0x00 = u0  u1 2  3  4  5   f1     f5   6   7  8  9  u3 u4
0x10 = esc 1  w  e  r  t   f2     f6   y   u  i  o  0  f9
0x20 = `   q  s  d  f  g   f3     f7   h   j  k  l  p  f10
0x30 = -   a  x  c  v  b   f4     f8   n   m  ,  .  \; f11
0x40 = =   z  la lc ls ent tab    bksp spc rs rc ra /  f12
0x50 = [   ]  \\ \' lg pgup pgdn  del  mnu rg left down up right
"#
);

// USB config
const VENDOR_ID: u16 = 0x6e0f;
const PRODUCT_ID: u16 = 0x0002;
const MANUFACTURER: &str = "Jacott";
const PRODUCT: &str = "RPK sk84";
const SERIAL_NUMBER: &str = "rpk:0002";
const MAX_POWER: u16 = 100;

// Size of LAYOUT MAPPING + runtime overheads for active layers and macros
const LAYOUT_MAX: usize = 8 * 1024;

// Flash file system
const FLASH_SIZE: usize = 2 * 1024 * 1024; // Full size of flash disk
const FS_BASE: usize = 0x100000; // where the file-system storage starts
const FS_SIZE: usize = FLASH_SIZE - FS_BASE; // leaving rest of disk for storage

// Key switch configuration
const ROW_IS_OUTPUT: bool = true;
macro_rules! config_pins {
    (peripherals: $p:ident) => {
        rpk_firmware::config_matrix_pins_rp!(peripherals: $p,
            input: [PIN_0, PIN_1, PIN_2, PIN_3, PIN_4, PIN_5, PIN_6,
                PIN_13, PIN_14, PIN_15, PIN_16, PIN_17, PIN_18, PIN_19],
            output: [PIN_7, PIN_8, PIN_9, PIN_10, PIN_11, PIN_12])
    };
}

// ----------- End of user config ----------------

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

fn reset() {
    cortex_m::peripheral::SCB::sys_reset()
}

fn reset_to_usb_boot() {
    embassy_rp::rom_data::reset_to_usb_boot(0, 0);
    #[allow(clippy::empty_loop)]
    loop {
        // Waiting for the reset to happen
    }
}

// Use above consts to configure flash and file-system
type Flash = flash::Flash<'static, FLASH, Async, FLASH_SIZE>;
type Rfs = NorflashRingFs<
    'static,
    Flash,
    FS_BASE,
    FS_SIZE,
    { flash::ERASE_SIZE as u32 },
    { flash::PAGE_SIZE },
>;

static FLASH: StaticCell<Flash> = StaticCell::new();
static RFS: StaticCell<Rfs> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let p = embassy_rp::init(Default::default());

    let driver = Driver::new(p.USB, Irqs);

    let (input_pins, output_pins) = config_pins!(peripherals: p);

    let flash: &'static mut Flash = FLASH.init(Flash::new(p.FLASH, p.DMA_CH0));
    let fs: &'static Rfs = RFS.init(Rfs::new(flash).unwrap());

    let builder = rpk_firmware::KeyboardBuilder::new(
        VENDOR_ID,
        PRODUCT_ID,
        fs,
        driver,
        input_pins,
        output_pins,
        LAYOUT_MAPPING,
    )
    .reset(&reset)
    .reset_to_usb_boot(&reset_to_usb_boot)
    .manufacturer(MANUFACTURER)
    .product(PRODUCT)
    .serial_number(SERIAL_NUMBER)
    .max_power(MAX_POWER);

    let keyboard = builder.build::<ROW_IS_OUTPUT, LAYOUT_MAX>();
    keyboard.run(spawner).await;
}

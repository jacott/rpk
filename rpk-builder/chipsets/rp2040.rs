#![cfg_attr(any(), rustfmt_skip)]
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
use static_cell::StaticCell;

// ---------------- User Config ------------------
const LAYOUT_MAPPING: &[u16] = {{LAYOUT_MAPPING/[]}};

// Keyboard specific config
const VENDOR_ID: u16 = {{VENDOR_ID/0xc0de}};
const PRODUCT_ID: u16 = {{PRODUCT_ID/0xcafe}};
const ROW_IS_OUTPUT: bool = {{ROW_IS_OUTPUT/true}};
// see main fn below for more keyboard specific config

// Size of Largest possible LAYOUT_MAPPING
const LAYOUT_MAX: usize = {{LAYOUT_MAX/8 * 1024}};

const FLASH_SIZE: usize = {{FLASH_SIZE/2 * 1024 * 1024}}; // Full size of flash disk
const FS_BASE: usize = {{FS_BASE/0x100000}}; // where the file-system storage starts
const FS_SIZE: usize = {{FS_SIZE/FLASH_SIZE - FS_BASE}}; // leaving rest of disk for storage

const MANUFACTURER: &str = "{{MANUFACTURER/manufacturer}}";
const PRODUCT: &str = "{{PRODUCT/product}}";
const SERIAL_NUMBER: &str = "{{SERIAL_NUMBER/rpk:serial_number}}";
const MAX_POWER: u16 = {{MAX_POWER/100}};

// ----------- End of user config ----------------

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

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let p = embassy_rp::init(Default::default());
    let driver = Driver::new(p.USB, Irqs);
    let flash: &'static mut Flash = FLASH.init(Flash::new(p.FLASH, p.DMA_CH0));
    let fs: &'static Rfs = RFS.init(Rfs::new(flash).unwrap());

    // ---------- Config pins here -----------------
    let (input_pins, output_pins) = rpk_firmware::config_matrix_pins_rp!(peripherals: p,
        input: {{INPUT_PINS/[]}}, output: {{OUTPUT_PINS/[]}});

    // ------ Configure rest of keyboard here ------
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
    // -------------- End of config ----------------

    let keyboard = builder.build::<ROW_IS_OUTPUT, LAYOUT_MAX>();
    keyboard.run(spawner).await;
}

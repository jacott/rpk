#![no_std]
#![no_main]

rpk_builder::configure_keyboard!();

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) -> ! {
    let p = rpk_builder::rp::init(Default::default());
    let driver = Driver::new(p.USB, Irqs);
    let (input_pins, output_pins) = config_pins!(peripherals: p);
    let flash = Flash::new(p.FLASH, p.DMA_CH0);

    run_keyboard!(spawner, driver, input_pins, output_pins, flash);
}

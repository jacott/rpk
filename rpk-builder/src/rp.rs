use crate::{config, key_scanner, mapper, ring_fs, NoopRawMutex};
pub use embassy_rp::{bind_interrupts, flash, gpio, init, peripherals, rom_data, usb};

pub async fn run_mapper<
    const ROW_COUNT: usize,
    const COL_COUNT: usize,
    const LAYOUT_MAX: usize,
    const SCANNER_BUFFER_SIZE: usize,
    const REPORT_BUFFER_SIZE: usize,
>(
    layout_mapping: &'static [u16],
    key_scan_channel: &'static key_scanner::KeyScannerChannel<NoopRawMutex, SCANNER_BUFFER_SIZE>,
    mapper_channel: &'static mapper::MapperChannel<NoopRawMutex, REPORT_BUFFER_SIZE>,
    fs: &'static dyn ring_fs::RingFs<'static>,
) {
    let mut mapper =
        mapper::Mapper::<'static, ROW_COUNT, COL_COUNT, LAYOUT_MAX, _, REPORT_BUFFER_SIZE>::new(
            mapper_channel,
        );
    {
        if !match fs.file_reader_by_index(0) {
            Ok(fr) => {
                if let Err(err) = mapper.load_layout(config::ConfigFileIter::new(fr).skip(2)) {
                    crate::info!("error loading layout {:?}", err);
                    false
                } else {
                    true
                }
            }
            Err(err) => {
                crate::info!("error reading layout {:?}", err);
                false
            }
        } {
            mapper.load_layout(layout_mapping.iter().copied()).unwrap();
        }
    }

    loop {
        if let mapper::ControlMessage::LoadLayout { file_location } =
            mapper.run(key_scan_channel).await
        {
            crate::debug!("load layout here {}", file_location);
            match fs.file_reader_by_location(file_location) {
                Ok(fr) => {
                    if let Err(err) = mapper.load_layout(config::ConfigFileIter::new(fr).skip(2)) {
                        crate::info!("error loading layout {:?}", err);
                        mapper.load_layout(layout_mapping.iter().copied()).unwrap();
                    }
                }
                Err(err) => crate::info!("error reading layout {:?}", err),
            }
        }
    }
}

#[macro_export]
macro_rules! rp_run_keyboard {
    () => {
        rpk_builder::configure_keyboard!();

        use rpk_builder::rp;
        use rp::{
            gpio, bind_interrupts,
            flash,
            flash::Async,
            usb::{Driver, InterruptHandler},
        };
        use rp::gpio::{AnyPin, Input, Output};
        use rp::peripherals::{FLASH, USB};
        use rpk_builder::norflash_ring_fs::NorflashRingFs;
        use rpk_builder::StaticCell;
        use rpk_builder::{mapper, key_scanner, ring_fs::RingFs,
            UsbState, UsbConfigurator, UsbBuffers, config, usb};
        use rpk_builder::NoopRawMutex;

        type ScanChannel = key_scanner::KeyScannerChannel<NoopRawMutex, SCANNER_BUFFER_SIZE>;
        type MapperChannel = mapper::MapperChannel<NoopRawMutex, REPORT_BUFFER_SIZE>;
        type ConfigInterface = config::ConfigInterface<'static, 'static>;

        static KEY_SCAN_CHANNEL: StaticCell<ScanChannel> = StaticCell::new();
        static MAPPER_CHANNEL: StaticCell<MapperChannel> = StaticCell::new();

        static FLASH: StaticCell<Flash> = StaticCell::new();
        static RFS: StaticCell<Rfs> = StaticCell::new();

        static USB_BUFFERS: StaticCell<UsbBuffers> = StaticCell::new();
        static USB_CONFIG: StaticCell<UsbConfigurator> = StaticCell::new();
        static SHARED_HID_STATE: StaticCell<UsbState> = StaticCell::new();
        static CONFIG_INTERFACE: StaticCell<ConfigInterface> = StaticCell::new();

        bind_interrupts!(struct Irqs {
            USBCTRL_IRQ => InterruptHandler<USB>;
        });

        fn reset() {
            cortex_m::peripheral::SCB::sys_reset()
        }

        fn reset_to_usb_boot() {
            rpk_builder::rp::rom_data::reset_to_usb_boot(0, 0);
            #[allow(clippy::empty_loop)]
            loop {
                // Waiting for the reset to happen
            }
        }

        const DEBOUNCE_TUNE: usize = 8;

        #[embassy_executor::task]
        async fn scanner(
            input_pins: [Input<'static>; INPUT_N],
            output_pins: [Output<'static>; OUTPUT_N],
            key_scan_channel: &'static ScanChannel
        ) {
            let mut scanner = key_scanner::KeyScanner::new(
                input_pins,
                output_pins,
                key_scan_channel,
            );
            scanner.run::<ROW_IS_OUTPUT, DEBOUNCE_TUNE>().await;
        }

        #[embassy_executor::task]
        async fn mapper(
            layout_mapping: &'static [u16],
            key_scan_channel: &'static ScanChannel,
            mapper_channel: &'static MapperChannel,
            fs: &'static dyn RingFs<'static>,
        ) {
            rp::run_mapper::<
            ROW_COUNT, COL_COUNT, LAYOUT_MAX,
            SCANNER_BUFFER_SIZE, REPORT_BUFFER_SIZE,
            >(layout_mapping, key_scan_channel, mapper_channel, fs).await;
        }

        #[embassy_executor::task]
        async fn hid_reporter(
            mapper_channel: &'static MapperChannel,
            shared_hid_writer: usb::SharedHidWriter<'static, Driver<'static, USB>>,

        ) {
            let mut reporter = rpk_builder::key_reporter::Reporter::new(shared_hid_writer);
            loop {
                reporter.report(mapper_channel.receive().await).await;
            }
        }

        #[embassy_executor::task]
        async fn hid_reader(
            shared_hid_reader: usb::SharedHidReader<'static, Driver<'static, USB>>,
        ) {
            rpk_builder::usb::HidEpHandler.run(shared_hid_reader).await;
        }

        #[embassy_executor::task]
        async fn vendor_interface(
            mut config_ep: usb::ConfigEndPoint<'static, Driver<'static, USB>>
        ) {
            config_ep.run().await;
        }

        #[embassy_executor::task]
        async fn timer(timer: &'static mapper::MapperTimer) {
            mapper::MapperTimer::run(timer).await;
        }

        #[embassy_executor::main]
        async fn main(spawner: embassy_executor::Spawner) -> ! {
            let p = rpk_builder::rp::init(Default::default());
            let (input_pins, output_pins) = config_pins!(peripherals: p);

            let key_scan_channel: &'static ScanChannel = KEY_SCAN_CHANNEL.init(ScanChannel::default());
            let mapper_channel: &'static MapperChannel = MAPPER_CHANNEL.init(MapperChannel::default());

            let flash: &'static mut Flash = FLASH.init(Flash::new(p.FLASH, p.DMA_CH0));
            let fs: &'static Rfs = RFS.init(Rfs::new(flash).unwrap());

            let shared_hid_state: &'static mut UsbState<'static> = SHARED_HID_STATE.init(UsbState::default());
            let driver = Driver::new(p.USB, Irqs);

            let usb_buffers: &'static mut UsbBuffers = USB_BUFFERS.init(UsbBuffers::default());
            let usb_config: &'static mut UsbConfigurator = USB_CONFIG.init(CONFIG_BUILDER.usb_configurator());

            let usb_builder = usb_config.usb_builder(driver, usb_buffers).unwrap();

            let (shared_hid_writer, shared_hid_reader, usb_builder) =
            CONFIG_BUILDER.shared_hid_iface(
                usb_config, shared_hid_state, usb_builder);

            let config_interface: &'static mut ConfigInterface =
            CONFIG_INTERFACE.init(ConfigInterface::new(fs, mapper_channel.control()));

            let (config_ep, usb_builder) = CONFIG_BUILDER.cfg_ep(config_interface, usb_builder);

            let mut usb = usb_builder.build();

            rpk_builder::firmware_functions::handle_reset(Some(&reset));
            rpk_builder::firmware_functions::handle_reset_to_usb_boot(Some(&reset_to_usb_boot));

            spawner.spawn(timer(mapper_channel.timer())).unwrap();
            spawner.spawn(scanner(input_pins, output_pins, key_scan_channel)).unwrap();
            spawner.spawn(mapper(&LAYOUT_MAPPING, key_scan_channel, mapper_channel, fs)).unwrap();
            spawner.spawn(hid_reporter(mapper_channel, shared_hid_writer)).unwrap();
            spawner.spawn(hid_reader(shared_hid_reader)).unwrap();
            spawner.spawn(vendor_interface(config_ep)).unwrap();

            usb.run().await;
        }

    };
}

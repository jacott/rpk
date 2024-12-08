use embassy_executor::Spawner;
use embassy_futures::select::{select3, select4};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_usb::{
    class::hid::{ReportId, RequestHandler},
    control::OutResponse,
    driver::{Driver, Endpoint, EndpointOut},
    Builder, Config,
};
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::digital::Wait;
use static_cell::StaticCell;

use crate::{
    config::{ConfigFileIter, ConfigInterface},
    firmware_functions::{self, ResetFn},
    info,
    key_reporter::Reporter,
    key_scanner::{KeyScanner, KeyScannerChannel},
    mapper::{self, Mapper, MapperChannel, MapperTimer},
    ring_fs::RingFs,
    usb::{Configurator, State, UsbBuffers, SHARED_REPORT_DESC},
};

// How many scanned keys can be stored before blocking scanner
const SCANNER_BUFFER_SIZE: usize = 32;
// How many key events can be sent to usb before blocking mapper
const REPORT_BUFFER_SIZE: usize = 32;
// How sensitive DEBOUNCE is higher numbers are more likely to stop debounce
const DEBOUNCE_TUNE: usize = 8;

// Configure comms channels
type ScanChannel = KeyScannerChannel<NoopRawMutex, SCANNER_BUFFER_SIZE>;
type MpChannel = MapperChannel<NoopRawMutex, REPORT_BUFFER_SIZE>;
type ScanConfigInterface = ConfigInterface<'static, 'static>;
static KEY_SCAN_CHANNEL: StaticCell<ScanChannel> = StaticCell::new();
static MAPPER_CHANNEL: StaticCell<MpChannel> = StaticCell::new();
static CONFIG_INTERFACE: StaticCell<ScanConfigInterface> = StaticCell::new();

static USB_CONFIG: StaticCell<Configurator> = StaticCell::new();
static USB_BUFFERS: StaticCell<UsbBuffers> = StaticCell::new();
static SHARED_HID_STATE: StaticCell<State> = StaticCell::new();

#[embassy_executor::task]
async fn timer_task(timer: &'static MapperTimer) {
    MapperTimer::run(timer).await;
}

async fn usb_run<'d, D: Driver<'d>, const REPORT_BUFFER_SIZE: usize>(
    usb_config: &'d mut Configurator<'d>,
    mut usb_builder: Builder<'d, D>,
    config_interface: &'d mut ConfigInterface<'d, 'd>,
    mapper_channel: &'d MapperChannel<NoopRawMutex, REPORT_BUFFER_SIZE>,
    keyboard_state: &'d mut State<'d>,
) {
    let (shared_hid_writer, shared_hid_reader) = usb_config.add_iface::<_, 10, 34>(
        &mut usb_builder,
        &SHARED_REPORT_DESC,
        true,
        1,
        1,
        keyboard_state,
    );

    let cfg_fut = {
        let mut function = usb_builder.function(0xFF, 0, 0);
        let mut interface = function.interface();
        let mut alt = interface.alt_setting(0xFF, 0, 0, None);
        let mut read_ep = alt.endpoint_bulk_out(64);
        async move {
            loop {
                read_ep.wait_enabled().await;
                loop {
                    let mut buf = [0; 64];
                    match read_ep.read(&mut buf).await {
                        Ok(n) => {
                            if n > 0 {
                                config_interface.receive(&buf[..n]).await
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    };

    let key_event_fut = async move {
        let mut reporter = Reporter::new(shared_hid_writer);
        loop {
            reporter.report(mapper_channel.receive().await).await;
        }
    };

    let mut request_handler = MyRequestHandler {};

    let hid_fut = shared_hid_reader.unwrap().run(false, &mut request_handler);

    let mut usb = usb_builder.build();
    let usb_fut = usb.run();

    select4(key_event_fut, usb_fut, hid_fut, cfg_fut).await;
}

async fn mapper_run<const ROW_COUNT: usize, const COL_COUNT: usize, const LAYOUT_MAX: usize>(
    mapper_channel: &'static MpChannel,
    key_scan_channel: &'static ScanChannel,
    fs: &'static dyn RingFs<'static>,
    layout_mapping: &'static [u16],
) {
    let mut mapper =
        Mapper::<ROW_COUNT, COL_COUNT, LAYOUT_MAX, _, REPORT_BUFFER_SIZE>::new(mapper_channel);

    {
        if !match fs.file_reader_by_index(0) {
            Ok(fr) => {
                if let Err(err) = mapper.load_layout(ConfigFileIter::new(fr).skip(2)) {
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
                    if let Err(err) = mapper.load_layout(ConfigFileIter::new(fr).skip(2)) {
                        crate::info!("error loading layout {:?}", err);
                        mapper.load_layout(layout_mapping.iter().copied()).unwrap();
                    }
                }
                Err(err) => crate::info!("error reading layout {:?}", err),
            }
        }
    }
}

pub struct KeyboardBuilder<
    D: Driver<'static>,
    I: InputPin + Wait,
    O: OutputPin,
    const INPUT_COUNT: usize,
    const OUTPUT_COUNT: usize,
> {
    reset: Option<ResetFn>,
    reset_to_usb_boot: Option<ResetFn>,
    usb_config: Config<'static>,
    fs: &'static dyn RingFs<'static>,
    driver: Option<D>,
    input_pins: Option<[I; INPUT_COUNT]>,
    output_pins: Option<[O; OUTPUT_COUNT]>,
    layout_mapping: &'static [u16],
}

pub struct Keyboard<
    D: Driver<'static>,
    I: InputPin + Wait,
    O: OutputPin,
    const ROW_IS_OUTPUT: bool,
    const INPUT_COUNT: usize,
    const OUTPUT_COUNT: usize,
    const LAYOUT_MAX: usize,
> {
    builder: KeyboardBuilder<D, I, O, INPUT_COUNT, OUTPUT_COUNT>,
    key_scan_channel: &'static KeyScannerChannel<NoopRawMutex, SCANNER_BUFFER_SIZE>,
    mapper_channel: &'static MapperChannel<NoopRawMutex, REPORT_BUFFER_SIZE>,
    config_interface: &'static mut ConfigInterface<'static, 'static>,
}

const fn check_settings<const INPUT_COUNT: usize, const OUTPUT_COUNT: usize>() -> bool {
    assert!(DEBOUNCE_TUNE < usize::BITS as usize);
    assert!(INPUT_COUNT > 0 && INPUT_COUNT < 128);
    assert!(OUTPUT_COUNT > 0 && OUTPUT_COUNT < 128);
    true
}

impl<
        D: Driver<'static> + 'static,
        I: InputPin + Wait,
        O: OutputPin,
        const ROW_IS_OUTPUT: bool,
        const INPUT_COUNT: usize,
        const OUTPUT_COUNT: usize,
        const LAYOUT_MAX: usize,
    > Keyboard<D, I, O, ROW_IS_OUTPUT, INPUT_COUNT, OUTPUT_COUNT, LAYOUT_MAX>
{
    const OKAY: bool = check_settings::<INPUT_COUNT, OUTPUT_COUNT>();

    pub async fn run(mut self, spawner: Spawner) -> ! {
        assert!(Self::OKAY);

        firmware_functions::handle_reset(self.builder.reset.take());
        firmware_functions::handle_reset_to_usb_boot(self.builder.reset_to_usb_boot.take());

        let mut scanner = KeyScanner::new(
            self.builder.input_pins.take().unwrap(),
            self.builder.output_pins.take().unwrap(),
            self.key_scan_channel,
        );
        let driver = self.builder.driver.take().unwrap();
        let usb_config = self.builder.usb_config;

        let usb_config: &'static mut Configurator = USB_CONFIG.init(Configurator::new(usb_config));
        let usb_buffers: &'static mut UsbBuffers = USB_BUFFERS.init(UsbBuffers::default());

        let usb_builder = usb_config.usb_builder(driver, usb_buffers).unwrap();

        let shared_hid_state: &'static mut State<'static> = SHARED_HID_STATE.init(State::default());

        let usb_fut = usb_run(
            usb_config,
            usb_builder,
            self.config_interface,
            self.mapper_channel,
            shared_hid_state,
        );

        let scanner_fut = scanner.run::<ROW_IS_OUTPUT, DEBOUNCE_TUNE>();

        spawner
            .spawn(timer_task(self.mapper_channel.timer()))
            .unwrap();

        if ROW_IS_OUTPUT {
            let mapper_fut = mapper_run::<
                OUTPUT_COUNT, // Output is Row
                INPUT_COUNT,  // Input is Column
                LAYOUT_MAX,
            >(
                self.mapper_channel,
                self.key_scan_channel,
                self.builder.fs,
                self.builder.layout_mapping,
            );

            select3(usb_fut, mapper_fut, scanner_fut).await;
        } else {
            let mapper_fut = mapper_run::<
                INPUT_COUNT,  // Input is Row
                OUTPUT_COUNT, // Output is Column
                LAYOUT_MAX,
            >(
                self.mapper_channel,
                self.key_scan_channel,
                self.builder.fs,
                self.builder.layout_mapping,
            );

            select3(usb_fut, mapper_fut, scanner_fut).await;
        }
        unreachable!()
    }
}

impl<
        D: Driver<'static> + 'static,
        I: InputPin + Wait,
        O: OutputPin,
        const INPUT_COUNT: usize,
        const OUTPUT_COUNT: usize,
    > KeyboardBuilder<D, I, O, INPUT_COUNT, OUTPUT_COUNT>
{
    pub fn new(
        vid: u16,
        pid: u16,
        fs: &'static dyn RingFs<'static>,
        driver: D,
        input_pins: [I; INPUT_COUNT],
        output_pins: [O; OUTPUT_COUNT],
        layout_mapping: &'static [u16],
    ) -> Self {
        Self {
            reset: None,
            reset_to_usb_boot: None,
            usb_config: Config::new(vid, pid),
            driver: Some(driver),
            fs,
            input_pins: Some(input_pins),
            output_pins: Some(output_pins),
            layout_mapping,
        }
    }

    pub fn reset(mut self, value: ResetFn) -> Self {
        self.reset = Some(value);
        self
    }

    pub fn reset_to_usb_boot(mut self, value: ResetFn) -> Self {
        self.reset_to_usb_boot = Some(value);
        self
    }

    pub fn manufacturer(mut self, value: &'static str) -> Self {
        self.usb_config.manufacturer = Some(value);
        self
    }

    pub fn product(mut self, value: &'static str) -> Self {
        self.usb_config.product = Some(value);
        self
    }

    pub fn serial_number(mut self, value: &'static str) -> Self {
        self.usb_config.serial_number = Some(value);
        self
    }

    pub fn max_power(mut self, value: u16) -> Self {
        self.usb_config.max_power = value;
        self
    }

    pub fn build<const ROW_IS_OUTPUT: bool, const LAYOUT_MAX: usize>(
        self,
    ) -> Keyboard<D, I, O, ROW_IS_OUTPUT, INPUT_COUNT, OUTPUT_COUNT, LAYOUT_MAX> {
        let key_scan_channel: &'static ScanChannel = KEY_SCAN_CHANNEL.init(ScanChannel::default());
        let mapper_channel: &'static MpChannel = MAPPER_CHANNEL.init(MpChannel::default());
        let config_interface: &'static mut ScanConfigInterface =
            CONFIG_INTERFACE.init(ConfigInterface::new(self.fs, mapper_channel.control()));

        Keyboard {
            builder: self,
            key_scan_channel,
            mapper_channel,
            config_interface,
        }
    }
}

struct MyRequestHandler {}

impl RequestHandler for MyRequestHandler {
    fn get_report(&mut self, id: ReportId, _buf: &mut [u8]) -> Option<usize> {
        info!("Get report for {:?}", id);
        None
    }

    fn set_report(&mut self, id: ReportId, data: &[u8]) -> OutResponse {
        info!("Set report for {:?}: {:?}", id, data);
        OutResponse::Accepted
    }

    fn set_idle_ms(&mut self, id: Option<ReportId>, dur: u32) {
        info!("Set idle rate for {:?} to {:?}", id, dur);
    }

    fn get_idle_ms(&mut self, id: Option<ReportId>) -> Option<u32> {
        info!("Get idle rate for {:?}", id);
        None
    }
}

use embassy_futures::{join::join, select::select};
use embassy_usb::{
    Builder,
    class::hid::{ReportId, RequestHandler},
    control::OutResponse,
    driver::{Driver, Endpoint, EndpointIn, EndpointOut},
};
use rpk_common::usb_vendor_message::MAX_BULK_LEN;
use rpk_firmware::{
    config::{ConfigInterface, HostMessage},
    hid, mapper,
    usb::{Configurator, SHARED_REPORT_DESC, State},
};

pub type SharedHidWriter<'d, D> = hid::HidWriter<'d, D, 34>;
pub type SharedHidReader<'d, D> = hid::HidReader<'d, D, 10>;

pub struct ConfigBuilder {
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: &'static str,
    pub product: &'static str,
    pub serial_number: &'static str,
    pub max_power: u16,
}
impl ConfigBuilder {
    pub fn usb_configurator<'d>(&self) -> Configurator<'d> {
        let mut conf = embassy_usb::Config::new(self.vendor_id, self.product_id);
        conf.manufacturer = Some(self.manufacturer);
        conf.product = Some(self.product);
        conf.serial_number = Some(self.serial_number);
        conf.max_power = self.max_power;
        Configurator::new(conf)
    }

    pub fn shared_hid_iface<'d, D: Driver<'d>>(
        &self,
        usb_config: &'d mut Configurator<'d>,
        keyboard_state: &'d mut State<'d>,
        mut usb_builder: Builder<'d, D>,
    ) -> (
        SharedHidWriter<'d, D>,
        SharedHidReader<'d, D>,
        Builder<'d, D>,
    ) {
        let (shared_hid_writer, shared_hid_reader) = usb_config.add_iface::<_, 10, 34>(
            &mut usb_builder,
            &SHARED_REPORT_DESC,
            true,
            1,
            1,
            keyboard_state,
        );

        (shared_hid_writer, shared_hid_reader.unwrap(), usb_builder)
    }

    pub fn cfg_ep<'d, D: Driver<'d>>(
        &self,
        config_interface: ConfigInterface<'d, 'd, 2>,
        mut usb_builder: Builder<'d, D>,
    ) -> (ConfigEndPoint<'d, D>, Builder<'d, D>) {
        let mut function = usb_builder.function(0xFF, 0, 0);
        let mut interface = function.interface();
        let mut alt = interface.alt_setting(0xFF, 0, 0, None);
        let read_ep = alt.endpoint_bulk_out(None, MAX_BULK_LEN);
        let write_ep = alt.endpoint_bulk_in(None, MAX_BULK_LEN);

        drop(function);

        (
            ConfigEndPoint {
                config_interface,
                read_ep,
                write_ep,
            },
            usb_builder,
        )
    }
}

const HOST_CHANNEL_LEN: usize = 2;

pub struct ConfigEndPoint<'d, D: Driver<'d>> {
    config_interface: ConfigInterface<'d, 'd, HOST_CHANNEL_LEN>,
    read_ep: D::EndpointOut,
    write_ep: D::EndpointIn,
}
impl<'d, D: Driver<'d>> ConfigEndPoint<'d, D> {
    pub async fn run(&mut self) {
        let host_channel = self.config_interface.host_channel;
        let r = async {
            let mut buf = [0; MAX_BULK_LEN as usize];
            loop {
                self.read_ep.wait_enabled().await;
                while let Ok(n) = self.read_ep.read(&mut buf).await {
                    if n > 0 {
                        self.config_interface.receive(&buf[..n]).await;
                    }
                }
            }
        };
        let s = async {
            let key_logger = mapper::KEY_SCAN_LOGGER.get();
            let mut key_msg = HostMessage::key_scan();
            loop {
                match select(host_channel.receive(), key_logger.receive()).await {
                    embassy_futures::select::Either::First(msg) => {
                        let _ = self.write_ep.write(msg.as_slice()).await;
                    }
                    embassy_futures::select::Either::Second(key) => {
                        key_msg.set_key(key.as_memo_bytes());
                        let _ = self.write_ep.write(key_msg.as_slice()).await;
                    }
                }
            }
        };
        join(r, s).await;
    }
}

pub struct HidEpHandler;
impl HidEpHandler {
    pub async fn run<'d, D: Driver<'d>>(mut self, ep_reader: SharedHidReader<'d, D>) {
        ep_reader.run(false, &mut self).await;
    }
}

impl RequestHandler for HidEpHandler {
    fn get_report(&mut self, id: ReportId, _buf: &mut [u8]) -> Option<usize> {
        crate::info!("Get report for {:?}", id);
        None
    }

    fn set_report(&mut self, id: ReportId, data: &[u8]) -> OutResponse {
        crate::info!("Set report for {:?}: {:?}", id, data);
        OutResponse::Accepted
    }

    fn set_idle_ms(&mut self, id: Option<ReportId>, dur: u32) {
        crate::info!("Set idle rate for {:?} to {:?}", id, dur);
    }

    fn get_idle_ms(&mut self, id: Option<ReportId>) -> Option<u32> {
        crate::info!("Get idle rate for {:?}", id);
        None
    }
}

#[cfg(test)]
#[path = "usb_test.rs"]
mod test;

use embassy_usb::{
    class::hid::{ReportId, RequestHandler},
    control::OutResponse,
    driver::{Driver, Endpoint, EndpointOut},
    Builder,
};
use rpk_firmware::{
    config::ConfigInterface,
    hid,
    usb::{Configurator, State, SHARED_REPORT_DESC},
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
        config_interface: &'d mut ConfigInterface<'d, 'd>,
        mut usb_builder: Builder<'d, D>,
    ) -> (ConfigEndPoint<'d, D>, Builder<'d, D>) {
        let mut function = usb_builder.function(0xFF, 0, 0);
        let mut interface = function.interface();
        let mut alt = interface.alt_setting(0xFF, 0, 0, None);
        let read_ep = alt.endpoint_bulk_out(64);

        drop(function);

        (
            ConfigEndPoint {
                config_interface,
                read_ep,
            },
            usb_builder,
        )
    }
}

pub struct ConfigEndPoint<'d, D: Driver<'d>> {
    config_interface: &'d mut ConfigInterface<'d, 'd>,
    read_ep: D::EndpointOut,
}
impl<'d, D: Driver<'d>> ConfigEndPoint<'d, D> {
    pub async fn run(&mut self) {
        loop {
            self.read_ep.wait_enabled().await;
            loop {
                let mut buf = [0; 64];
                match self.read_ep.read(&mut buf).await {
                    Ok(n) => {
                        if n > 0 {
                            self.config_interface.receive(&buf[..n]).await
                        }
                    }
                    Err(_) => break,
                }
            }
        }
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

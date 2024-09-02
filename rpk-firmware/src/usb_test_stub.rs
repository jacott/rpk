use embassy_usb::driver::{
    Bus, ControlPipe, Driver, Endpoint, EndpointAddress, EndpointIn, EndpointInfo, EndpointOut,
    EndpointType,
};

extern crate std;
use std::sync::{Arc, Mutex};
use std::vec::Vec;

pub struct MyEndpointIn {
    pub messages: Arc<Mutex<Vec<Vec<u8>>>>,
    pub info: EndpointInfo,
}
impl Endpoint for MyEndpointIn {
    fn info(&self) -> &EndpointInfo {
        &self.info
    }

    async fn wait_enabled(&mut self) {}
}
impl EndpointIn for MyEndpointIn {
    async fn write(&mut self, buf: &[u8]) -> Result<(), embassy_usb::driver::EndpointError> {
        let mut guard = self.messages.lock().unwrap();
        guard.push(buf.into());
        Ok(())
    }
}
impl Default for MyEndpointIn {
    fn default() -> Self {
        Self {
            messages: Default::default(),
            info: EndpointInfo {
                addr: EndpointAddress::from(0),
                ep_type: EndpointType::Interrupt,
                max_packet_size: 64,
                interval_ms: 1,
            },
        }
    }
}

pub struct MyEndpointOut;
impl Endpoint for MyEndpointOut {
    fn info(&self) -> &EndpointInfo {
        unimplemented!()
    }

    async fn wait_enabled(&mut self) {}
}
impl EndpointOut for MyEndpointOut {
    async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, embassy_usb::driver::EndpointError> {
        unimplemented!()
    }
}

pub struct MyBus;
impl Bus for MyBus {
    async fn enable(&mut self) {}

    async fn disable(&mut self) {}

    async fn poll(&mut self) -> embassy_usb::driver::Event {
        unimplemented!()
    }

    fn endpoint_set_enabled(
        &mut self,
        _ep_addr: embassy_usb::driver::EndpointAddress,
        _enabled: bool,
    ) {
        unimplemented!()
    }

    fn endpoint_set_stalled(
        &mut self,
        _ep_addr: embassy_usb::driver::EndpointAddress,
        _stalled: bool,
    ) {
        unimplemented!()
    }

    fn endpoint_is_stalled(&mut self, _ep_addr: embassy_usb::driver::EndpointAddress) -> bool {
        unimplemented!()
    }

    async fn remote_wakeup(&mut self) -> Result<(), embassy_usb::driver::Unsupported> {
        unimplemented!()
    }
}

pub struct MyControlPipe;
impl ControlPipe for MyControlPipe {
    fn max_packet_size(&self) -> usize {
        unimplemented!()
    }

    async fn setup(&mut self) -> [u8; 8] {
        unimplemented!()
    }

    async fn data_out(
        &mut self,
        _buf: &mut [u8],
        _first: bool,
        _last: bool,
    ) -> Result<usize, embassy_usb::driver::EndpointError> {
        unimplemented!()
    }

    async fn data_in(
        &mut self,
        _data: &[u8],
        _first: bool,
        _last: bool,
    ) -> Result<(), embassy_usb::driver::EndpointError> {
        unimplemented!()
    }

    async fn accept(&mut self) {
        unimplemented!()
    }

    async fn reject(&mut self) {
        unimplemented!()
    }

    async fn accept_set_address(&mut self, _addr: u8) {
        unimplemented!()
    }
}

pub struct MyDriver;
impl Driver<'_> for MyDriver {
    type EndpointOut = MyEndpointOut;

    type EndpointIn = MyEndpointIn;

    type ControlPipe = MyControlPipe;

    type Bus = MyBus;

    fn alloc_endpoint_out(
        &mut self,
        _ep_type: embassy_usb::driver::EndpointType,
        _max_packet_size: u16,
        _interval_ms: u8,
    ) -> Result<Self::EndpointOut, embassy_usb::driver::EndpointAllocError> {
        unimplemented!()
    }

    fn alloc_endpoint_in(
        &mut self,
        _ep_type: embassy_usb::driver::EndpointType,
        _max_packet_size: u16,
        _interval_ms: u8,
    ) -> Result<Self::EndpointIn, embassy_usb::driver::EndpointAllocError> {
        unimplemented!()
    }

    fn start(self, _control_max_packet_size: u16) -> (Self::Bus, Self::ControlPipe) {
        unimplemented!()
    }
}

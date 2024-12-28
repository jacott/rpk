extern crate std;
use core::cmp::min;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel};
use embassy_usb::driver::{
    Bus, ControlPipe, Driver, Endpoint, EndpointAddress, EndpointIn, EndpointInfo, EndpointOut,
    EndpointType,
};
use rpk_common::usb_vendor_message::MAX_BULK_LEN;
use std::rc::Rc;
use std::vec::Vec;

#[derive(Clone)]
pub struct MessageChannel(Rc<Channel<NoopRawMutex, Vec<u8>, 10>>);
impl MessageChannel {
    pub async fn send(&self, msg: Vec<u8>) {
        self.0.send(msg).await;
    }

    pub fn get(&self) -> Vec<u8> {
        self.0.try_receive().unwrap()
    }

    pub async fn receive(&self) -> Vec<u8> {
        self.0.receive().await
    }
}

impl Default for MessageChannel {
    fn default() -> Self {
        Self(Rc::new(Channel::new()))
    }
}

pub struct MyEndpointIn {
    pub messages: MessageChannel,
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
        self.messages.send(Vec::from(buf)).await;
        Ok(())
    }
}
impl Default for MyEndpointIn {
    fn default() -> Self {
        Self {
            messages: MessageChannel::default(),
            info: EndpointInfo {
                addr: EndpointAddress::from(0),
                ep_type: EndpointType::Interrupt,
                max_packet_size: MAX_BULK_LEN as u16,
                interval_ms: 1,
            },
        }
    }
}

pub struct MyEndpointOut {
    pub messages: Channel<NoopRawMutex, Vec<u8>, 10>,
}
impl Endpoint for MyEndpointOut {
    fn info(&self) -> &EndpointInfo {
        unimplemented!()
    }

    async fn wait_enabled(&mut self) {}
}
impl EndpointOut for MyEndpointOut {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, embassy_usb::driver::EndpointError> {
        loop {
            let msg = self.messages.receive().await;
            let msg = &msg[..min(buf.len(), msg.len())];
            buf[..msg.len()].copy_from_slice(msg);
            return Ok(msg.len());
        }
    }
}
impl Default for MyEndpointOut {
    fn default() -> Self {
        Self {
            messages: Channel::new(),
        }
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

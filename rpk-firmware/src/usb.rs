use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};
use embassy_usb::{
    class::hid::{ReportId, RequestHandler},
    control::{InResponse, OutResponse, Recipient, Request, RequestType},
    driver::Driver,
    types::InterfaceNumber,
    Builder, Config, Handler,
};

use crate::hid::{HidReader, HidWriter};

// HID
const HID_DESC_DESCTYPE_HID: u8 = 0x21;
const HID_DESC_DESCTYPE_HID_REPORT: u8 = 0x22;
const HID_DESC_SPEC_1_11: [u8; 2] = [0x11, 0x01];
const HID_DESC_COUNTRY_UNSPEC: u8 = 0x00;

const HID_REQ_SET_IDLE: u8 = 0x0a;
const HID_REQ_GET_IDLE: u8 = 0x02;
const HID_REQ_GET_REPORT: u8 = 0x01;
const HID_REQ_SET_REPORT: u8 = 0x09;
const HID_REQ_GET_PROTOCOL: u8 = 0x03;
const HID_REQ_SET_PROTOCOL: u8 = 0x0b;

#[rustfmt::skip]
pub const SHARED_REPORT_DESC: [u8; 59 + 73 + 25 + 25] = [
    // NKRO_DESC [u8; 59]
    0x05, 0x01, // (GLOBAL) USAGE_PAGE         0x0001 Generic Desktop Page
    0x09, 0x06, // (LOCAL)  USAGE              0x00010006 Keyboard (Application Collection)
    0xA1, 0x01, // (MAIN) COLLECTION 0x01 Application (Usage=0x00010006: Page=Generic Desktop Page,
                // Usage=Keyboard, Type=Application Collection)
    0x85, 0x06, //   (GLOBAL) REPORT_ID          0x06 (6)
    0x05, 0x07, //   (GLOBAL) USAGE_PAGE         0x0007 Keyboard/Keypad Page
    0x19, 0xE0, //   (LOCAL)  USAGE_MINIMUM      0x000700E0 Keyboard LeftControl (Dynamic Value)
    0x29, 0xE7, //   (LOCAL)  USAGE_MAXIMUM      0x000700E7 Keyboard Right GUI (Dynamic Value)
    0x15, 0x00, //   (GLOBAL) LOGICAL_MINIMUM    0x00 (0)
    0x25, 0x01, //   (GLOBAL) LOGICAL_MAXIMUM    0x01 (1)
    0x95, 0x08, //   (GLOBAL) REPORT_COUNT       0x08 (8) Number of fields
    0x75, 0x01, //   (GLOBAL) REPORT_SIZE        0x01 (1) Number of bits per field
    0x81, 0x02, //   (MAIN) INPUT 0x00000002 (8 fields x 1 bit) 0=Data 1=Variable 0=Absolute
                //     0=NoWrap 0=Linear 0=PrefState 0=NoNull 0=NonVolatile 0=Bitmap
    0x05, 0x07, //   (GLOBAL) USAGE_PAGE         0x0007 Keyboard/Keypad Page
    0x19, 0x00, //   (LOCAL) USAGE_MINIMUM 0x00070000 Keyboard No event indicated (Selector)
    0x29, 0xFE, //   (LOCAL)  USAGE_MAXIMUM      0x000700FE
    0x15, 0x00, //   (GLOBAL) LOGICAL_MINIMUM    0x00 (0)
    0x25, 0x01, //   (GLOBAL) LOGICAL_MAXIMUM    0x01 (1)
    0x95, 0xFF, //   (GLOBAL) REPORT_COUNT       0xFF (255) Number of fields
    0x75, 0x01, //   (GLOBAL) REPORT_SIZE        0x01 (1) Number of bits per field
    0x81, 0x02, //   (MAIN) INPUT 0x00000002 (255 fields x 1 bit) 0=Data 1=Variable 0=Absolute
                //     0=NoWrap 0=Linear 0=PrefState 0=NoNull 0=NonVolatile 0=Bitmap
    0x05, 0x08, //   (GLOBAL) USAGE_PAGE         0x0008 LED Page
    0x19, 0x01, //   (LOCAL)  USAGE_MINIMUM      0x00080001 Num Lock (On/Off Control)
    0x29, 0x05, //   (LOCAL)  USAGE_MAXIMUM      0x00080005 Kana (On/Off Control)
    0x95, 0x05, //   (GLOBAL) REPORT_COUNT       0x05 (5) Number of fields
    0x75, 0x01, //   (GLOBAL) REPORT_SIZE        0x01 (1) Number of bits per field
    0x91, 0x02, //   (MAIN) OUTPUT 0x00000002 (5 fields x 1 bit) 0=Data 1=Variable 0=Absolute
                //   0=NoWrap 0=Linear 0=PrefState 0=NoNull 0=NonVolatile 0=Bitmap
    0x95, 0x01, //   (GLOBAL) REPORT_COUNT       0x01 (1) Number of fields
    0x75, 0x03, //   (GLOBAL) REPORT_SIZE        0x03 (3) Number of bits per field
    0x91, 0x01, //   (MAIN) OUTPUT 0x00000001 (1 field x 3 bits) 1=Constant 0=Array 0=Absolute
                //     0=NoWrap 0=Linear 0=PrefState 0=NoNull 0=NonVolatile 0=Bitmap
    0xC0,       // (MAIN)   END_COLLECTION     Application

    // MOUSE_DESC  [u8; 73]
    0x05, 0x01, // (GLOBAL) USAGE_PAGE         0x0001 Generic Desktop Page
    0x09, 0x02, // (LOCAL)  USAGE              0x00010002 Mouse (Application Collection)
    0xA1, 0x01, // (MAIN) COLLECTION 0x01 Application (Usage=0x00010002: Page=Generic Desktop Page,
                //  Usage=Mouse, Type=Application Collection)
    0x85, 0x02, //   (GLOBAL) REPORT_ID          0x02 (2)
    0x09, 0x01, //   (LOCAL)  USAGE              0x00010001 Pointer (Physical Collection)
    0xA1, 0x00, //   (MAIN) COLLECTION 0x00 Physical (Usage=0x00010001: Page=Generic Desktop Page,
                //    Usage=Pointer, Type=Physical Collection)
    0x05, 0x09, //     (GLOBAL) USAGE_PAGE         0x0009 Button Page
    0x19, 0x01, //     (LOCAL) USAGE_MINIMUM 0x00090001 Button 1 Primary/trigger (Selector, On/Off
                //      Control, Momentary Control, or One Shot Control)
    0x29, 0x08, //     (LOCAL) USAGE_MAXIMUM 0x00090008 Button 8 (Selector, On/Off Control,
                //      Momentary Control, or One Shot Control)
    0x15, 0x00, //     (GLOBAL) LOGICAL_MINIMUM    0x00 (0)
    0x25, 0x01, //     (GLOBAL) LOGICAL_MAXIMUM    0x01 (1)
    0x95, 0x08, //     (GLOBAL) REPORT_COUNT       0x08 (8) Number of fields
    0x75, 0x01, //     (GLOBAL) REPORT_SIZE        0x01 (1) Number of bits per field
    0x81, 0x02, //     (MAIN) INPUT 0x00000002 (8 fields x 1 bit) 0=Data 1=Variable 0=Absolute
                //       0=NoWrap 0=Linear 0=PrefState 0=NoNull 0=NonVolatile 0=Bitmap
    0x05, 0x01, //     (GLOBAL) USAGE_PAGE         0x0001 Generic Desktop Page
    0x09, 0x30, //     (LOCAL)  USAGE              0x00010030 X (Dynamic Value)
    0x09, 0x31, //     (LOCAL)  USAGE              0x00010031 Y (Dynamic Value)
    0x15, 0x81, //     (GLOBAL) LOGICAL_MINIMUM    0x81 (-127)
    0x25, 0x7F, //     (GLOBAL) LOGICAL_MAXIMUM    0x7F (127)
    0x95, 0x02, //     (GLOBAL) REPORT_COUNT       0x02 (2) Number of fields
    0x75, 0x08, //     (GLOBAL) REPORT_SIZE        0x08 (8) Number of bits per field
    0x81, 0x06, //     (MAIN) INPUT 0x00000006 (2 fields x 8 bits) 0=Data 1=Variable 1=Relative
                //       0=NoWrap 0=Linear 0=PrefState 0=NoNull 0=NonVolatile 0=Bitmap
    0x09, 0x38, //     (LOCAL)  USAGE              0x00010038 Wheel (Dynamic Value)
    0x15, 0x81, //     (GLOBAL) LOGICAL_MINIMUM    0x81 (-127)
    0x25, 0x7F, //     (GLOBAL) LOGICAL_MAXIMUM    0x7F (127)
    0x95, 0x01, //     (GLOBAL) REPORT_COUNT       0x01 (1) Number of fields
    0x75, 0x08, //     (GLOBAL) REPORT_SIZE        0x08 (8) Number of bits per field
    0x81, 0x06, //     (MAIN) INPUT 0x00000006 (1 field x 8 bits) 0=Data 1=Variable 1=Relative
                //       0=NoWrap 0=Linear 0=PrefState 0=NoNull 0=NonVolatile 0=Bitmap
    0x05, 0x0C, //     (GLOBAL) USAGE_PAGE         0x000C Consumer Page
    0x0A, 0x38, 0x02,//(LOCAL)  USAGE              0x000C0238 AC Pan (Linear Control)
    0x15, 0x81, //     (GLOBAL) LOGICAL_MINIMUM    0x81 (-127)
    0x25, 0x7F, //     (GLOBAL) LOGICAL_MAXIMUM    0x7F (127)
    0x95, 0x01, //     (GLOBAL) REPORT_COUNT       0x01 (1) Number of fields
    0x75, 0x08, //     (GLOBAL) REPORT_SIZE        0x08 (8) Number of bits per field
    0x81, 0x06, //     (MAIN) INPUT 0x00000006 (1 field x 8 bits) 0=Data 1=Variable 1=Relative
                //       0=NoWrap 0=Linear 0=PrefState 0=NoNull 0=NonVolatile 0=Bitmap
    0xC0,       //   (MAIN)   END_COLLECTION     Physical
    0xC0,       // (MAIN)   END_COLLECTION     Application

    //  SYS_CTL_DESC: [u8; 25]
    0x05, 0x01, // (GLOBAL) USAGE_PAGE         0x0001 Generic Desktop Page
    0x09, 0x80, // (LOCAL)  USAGE              0x00010080 System Control (Application Collection)
    0xA1, 0x01, // (MAIN) COLLECTION 0x01 Application (Usage=0x00010080: Page=Generic Desktop Page,
                //  Usage=System Control, Type=Application Collection)
    0x85, 0x03, //   (GLOBAL) REPORT_ID          0x03 (3)
    0x19, 0x01, //   (LOCAL)  USAGE_MINIMUM      0x00010001 Pointer (Physical Collection)
    0x2A, 0xB7,0,//  (LOCAL)  USAGE_MAXIMUM      0x000100B7 System Display Toggle LCD Autoscale (One Shot Control)
    0x15, 0x01, //   (GLOBAL) LOGICAL_MINIMUM    0x01 (1)
    0x26, 0xB7,0,//  (GLOBAL) LOGICAL_MAXIMUM    0x00B7 (183)
    0x95, 0x01, //   (GLOBAL) REPORT_COUNT       0x01 (1) Number of fields
    0x75, 0x10, //   (GLOBAL) REPORT_SIZE        0x10 (16) Number of bits per field
    0x81, 0x00, //   (MAIN)   INPUT              0x00000000 (1 field x 16 bits) 0=Data 0=Array 0=Absolute
    0xC0,       // (MAIN)   END_COLLECTION     Application


    // CONSUMER_CTL_DESC: [u8; 25]
    0x05, 0x0C, // (GLOBAL) USAGE_PAGE         0x000C Consumer Page
    0x09, 0x01, // (LOCAL)  USAGE              0x000C0001 Consumer Control (Application Collection)
    0xA1, 0x01, // (MAIN) COLLECTION 0x01 Application (Usage=0x000C0001: Page=Consumer Page,
                //  Usage=Consumer Control, Type=Application Collection)
    0x85, 0x04, //   (GLOBAL) REPORT_ID          0x04 (4)
    0x19, 0x01, //   (LOCAL)  USAGE_MINIMUM      0x000C0001 Consumer Control (Application Collection)
    0x2A, 0xA0,2,//  (LOCAL)  USAGE_MAXIMUM      0x000C02A0 AC Soft Key Left (Selector)
    0x15, 0x01, //   (GLOBAL) LOGICAL_MINIMUM    0x01 (1)
    0x26, 0xA0,2,//  (GLOBAL) LOGICAL_MAXIMUM    0x02A0 (672)
    0x95, 0x01, //   (GLOBAL) REPORT_COUNT       0x01 (1) Number of fields
    0x75, 0x10, //   (GLOBAL) REPORT_SIZE        0x10 (16) Number of bits per field
    0x81, 0x00, //   (MAIN)   INPUT              0x00000000 (1 field x 16 bits) 0=Data 0=Array 0=Absolute
    0xC0,       // (MAIN)   END_COLLECTION     Application
];

/// Internal state for USB HID.
pub struct State<'d> {
    control: MaybeUninit<Control<'d>>,
    out_report_offset: AtomicUsize,
}
impl Default for State<'_> {
    fn default() -> Self {
        Self::new()
    }
}
impl State<'_> {
    /// Create a new `State`.
    pub const fn new() -> Self {
        State {
            control: MaybeUninit::uninit(),
            out_report_offset: AtomicUsize::new(0),
        }
    }
}

const CONFIG_SIZE: usize = 128;
const BOS_SIZE: usize = 32;
const MSOS_SIZE: usize = 0;
const CONTROL_SIZE: usize = 256;

pub struct UsbBuffers {
    config_descriptor_buf: [u8; CONFIG_SIZE],
    bos_descriptor_buf: [u8; BOS_SIZE],
    msos_descriptor_buf: [u8; MSOS_SIZE],
    control_buf: [u8; CONTROL_SIZE],
}

impl Default for UsbBuffers {
    fn default() -> Self {
        Self {
            config_descriptor_buf: [0; CONFIG_SIZE],
            bos_descriptor_buf: [0; BOS_SIZE],
            msos_descriptor_buf: [0; MSOS_SIZE],
            control_buf: [0; CONTROL_SIZE],
        }
    }
}

pub struct Configurator<'d> {
    device_config: Option<Config<'d>>,
    max_packet_size: u16,
    poll_ms: u8,
}

impl<'d> Configurator<'d> {
    pub fn new(device_config: Config<'d>) -> Self {
        Self {
            device_config: Some(device_config),
            max_packet_size: device_config.max_packet_size_0 as u16,
            poll_ms: 1,
        }
    }

    pub fn usb_builder<D: Driver<'d>>(
        &mut self,
        driver: D,
        buffers: &'d mut UsbBuffers,
    ) -> Option<Builder<'d, D>> {
        //        return a tuple with the builder and the buffers which we create here; not in new func

        self.device_config.take().map(|device_config| {
            Builder::new(
                driver,
                device_config,
                &mut buffers.config_descriptor_buf,
                &mut buffers.bos_descriptor_buf,
                &mut buffers.msos_descriptor_buf,
                &mut buffers.control_buf,
            )
        })
    }

    pub fn add_iface<'a, D: Driver<'d>, const READ_N: usize, const WRITE_N: usize>(
        &'d self,
        builder: &'a mut Builder<'d, D>,
        descriptor: &'static [u8],
        need_reader: bool,
        subclass: u8,
        protocol: u8,
        state: &'d mut State<'d>,
    ) -> (HidWriter<'d, D, WRITE_N>, Option<HidReader<'d, D, READ_N>>) {
        let mut func = builder.function(3, subclass, protocol);
        let mut iface = func.interface();
        let if_num = iface.interface_number();
        let mut alt = iface.alt_setting(3, subclass, protocol, None);

        let len = descriptor.len();
        alt.descriptor(
            HID_DESC_DESCTYPE_HID,
            &[
                0x11,               // HID Class spec Version
                0x01,               //
                0,                  // Country code not supported
                1,                  // Number of following descriptors
                34,                 // We have a HID report descriptor the host should read
                (len & 0xFF) as u8, // HID report descriptor size,
                (len >> 8 & 0xFF) as u8,
            ],
        );

        let ep_in = alt.endpoint_interrupt_in(self.max_packet_size, self.poll_ms);
        let ep_out = if need_reader {
            Some(alt.endpoint_interrupt_out(self.max_packet_size, self.poll_ms))
        } else {
            None
        };

        drop(func);

        let control = Control::new(
            if_num,
            descriptor,
            None, // TODO  &self.request_handler,
            &state.out_report_offset,
        );
        let control = state.control.write(control);
        builder.handler(control);
        (
            HidWriter::new(ep_in),
            ep_out.map(|ep_out| HidReader::new(ep_out, &state.out_report_offset)),
        )
    }
}

struct Control<'d> {
    if_num: InterfaceNumber,
    report_descriptor: &'d [u8],
    request_handler: Option<&'d mut dyn RequestHandler>,
    out_report_offset: &'d AtomicUsize,
    hid_descriptor: [u8; 9],
}
impl<'d> Control<'d> {
    fn new(
        if_num: InterfaceNumber,
        report_descriptor: &'d [u8],
        request_handler: Option<&'d mut dyn RequestHandler>,
        out_report_offset: &'d AtomicUsize,
    ) -> Self {
        Control {
            if_num,
            report_descriptor,
            request_handler,
            out_report_offset,
            hid_descriptor: [
                9,                                           // Length of buf inclusive of size prefix
                HID_DESC_DESCTYPE_HID,                       // Descriptor type
                HID_DESC_SPEC_1_11[0],                       // HID Class spec version
                HID_DESC_SPEC_1_11[1],                       //
                HID_DESC_COUNTRY_UNSPEC,                     // Country code not supported
                1,                                           // Number of following descriptors
                HID_DESC_DESCTYPE_HID_REPORT, // We have a HID report descriptor the host should read
                (report_descriptor.len() & 0xFF) as u8, // HID report descriptor size,
                (report_descriptor.len() >> 8 & 0xFF) as u8, //
            ],
        }
    }
}
impl Handler for Control<'_> {
    fn reset(&mut self) {
        self.out_report_offset.store(0, Ordering::Release);
    }

    fn control_out(&mut self, req: Request, data: &[u8]) -> Option<OutResponse> {
        if (req.request_type, req.recipient, req.index)
            != (
                RequestType::Class,
                Recipient::Interface,
                self.if_num.0 as u16,
            )
        {
            return None;
        }

        match req.request {
            HID_REQ_SET_IDLE => {
                // How often we should send the keyboard state
                if let Some(handler) = self.request_handler.as_mut() {
                    let id = req.value as u8;
                    let id = (id != 0).then_some(ReportId::In(id));
                    let dur = u32::from(req.value >> 8);
                    let dur = if dur == 0 { u32::MAX } else { 4 * dur };
                    handler.set_idle_ms(id, dur);
                }
                Some(OutResponse::Accepted)
            }
            HID_REQ_SET_REPORT => {
                match (report_id_try_from(req.value), self.request_handler.as_mut()) {
                    (Ok(id), Some(handler)) => Some(handler.set_report(id, data)),
                    _ => Some(OutResponse::Rejected),
                }
            }
            HID_REQ_SET_PROTOCOL => {
                if req.value == 1 {
                    Some(OutResponse::Accepted)
                } else {
                    crate::warn!("HID Boot Protocol is unsupported.");
                    Some(OutResponse::Rejected) // UNSUPPORTED: Boot Protocol
                }
            }
            _ => Some(OutResponse::Rejected),
        }
    }

    fn control_in<'a>(&'a mut self, req: Request, buf: &'a mut [u8]) -> Option<InResponse<'a>> {
        if req.index != self.if_num.0 as u16 {
            return None;
        }

        match (req.request_type, req.recipient) {
            (RequestType::Standard, Recipient::Interface) => match req.request {
                Request::GET_DESCRIPTOR => match (req.value >> 8) as u8 {
                    HID_DESC_DESCTYPE_HID_REPORT => {
                        Some(InResponse::Accepted(self.report_descriptor))
                    }
                    HID_DESC_DESCTYPE_HID => Some(InResponse::Accepted(&self.hid_descriptor)),
                    _ => Some(InResponse::Rejected),
                },

                _ => Some(InResponse::Rejected),
            },
            (RequestType::Class, Recipient::Interface) => {
                match req.request {
                    HID_REQ_GET_REPORT => {
                        let size = match report_id_try_from(req.value) {
                            Ok(id) => self
                                .request_handler
                                .as_mut()
                                .and_then(|x| x.get_report(id, buf)),
                            Err(_) => None,
                        };

                        if let Some(size) = size {
                            Some(InResponse::Accepted(&buf[0..size]))
                        } else {
                            Some(InResponse::Rejected)
                        }
                    }
                    HID_REQ_GET_IDLE => {
                        if let Some(handler) = self.request_handler.as_mut() {
                            let id = req.value as u8;
                            let id = (id != 0).then_some(ReportId::In(id));
                            if let Some(dur) = handler.get_idle_ms(id) {
                                let dur = u8::try_from(dur / 4).unwrap_or(0);
                                buf[0] = dur;
                                Some(InResponse::Accepted(&buf[0..1]))
                            } else {
                                Some(InResponse::Rejected)
                            }
                        } else {
                            Some(InResponse::Rejected)
                        }
                    }
                    HID_REQ_GET_PROTOCOL => {
                        // UNSUPPORTED: Boot Protocol
                        buf[0] = 1;
                        Some(InResponse::Accepted(&buf[0..1]))
                    }
                    _ => Some(InResponse::Rejected),
                }
            }
            _ => None,
        }
    }
}

const fn report_id_try_from(value: u16) -> Result<ReportId, ()> {
    match value >> 8 {
        1 => Ok(ReportId::In(value as u8)),
        2 => Ok(ReportId::Out(value as u8)),
        3 => Ok(ReportId::Feature(value as u8)),
        _ => Err(()),
    }
}

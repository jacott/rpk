use anyhow::{anyhow, Result};
use futures_lite::future::block_on;
use nusb::{transfer::Direction, Interface};
use rpk_common::usb_vendor_message::{
    CLOSE_SAVE_CONFIG, OPEN_SAVE_CONFIG, RESET_KEYBOARD, RESET_TO_USB_BOOT,
};

fn u16tou8(words: &[u16]) -> impl Iterator<Item = u8> + use<'_> {
    words.iter().flat_map(|a| a.to_le_bytes())
}

pub struct KeyboardCtl {
    intf: Interface,
    epout: u8,
    _epin: u8,
}

impl KeyboardCtl {
    pub fn find_vendor_interface(dev: &nusb::Device) -> Result<Self> {
        if let Some((i, epout, epin)) = dev.configurations().find_map(|c| {
            c.interfaces().find_map(|i| {
                i.alt_settings().find(|a| a.class() == 255).map(|i| {
                    let mut epout = 0;
                    let mut epin = 0;
                    for ep in i.endpoints() {
                        match ep.direction() {
                            Direction::Out => epout = ep.address(),
                            Direction::In => epin = ep.address(),
                        }
                    }
                    (i.interface_number(), epout, epin)
                })
            })
        }) {
            let intf = dev.claim_interface(i)?;
            Ok(Self {
                intf,
                epout,
                _epin: epin,
            })
        } else {
            Err(anyhow!("Keyboard interface not found"))
        }
    }

    pub fn save_config(&self, data: &[u16]) -> Result<()> {
        self.out(vec![OPEN_SAVE_CONFIG])?;
        let len = 4 + ((data.len() as u32) << 1);

        let (a, b) = data.split_at(30);
        let iter = len.to_le_bytes().into_iter().chain(u16tou8(a));
        if a.len() < 30 {
            return self.out([CLOSE_SAVE_CONFIG].iter().copied().chain(iter).collect());
        } else {
            self.out(iter.collect())?;
        }

        for chunk in b.chunks(32) {
            if chunk.len() < 32 {
                return self.out(
                    [CLOSE_SAVE_CONFIG]
                        .iter()
                        .copied()
                        .chain(u16tou8(chunk))
                        .collect(),
                );
            } else {
                self.out(u16tou8(chunk).collect())?;
            }
        }

        self.out(vec![CLOSE_SAVE_CONFIG])
    }

    pub fn reset_keyboard(&self) -> Result<()> {
        self.out(vec![RESET_KEYBOARD])
    }

    pub fn reset_to_usb_boot_from_usb(&self) -> Result<()> {
        self.out(vec![RESET_TO_USB_BOOT])
    }

    fn out(&self, data: Vec<u8>) -> Result<()> {
        block_on(self.intf.bulk_out(self.epout, data))
            .into_result()
            .map(|_| ())
            .map_err(|err| anyhow!("USB comms error: {}", err))
    }
}

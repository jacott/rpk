use embassy_futures::select::select_slice;
use embassy_sync::{blocking_mutex::raw::RawMutex, channel::Channel};
use embassy_time::{Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::digital::Wait;
use heapless::Vec;

use crate::info;

const WAIT_NANOS: u64 = 100;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ScanKey {
    row: u8,
    col: u8,
}
impl ScanKey {
    pub fn new(row: u8, col: u8, is_down: bool) -> Self {
        Self {
            row: row | if is_down { 0x80 } else { 0 },
            col,
        }
    }

    pub fn none() -> Self {
        Self {
            row: 0xff,
            col: 0xff,
        }
    }

    pub fn is_none(&self) -> bool {
        self.col == 0xff
    }

    pub fn row(&self) -> usize {
        (self.row & 0x7f) as usize
    }

    pub fn column(&self) -> usize {
        self.col as usize
    }

    pub fn is_down(&self) -> bool {
        self.row & 0x80 == 0x80
    }

    pub fn is_same_key(&self, other: ScanKey) -> bool {
        self.col == other.col && self.row & 0x7f == other.row & 0x7f
    }

    pub fn same_key(&self, other: ScanKey) -> bool {
        self.col == other.col && self.row & 0x7f == other.row & 0x7f
    }

    pub fn as_memo(&self) -> u16 {
        self.row as u16 | ((self.col as u16) << 8)
    }

    pub fn from_memo(memo: u16) -> Self {
        Self {
            row: memo as u8,
            col: (memo >> 8) as u8,
        }
    }
}

pub struct KeyScannerChannel<M: RawMutex, const N: usize>(Channel<M, ScanKey, N>);

pub struct KeyScanner<
    'c,
    I: InputPin + Wait,
    O: OutputPin,
    M: RawMutex,
    const INPUT_N: usize,
    const OUTPUT_N: usize,
    const PS: usize,
> {
    input_pins: [I; INPUT_N],
    output_pins: [O; OUTPUT_N],
    /// Keeps track of all key switch changes and debounce settling timer.  Bit 7 indicates debouncing,
    /// bits 6-2 are debounce counter, bit 1 indicates last reported switch position, bit 0 indicates
    /// actual switch position.
    state: [[u8; INPUT_N]; OUTPUT_N],
    scan_start: Option<Instant>,
    channel: &'c KeyScannerChannel<M, PS>,
    debounce: usize,
}

impl<M: RawMutex, const N: usize> Default for KeyScannerChannel<M, N> {
    fn default() -> Self {
        Self(Channel::new())
    }
}

impl<M: RawMutex, const N: usize> KeyScannerChannel<M, N> {
    pub async fn receive(&self) -> ScanKey {
        self.0.receive().await
    }

    pub fn try_send(&self, msg: ScanKey) {
        self.0.try_send(msg).ok();
    }

    pub async fn get_offset(&self) -> u32 {
        let key1 = self.receive().await;
        let key2 = self.receive().await;
        u32::from_le_bytes([key1.row, key1.col, key2.row, key2.col])
    }
}

impl<
        'c,
        I: InputPin + Wait,
        O: OutputPin,
        M: RawMutex,
        const INPUT_N: usize,
        const OUTPUT_N: usize,
        const PS: usize,
    > KeyScanner<'c, I, O, M, INPUT_N, OUTPUT_N, PS>
{
    pub fn new(
        input_pins: [I; INPUT_N],
        output_pins: [O; OUTPUT_N],
        channel: &'c KeyScannerChannel<M, PS>,
    ) -> Self {
        Self {
            input_pins,
            output_pins,
            state: [[0; INPUT_N]; OUTPUT_N],
            scan_start: None,
            channel,
            debounce: 0,
        }
    }

    pub async fn run<const ROW_IS_OUTPUT: bool, const DEBOUNCE_TUNE: usize>(&mut self) {
        assert!(DEBOUNCE_TUNE < usize::BITS as usize);
        loop {
            // If no key for over 2 secs, wait for interupt
            if self.scan_start.is_some_and(|s| s.elapsed().as_secs() > 1) {
                let _waited = self.wait_for_key().await;
            }

            self.scan::<ROW_IS_OUTPUT, DEBOUNCE_TUNE>().await;
        }
    }

    pub async fn wait_for_key(&mut self) -> bool {
        self.scan_start = None;

        // First, set all output pin to low
        for out in self.output_pins.iter_mut() {
            let _ = out.set_low();
        }
        Timer::after_micros(1).await;
        info!("Waiting for low");

        let mut futs: Vec<_, INPUT_N> = self
            .input_pins
            .iter_mut()
            .map(|input_pin| input_pin.wait_for_low())
            .collect();
        let _ = select_slice(futs.as_mut_slice()).await;

        // Set all output pins back to low
        for out in self.output_pins.iter_mut() {
            let _ = out.set_high();
        }

        true
    }

    pub async fn scan<const ROW_IS_OUTPUT: bool, const DEBOUNCE_TUNE: usize>(&mut self) {
        // debounce on, down cleared for compare
        let debounce = debounce_sensitivity::<DEBOUNCE_TUNE>(self.debounce);

        // We will soon sleep if all up
        let mut is_all_up = true;

        for (output_idx, (op, s)) in self
            .output_pins
            .iter_mut()
            .zip(self.state.iter_mut())
            .enumerate()
        {
            let _ = op.set_low();
            Timer::after_nanos(WAIT_NANOS).await;

            for (input_idx, (ip, s)) in self.input_pins.iter_mut().zip(s.iter_mut()).enumerate() {
                let settle = *s & !3; // down states cleared for compare

                let is_down = if ip.is_low().unwrap_or(false) { 1 } else { 0 };
                let mut changed = *s & 1 != is_down;

                if settle != 0 {
                    if settle == debounce {
                        // we are now settled; just keep down states
                        *s &= 3;
                        changed = matches!(*s, 1 | 2);
                    } else {
                        // settling keys need to be polled
                        is_all_up = false;
                        if changed {
                            // restart settle counter
                            *s = debounce_start::<DEBOUNCE_TUNE>(is_down, *s & 2, self.debounce);
                        }
                        continue;
                    }
                }
                if is_down == 1 {
                    is_all_up = false;
                }

                if changed {
                    // set debounce state and counter to prev value
                    *s = debounce_start::<DEBOUNCE_TUNE>(is_down, is_down << 1, self.debounce);
                    self.channel
                        .0
                        .send(if ROW_IS_OUTPUT {
                            ScanKey::new(output_idx as u8, input_idx as u8, is_down == 1)
                        } else {
                            ScanKey::new(input_idx as u8, output_idx as u8, is_down == 1)
                        })
                        .await;
                }
            }

            let _ = op.set_high();
        }

        self.debounce = self.debounce.wrapping_add(2);

        if is_all_up {
            if self.scan_start.is_none() {
                self.scan_start = Some(Instant::now());
            }
        } else if self.scan_start.is_some() {
            self.scan_start = None;
        }
    }
}

fn debounce_sensitivity<const DEBOUNCE_TUNE: usize>(debounce: usize) -> u8 {
    ((if DEBOUNCE_TUNE < 4 {
        debounce << (4 - DEBOUNCE_TUNE) // more sensitive
    } else {
        debounce >> (DEBOUNCE_TUNE - 4) // less sensitive
    } as u8)
        << 2)
        | 0x80
}

fn debounce_start<const DEBOUNCE_TUNE: usize>(
    is_down: u8,
    reported_down: u8,
    debounce: usize,
) -> u8 {
    debounce_sensitivity::<DEBOUNCE_TUNE>(debounce.wrapping_sub(if DEBOUNCE_TUNE < 4 {
        0
    } else {
        1 << (DEBOUNCE_TUNE - 3)
    })) | (is_down | reported_down)
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
#[path = "key_scanner_test.rs"]
mod test;

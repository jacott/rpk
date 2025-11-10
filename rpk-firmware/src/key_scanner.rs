use core::{cmp::max, pin::pin, sync::atomic};

use embassy_futures::select::select_slice;
use embassy_sync::{blocking_mutex::raw::RawMutex, channel::Channel};
use embassy_time::{Duration, Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::digital::Wait;
use rpk_common::globals;

const IDLE_WAIT_COUNT: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq)]
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

    pub fn as_memo_bytes(&self) -> (u8, u8) {
        (self.row, self.col)
    }

    pub fn from_memo(memo: u16) -> Self {
        Self {
            row: memo as u8,
            col: (memo >> 8) as u8,
        }
    }

    pub(crate) fn set_down(&mut self, down: bool) {
        if down {
            self.row |= 0x80;
        } else {
            self.row &= !0x80;
        }
    }
}

pub struct KeyScannerChannel<M: RawMutex, const N: usize>(Channel<M, ScanKey, N>);
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

pub struct KeyScanner<
    'c,
    I: InputPin + Wait,
    O: OutputPin,
    M: RawMutex,
    const INPUT_N: usize,
    const OUTPUT_N: usize,
    const PS: usize,
> {
    channel: &'c KeyScannerChannel<M, PS>,

    input_pins: [I; INPUT_N],
    output_pins: [O; OUTPUT_N],

    /// Keeps track of all key switch changes and debounce settling timer.  > 3 indicates debouncing,
    /// bits 7-2 are debounce counter, bit 1 indicates last reported switch position, bit 0 indicates
    /// actual switch position.
    state: [[u8; INPUT_N]; OUTPUT_N],

    /// Time allowed per output pin.
    time_per_output_pin: Duration,

    /// Best result for how much time left after scanning all output pins. We can use this to adjust
    /// [`Self::time_per_output_pin`].
    scan_free_time: i32,

    /// are all keys up?
    all_up: bool,

    /// A countdown of scans when all keys are up before calling `wait_for_key`.
    all_up_limit: u32,

    /// How many scan cycles have been completed since we incremented `debounce_count`.
    scan_count: u16,
    /// debounce timer assigned to a key on change.
    debounce_count: u8,

    debounce_count_max: u8,
    scan_count_max: u16,

    /// The configuarable time to wait for a key to debounce. The format is compressed.
    debounce_ms_atomic: &'c atomic::AtomicU16,
    debounce_ms_prev: u16,
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
        debounce_ms_atomic: &'c atomic::AtomicU16,
    ) -> Self {
        Self {
            input_pins,
            output_pins,
            state: [[0; INPUT_N]; OUTPUT_N],
            all_up: false,
            all_up_limit: 0,
            channel,
            scan_count: 0,
            debounce_count: 0,
            debounce_count_max: 0,
            scan_count_max: 0,
            time_per_output_pin: Duration::from_micros(32),
            scan_free_time: i32::MIN,
            debounce_ms_atomic,
            debounce_ms_prev: 0,
        }
    }

    pub async fn run<const ROW_IS_OUTPUT: bool>(&mut self) {
        self.wait_for_key().await;
        loop {
            self.scan::<ROW_IS_OUTPUT>().await;

            // If no key for over IDLE_WAIT_MS, wait for interupt
            if self.all_up {
                if self.all_up_limit == 0 {
                    self.wait_for_key().await;
                } else {
                    self.all_up_limit -= 1;
                }
            }
        }
    }

    pub async fn wait_for_key(&mut self) {
        self.calc_debounce_cycle();
        self.all_up = false;

        for out in self.output_pins.iter_mut() {
            let _ = out.set_low();
        }
        Timer::after_micros(10).await;
        {
            let mut futs = self
                .input_pins
                .iter_mut()
                .map(|input_pin| input_pin.wait_for_low());
            let mut futs: [_; INPUT_N] = core::array::from_fn(|_| futs.next().unwrap());
            let _ = select_slice(pin!(futs.as_mut_slice())).await;
        }

        for out in self.output_pins.iter_mut() {
            let _ = out.set_high();
        }
    }

    #[inline]
    fn half_time_per_output_pin(&self) -> Duration {
        Duration::from_ticks((self.time_per_output_pin.as_ticks() as u32 >> 1) as u64)
    }

    pub async fn scan<const ROW_IS_OUTPUT: bool>(&mut self) {
        let mut now = Instant::now();
        now += self.half_time_per_output_pin();

        // debounce on, down cleared for compare
        self.scan_count += 1;
        if self.scan_count > self.scan_count_max {
            self.scan_count = 0;
            self.debounce_count += 1;
            if self.debounce_count > self.debounce_count_max {
                self.debounce_count = 0;
            }
        }
        let debounce_count = (self.debounce_count << 2) | 128;

        // We will soon sleep if all up
        let mut is_all_up = true;
        for (output_idx, (op, s)) in self
            .output_pins
            .iter_mut()
            .zip(self.state.iter_mut())
            .enumerate()
        {
            let _ = op.set_low();
            Timer::at(now).await;
            now += self.time_per_output_pin;

            for (input_idx, (ip, s)) in self.input_pins.iter_mut().zip(s.iter_mut()).enumerate() {
                let settle = *s & !3; // down states cleared for compare

                let key_state = if ip.is_low().unwrap_or(false) { 1 } else { 0 };
                let mut changed = *s & 1 != key_state;
                if settle != 0 {
                    if settle == debounce_count {
                        // we are now settled; just keep down states
                        *s &= 3;
                        changed = matches!(*s, 1 | 2);
                    } else {
                        // settling keys need to be polled
                        is_all_up = false;
                        if changed {
                            // restart settle counter
                            *s = start_debounce(
                                key_state | *s & 2,
                                self.debounce_count,
                                self.debounce_count_max,
                            );
                            continue;
                        }
                    }
                } else if changed {
                    is_all_up = false;
                    *s =
                        start_debounce(key_state * 3, self.debounce_count, self.debounce_count_max);
                }

                if key_state == 1 {
                    is_all_up = false;
                }

                if changed {
                    let skey = if ROW_IS_OUTPUT {
                        ScanKey::new(output_idx as u8, input_idx as u8, key_state == 1)
                    } else {
                        ScanKey::new(input_idx as u8, output_idx as u8, key_state == 1)
                    };
                    self.channel.0.send(skey).await;
                }
            }

            let _ = op.set_high();
        }

        now -= self.half_time_per_output_pin();
        let realnow = Instant::now();

        let rem = if realnow > now {
            -(realnow.duration_since(now).as_ticks() as i32)
        } else {
            Timer::at(now).await;
            now.duration_since(realnow).as_ticks() as i32
        };

        if rem > self.scan_free_time {
            self.scan_free_time = rem;
        }

        if is_all_up {
            if !self.all_up {
                self.all_up = true;
                self.all_up_limit = IDLE_WAIT_COUNT;
            }
        } else {
            self.all_up = false;
        }
    }

    fn calc_debounce_cycle(&mut self) {
        let changed = if self.scan_free_time > 0 {
            let ht = self.time_per_output_pin.as_ticks() as i32 >> 1;

            if self.scan_free_time >= 8 && self.scan_free_time > ht {
                self.time_per_output_pin = Duration::from_ticks(ht as u64);
                true
            } else {
                false
            }
        } else if self.scan_free_time == i32::MIN {
            return;
        } else {
            let mut pt = (self.time_per_output_pin.as_ticks() as u32) << 1;
            let ft = -self.scan_free_time as u32;

            while pt < ft {
                pt <<= 1
            }
            self.time_per_output_pin = Duration::from_ticks(pt as u64);
            true
        };

        let m16 = self.debounce_ms_atomic.load(atomic::Ordering::Relaxed);
        if changed || m16 != self.debounce_ms_prev {
            self.debounce_ms_prev = m16;
            let dt = Duration::from_micros(globals::key_settle_time_uncompress(m16 as u32) as u64)
                .as_ticks() as u32;

            let dcycles = max(
                1,
                dt / (OUTPUT_N as u32 * self.time_per_output_pin.as_ticks() as u32),
            );
            let dtmag = dcycles.ilog2();
            if dtmag > 5 {
                self.debounce_count_max = 31;
                self.scan_count_max = (dcycles >> 5) as u16 - 1;
            } else {
                self.debounce_count_max = (1 << dtmag) - 1;
                self.scan_count_max = 0;
            }
        }
    }
}

#[inline]
fn start_debounce(key_state: u8, debounce_count: u8, debounce_count_max: u8) -> u8 {
    key_state
        | 128
        | (if debounce_count == 0 {
            debounce_count_max
        } else {
            debounce_count - 1
        }) << 2
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
#[path = "key_scanner_test.rs"]
mod test;

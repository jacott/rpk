use core::sync::atomic;

use embassy_futures::select::select_slice;
use embassy_sync::{blocking_mutex::raw::RawMutex, channel::Channel};
use embassy_time::{Duration, Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::digital::Wait;

const IDLE_WAIT_MS: u32 = 2_000;

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
    input_pins: [I; INPUT_N],
    output_pins: [O; OUTPUT_N],
    /// Keeps track of all key switch changes and debounce settling timer.  > 3 indicates debouncing,
    /// bits 7-2 are debounce counter, bit 1 indicates last reported switch position, bit 0 indicates
    /// actual switch position.
    state: [[u8; INPUT_N]; OUTPUT_N],
    all_up: bool,
    all_up_limit: u32,
    channel: &'c KeyScannerChannel<M, PS>,
    cycle: u32,
    debounce_modulus: u32,
    debounce_divisor: u32,
    pin_wait: Duration,
    now: Instant,
    clock: Instant,
    debounce_ms_atomic: &'c atomic::AtomicU16,
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
        let mut ks = Self {
            input_pins,
            output_pins,
            state: [[0; INPUT_N]; OUTPUT_N],
            all_up: false,
            all_up_limit: 0,
            channel,
            cycle: 0,
            debounce_modulus: 0,
            debounce_divisor: 0,
            pin_wait: Duration::from_micros(5),
            now: Instant::now(),
            clock: Instant::now(),
            debounce_ms_atomic,
        };

        ks.calc_debounce_cycle();
        ks.all_up_limit = 10;

        ks
    }

    pub async fn run<const ROW_IS_OUTPUT: bool>(&mut self) {
        loop {
            self.scan::<ROW_IS_OUTPUT>().await;

            // If no key for over IDLE_WAIT_MS, wait for interupt
            if self.all_up && self.cycle >= self.all_up_limit {
                self.wait_for_key().await;
            }
        }
    }

    pub async fn wait_for_key(&mut self) {
        {
            // calc pin_wait; make twice what's needed so idle half the time
            let t = (Instant::now() - self.clock).as_micros() as u32;
            let t = (t.max(20) - 20) / self.cycle; // cycle duration with 20µs leeway
            let pw = t / (OUTPUT_N as u32);
            let opw = self.pin_wait.as_micros() as u32;
            if opw < pw {
                self.pin_wait = Duration::from_micros((pw << 1).clamp(1, 300) as u64);
            }
        }
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
            let _ = select_slice(futs.as_mut_slice()).await;
        }

        for out in self.output_pins.iter_mut() {
            let _ = out.set_high();
        }

        self.calc_debounce_cycle();
        self.now = Instant::now();
    }

    pub async fn scan<const ROW_IS_OUTPUT: bool>(&mut self) {
        // debounce on, down cleared for compare
        let debounce = self.debounce_from_cycle();

        // We will soon sleep if all up
        let mut is_all_up = true;
        for (output_idx, (op, s)) in self
            .output_pins
            .iter_mut()
            .zip(self.state.iter_mut())
            .enumerate()
        {
            let _ = op.set_low();
            self.now += self.pin_wait;
            Timer::at(self.now).await;

            for (input_idx, (ip, s)) in self.input_pins.iter_mut().zip(s.iter_mut()).enumerate() {
                let settle = *s & !3; // down states cleared for compare

                let key_state = if ip.is_low().unwrap_or(false) { 1 } else { 0 };
                let mut changed = *s & 1 != key_state;

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
                            *s =
                                start_debounce(debounce, key_state | *s & 2, self.debounce_modulus);
                        }
                        continue;
                    }
                } else if changed {
                    is_all_up = false;
                    *s = start_debounce(debounce, key_state * 3, self.debounce_modulus);
                }

                if key_state == 1 {
                    is_all_up = false;
                }

                if changed {
                    self.channel
                        .0
                        .send(if ROW_IS_OUTPUT {
                            ScanKey::new(output_idx as u8, input_idx as u8, key_state == 1)
                        } else {
                            ScanKey::new(input_idx as u8, output_idx as u8, key_state == 1)
                        })
                        .await;
                }
            }

            let _ = op.set_high();
        }

        self.cycle = self.cycle.wrapping_add(1);

        if is_all_up {
            if !self.all_up {
                self.all_up = true;
                self.cycle = 0;
                self.clock = self.now;
            }
        } else {
            self.all_up = false;
        }
    }

    fn calc_debounce_cycle(&mut self) {
        let w = self.pin_wait.as_micros() as u32 * OUTPUT_N as u32;

        self.all_up_limit = IDLE_WAIT_MS * 1000 / w;

        let m16 = self.debounce_ms_atomic.load(atomic::Ordering::Relaxed) as u32;
        let m = (if m16 < 16384 {
            (m16 * 250000 / 65535) * 10
        } else {
            (m16 * 25000 / 65535) * 100
        })
        .clamp(1, 2_500_000);

        let l = m / w;
        let f = l / 252 + 1;
        self.debounce_divisor = f;
        self.debounce_modulus = (4 + (l / f).clamp(4, 248)) & !3;
    }

    fn debounce_from_cycle(&self) -> u8 {
        4 + (((self.cycle / self.debounce_divisor) % self.debounce_modulus) as u8 & !3)
    }
}

fn start_debounce(debounce: u8, key_state: u8, modulus: u32) -> u8 {
    key_state
        | if debounce < 8 {
            (modulus - 4) as u8
        } else {
            debounce - 4
        }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
#[path = "key_scanner_test.rs"]
mod test;

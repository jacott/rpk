extern crate std;
use embassy_futures::{
    block_on, join,
    select::{self},
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, signal::Signal};
use embedded_hal::digital::{Error, ErrorType};
use std::rc::Rc;

use super::*;

#[derive(Debug)]
struct TestError;

#[derive(Clone)]
struct Pin(Rc<PinShared>);
impl core::fmt::Debug for Pin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let state = self.0.get_state();

        f.debug_struct("Pin")
            .field("n", &self.0.n)
            .field("state", &state)
            .finish()
    }
}
impl Pin {
    fn new(n: u8) -> Self {
        Self(Rc::new(PinShared {
            n,
            signal: Signal::new(),
        }))
    }
}

struct PinShared {
    n: u8,
    signal: Signal<NoopRawMutex, bool>,
}
impl PinShared {
    fn get_state(&self) -> Option<bool> {
        let state = self.signal.try_take();
        if let Some(is_high) = state {
            self.signal.signal(is_high);
        }
        state
    }
}

impl Error for TestError {
    fn kind(&self) -> embedded_hal::digital::ErrorKind {
        embedded_hal::digital::ErrorKind::Other
    }
}

impl ErrorType for Pin {
    type Error = TestError;
}

impl InputPin for Pin {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(matches!(self.0.get_state(), Some(true)))
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(matches!(self.0.get_state(), Some(false)))
    }
}

impl OutputPin for Pin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.0.signal.signal(false);
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.0.signal.signal(true);
        Ok(())
    }
}

impl Wait for Pin {
    async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
        while !self.0.signal.wait().await {}
        Ok(())
    }

    async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
        while self.0.signal.wait().await {}
        Ok(())
    }

    async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
        self.wait_for_low().await?;
        self.wait_for_high().await
    }

    async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
        self.wait_for_high().await?;
        self.wait_for_low().await
    }

    async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
        if self.0.signal.wait().await {
            self.wait_for_low().await?;
        } else {
            self.wait_for_high().await?;
        }
        Ok(())
    }
}

macro_rules! setup {
    ($scan:ident, $p1:ident, $channel:ident, $scanner:ident $debounce_ms:literal $b:block) => {
        block_on(async move {
            let mut $p1 = Pin::new(1);
            let p2 = Pin::new(2);
            $p1.set_high().ok();

            let $channel = KeyScannerChannel::<NoopRawMutex, 16>::default();
            let debounce_ms_atomic = atomic::AtomicU16::new($debounce_ms);
            let mut $scanner =
                KeyScanner::new([$p1.clone()], [p2.clone()], &$channel, &debounce_ms_atomic);
            $scanner.time_per_output_pin = Duration::from_micros(50);
            $scanner.calc_debounce_cycle();

            assert_eq!($p1.0.n, 1);

            macro_rules! $scan {
                () => {
                    scan!(1)
                };
                ($c:expr, $exp:expr) => {
                    assert_eq!(
                        scan!($c),
                        $exp,
                        "\nwe got:  {:#b}\nwanted:  {:#b}",
                        $scanner.state[0][0],
                        $exp
                    )
                };
                ($c:expr) => {{
                    for _ in 0..$c {
                        $scanner.scan::<true>().await;
                    }
                    $scanner.state[0][0]
                }};
            }

            $b
        })
    };
}

#[test]
fn debounce_low_sensitivity() {
    setup!(scan, p1, channel, scanner 655 {
        scan!(1, 0);

        assert_eq!(scanner.debounce_divisor, 2);
        assert_eq!(scanner.debounce_modulus, 252);
        assert_eq!(scanner.cycle, 0);


        p1.set_low().ok();
        scan!(1, 251);

        assert!(channel.0.try_receive().unwrap().is_down());

        scan!(10, 251);

        p1.set_high().ok();
        scan!(200, 6);

        p1.set_low().ok();
        scan!(3, 107);

        p1.set_high().ok();
        scan!(490, 106);

        assert!(channel.0.try_receive().is_err());

        scan!(1, 2);

        assert!(!channel.0.try_receive().unwrap().is_down());
    });
}

#[test]
fn debounce() {
    setup!(scan, p1, channel, scanner 30 {
        p1.set_low().ok();

        assert_eq!(scanner.debounce_divisor, 1);
        assert_eq!(scanner.debounce_modulus, 24);
        assert_eq!(scanner.cycle, 0);

        scan!(1, 0b1_0111);
        assert!(channel.0.try_receive().unwrap().is_down());
        scan!(1, 0b1_0111);

        p1.set_high().ok();

        scan!(2, 0b1_0110);

        p1.set_low().ok();

        scan!(20, 0b0_0111);


        scan!(1, 3);
        assert!(channel.0.try_receive().is_err());

        assert_eq!(scanner.cycle, 25);

        scan!(8, 3);

        p1.set_high().ok();

        scan!(1, 0b1000);
        assert!(!channel.0.try_receive().unwrap().is_down());
        scan!(18, 0b1000);
        scan!(1, 0);

        p1.set_low().ok();
        scan!(16, 0b10111);
        scan!(1, 0b11);

        assert!(channel.0.try_receive().unwrap().is_down());
    });
}

#[test]
fn wait_for_key() {
    setup!(_pscan, p1, channel, scanner 10 {
        scanner.cycle = 3;
        scanner.idle_start = Instant::now() - Duration::from_millis(1);
        p1.set_high().ok();
        let ordering = Channel::<NoopRawMutex, &'static str, 10>::new();
        let o1 = &ordering;
        let o2 = &ordering;
        let wf = async move {
            o1.try_send("wf0").unwrap();
            scanner.wait_for_key().await;
            o1.try_send("wf1").unwrap();
            scanner
        };
        let sf = async move {
            o2.receive().await;
            Timer::after_micros(10).await;
            p1.set_low().ok();
            o2.receive().await;
            true
        };
        let ans = select::select(join::join(wf,sf), Timer::after_millis(10)).await;
        let select::Either::First((scanner, true)) = ans else {
            panic!("Expected scanner");
        };
        assert_eq!(scanner.all_up_limit, 5988);
        assert_eq!(scanner.time_per_output_pin, Duration::from_ticks(334));
        assert_eq!(scanner.debounce_divisor, 1);
        assert_eq!(scanner.debounce_modulus, 8);
    });
}

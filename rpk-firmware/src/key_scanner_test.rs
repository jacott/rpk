extern crate std;
use embassy_futures::block_on;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embedded_hal::digital::{Error, ErrorType};
use std::rc::Rc;
use std::sync::Mutex;

use super::*;

#[derive(Debug)]
struct TestError;

#[derive(Debug, Clone)]
struct Pin(Rc<PinShared>);

#[derive(Debug)]
struct PinShared {
    n: u8,
    state: Mutex<bool>,
}

impl Pin {
    fn new(n: u8) -> Self {
        Self(Rc::new(PinShared {
            n,
            state: Mutex::new(false),
        }))
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
        let guard = self.0.state.lock().unwrap();
        Ok(*guard)
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        let guard = self.0.state.lock().unwrap();
        Ok(!*guard)
    }
}

impl OutputPin for Pin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        let mut guard = self.0.state.lock().unwrap();
        *guard = false;
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        let mut guard = self.0.state.lock().unwrap();
        *guard = true;
        Ok(())
    }
}

impl Wait for Pin {
    async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
        std::unimplemented!()
    }

    async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
        std::unimplemented!()
    }

    async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
        std::unimplemented!()
    }

    async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
        std::unimplemented!()
    }

    async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
        std::unimplemented!()
    }
}

macro_rules! setup {
    ($scan:ident, $p1:ident, $channel:ident, $scanner:ident $tune:literal $b:block) => {
        block_on(async move {
            let mut $p1 = Pin::new(1);
            let p2 = Pin::new(2);
            $p1.set_high().ok();

            let $channel = KeyScannerChannel::<NoopRawMutex, 16>::default();
            let mut $scanner = KeyScanner::new([$p1.clone()], [p2.clone()], &$channel);
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
                        $scanner.scan::<true, $tune>().await;
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
    setup!(scan, p1, channel, scanner 9 {
        scan!(1, 0);
        p1.set_low().ok();
        scan!(1, 251);

        assert!(channel.0.try_receive().unwrap().is_down());

        scan!(10, 251);

        p1.set_high().ok();
        scan!(200, 250);

        p1.set_low().ok();
        scan!(3, 175);

        p1.set_high().ok();
        scan!(2, 174);

        p1.set_low().ok();
        scan!(1, 175);

        assert!(channel.0.try_receive().is_err());
    });
}

#[test]
fn debounce() {
    setup!(scan, p1, channel, scanner 3 {
        p1.set_low().ok();

        scan!(1, 0b1000_0011);
        assert!(channel.0.try_receive().unwrap().is_down());
        scan!(1, 0b1000_0011);

        p1.set_high().ok();

        scan!(2, 0b1010_0010);

        p1.set_low().ok();

        scan!(1, 0b1100_0011);


        scan!(128, 3);
        assert!(channel.0.try_receive().is_err());

        assert_eq!(scanner.debounce, 266);

        p1.set_high().ok();

        scan!(1, 208);
        assert!(!channel.0.try_receive().unwrap().is_down());
        scan!(3, 208);
        scan!(10, 0);

        p1.set_low().ok();
        scan!(15, 0b11);

        assert!(channel.0.try_receive().unwrap().is_down());
    });
}

extern crate alloc;
extern crate std;

use alloc::vec;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, signal::Signal};
use embedded_hal::digital::{Error, ErrorType, InputPin, OutputPin};
use embedded_hal_async::digital::Wait;
use std::rc::Rc;
use std::sync::Mutex;
use std::vec::Vec;

pub trait Observer {
    fn update(&self, pin: Pin);
}

#[derive(Debug)]
struct KeyMatrixInner {
    switches: Vec<bool>,
    inputs: Vec<Pin>,
    outputs: Vec<Pin>,
}

#[derive(Clone)]
pub struct KeyMatrix {
    inner: Rc<Mutex<KeyMatrixInner>>,
}
impl KeyMatrix {
    pub fn new(inputs: Vec<Pin>, outputs: Vec<Pin>) -> Self {
        let me = Self {
            inner: Rc::new(Mutex::new(KeyMatrixInner {
                switches: vec![false; inputs.len() * outputs.len()],
                inputs,
                outputs,
            })),
        };

        for o in me.inner.lock().unwrap().outputs.iter() {
            o.add_observer(Rc::new(me.clone()))
        }

        me
    }

    pub fn down(&self, ipin: usize, opin: usize) {
        self.set_switch(ipin, opin, true);
    }

    pub fn up(&self, ipin: usize, opin: usize) {
        self.set_switch(ipin, opin, false);
    }

    pub fn set_switch(&self, ipin: usize, opin: usize, is_down: bool) {
        let mut inner = self.inner();
        let idx = ipin * inner.outputs.len() + opin;
        if inner.switches[idx] != is_down {
            inner.switches[idx] = is_down;
            if is_down {
                if inner.outputs[opin].is_low().unwrap() {
                    inner.inputs[ipin].set_low().unwrap();
                }
            } else {
                inner.inputs[ipin].set_high().unwrap();
            }
        }
    }

    fn inner(&self) -> std::sync::MutexGuard<'_, KeyMatrixInner> {
        self.inner.lock().unwrap()
    }
}
impl Observer for KeyMatrix {
    fn update(&self, mut pin: Pin) {
        let inner = self.inner();
        let n = pin.num();
        let is_down = pin.is_low().unwrap();
        let opin = inner.outputs.iter().position(|p| p.num() == n).unwrap();

        for (ipin, p) in inner.inputs.iter().enumerate() {
            if is_down {
                let idx = ipin * inner.outputs.len() + opin;
                if inner.switches[idx] {
                    p.clone().set_low().unwrap();
                }
            } else {
                p.clone().set_high().unwrap();
            }
        }
    }
}

#[derive(Debug)]
pub struct TestError;

#[derive(Clone)]
pub struct Pin(Rc<PinShared>);
impl core::fmt::Debug for Pin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let state = self.get_state();

        f.debug_struct("Pin")
            .field("n", &self.0.n)
            .field("state", &state)
            .finish()
    }
}
impl Pin {
    pub fn new(n: u8) -> Self {
        Self(Rc::new(PinShared {
            n,
            observer: Mutex::new(None),
            inner: Mutex::new(PinInner { is_high: None }),
            signal: Signal::new(),
        }))
    }

    pub fn num(&self) -> u8 {
        self.0.n
    }

    pub fn get_state(&self) -> Option<bool> {
        self.0.lock().is_high
    }

    fn add_observer(&self, observer: Rc<dyn Observer>) {
        *self.0.observer.lock().unwrap() = Some(observer);
    }
}

struct PinInner {
    is_high: Option<bool>,
}

struct PinShared {
    n: u8,
    observer: Mutex<Option<Rc<dyn Observer>>>,
    inner: Mutex<PinInner>,
    signal: Signal<NoopRawMutex, bool>,
}
impl PinShared {
    fn get_state(&self) -> Option<bool> {
        self.lock().is_high
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, PinInner> {
        self.inner.lock().unwrap()
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
        if !matches!(self.0.get_state(), Some(false)) {
            self.0.lock().is_high = Some(false);
            self.0.signal.signal(false);
            if let Some(o) = self.0.observer.lock().unwrap().clone() {
                o.update(self.clone());
            }
        }

        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        if !matches!(self.0.get_state(), Some(true)) {
            self.0.lock().is_high = Some(true);
            self.0.signal.signal(true);
            if let Some(o) = self.0.observer.lock().unwrap().clone() {
                o.update(self.clone());
            }
        }
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

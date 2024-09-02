use core::cell::RefCell;

use crate::{f32_from_u16, math::cubic_bezier};

#[derive(Debug, Clone, Copy)]
pub struct MouseConfig {
    pub movement: MouseAnalogSetting,
    pub scroll: MouseAnalogSetting,
}
impl Default for MouseConfig {
    fn default() -> Self {
        MouseConfig::normal()
    }
}
impl MouseConfig {
    pub const fn slow() -> Self {
        Self {
            movement: MouseAnalogSetting {
                curve: (0.1, 0.5),
                max_time: 1_000.0,
                min_ticks_per_ms: 0.02,
                max_ticks_per_ms: 1.0,
            },
            scroll: MouseAnalogSetting {
                curve: (0.0, 0.0),
                max_time: 5_000.0,
                min_ticks_per_ms: 0.01,
                max_ticks_per_ms: 0.25,
            },
        }
    }
    pub const fn normal() -> Self {
        Self {
            movement: MouseAnalogSetting {
                curve: (0.1, 0.5),
                max_time: 1_000.0,
                min_ticks_per_ms: 0.02,
                max_ticks_per_ms: 3.0,
            },
            scroll: MouseAnalogSetting {
                curve: (0.0, 0.0),
                max_time: 5_000.0,
                min_ticks_per_ms: 0.01,
                max_ticks_per_ms: 0.5,
            },
        }
    }
    pub const fn fast() -> Self {
        Self {
            movement: MouseAnalogSetting {
                curve: (0.1, 0.5),
                max_time: 1_000.0,
                min_ticks_per_ms: 0.02,
                max_ticks_per_ms: 5.0,
            },
            scroll: MouseAnalogSetting {
                curve: (0.0, 0.0),
                max_time: 5_000.0,
                min_ticks_per_ms: 0.01,
                max_ticks_per_ms: 1.0,
            },
        }
    }
    pub fn input(&self, i: usize) -> &MouseAnalogSetting {
        if i < 2 {
            &self.movement
        } else {
            &self.scroll
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MouseAnalogSetting {
    pub curve: (f32, f32),
    pub max_time: f32,
    pub min_ticks_per_ms: f32,
    pub max_ticks_per_ms: f32,
}
impl MouseAnalogSetting {
    pub fn deserialize(bin: &mut impl Iterator<Item = u16>) -> Option<MouseAnalogSetting> {
        Some(MouseAnalogSetting {
            curve: (
                f32_from_u16(bin.next()?, bin.next()?),
                f32_from_u16(bin.next()?, bin.next()?),
            ),
            max_time: f32_from_u16(bin.next()?, bin.next()?),
            min_ticks_per_ms: f32_from_u16(bin.next()?, bin.next()?),
            max_ticks_per_ms: f32_from_u16(bin.next()?, bin.next()?),
        })
    }

    pub fn mouse_delta(&self, time_since_last_report: f32, now: u32, k: &MouseMove) -> i8 {
        if k.start == 0 {
            return 0;
        }
        let mut guard = k.fraction.borrow_mut();
        let r = self.t2rate(time_since_last_report, now, k.start) + *guard;
        let ticks = r as i8;
        *guard = r - ticks as f32;
        ticks * k.delta
    }

    fn t2rate(&self, period: f32, now: u32, then: u32) -> f32 {
        let t = now - then;
        let (c0, c1) = self.curve;
        let t = min(t as f32, self.max_time) / self.max_time;
        min(
            (self.min_ticks_per_ms + self.max_ticks_per_ms * cubic_bezier(t, c0, c1)) * period,
            127.0,
        )
    }
}

#[derive(Default)]
pub struct MouseMove {
    pub start: u32,
    pub fraction: RefCell<f32>,
    pub delta: i8,
}
impl MouseMove {
    pub fn action(&mut self, is_down: bool, delta: i8) {
        if is_down {
            if self.start == 0 || delta != self.delta {
                self.delta = delta;
            }
        } else if self.start != 0 && delta == self.delta {
            self.start = 0;
        }
    }
}

fn min(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}

#[cfg(test)]
#[path = "mouse_test.rs"]
mod test;

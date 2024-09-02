use rpk_common::{
    keycodes::key_range,
    mouse::{MouseConfig, MouseMove},
};

use super::KeyEvent;

#[derive(Default)]
pub(super) struct Mouse {
    mouse_keys: u8,
    mouse_move: [MouseMove; 4],
    down_count: u8,
    /// in milliseconds
    first_down_time: u64,
    last_report_time: u32,
    config: MouseConfig,
}

impl Mouse {
    pub(super) fn clear_all(&mut self) {
        self.mouse_keys = 0;
        for m in self.mouse_move.iter_mut() {
            *m = Default::default();
        }
        self.down_count = 0;
        self.first_down_time = 0;
        self.last_report_time = 0;
    }

    pub(super) fn action(&mut self, code: u16, is_down: bool, now: u64) -> Option<KeyEvent> {
        match code {
            key_range::MOUSE_BUTTON..=key_range::MOUSE_BUTTON_END => {
                if is_down {
                    self.mouse_keys |= 1 << code;
                } else {
                    self.mouse_keys &= !(1 << code);
                }
                Some(KeyEvent::mouse_button(self.mouse_keys))
            }
            key_range::MOUSE_DELTA..=key_range::MOUSE_DELTA_END => {
                let kc = code - key_range::MOUSE_DELTA;
                let delta = if kc & 1 == 0 { -1 } else { 1 };
                let mm = &mut self.mouse_move[(kc >> 1) as usize];
                mm.action(is_down, delta);

                if is_down {
                    if self.down_count < 4 {
                        self.down_count += 1;
                        if self.down_count == 1 {
                            self.first_down_time = now - 1;
                            self.last_report_time = 0;
                        }
                        mm.start = (now - self.first_down_time) as u32;
                    }
                } else if self.down_count > 1 {
                    self.down_count -= 1;
                } else {
                    self.first_down_time = 0;
                    self.down_count = 0;
                }
                None
            }
            _ => {
                unreachable!("Mouse key unimplemented: {}", code);
            }
        }
    }

    pub(super) fn pending_events(&mut self, now: u64) -> impl Iterator<Item = KeyEvent> + use<'_> {
        let rel_ms = if self.down_count == 0 {
            u32::MAX
        } else {
            (now - self.first_down_time) as u32
        };
        let time_since_last_report = (rel_ms - self.last_report_time) as f32;
        self.last_report_time = rel_ms;
        self.pending_events1(time_since_last_report, rel_ms)
    }

    fn pending_events1(
        &self,
        time_since_last_report: f32,
        rel_ms: u32,
    ) -> impl Iterator<Item = KeyEvent> + use<'_> {
        let config = &self.config;
        self.mouse_move
            .iter()
            .take(if self.down_count == 0 { 0 } else { 4 })
            .enumerate()
            .filter_map(move |(i, k)| {
                let s = config
                    .input(i)
                    .mouse_delta(time_since_last_report, rel_ms, k);
                if s != 0 {
                    Some(KeyEvent::mouse_move(i as u8, s, self.mouse_keys))
                } else {
                    None
                }
            })
    }

    pub(crate) fn next_event_time(&self) -> u64 {
        if self.first_down_time == 0 {
            u64::MAX
        } else {
            (self.last_report_time + 16) as u64 + self.first_down_time
        }
    }

    #[cfg(test)]
    pub(crate) fn get_config(&self) -> &MouseConfig {
        &self.config
    }

    pub(crate) fn set_config(&mut self, config: &MouseConfig) {
        self.config = *config;
    }
}

#[cfg(test)]
#[path = "mouse_test.rs"]
mod test;

use rpk_common::keycodes::macro_types;

use super::KeyPlusMod;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum SequenceMode {
    Hold,
    Release,
    Tap,
}
impl SequenceMode {
    fn code(&self) -> u16 {
        match self {
            Self::Hold => macro_types::HOLD,
            Self::Release => macro_types::RELEASE,
            Self::Tap => macro_types::TAP,
        }
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct TapDance {
    pub(crate) wait_until: u64,
    pub(crate) location: u32,
    pub(crate) tap_timeout: u16,
    pub(crate) rem: u16,
}
impl TapDance {
    pub(crate) fn is_running(&self) -> bool {
        self.location != 0
    }

    pub(crate) fn clear(&mut self) {
        self.location = 0;
        self.wait_until = u64::MAX;
    }

    pub(crate) fn start(&mut self, tap_timeout: u16, location: u32, rem: u16) {
        self.wait_until = u64::MAX;
        self.location = location;
        self.tap_timeout = tap_timeout;
        self.rem = rem;
    }

    pub(crate) fn is_same(&self, location: u32, len: u16) -> bool {
        self.location + self.rem as u32 == location + len as u32
    }
}
impl Default for TapDance {
    fn default() -> Self {
        Self {
            wait_until: u64::MAX,
            location: 0,
            tap_timeout: u16::MAX,
            rem: 0,
        }
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Macro {
    Modifier(KeyPlusMod),
    DualAction(u16, u16, u16, u16),
    Noop,
    HoldRelease {
        hold: u16,
        release: u16,
    },
    Sequence {
        mode: SequenceMode,
        location: u32,
        rem: u16,
    },
    Delay(u16),
    TapDance(u32, u16),
}
impl Macro {
    pub fn decode(location: usize, data: Option<&[u16]>) -> Self {
        match data {
            Some(data) => match data[0] & 0xff {
                macro_types::MODIFIER => {
                    Macro::Modifier(KeyPlusMod::new(data[1], (data[0] >> 8) as u8))
                }
                macro_types::DUAL_ACTION => {
                    let (t1, t2) = if data.len() > 3 {
                        (data[3], if data.len() > 4 { data[4] } else { u16::MAX })
                    } else {
                        (u16::MAX, u16::MAX)
                    };
                    Macro::DualAction(data[1], data[2], t1, t2)
                }
                macro_types::TAPDANCE => {
                    Macro::TapDance(location as u32 + 1, data.len() as u16 - 1)
                }
                macro_types::HOLD_RELEASE => Macro::HoldRelease {
                    hold: data[1],
                    release: data[2],
                },
                macro_types::DELAY => Macro::Delay(data[1]),
                mode => {
                    if let Some(mode) = Macro::sequence_mode(mode) {
                        Macro::Sequence {
                            mode,
                            location: location as u32 + 1,
                            rem: data.len() as u16 - 1,
                        }
                    } else {
                        unreachable!("code {}", mode)
                    }
                }
            },
            None => Macro::Noop,
        }
    }

    pub(crate) fn update(&self, store: &mut [u16]) {
        if let Macro::Sequence {
            mode,
            location,
            rem,
        } = &self
        {
            let rem = *rem;
            let l = location.to_le_bytes();

            store[..3].copy_from_slice(&[
                mode.code() | ((l[2] as u16) << 8),
                l[0] as u16 | ((l[1] as u16) << 8),
                rem,
            ]);
        }
    }

    pub(crate) fn stack_size(&self) -> usize {
        if matches!(self, Self::Sequence { .. }) {
            3
        } else {
            0
        }
    }

    pub(crate) fn push(self, store: &mut [u16]) -> (Self, usize) {
        let size = self.stack_size();
        if size != 0 && store.len() > size {
            let start = store.len() - size;
            self.update(&mut store[start..]);
            return (self, size);
        }

        (self, 0)
    }

    pub(crate) fn pop(data: &[u16]) -> (Self, usize) {
        (Self::restore(&data[3..]), 3)
    }

    fn restore(data: &[u16]) -> Self {
        if data.is_empty() {
            return Macro::Noop;
        }
        if let Some(mode) = Self::sequence_mode(data[0] & 0xff) {
            let location = data[1] as u32 | (((data[0] & 0xff00) as u32) << 16);
            Self::Sequence {
                mode,
                location,
                rem: data[2],
            }
        } else {
            unreachable!("Not supported {}", data[0] & 0xff)
        }
    }

    fn sequence_mode(mode: u16) -> Option<SequenceMode> {
        Some(match mode {
            macro_types::TAP => SequenceMode::Tap,
            macro_types::HOLD => SequenceMode::Hold,
            macro_types::RELEASE => SequenceMode::Release,
            _ => return None,
        })
    }
}

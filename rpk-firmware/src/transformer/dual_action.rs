use core::cmp::min;

use DualActionTimer::*;

use super::TimedScanKey;

#[derive(Copy, Clone, Debug, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(super) enum DualActionTimer {
    #[default]
    NoDual,
    Wait {
        scan_key: TimedScanKey,
        duration: u16,
        tap_timeout: u16,
        tap: u16,
        hold: u16,
        count: usize,
    },
    Hold {
        scan_key: TimedScanKey,
        hold: u16,
    },
    Tap {
        scan_key: TimedScanKey,
        tap: u16,
    },
}
impl DualActionTimer {
    pub(crate) fn start(
        &mut self,
        scan_key: TimedScanKey,
        tap: u16,
        hold: u16,
        duration: u16,
        tap_timeout: u16,
    ) {
        debug_assert!(matches!(self, NoDual));
        *self = Wait {
            scan_key,
            duration,
            tap_timeout,
            tap,
            hold,
            count: 2,
        };
    }

    pub(crate) fn is_no_timer(&self) -> bool {
        !matches!(self, Wait { .. })
    }

    pub(crate) fn wait_until(&self) -> u64 {
        match self {
            Wait {
                scan_key: TimedScanKey(_, time),
                duration,
                tap_timeout,
                count,
                ..
            } => {
                let n = time + *duration as u64;
                if *count == 0 {
                    min(n, *time + *tap_timeout as u64)
                } else {
                    n
                }
            }
            _ => u64::MAX,
        }
    }

    pub(crate) fn key_switch(&mut self, next_key: TimedScanKey) -> bool {
        match self {
            NoDual => true,
            Wait {
                scan_key,
                duration,
                tap_timeout,
                tap,
                hold,
                count,
            } => {
                if next_key.same_key(scan_key) {
                    *self = Tap {
                        scan_key: *scan_key,
                        tap: *tap,
                    };
                } else {
                    if *count == 0 {
                        *self = Hold {
                            scan_key: *scan_key,
                            hold: *hold,
                        };
                        return false;
                    }

                    *self = Wait {
                        scan_key: *scan_key,
                        duration: *duration,
                        tap: *tap,
                        tap_timeout: if *count == 1 {
                            *tap_timeout + (next_key.1 - scan_key.1) as u16
                        } else {
                            *tap_timeout
                        },
                        hold: *hold,
                        count: *count - 1,
                    };
                }
                false
            }
            _ => false,
        }
    }

    pub(crate) fn timer_expired(&mut self) {
        if let Wait { scan_key, hold, .. } = self {
            *self = Hold {
                scan_key: *scan_key,
                hold: *hold,
            };
        }
    }
}

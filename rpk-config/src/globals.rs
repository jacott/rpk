use std::collections::HashMap;

use lazy_static::lazy_static;

pub(crate) mod spec {
    use rpk_common::{
        globals,
        mouse::{MouseAnalogSetting, MouseConfig},
    };

    #[derive(Clone, Copy, Debug)]
    pub(crate) enum GlobalType {
        Timeout {
            value: u16,
            max: u16,
            min: u16,
        },
        TimeoutCurve {
            value: u16,
            max: f32,
            min: f32,
            dp: i32,
            multiplier: f32,
        },
        MouseProfile(MouseConfig),
    }
    impl GlobalType {
        pub(crate) fn parse(&self, text: &str) -> Result<Self, String> {
            match self {
                Timeout { max, min, .. } => Ok(Timeout {
                    value: parse_duration(text, *min, *max)?,
                    max: *max,
                    min: *min,
                }),
                TimeoutCurve {
                    max,
                    min,
                    dp,
                    multiplier,
                    ..
                } => {
                    if let Ok(n) = text.parse::<f32>() {
                        if n >= *min && n <= *max {
                            return Ok(TimeoutCurve {
                                value: (n * multiplier) as u16,
                                max: *max,
                                min: *min,
                                dp: *dp,
                                multiplier: *multiplier,
                            });
                        }
                    }
                    let dp = *dp as usize;
                    Err(format!(
                        "Invalid duration; only {min:.*} to {max:.*} milliseconds are valid",
                        dp, dp
                    ))
                }
                _ => panic!("Unsupported"),
            }
        }
    }

    pub fn parse_duration(text: &str, min: u16, max: u16) -> Result<u16, String> {
        if let Ok(value) = text.parse::<u16>() {
            if value.clamp(min, max) == value {
                return Ok(value);
            }
        }
        Err(format!(
            "Invalid duration; only {min} to {max} milliseconds are valid"
        ))
    }

    use GlobalType::*;

    use crate::f32_to_u16;

    #[derive(Clone, Copy, Debug)]
    pub(crate) struct GlobalProp {
        pub(crate) index: u16,
        pub(crate) spec: GlobalType,
    }

    impl GlobalProp {
        pub(crate) fn new_default(name: &str) -> Result<GlobalProp, String> {
            super::DEFAULTS
                .get(name)
                .ok_or_else(|| format!("Invalid global '{}'", name))
                .copied()
        }

        #[cfg(test)]
        pub(crate) fn default_name(&self) -> Option<&'static str> {
            super::INDEX_TO_NAME.get(self.index as usize).copied()
        }

        #[cfg(test)]
        pub(crate) fn deserialize(data: &mut impl Iterator<Item = u16>) -> Option<Self> {
            let index = data.next()?;
            let name = super::INDEX_TO_NAME.get(index as usize).copied()?;
            let mut gp = GlobalProp::new_default(name).ok()?;
            match gp.spec {
                Timeout { ref mut value, .. } | TimeoutCurve { ref mut value, .. } => {
                    *value = data.next()?
                }
                MouseProfile(ref mut config) => {
                    config.movement = MouseAnalogSetting::deserialize(data)?;
                    config.scroll = MouseAnalogSetting::deserialize(data)?;
                }
            }

            Some(gp)
        }

        pub(crate) fn serialize(self) -> Box<dyn Iterator<Item = u16>> {
            match self.spec {
                Timeout { value, .. } | TimeoutCurve { value, .. } => {
                    Box::new([self.index, value].into_iter())
                }
                MouseProfile(MouseConfig { movement, scroll }) => Box::new(
                    [self.index]
                        .into_iter()
                        .chain(mouse_to_binary(movement))
                        .chain(mouse_to_binary(scroll)),
                ),
            }
        }

        pub(crate) fn set_sub_field(
            &mut self,
            field: &str,
            name: &str,
            value: &str,
        ) -> Result<(), String> {
            match self.spec {
                MouseProfile(ref mut config) => {
                    let a = match field {
                        "movement" => &mut config.movement,
                        "scroll" => &mut config.scroll,
                        _ => return Err(format!("unknown field {field}")),
                    };

                    match name {
                        "curve" => a.curve = parse_curve(value)?,
                        "max_time" => a.max_time = parse_float(value)?,
                        "min_ticks_per_ms" => a.min_ticks_per_ms = parse_float(value)?,
                        "max_ticks_per_ms" => a.max_ticks_per_ms = parse_float(value)?,
                        _ => return Err(format!("unknown field {field}.{name}")),
                    }

                    Ok(())
                }
                _ => Err(format!("unknown field {field}")),
            }
        }
    }

    pub(crate) fn mouse_to_binary(config: MouseAnalogSetting) -> impl Iterator<Item = u16> {
        f32_to_u16(config.curve.0)
            .chain(f32_to_u16(config.curve.1))
            .chain(f32_to_u16(config.max_time))
            .chain(f32_to_u16(config.min_ticks_per_ms))
            .chain(f32_to_u16(config.max_ticks_per_ms))
    }

    fn parse_curve(value: &str) -> Result<(f32, f32), String> {
        let value = value.trim_matches(['[', ']']);
        let (a, b) = value
            .split_once(',')
            .ok_or_else(|| format!("invalid value {value}"))?;
        Ok((parse_float(a)?, parse_float(b)?))
    }

    fn parse_float(a: &str) -> Result<f32, String> {
        a.trim().parse::<f32>().map_err(|e| format!("{} {a}", e))
    }

    pub(super) const GLOBALS: [GlobalProp; 7] = [
        GlobalProp {
            index: globals::MOUSE_PROFILE1,
            spec: GlobalType::MouseProfile(MouseConfig::slow()),
        },
        GlobalProp {
            index: globals::MOUSE_PROFILE2,
            spec: GlobalType::MouseProfile(MouseConfig::normal()),
        },
        GlobalProp {
            index: globals::MOUSE_PROFILE3,
            spec: GlobalType::MouseProfile(MouseConfig::fast()),
        },
        GlobalProp {
            index: globals::DUAL_ACTION_TIMEOUT,
            spec: GlobalType::Timeout {
                value: globals::DUAL_ACTION_TIMEOUT_DEFAULT,
                min: 0,
                max: 5000,
            },
        },
        GlobalProp {
            index: globals::DUAL_ACTION_TIMEOUT2,
            spec: GlobalType::Timeout {
                value: globals::DUAL_ACTION_TIMEOUT2_DEFAULT,
                min: 0,
                max: 5000,
            },
        },
        GlobalProp {
            index: globals::DEBOUNCE_SETTLE_TIME,
            spec: GlobalType::TimeoutCurve {
                value: globals::DEBOUNCE_SETTLE_TIME_DEFAULT,
                min: 0.1,
                max: 2500.0,
                dp: 1,
                multiplier: 65535.0 / 2500.0,
            },
        },
        GlobalProp {
            index: globals::TAPDANCE_TAP_TIMEOUT,
            spec: GlobalType::Timeout {
                value: globals::TAPDANCE_TAP_TIMEOUT_DEFAULT,
                min: 0,
                max: 5000,
            },
        },
    ];
}

lazy_static! {
    pub(crate) static ref INDEX_TO_NAME: [&'static str; spec::GLOBALS.len()] = [
        "mouse_profile1",
        "mouse_profile2",
        "mouse_profile3",
        "dual_action_timeout",
        "dual_action_timeout2",
        "debounce_settle_time",
        "tapdance_tap_timeout",
    ];
    pub(crate) static ref DEFAULTS: HashMap<&'static str, spec::GlobalProp> = {
        let mut m = HashMap::new();
        m.insert("overload_tap_timeout", spec::GLOBALS[3]);
        for (k, v) in INDEX_TO_NAME.iter().zip(spec::GLOBALS.iter()) {
            m.insert(k, *v);
        }
        m
    };
}

#[cfg(test)]
#[path = "globals_test.rs"]
mod test;

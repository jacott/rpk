use std::collections::HashMap;

use lazy_static::lazy_static;

pub mod spec {
    use rpk_common::{
        globals,
        mouse::{MouseAnalogSetting, MouseConfig},
    };

    #[derive(Clone, Copy, Debug)]
    pub enum GlobalType {
        Timeout {
            value: u16,
            max: u16,
            min: u16,
            dp: i32,
        },
        MouseProfile(MouseConfig),
    }
    use GlobalType::*;

    use crate::f32_to_u16;

    #[derive(Clone, Copy, Debug)]
    pub struct GlobalProp {
        pub index: u16,
        pub spec: GlobalType,
    }

    impl GlobalProp {
        pub fn new_default(name: &str) -> Result<GlobalProp, String> {
            super::DEFAULTS
                .get(name)
                .ok_or_else(|| format!("Invalid global '{}'", name))
                .copied()
        }

        pub fn default_name(&self) -> Option<&'static str> {
            super::INDEX_TO_NAME.get(self.index as usize).copied()
        }

        #[cfg(test)]
        pub fn deserialize(data: &mut impl Iterator<Item = u16>) -> Option<Self> {
            let index = data.next()?;
            let name = super::INDEX_TO_NAME.get(index as usize).copied()?;
            let mut gp = GlobalProp::new_default(name).ok()?;
            match gp.spec {
                Timeout { ref mut value, .. } => *value = data.next()?,
                MouseProfile(ref mut config) => {
                    config.movement = MouseAnalogSetting::deserialize(data)?;
                    config.scroll = MouseAnalogSetting::deserialize(data)?;
                }
            }

            Some(gp)
        }

        pub fn serialize(self) -> Box<dyn Iterator<Item = u16>> {
            match self.spec {
                Timeout { value, .. } => Box::new([self.index, value].into_iter()),
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

    pub fn mouse_to_binary(config: MouseAnalogSetting) -> impl Iterator<Item = u16> {
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

    pub(super) const GLOBALS: [GlobalProp; 6] = [
        GlobalProp {
            index: globals::DUAL_ACTION_TIMEOUT,
            spec: GlobalType::Timeout {
                value: globals::DUAL_ACTION_TIMEOUT_DEFAULT,
                min: 0,
                max: 5000,
                dp: 0,
            },
        },
        GlobalProp {
            index: globals::DUAL_ACTION_TIMEOUT2,
            spec: GlobalType::Timeout {
                value: globals::DUAL_ACTION_TIMEOUT2_DEFAULT,
                min: 0,
                max: 5000,
                dp: 0,
            },
        },
        GlobalProp {
            index: globals::DEBOUNCE_SETTLE_TIME,
            spec: GlobalType::Timeout {
                value: globals::DEBOUNCE_SETTLE_TIME_DEFAULT,
                min: 1,
                max: 250,
                dp: 1,
            },
        },
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
    ];
}

lazy_static! {
    pub static ref INDEX_TO_NAME: [&'static str; 6] = [
        "dual_action_timeout",
        "dual_action_timeout2",
        "debounce_settle_time",
        "mouse_profile1",
        "mouse_profile2",
        "mouse_profile3",
    ];
    pub static ref DEFAULTS: HashMap<&'static str, spec::GlobalProp> = {
        let mut m = HashMap::new();
        m.insert("overload_tap_timeout", spec::GLOBALS[0]);
        for (k, v) in INDEX_TO_NAME.iter().zip(spec::GLOBALS.iter()) {
            m.insert(k, *v);
        }
        m
    };
}

#[cfg(test)]
#[path = "globals_test.rs"]
mod test;

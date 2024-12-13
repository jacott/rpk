use std::path::Path;

use key_range::{LAYER_MIN, MACROS_MIN, TOGGLE_MIN};

use crate::{globals::spec::GlobalType, test::kc};

use super::*;

pub fn pretty_compile(src: &str) -> Result<KeyboardConfig> {
    crate::pretty_compile(Path::new("test"), src)
}

pub fn test_compile(source: &str) -> Result<KeyboardConfig> {
    compile(PathBuf::from(""), source)
}

macro_rules! compile_global {
    ($src:ident, $result:ident, $name:expr, $value:expr, $x:tt) => {{
        let $src = format!("[global]\n{} = {}\n", $name, $value);
        let $result = test_compile($src.as_str());

        $x
    }};
}

fn key_position(config: &KeyboardConfig, sym: &str, index: usize) -> u16 {
    config.get_aliases(sym).unwrap()[index]
}

#[test]
fn firmware_section() {
    let src = r#"
[firmware]

vendor_id = 0x6e0f
product_id = 0x0000
serial_number = rpk:0001

manufacturer = Jacott
product = RPK macropad-3x3
max_power = 450

chip = rp2040
output_pins = [PIN_4, # comment pin 4]
PIN_5, PIN_6]  # comment
input_pins = [PIN_7 PIN_8, PIN_9]
row_is_output = true

max_layout_size = 8 * 1024

# Flash ring file system
flash_size = 2 * 1024 * 1024
fs_base = 0x100000
FS_SIZE = flash_size - fs_base

report_buffer_size = 32
scanner_buffer_size = 16

[matrix:3x3]
0x00 = 7 8 9
"#;

    let config = pretty_compile(src).expect("should allow firmware");

    assert_eq!(
        config.firmware_get_str("output_pins").unwrap(),
        "[PIN_4, # comment pin 4]\nPIN_5, PIN_6]  # comment"
    );
    assert_eq!(
        config.text(&config.firmware_get("fs_size").unwrap()),
        "flash_size - fs_base"
    );
}

#[test]
fn keycode_names() {
    let src = r#"
[matrix:1x4]
\0x00 = \= b c d

[main]

equal =  1
\b = \\
c = mediaplaypause
d = AC_Select_All

[shift]
b = S-\[

"#;

    let config = pretty_compile(src).expect("should allow escaping");
    assert_eq!(config.code_at("main", 0x0), kc("1"));
    assert_eq!(config.code_at("main", 0x1), kc("\\"));
    assert_eq!(config.code_at("main", 0x2), 232);
    assert_eq!(config.code_at("main", 0x3), 798);
}

#[test]
fn toggle_layout() {
    let src = r#"
[matrix:1x2]
0x00 = ; b

[main]

; = toggle(shift)
b = toggle(control)

"#;

    let config = pretty_compile(src).expect("should allow toggle");
    assert_eq!(config.code_at("main", 0x0), TOGGLE_MIN + 1);
    assert_eq!(config.code_at("main", 0x1), TOGGLE_MIN);
}

#[test]
fn dual_action_macro() {
    let src = r#"
[matrix:1x2]
0x00 = a b

[main]

a = dualaction(rightshift, a, 251, 45)
b = dualaction(hold(leftshift a), C-c)
"#;

    let config = pretty_compile(src).expect("should allow overload");

    let da_a = config.macros.first().expect("should find a's dualaction");

    let exp = Macro::TimedDualAction(kc("a"), kc("rightshift"), 251, 45);
    assert_eq!(da_a, &exp);

    let hold_sa = config.macros.get(1).expect("should find hold(leftshift a)");

    let exp = Macro::Hold(vec![kc("leftshift"), kc("a")]);
    assert_eq!(hold_sa, &exp);

    let da_b = config.macros.get(3).expect("should find b's dualaction");

    let exp = Macro::DualAction(MACROS_MIN + 2, MACROS_MIN + 1);
    assert_eq!(da_b, &exp);
}

#[test]
fn overload_action() {
    // aka dual_action_macro
    let src = r#"
[matrix:1x2]
0x00 = a b

[main]

a = overload(shift, x)
b = overload(control, C-c, 36, 45)
"#;

    let config = pretty_compile(src).expect("should allow overload");

    let shift_x = config.macros.first().expect("should find C-x");

    let exp = Macro::DualAction(kc("x"), LAYER_MIN + 1);
    assert_eq!(shift_x, &exp);

    let mod_cc = config.macros.get(1).expect("should find C-x");

    let exp = Macro::Modifier {
        keycode: kc("c"),
        modifiers: 1,
    };
    assert_eq!(mod_cc, &exp);

    let control_cc = config.macros.get(2).expect("should find C-x");

    let exp = Macro::TimedDualAction(MACROS_MIN + 1, LAYER_MIN, 36, 45);
    assert_eq!(control_cc, &exp);
}

#[test]
fn macro_tap() {
    let src = r#"
[matrix:1x2]
0x00 = a b

[main]

a = macro(hello space W orld!!!)
b = macro(toggle(shift) 123, <abc} toggle(shift))
"#;

    let config = pretty_compile(src).expect("should allow tap macros");

    {
        let exp = Macro::Modifier {
            keycode: kc("w"),
            modifiers: 2,
        };
        let cx = config.macros.first().expect("should find S-w");
        assert_eq!(cx, &exp);

        let exp = Macro::Modifier {
            keycode: kc("1"),
            modifiers: 2,
        };
        let cx = config.macros.get(1).expect("should find S-1");
        assert_eq!(cx, &exp);

        let exp_code = MACROS_MIN + 2;

        assert_eq!(config.code_at("main", 0x00), exp_code);

        let cx = config
            .macros
            .get((exp_code - key_range::MACROS_MIN) as usize)
            .expect("should find macro");

        let mut codes = vec![];
        for c in "hello World".chars() {
            let code = if c == ' ' {
                kc("space")
            } else if c == 'W' {
                MACROS_MIN
            } else {
                kc(c.to_string().as_str())
            };
            codes.push(code);
        }
        for _ in 0..3 {
            codes.push(MACROS_MIN + 1);
        }

        let exp = Macro::Tap(codes);

        assert_eq!(cx, &exp);
    }

    {
        let exp_code = MACROS_MIN + 5;

        assert_eq!(config.code_at("main", 0x01), exp_code);

        let cx = config
            .macros
            .get((exp_code - key_range::MACROS_MIN) as usize)
            .expect("should find macro");

        let codes = vec![1793, 30, 31, 32, 54, 4099, 4, 5, 6, 4100, 1793];

        let exp = Macro::Tap(codes);

        assert_eq!(cx, &exp);
    }
}

#[test]
fn macro_tap_hold_release() {
    let src = r#"
[matrix:1x2]
0x00 = a b

[main]

a = macro(hold(aab) release(ba))
b = macro(a hold(aab) release(ba))
"#;

    let config = pretty_compile(src).expect("should allow hold/release macros");

    {
        let exp = Macro::Hold(vec![4, 4, 5]);
        let cx = config.macros.first().expect("should find hold");
        assert_eq!(cx, &exp);

        let exp = Macro::Release(vec![5, 4]);
        let cx = config.macros.get(1).expect("should find release");
        assert_eq!(cx, &exp);

        assert_eq!(config.code_at("main", 0x00), MACROS_MIN + 2);
        assert_eq!(config.code_at("main", 0x01), MACROS_MIN + 3);

        let cx = config.macros.get(2).expect("should find macro");
        let exp = Macro::HoldRelease {
            hold: MACROS_MIN,
            release: MACROS_MIN + 1,
        };
        assert_eq!(cx, &exp);

        let cx = config.macros.get(3).expect("should find macro");
        let exp = Macro::Tap(vec![4, MACROS_MIN, MACROS_MIN + 1]);
        assert_eq!(cx, &exp);
    }
}

#[test]
fn modifier_macros() {
    let src = r#"
[matrix:1x2]
0x00 = a b

[main]

a = C-x
b = S-G-z
"#;

    let config = pretty_compile(src).expect("should allow modifier_macros");

    let exp_code = MACROS_MIN;

    assert_eq!(config.code_at("main", 0x00), exp_code);
    assert_eq!(config.code_at("main", 0x01), exp_code + 1);

    let cx = config
        .macros
        .get((exp_code - key_range::MACROS_MIN) as usize)
        .expect("should find C-x");

    let exp = Macro::Modifier {
        keycode: kc("x"),
        modifiers: 1,
    };

    assert_eq!(cx, &exp);

    let smz = config
        .macros
        .get((exp_code + 1 - key_range::MACROS_MIN) as usize)
        .expect("should find S-G-z");

    let exp = Macro::Modifier {
        keycode: kc("z"),
        modifiers: 10,
    };

    assert_eq!(smz, &exp);

    let codes = crate::text_to_binary(src).unwrap();

    let m0 = MACROS_MIN;
    let m1 = m0 + 1;

    assert_eq!(
        &codes,
        &[
            1, 258, 6, 2, 0, 9, 10, 11, 12, 13, 14, 17, 19, 21, 1, 2, 4, 8, 64, 0, m0, m1, 256, 27,
            2560, 29
        ]
    );
}

#[test]
fn compiling_to_binary() {
    let codes = crate::text_to_binary(
        r#"
[matrix:3x3]

0x00 = 7 8 9
0x10 = 4 5 6
0x20 = 1 2 3

[main]

3 = layer(shift)

[shift]

7 = a
8 = b
9 = c

4 = d
5 = e
6 = f

"#,
    )
    .unwrap();

    assert_eq!(
        &codes,
        &[
            1, 771, 6, 0, 0, 7, 8, 18, 19, 20, 21, 31, 1, 2, 4, 5, 6, 7, 8, 9, 0, 0, 0, 4, 8, 64,
            0, 36, 37, 38, 33, 34, 35, 30, 31, 1537
        ]
    );
}

#[test]
fn aliases() {
    let src = r#"
[aliases]

0x00 = a # can use 'a' for key_position 0,0
0x23 = b
0x43 = a
"#;

    let config = test_compile(src).unwrap();

    assert_eq!(key_position(&config, "a", 1), 0x0403);
    assert_eq!(key_position(&config, "b", 0), 0x0203);
}

#[test]
fn matrix() {
    let src = r#"
[matrix:3x3]

0x00 = 7 8 9
0x10 = 4      # can split rows up
0x11 = 5 6
0x20 = 1 2 3

[aliases]

0x21 = a
"#;

    let config = test_compile(src).unwrap();

    assert_eq!(key_position(&config, "a", 0), 0x201);
    assert_eq!(key_position(&config, "2", 0), 0x201);
    assert_eq!(key_position(&config, "6", 0), 0x102);

    assert_eq!(config.code_at("main", 0x102), 0x23);
}

#[test]
fn layer() {
    let src = r#"
[matrix:2x2]
0x00 = a b
0x10 = c d

[my_layer:S]
a = z
"#;

    let config = pretty_compile(src).unwrap();

    let layer = config.layers.get("my_layer").unwrap();

    assert_eq!(layer.suffix, 2);
    assert_eq!(layer.code_at(0), 29);
}

#[test]
fn invalid_alias_multi_assign() {
    let src = r#"
[matrix:2x2]
0x00 = a b
0x10 = c d

[aliases]

a = shift foo
"#;

    let config = test_compile(src).err().unwrap();

    assert_eq!(config.message, "Only one value may be assigned");
    assert_eq!(config.span.unwrap(), 58..61);
}

#[test]
fn invalid_layer_multi_assign() {
    let src = r#"
[matrix:2x2]
0x00 = a b
0x10 = c d

[aliases]

a = shift
b = shift

[my_layer]
shift = x y
c = 1 2
"#;

    let config = test_compile(src).err().unwrap();

    assert_eq!(
        config.message,
        "Only one value may be assigned to an multi-positioned alias"
    );
    assert_eq!(config.span.unwrap(), 90..91);
}

#[test]
fn invalid_assign() {
    let src = r#"
[my_layer]
shift 1
a 2
"#;

    let config = test_compile(src).err().unwrap();

    assert_eq!(config.message, "Missing =");
    assert_eq!(config.span.unwrap(), 12..18);
}

#[test]
fn setlayout() {
    let src = r#"
[matrix:2x2]
0x00 = a b
0x10 = c d

[main]

a = setlayout(my_layout)

[my_layout]

0x00 = 1 2
c = 3 4
"#;

    let config = pretty_compile(src).unwrap();

    let layer = config.layers.get("main").unwrap();

    assert_eq!(layer.code_at(0x0000), key_range::SET_LAYOUT_MIN + 6);
    let layer = config.layers.get("my_layout").unwrap();

    assert_eq!(layer.code_at(0x0000), 30);
    assert_eq!(layer.code_at(0x0001), 31);
    assert_eq!(layer.code_at(0x0100), 32);
    assert_eq!(layer.code_at(0x0101), 33);
}

#[test]
fn oneshot() {
    let src = r#"
[matrix:2x2]
0x00 = a b
0x10 = c d

[main]

a = oneshot(my_layer)
b = oneshot(l2)

[my_layer]
a = z
[l2]
a = 2
"#;

    let config = pretty_compile(src).unwrap();

    let layer = config.layers.get("main").unwrap();

    assert_eq!(layer.code_at(0), key_range::ONESHOT + 6);
    assert_eq!(layer.code_at(1), key_range::ONESHOT + 7);
}

#[test]
fn layer_action() {
    let src = r#"
[matrix:2x3]
0x00 = a b leftshift
0x10 = c d rightshift

[main]
a = layer(my_layer)

[shift]

c = 0
a = z

[my_layer]
b = 1
"#;

    let config = test_compile(src).unwrap();

    let layer = config.layers.get("main").unwrap();

    assert_eq!(layer.code_at(0x0000), key_range::LAYER_MIN + 6);

    let layer = config.layers.get("my_layer").unwrap();

    assert_eq!(layer.suffix, 0);
    assert_eq!(layer.code_at(0x0001), 30);

    let bytes = config.serialize();

    assert_eq!(bytes.len(), 32);

    assert_eq!(
        &bytes,
        &[
            1, 515, 7, 0, 0, 8, 9, 14, 15, 16, 17, 24, 27, 1, 2, 0, 29, 256, 39, 4, 8, 64, 0, 1542,
            5, 225, 6, 7, 229, 0, 1, 30
        ]
    );

    let c2 = KeyboardConfig::deserialize(&bytes);

    assert_eq!(c2.row_count, 2);
    assert_eq!(c2.col_count, 3);

    assert_eq!(c2.next_layer, 7);
    assert_eq!(c2.layers.len(), 7);

    let layer = c2.layers.get("main").unwrap();

    assert_eq!(layer.code_at(0x0000), key_range::LAYER_MIN + 6);

    let layer = c2.layers.get("layer6").unwrap();

    assert_eq!(layer.code_at(0x0001), keycodes::key_code("1").unwrap());
}

#[test]
fn bad_layer_modifier() {
    let src = r#"
[matrix:2x2]
0x00 = a b
0x10 = c d

[main]

c = down # comment

[my_layer:S]
a = z

[my_layer:S-C]

b = x

"#;

    let err = test_compile(src).err().unwrap();

    assert_eq!(&err.message, "layer suffix may not be changed; S != S-C");
    assert_eq!(
        err.span.as_ref().unwrap(),
        &(86..98),
        "error was {:?}",
        &err
    );
}

#[test]
fn invalid_layer_suffix() {
    let src = r#"
[matrix:2x2]
0x00 = a b
0x10 = c d

[main]

c = down # comment

[my_layer:S:C]

a = z

"#;

    let err = test_compile(src).err().unwrap();

    assert_eq!(&err.message, "Invalid layer suffix 'S:C'");
    assert_eq!(
        err.span.as_ref().unwrap(),
        &(75..78),
        "error was {:?}",
        &err
    );
}

#[test]
fn mouse_profile() {
    let config = pretty_compile(
        r#"
[global.mouse_profile1.movement]

curve = [0.08, 0.9]
max_time = 2000
min_ticks_per_ms = 0.02
max_ticks_per_ms = 3.0

[global.mouse_profile1.scroll]

curve = [0.8, 0.05]
max_time = 3000
min_ticks_per_ms = 0.07
max_ticks_per_ms = 3.5

[global.mouse_profile3.movement]

curve = [0.08, 0.1]
max_time = 2100
min_ticks_per_ms = 0.2
max_ticks_per_ms = 5.0
"#,
    )
    .unwrap();

    let GlobalType::MouseProfile(mp) = config.global("mouse_profile1").unwrap().spec else {
        panic!("expected mouse profile");
    };

    assert_eq!(mp.movement.max_time, 2000.0);
    assert_eq!(mp.movement.curve, (0.08, 0.9));
    assert_eq!(mp.movement.min_ticks_per_ms, 0.02);
    assert_eq!(mp.movement.max_ticks_per_ms, 3.0);
    assert_eq!(mp.scroll.max_time, 3000.0);
    assert_eq!(mp.scroll.max_ticks_per_ms, 3.5);

    let bin = config.serialize();
    assert_eq!(bin.len(), 66);

    let c2 = KeyboardConfig::deserialize(&bin);

    let GlobalType::MouseProfile(mp2) = c2.global("mouse_profile1").unwrap().spec else {
        panic!("expected mouse profile");
    };

    assert_eq!(mp.movement.curve, mp2.movement.curve);
    assert_eq!(mp.movement.max_time, mp2.movement.max_time);
    assert_eq!(mp.scroll.curve, mp2.scroll.curve);
    assert_eq!(mp.scroll.max_ticks_per_ms, mp2.scroll.max_ticks_per_ms);

    let GlobalType::MouseProfile(mp3) = c2.global("mouse_profile3").unwrap().spec else {
        panic!("expected mouse profile");
    };

    assert_eq!(mp3.movement.max_time, 2100.0);
    assert_eq!(mp3.movement.curve, (0.08, 0.1));
    assert_eq!(mp3.movement.min_ticks_per_ms, 0.2);
    assert_eq!(mp3.movement.max_ticks_per_ms, 5.0);
    assert_eq!(mp3.scroll.max_time, 5000.0);
    assert_eq!(mp3.scroll.max_ticks_per_ms, 1.0);
}

#[test]
fn unicode() {
    let src = r#"
[global]

unicode_prefix = C-S-u
unicode_suffix = macro(return delay(5))

[matrix:2x2]
0x00 = a b
0x10 = c d

[emoji]
a = unicode(1f195)
b = macro(hello space 🥵🌏)

[main]

c = layer(nav)
d = layer(emoji)

[nav]

a = left

"#;

    let config = pretty_compile(src).unwrap();

    let mut iter = config.macros.iter();

    let mac = iter.next().expect("should find unicode_prefix");
    let exp = Macro::Modifier {
        keycode: kc("u"),
        modifiers: 3,
    };
    assert_eq!(mac, &exp);

    let mac = iter.next().expect("should find a's unicode");
    let exp = Macro::Delay(5);
    assert_eq!(mac, &exp);

    let mac = iter.next().expect("should find a's unicode");
    let exp = Macro::Tap(vec![40, MACROS_MIN + 1]);
    assert_eq!(mac, &exp);

    let mac = iter.next().expect("should find a's unicode");
    let exp = Macro::Tap(vec![
        MACROS_MIN,
        kc("1"),
        kc("f"),
        kc("1"),
        kc("9"),
        kc("5"),
        MACROS_MIN + 2,
    ]);
    assert_eq!(mac, &exp);

    let mac = iter.next().expect("should find a's unicode");
    let exp = Macro::Tap(vec![
        MACROS_MIN,
        kc("1"),
        kc("f"),
        kc("9"),
        kc("7"),
        kc("5"),
        MACROS_MIN + 2,
    ]);
    assert_eq!(mac, &exp);

    let mac = iter.next().expect("should find a's unicode");
    let exp = Macro::Tap(vec![
        MACROS_MIN,
        kc("1"),
        kc("f"),
        kc("3"),
        kc("0"),
        kc("f"),
        MACROS_MIN + 2,
    ]);
    assert_eq!(mac, &exp);

    let mac = iter.next().expect("should find a's unicode");
    let exp = Macro::Tap(vec![
        kc("h"),
        kc("e"),
        kc("l"),
        kc("l"),
        kc("o"),
        kc("space"),
        MACROS_MIN + 4,
        MACROS_MIN + 5,
    ]);
    assert_eq!(mac, &exp);
}

#[test]
fn global_dual_action_timeout() {
    let config = test_compile("").unwrap();

    assert!(config.global("dual_action_timeout").is_none());

    compile_global!(src, config, "dual_action_timeout", 500, {
        let config = config.unwrap();
        assert!(matches!(
            config.global("dual_action_timeout").unwrap().spec,
            GlobalType::Timeout {
                value: 500,
                min: 0,
                max: 5000,
                dp: 0
            }
        ));

        let bin = config.serialize();

        assert_eq!(
            bin,
            [1, 0, 7, 0, 2, 0, 500, 8, 9, 10, 11, 12, 13, 14, 15, 1, 2, 4, 8, 64, 0, 0]
        );
    });
}

#[test]
fn global_dual_action_timeout2() {
    let config = test_compile("").unwrap();

    assert!(config.global("dual_action_timeout2").is_none());

    compile_global!(src, config, "dual_action_timeout2", 50, {
        let config = config.unwrap();
        assert!(matches!(
            config.global("dual_action_timeout2").unwrap().spec,
            GlobalType::Timeout {
                value: 50,
                min: 0,
                max: 5000,
                dp: 0
            }
        ));

        let bin = config.serialize();

        assert_eq!(
            bin,
            [1, 0, 7, 0, 2, 1, 50, 8, 9, 10, 11, 12, 13, 14, 15, 1, 2, 4, 8, 64, 0, 0]
        );
    });
}

#[test]
fn global_debounce_settle_time() {
    let config = test_compile("").unwrap();

    assert!(config.global("debounce_settle_time").is_none());

    compile_global!(src, config, "debounce_settle_time", 23.5, {
        let config = config.unwrap();
        assert!(matches!(
            config.global("debounce_settle_time").unwrap().spec,
            GlobalType::Timeout {
                value: 235,
                min: 1,
                max: 250,
                dp: 1
            }
        ));

        let bin = config.serialize();

        assert_eq!(
            bin,
            [1, 0, 7, 0, 2, 2, 235, 8, 9, 10, 11, 12, 13, 14, 15, 1, 2, 4, 8, 64, 0, 0]
        );
    });
}

#[test]
fn invalid_global_name() {
    compile_global!(src, config, "overload_tap_tmeout", "500", {
        let err = config.err().unwrap();

        assert_eq!(err.message, "Invalid global 'overload_tap_tmeout'");
        assert_eq!(err.span, Some(9..28));
    });
}

#[test]
fn defaults() {
    let src = r#"
[matrix:2x2]
0x00 = a b
0x10 = c d
"#;

    let config = test_compile(src).unwrap();

    for (i, name) in ["control", "shift", "alt", "gui", "altgr"]
        .into_iter()
        .enumerate()
    {
        let layer = config.layers.get(name).unwrap();

        let v = if name == "altgr" { 0x40 } else { 1 << i };
        assert_eq!(layer.suffix, v);
    }
}

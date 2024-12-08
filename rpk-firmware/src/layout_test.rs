use crate::mapper::macros::SequenceMode;

use super::*;

#[macro_export]
macro_rules! assert_kpm {
    ($a:expr,$e:expr) => {
        assert_kpm!($a,$e,0);
    };
    ($a:expr,$e:expr,$m:expr) => {
        assert_kpm!($a,c rpk_config::keycodes::key_code($e).unwrap(), $m);
    };
    ($a:expr,c $e:expr) => {
        assert_kpm!($a, c $e, 0);
    };
    ($a:expr,c $e:expr,$m:expr) => {
        assert_eq!($a.unwrap(), KeyPlusMod::new($e, $m));
    };
}

#[test]
fn mouse_config() {
    let codes = rpk_config::text_to_binary(
        r#"
[matrix:1x3]

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
"#,
    )
    .unwrap();

    let mut mgr = Manager::<1, 3, 100>::default();

    mgr.load(codes).unwrap();

    let p1 = mgr.get_mouse_profile(0).unwrap();

    assert_eq!(p1.movement.min_ticks_per_ms, 0.02);
}

#[test]
fn clear_modifier_layers() {
    let codes = rpk_config::text_to_binary(
        r#"
[matrix:1x3]

0x00 = a b c

[main]

c = layer(l1)

[l1]

c = layer(l2)

[l2]

a = 1
"#,
    )
    .unwrap();

    let mut mgr = Manager::<1, 3, 100>::default();

    mgr.load(codes).unwrap();

    mgr.push_layer(2);
    mgr.push_layer(6);
    mgr.push_layer(7);
    mgr.push_layer(4);
    mgr.push_layer(1);
    mgr.push_layer(1);
    mgr.push_layer(7);

    assert_eq!(
        &mgr.mapping[mgr.layout_bottom..mgr.layout_top],
        &[5, 2, 6, 7, 4, 1, 1, 7]
    );

    mgr.clear_modifier_layers();

    assert_eq!(
        &mgr.mapping[mgr.layout_bottom..mgr.layout_top],
        &[5, 6, 7, 7]
    );
}

#[test]
fn macro_stack() {
    let mut mgr = Manager::<1, 3, 100>::default();
    mgr.clear_all();

    assert!(matches!(
        mgr.push_macro(Macro::Sequence {
            mode: SequenceMode::Tap,
            location: 2,
            rem: 5
        }),
        Macro::Sequence { .. }
    ));

    assert!(!mgr.push_memo(&[1, 2, 3]));

    assert!(matches!(
        mgr.push_macro(Macro::Sequence {
            mode: SequenceMode::Hold,
            location: 3,
            rem: 3
        }),
        Macro::Sequence { .. }
    ));

    let mac = mgr.pop_macro();

    assert!(matches!(
        mac,
        Macro::Sequence {
            mode: SequenceMode::Tap,
            location: 2,
            rem: 5
        }
    ));

    let mac = mgr.pop_macro();

    assert!(matches!(mac, Macro::Noop));
}

#[test]
fn memo_fifo() {
    let mut mgr = Manager::<1, 3, 100>::default();
    mgr.clear_all();

    assert_eq!(mgr.memo_bottom, 100);

    mgr.push_memo(&[1, 2, 3]);
    mgr.push_memo(&[4, 5, 6]);

    let mut memo = [0, 0, 0];

    assert!(mgr.pop_memo(|m| memo.copy_from_slice(m)));
    assert_eq!(&memo, &[1, 2, 3], "should be fifo");

    assert_eq!(mgr.memo_top, 96);
    assert!(mgr.push_memo(&[7, 8, 9]));
    assert_eq!(mgr.memo_top, 100);

    assert!(mgr.pop_memo(|m| memo.copy_from_slice(m)));
    assert_eq!(&memo, &[4, 5, 6], "should be fifo");

    assert!(mgr.pop_memo(|m| memo.copy_from_slice(m)));
    assert!(!mgr.pop_memo(|_| unreachable!()));
    assert_eq!(&memo, &[7, 8, 9], "should be fifo");

    assert_eq!(mgr.macro_stack, 92);
    assert_eq!(mgr.memo_bottom, 92);
    assert_eq!(mgr.memo_top, 92);

    mgr.push_memo(&[1]);

    assert_eq!(mgr.macro_stack, 98);
    assert_eq!(mgr.memo_bottom, 98);
    assert_eq!(mgr.memo_top, 100);
}

#[test]
fn mixed_memos_and_macros() {
    let mut mgr = Manager::<1, 3, 100>::default();
    mgr.clear_all();

    mgr.push_memo(&[1, 2, 3]);
    mgr.push_memo(&[4, 5, 6]);
    mgr.pop_memo(|_| {});

    assert_eq!(mgr.memo_top, 96);

    assert!(matches!(
        mgr.push_macro(Macro::Sequence {
            mode: SequenceMode::Tap,
            location: 2,
            rem: 5
        }),
        Macro::Sequence { .. }
    ));

    assert!(matches!(
        mgr.push_macro(Macro::Sequence {
            mode: SequenceMode::Release,
            location: 3,
            rem: 1
        }),
        Macro::Sequence { .. }
    ));

    assert_eq!(mgr.memo_top, 100);

    let mut memo = [0, 0, 0];

    assert!(mgr.pop_memo(|m| memo.copy_from_slice(m)));

    assert_eq!(&memo, &[4, 5, 6], "should be fifo");

    let mac = mgr.pop_macro();
    assert!(matches!(
        mac,
        Macro::Sequence {
            mode: SequenceMode::Tap,
            location: 2,
            rem: 5
        }
    ));
}

#[test]
fn load_macros() {
    let codes = rpk_config::text_to_binary(
        r#"
[matrix:1x3]

0x00 = a b c

[main]

c = S-z
a = C-G-k
"#,
    )
    .unwrap();

    let mut mgr = Manager::<1, 3, 100>::default();

    mgr.load(codes).unwrap();

    assert_kpm!(mgr.find_code(0, 2), c MACROS_MIN);
    assert_kpm!(mgr.find_code(0, 0), c MACROS_MIN+1);
}

#[test]
fn find_code_sparse() {
    let codes = rpk_config::text_to_binary(
        r#"
[matrix:5x10]

0x12 = a
0x16 = 2
0x45 = d
"#,
    )
    .unwrap();

    let mut mgr = Manager::<5, 10, 100>::default();

    mgr.load(codes).unwrap();

    assert_kpm!(mgr.find_code(1, 6), c 31);
    assert_eq!(mgr.find_code(2, 6), None);
    assert_kpm!(mgr.find_code(4, 5), c 7);
    assert_kpm!(mgr.find_code(1, 2), c 4);
}

#[test]
fn find_code() {
    let codes = rpk_config::text_to_binary(
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = d e f
"#,
    )
    .unwrap();

    let mut mgr = Manager::<2, 3, 100>::default();

    mgr.load(codes).unwrap();

    assert_kpm!(mgr.find_code(1, 2), "f");
    assert_kpm!(mgr.find_code(0, 1), c 5);
}

#[test]
fn multi_layers() {
    let codes = rpk_config::text_to_binary(
        r#"
[matrix:2x4]

0x00 = a b c d
0x10 = e f g h

[layer_1]

a = 1
f = 2
g = 3

[layer_2]

f = 4
"#,
    )
    .unwrap();

    let mut mgr = Manager::<2, 4, 100>::default();

    mgr.load(codes).unwrap();

    assert_kpm!(mgr.find_code(1, 2), "g");
    assert_kpm!(mgr.find_code(1, 1), "f");

    mgr.push_layer(6);

    assert_kpm!(mgr.find_code(1, 3), "h");
    assert_kpm!(mgr.find_code(1, 1), "2");

    mgr.push_layer(7);

    assert_kpm!(mgr.find_code(1, 3), "h");
    assert_kpm!(mgr.find_code(1, 1), "4");
    assert_kpm!(mgr.find_code(0, 0), "1");

    assert!(mgr.pop_layer(6));

    assert_kpm!(mgr.find_code(1, 3), "h");
    assert_kpm!(mgr.find_code(1, 1), "4");
    assert_kpm!(mgr.find_code(0, 0), "a");
}

#[test]
fn search_code_bug() {
    let codes = [
        258, 4096, 259, 4097, 260, 4098, 514, 4099, 515, 4100, 516, 4101, 770, 4102, 771, 4103,
        772, 4104,
    ];

    assert_eq!(search_code(&codes, 1, 1), 0);
    assert_eq!(search_code(&codes, 2, 6), 0);
    assert_eq!(search_code(&codes, 6, 1), 0);
    assert_eq!(search_code(&codes, 2, 2), 4099);
    assert_eq!(search_code(&codes, 3, 4), 4104);

    assert_eq!(search_code(&codes, 1, 2), 4096);
    assert_eq!(search_code(&codes, 2, 3), 4100);
    assert_eq!(search_code(&codes, 3, 3), 4103);
    assert_eq!(search_code(&codes, 3, 3), 4103);
    assert_eq!(search_code(&codes, 3, 2), 4102);
}

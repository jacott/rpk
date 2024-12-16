use core::{str, sync::atomic};

use embassy_futures::{block_on, join::join};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use key_range::{LAYER_MIN, MACROS_MIN};

use crate::kc;

use super::*;

extern crate std;

const KEY_UP: usize = 0;
const KEY_DOWN: usize = 1;

macro_rules! setup {
    ($press:ident, $assert_read:ident, $a:expr, $x:block) => {
        setup!(RC 2, 3, _t, $press, $assert_read, $a, $x);
    };
    ($t:ident, $press:ident, $assert_read:ident, $a:expr, $x:block) => {
        setup!(RC 2, 3, $t, $press, $assert_read, $a, $x);
    };
    (RC $r:expr, $c:expr, $t:ident, $press:ident, $assert_read:ident, $a:expr, $x:block) => {
        {
            let mapper_channel = MapperChannel::default();
            let debounce_ms_atomic = atomic::AtomicU8::new(8);
            let mut $t = Mapper::<$r, $c, 200, NoopRawMutex, 10>::new(&mapper_channel, &debounce_ms_atomic);

            let layout = rpk_config::text_to_binary($a).unwrap();
            $t.load_layout(layout).unwrap();

            macro_rules! $assert_read {
                (NONE) => {{
                    match mapper_channel.0.try_receive() {
                        Ok(ans) => {
                            assert!(false, "Unexpected key event {:?}", ans);
                        }
                        Err(_) => {}
                    }
                }};
                ($e1:expr, $e2:expr) => {{
                    match mapper_channel.0.try_receive() {
                        Ok(ans) => {
                            assert_eq!(ans, KeyEvent::basic(kc!($e2) as u8, $e1 == 1));
                        }
                        Err(err) => {
                            assert!(false, "No key press! {:?}", err);
                        }
                    }
                }};
                (E $e:expr) => {{
                    if let Ok(ans) = mapper_channel.0.try_receive() {
                        assert_eq!(ans, $e);
                    } else {
                        assert!(false, "Expected key event");
                    }
                }};
            }
            macro_rules! $press {
                ($row_idx:expr, $column_idx:expr, TAP) => {{
                    $press!($row_idx, $column_idx, true);
                    $press!($row_idx, $column_idx, false);
                }};
                ($row_idx:expr, $column_idx:expr, $is_down:expr) => {{
                    let key = ScanKey::new($row_idx, $column_idx, $is_down);
                    $t.key_switch(TimedScanKey(key, $t.now));
                }};
            }

            $x;
        }
    };
}

#[test]
fn large_keyboard() {
    setup!(
        RC 6,14,t,
        press,
        assert_read,
        r#"
[matrix:6x14]

0x00 = u0  u1 2  3  4  5   f1     f5   6   7  8  u2 u3 u4
0x10 = esc 1  w  e  r  t   f2     f6   y   u  i  o  0  f9
0x20 = `   q  s  d  f  g   f3     f7   h   j  k  l  p  f10
0x30 = -   a  x  c  v  b   f4     f8   n   m  ,  .  \; f11
0x40 = =   z  la lc ls ent tab    bksp spc rs rc ra /  f12
0x50 = \[   ]  \\ \' lg pgup pgdn  del  mnu rg left down up right

[main]

apostrophe              = layer(layer5)

[layer5]

w = G-7
e = G-8
r = G-9

s = G-4
d = G-5
f = G-6

x = G-1
c = G-2
v = G-3

"#,
        {
            press!(5, 3, true);
            press!(3, 4, true);
            assert_read!(E KeyEvent::PendingModifiers(8, true));
            assert_read!(KEY_DOWN, "3");
        }
    );
}

#[test]
fn reset() {
    setup!(
        press,
        _assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

c = reset_keyboard
"#,
        {
            std::thread_local! {
                static CALL_COUNT: RefCell<usize> = const {RefCell::new(0)};
            }
            fn myreset() {
                CALL_COUNT.with_borrow_mut(|c| *c += 1);
            }
            firmware_functions::handle_reset(Some(&myreset));
            press!(0, 2, true);

            assert_eq!(CALL_COUNT.with_borrow(|c| *c), 0);

            press!(0, 2, false);

            assert_eq!(CALL_COUNT.with_borrow(|c| *c), 1);
        }
    );
}

#[test]
fn clear_all() {
    setup!(
        t,
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

a = overload(l2, hold(abcd leftshift mouse1 mouse2 hold(123 clear_all) 789))
b = S-mouse1
g = C-g

[l2]

a = q
"#,
        {
            press!(1, 2, true);
            assert_read!(E KeyEvent::PendingModifiers(1, true));
            press!(0, 0, TAP);

            assert_read!(KEY_DOWN, "g");
            assert_read!(KEY_DOWN, "a");
            assert_read!(KEY_DOWN, "b");
            assert_read!(KEY_DOWN, "c");
            assert_read!(KEY_DOWN, "d");
            assert_read!(KEY_DOWN, "leftshift");

            assert_read!(E KeyEvent::mouse_button(1));

            assert_eq!(t.layout.macro_stack(), 194);
            assert_eq!(t.active_actions[1][2], KeyPlusMod(4100, 0));
            t.check_time();
            assert_eq!(t.active_actions[1][2], KeyPlusMod(0, 0));

            assert_read!(E KeyEvent::Clear);

            assert_eq!(t.layout.macro_stack(), 200);
            assert_eq!(t.mouse.next_event_time(), u64::MAX);
            assert!(t.dual_action.is_no_timer());
            t.check_time();

            assert_read!(NONE);

            press!(0, 1, true);

            t.check_time();

            assert_read!(E KeyEvent::PendingModifiers(2, true));
            assert_read!(E KeyEvent::mouse_button(1));
        }
    );
}

#[test]
fn clear_layers() {
    setup!(
        t,
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

a = hold(leftshift leftalt layer(l2) clear_layers)

[l2]

a = q
"#,
        {
            press!(0, 0, true);
            assert_read!(KEY_DOWN, "leftshift");
            assert_read!(KEY_DOWN, "leftalt");

            assert_read!(E KeyEvent::Modifiers(0, false));
            assert!(!t.pop_layer(1));
            assert!(!t.pop_layer(2));
            assert!(!t.pop_layer(6));

            t.check_time();

            press!(0, 0, false);
            assert_read!(NONE);
        }
    );
}

#[test]
fn stop_active() {
    setup!(
        t,
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

a = hold(leftshift leftalt layer(l2) stop_active 12)

[l2]

a = q
"#,
        {
            press!(0, 0, true);
            assert_read!(KEY_DOWN, "leftshift");
            assert_read!(KEY_DOWN, "leftalt");

            assert_read!(E KeyEvent::Clear);
            assert!(!t.pop_layer(1));
            assert!(!t.pop_layer(2));
            assert!(t.pop_layer(6)); // we are still on

            assert_read!(KEY_DOWN, "1");
            assert_read!(KEY_DOWN, "2");

            t.check_time();

            press!(0, 0, false);
            assert_read!(NONE);
        }
    );
}

#[test]
fn reset_to_usb_boot() {
    setup!(
        press,
        _assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

c = reset_to_usb_boot
"#,
        {
            std::thread_local! {
                static CALL_COUNT: RefCell<usize> = const {RefCell::new(0)};
            }
            fn myreset() {
                CALL_COUNT.with_borrow_mut(|c| *c += 1);
            }
            firmware_functions::handle_reset_to_usb_boot(Some(&myreset));
            press!(0, 2, true);

            assert_eq!(CALL_COUNT.with_borrow(|c| *c), 0);

            press!(0, 2, false);

            assert_eq!(CALL_COUNT.with_borrow(|c| *c), 1);
        }
    );
}

#[test]
fn mouse_key_event() {
    assert_eq!(
        KeyEvent::mouse_move(0, 10, 123),
        KeyEvent::MouseMove(0, 10, 123)
    );
    assert_eq!(
        KeyEvent::mouse_move(1, -10, 0),
        KeyEvent::MouseMove(1, 246, 0)
    );
}

#[test]
fn mouse_accel() {
    setup!(
        t,
        press,
        assert_event,
        r#"
[matrix:2x3]

0x00 = 7 8 9
0x10 = 4 5 6

[main]

4 = mouseaccel1 mouseaccel2 mouseaccel3
"#,
        {
            let cfg = t.mouse.get_config();
            assert_eq!(cfg.movement.max_ticks_per_ms, 3.0);

            press!(1, 2, true);
            let cfg = t.mouse.get_config();
            assert_eq!(cfg.movement.max_ticks_per_ms, 5.0);

            press!(1, 0, true);
            let cfg = t.mouse.get_config();
            assert_eq!(cfg.movement.max_ticks_per_ms, 1.0);

            assert_event!(NONE);
        }
    );
}

#[test]
fn mouse_profile() {
    let now = 12341;
    setup!(
        t,
        press,
        assert_event,
        r#"
[global.mouse_profile2.movement]

curve = [1, 0]
max_time = 1000
min_ticks_per_ms = 1
max_ticks_per_ms = 6

[matrix:2x3]

0x00 = 7 8 9
0x10 = 4 5 6

[main]

7 = mousewheeldown
8 = mouseup
9 = mousewheelup
4 = mouseleft
5 = mousedown
6 = mouse5

"#,
        {
            t.now = now;
            press!(1, 2, true);
            assert_event!(E KeyEvent::MouseButton(16));

            assert_eq!(t.wait_time, u64::MAX);
            press!(1, 0, true);

            assert_eq!(t.wait_time, now + 15);

            t.now = now + 10;
            t.check_time();
            assert_event!(NONE);

            t.now = now + 200;
            t.check_time();
            assert_event!(E KeyEvent::MouseMove(0, 129, 16));

            press!(1, 0, false);

            assert_eq!(t.wait_time, u64::MAX);

            assert_event!(NONE);
        }
    );
}

#[test]
fn mouse_move() {
    let now = 12341;
    setup!(
        t,
        press,
        assert_event,
        r#"
[matrix:2x3]

0x00 = 7 8 9
0x10 = 4 5 6

[main]

7 = mousewheeldown
8 = mouseup
9 = mousewheelup
4 = mouseleft
5 = mousedown
6 = mouse5

"#,
        {
            t.now = now;
            press!(1, 2, true);
            assert_event!(E KeyEvent::MouseButton(16));

            let mm = KeyEvent::MouseMove(0, 196, 16);
            assert_eq!(t.wait_time, u64::MAX);
            press!(1, 0, true);

            assert_eq!(t.wait_time, now + 15);

            t.now = now + 10;
            t.check_time();
            assert_event!(NONE);

            t.now = now + 200;
            t.check_time();
            assert_event!(E mm);

            press!(1, 0, false);

            assert_eq!(t.wait_time, u64::MAX);

            assert_event!(NONE);
        }
    );
}

#[test]
fn toggle_layer() {
    setup!(
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

a = toggle(gui)


"#,
        {
            press!(0, 0, true);

            assert_read!(KEY_DOWN, "leftgui");

            press!(0, 0, false);

            assert_read!(NONE);

            press!(0, 0, true);

            assert_read!(KEY_UP, "leftgui");
            press!(0, 0, false);

            assert_read!(NONE);
        }
    );
}

#[test]
fn setlayout() {
    setup!(
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

a = setlayout(mylayout)

[mylayout]

0x00 = 1 2 3
0x10 = 4 5 6

"#,
        {
            press!(0, 0, TAP);

            assert_read!(NONE);

            press!(0, 0, true);

            assert_read!(KEY_DOWN, "1");
            press!(0, 0, false);
            assert_read!(KEY_UP, "1");

            press!(1, 2, TAP);

            assert_read!(KEY_DOWN, "6");
            assert_read!(KEY_UP, "6");
            assert_read!(NONE);
        }
    );
}

#[test]
fn onshot() {
    setup!(
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

a = oneshot(shift)
"#,
        {
            press!(0, 0, true);
            assert_read!(KEY_DOWN, "leftshift");

            press!(0, 0, false);
            assert_read!(NONE);

            press!(0, 1, TAP);

            assert_read!(KEY_DOWN, "b");
            assert_read!(KEY_UP, "b");
            assert_read!(KEY_UP, "leftshift");

            assert_read!(NONE);
        }
    );
}

#[test]
fn activate_layer() {
    setup!(
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

a = layer(layer_1)

[layer_1]

f = 1
"#,
        {
            press!(0, 0, true);
            press!(1, 1, true);

            assert_read!(KEY_DOWN, "1");

            press!(1, 1, false);

            assert_read!(KEY_UP, "1");

            press!(0, 0, false);

            press!(1, 1, true);
            assert_read!(KEY_DOWN, "f");
        }
    );
}

#[test]
fn globals() {
    setup!(
        t,
        _press,
        _assert_read,
        r#"
[global]

dual_action_timeout = 500
dual_action_timeout2 = 50
debounce_settle_time = 12.3

[matrix:2x3]

0x00 = a b c
0x10 = e f g
"#,
        {
            assert_eq!(t.layout.global(globals::DUAL_ACTION_TIMEOUT as usize), 500);
            assert_eq!(t.layout.global(globals::DUAL_ACTION_TIMEOUT2 as usize), 50);
            assert_eq!(t.layout.global(globals::DEBOUNCE_SETTLE_TIME as usize), 123);

            let debounce = t.debounce_ms_atomic.load(atomic::Ordering::Relaxed);

            assert_eq!(debounce, 123);
        }
    );
}

#[test]
fn memo_timed_scan_key() {
    let mut t = TimedScanKey(ScanKey::new(1, 2, true), 1274);
    let memo = t.as_memo();
    assert_eq!(t.1, TimedScanKey::from_memo(&memo).1);

    t.1 = u64::MAX;
    let memo = t.as_memo();
    assert_eq!(t.1, TimedScanKey::from_memo(&memo).1);

    t.1 = Instant::now().as_ticks();
    let memo = t.as_memo();
    assert_eq!(t.1, TimedScanKey::from_memo(&memo).1);
    assert_eq!(t.0.row(), 1);
    assert_eq!(t.0.column(), 2);
    assert!(t.0.is_down());
}

#[test]
fn multi_overload_in_macro() {
    setup!(
        t,
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]
e = macro(hold(overload(shift, e) overload(control, e)) release(layer(control) layer(shift)))
"#,
        {
            let mut now = 100;

            macro_rules! advance {
                ($t:expr) => {
                    now += $t;
                    t.now = now;
                    t.check_time();
                };
            }

            advance!(0);

            press!(1, 0, true);

            assert_read!(KEY_DOWN, "leftshift");
            assert_read!(NONE);

            advance!(180);

            assert_read!(KEY_DOWN, "leftcontrol");

            press!(1, 0, false);

            assert_read!(KEY_UP, "leftcontrol");
            assert_read!(KEY_UP, "leftshift");
            assert_read!(NONE);
        }
    );
}

#[test]
fn overload_bug() {
    setup!(
        t,
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

e = overload(l1, e)
g = overload(l2, g)

[l1]

f = 1

[l2]

f = 2
"#,
        // 1  key_switch     1,0,d, no-hold
        // 2  DualAction     id 0, tap 8, hold 1542, down
        // 3  key_switch     1,2,d, hold id 0
        // 4  hold_pending   no-prev, next  1,2,d

        // 5  layer          1542, true, 6)       -- timer runs
        // 6  key_switch     1,2,d, no-hold
        // 7  DualAction     id 1, tap 10, hold 1543, down

        // 8  key_switch     1,0,u hold id 1      -- before next timer
        // 9  hold_pending   no-prev, next 1,0,u  -- ** here we mess up **
        // 10 DualAction     id 0, tap 8, hold 1542, up current id 1
        // 11 basic          8, d
        // 12 basic          8, u

        // 13 key_switch     1,2,u, no-hold
        // 14 DualAction     id 1, tap 10, hold 1543, up
        // 15 layer          1543, false, 7)

        // 16 key_switch     1,1,d, no-hold
        // 17 basic          30, true)            -- giving wrong result
        // 18 basic          10, true)
        {
            static TIMEOUT: u64 = 180;
            let mut now = 1234;
            macro_rules! advance {
                ($t:expr) => {
                    now += $t;
                    t.now = now;
                    t.check_time();
                };
            }
            advance!(0);
            {
                press!(1, 0, true); // 1,2
                assert_eq!(t.dual_action.wait_until() - TIMEOUT, now);
                advance!(40);
                press!(1, 2, true); // 3,4
                let gkey = now;
                assert_eq!(t.layout.find_code(1, 1).unwrap().0, kc!("f"));

                advance!(200); // 5 -- 7
                assert_eq!(t.layout.find_code(1, 1).unwrap().0, kc!("1"));

                assert_read!(NONE);
                assert!(t.run_memo());
                assert_eq!(t.layout.find_code(1, 1).unwrap().0, kc!("1"));
                assert_read!(NONE);
                assert_eq!(t.dual_action.wait_until() - TIMEOUT, gkey);

                press!(1, 0, false); // 8 -- 12
                assert_read!(NONE);

                advance!(130);
                assert_eq!(t.layout.find_code(1, 1).unwrap().0, kc!("2"));

                advance!(130);
                assert_read!(NONE);

                press!(1, 2, false); // 13 -- 15
                assert!(t.run_memo());
                assert_read!(NONE);
                assert_eq!(t.layout.find_code(1, 1).unwrap().0, kc!("f"));

                press!(1, 1, true); // 16 -- 18
                assert_read!(KEY_DOWN, "f");
                assert_read!(NONE);
            }
        }
    );
}

#[test]
fn timed_dualaction() {
    setup!(
        t,
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

c = dualaction(leftshift, c, 252, 60)
g = overload(alt, g, 100)


"#,
        {
            let mut now = 100;

            macro_rules! advance {
                ($t:expr) => {
                    now += $t;
                    t.now = now;
                    t.check_time();
                };
            }

            advance!(0);

            // hold timeout
            {
                press!(1, 2, true);
                assert_read!(NONE);

                advance!(100);

                assert_read!(KEY_DOWN, "leftalt");
                assert_read!(NONE);

                press!(1, 2, false);

                assert_read!(KEY_UP, "leftalt");
                assert_read!(NONE);

                press!(0, 2, true);
                advance!(251);
                assert_read!(NONE);

                advance!(1);
                assert_read!(KEY_DOWN, "leftshift");
                assert_read!(NONE);

                press!(0, 2, false);

                assert_read!(KEY_UP, "leftshift");
                assert_read!(NONE);
            }

            // tap timeout
            {
                press!(1, 2, true);
                press!(1, 0, true);
                press!(1, 0, false);
                advance!(19);
                assert_read!(NONE);

                advance!(1);

                assert_read!(KEY_DOWN, "leftalt");
                assert_read!(NONE);

                press!(1, 2, false);

                assert_read!(KEY_UP, "leftalt");
                assert_read!(NONE);

                press!(0, 2, true);
                press!(1, 0, true);
                press!(1, 0, false);
                advance!(59);
                assert_read!(NONE);

                advance!(1);
                assert_read!(KEY_DOWN, "leftshift");
                assert_read!(NONE);

                press!(0, 2, false);

                assert_read!(KEY_UP, "leftshift");
                assert_read!(NONE);
            }
        }
    );
}

#[test]
fn overload() {
    setup!(
        t,
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

c = overload(shift, C-c)
g = C-S-m


"#,
        {
            let mut now = 100;

            macro_rules! advance {
                ($t:expr) => {
                    now += $t;
                    t.now = now;
                    t.check_time();
                };
            }

            advance!(0);

            // hold-dual + tap + hold, release-dual, release
            {
                press!(0, 2, true);
                press!(1, 0, true);
                press!(1, 0, false);
                press!(1, 1, true);

                assert_read!(KEY_DOWN, "leftshift");
                assert!(t.run_memo());
                assert_read!(KEY_DOWN, "e");
                assert!(t.run_memo());
                assert_read!(KEY_UP, "e");
                assert!(t.run_memo());
                assert_read!(KEY_DOWN, "f");

                assert!(!t.run_memo());
                assert_read!(NONE);

                press!(0, 2, false);
                press!(1, 1, false);

                assert_read!(KEY_UP, "leftshift");
                assert_read!(KEY_UP, "f");

                assert!(!t.run_memo());
                assert_read!(NONE);
            }

            // one-down + tap, one-up
            {
                press!(1, 0, true);
                press!(0, 2, true);
                press!(1, 0, false);

                assert_read!(KEY_DOWN, "e");
                advance!(10);
                advance!(10);

                assert_read!(NONE);

                advance!(180);

                assert_read!(KEY_DOWN, "leftshift");
                assert!(t.run_memo());
                assert_read!(KEY_UP, "e");
                assert_read!(NONE);

                press!(0, 2, false);

                assert_read!(KEY_UP, "leftshift");
                assert_read!(NONE);
            }

            // hold + one, timeout
            {
                press!(0, 2, true);
                press!(1, 0, true);
                assert_read!(NONE);

                advance!(180);

                assert_read!(KEY_DOWN, "leftshift");
                assert_read!(NONE);
                assert!(t.run_memo());
                assert_read!(KEY_DOWN, "e");
                assert_read!(NONE);

                press!(0, 2, false);
                press!(1, 0, false);

                assert_read!(KEY_UP, "leftshift");
                assert_read!(KEY_UP, "e");
                assert_read!(NONE);
            }

            // hold
            {
                assert!(matches!(t.dual_action, DualActionTimer::NoDual));
                assert_eq!(t.dual_action.wait_until(), u64::MAX);
                press!(0, 2, true);
                assert_read!(NONE);

                assert_eq!(t.wait_time, t.dual_action.wait_until());
                assert_eq!(t.wait_time, now + 180);
                let DualActionTimer::Wait {
                    tap,
                    hold,
                    count: 2,
                    ..
                } = t.dual_action
                else {
                    panic!("expected to be in wait 2");
                };
                assert_eq!(tap, MACROS_MIN);
                assert_eq!(hold, LAYER_MIN + 1,);

                assert_read!(NONE);

                advance!(180);

                assert_eq!(t.wait_time, u64::MAX);
                assert!(t.dual_action.is_no_timer());
                assert_read!(KEY_DOWN, "leftshift");

                press!(0, 2, false);

                assert_read!(KEY_UP, "leftshift");

                assert_eq!(t.wait_time, u64::MAX);
                assert!(t.dual_action.is_no_timer());
            }

            // tap
            {
                press!(0, 2, true);
                advance!(170);
                press!(0, 2, false);
                advance!(0);

                assert_read!(E KeyEvent::PendingModifiers(1, true));
                assert_read!(KEY_DOWN, "c");
                assert!(t.run_memo());
                assert_read!(KEY_UP, "c");
                assert_read!(KEY_UP, "leftcontrol");

                advance!(180);

                assert_read!(NONE);
            }

            // hold + one, release-hold
            {
                press!(0, 2, true);
                press!(1, 0, true);
                press!(0, 2, false);

                assert_read!(E KeyEvent::PendingModifiers(1, true));
                assert_read!(KEY_DOWN, "c");
                assert_read!(NONE);
                assert!(t.run_memo());
                assert_read!(KEY_DOWN, "e");

                assert_read!(NONE);
                assert!(t.run_memo());
                assert_read!(KEY_UP, "c");
                assert_read!(KEY_UP, "leftcontrol");

                press!(1, 0, false);
                assert_read!(KEY_UP, "e");
            }

            // hold + tap-one
            {
                press!(0, 2, true);
                assert_eq!(t.memo_count, 0);
                press!(1, 0, true);
                assert_eq!(t.memo_count, 1);
                press!(1, 0, false);
                assert_eq!(t.memo_count, 2);

                advance!(0);
                assert_read!(NONE);

                advance!(20);

                assert_read!(KEY_DOWN, "leftshift");
                assert_read!(NONE);
                assert!(t.run_memo());
                assert_read!(KEY_DOWN, "e");
                assert_read!(NONE);
                assert!(t.dual_action.is_no_timer());
                assert!(t.run_memo());

                assert_read!(KEY_UP, "e");
                assert_read!(NONE);

                // todo: use a timeout to allow for very quick release after tap to be like a hold +
                // one, release-hold
                press!(0, 2, false);
                assert_read!(KEY_UP, "leftshift");
            }

            // hold + one + two
            {
                press!(0, 2, true);
                press!(1, 0, true);
                press!(1, 1, true);
                advance!(0);
                assert_read!(NONE);
                advance!(20);

                assert_read!(KEY_DOWN, "leftshift");
                assert_read!(NONE);
                assert!(t.run_memo());
                assert_read!(KEY_DOWN, "e");
                assert_read!(NONE);
                assert!(t.run_memo());
                assert_read!(KEY_DOWN, "f");
                assert_read!(NONE);

                press!(0, 2, false);
                assert_read!(KEY_UP, "leftshift");
            }
        }
    );
}

#[test]
fn modifier_macros() {
    setup!(
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

c = C-S-G-j
g = C-S-m


"#,
        {
            press!(1, 2, true);
            assert_read!(E KeyEvent::PendingModifiers(3, true));
            assert_read!(KEY_DOWN, "m");

            press!(1, 2, false);
            assert_read!(KEY_UP, "m");
            assert_read!(E KeyEvent::Modifiers(3, false));
        }
    );
}

#[test]
fn modifier_layer() {
    setup!(
        t,
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[shift]

c = S-G-j
g = S-m

[ctlshft:C-S]

c = C-G-a

"#,
        {
            t.push_layer(1);
            assert_read!(KEY_DOWN, "leftshift");

            // S-m
            press!(1, 2, true);
            assert_read!(KEY_DOWN, "m");
            assert_read!(NONE);

            press!(1, 2, false);
            assert_read!(KEY_UP, "m");
            assert_read!(NONE);

            // S-G-j
            press!(0, 2, true);
            assert_read!(E KeyEvent::PendingModifiers(8, true));
            assert_read!(KEY_DOWN, "j");
            assert_read!(NONE);

            press!(0, 2, false);
            assert_read!(KEY_UP, "j");
            assert_read!(KEY_UP, "leftgui");

            t.pop_layer(1);
            assert_read!(KEY_UP, "leftshift");

            t.push_layer(6);
            assert_read!(E KeyEvent::Modifiers(3, true));
            assert_read!(NONE);

            // C-G-a
            press!(0, 2, true);
            assert_read!(E KeyEvent::PendingModifiers(2, false));
            assert_read!(E KeyEvent::PendingModifiers(8, true));
            assert_read!(KEY_DOWN, "a");
            assert_read!(NONE);

            press!(0, 2, false);
            assert_read!(KEY_UP, "a");
            assert_read!(E KeyEvent::PendingModifiers(8, false));
            assert_read!(KEY_DOWN, "leftshift");
        }
    );
}

#[test]
fn unicode() {
    setup!(
        t,
        press,
        assert_read,
        r#"
[global]

unicode_prefix = C-S-u
unicode_suffix = macro(return delay(20))

[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

a = layer(nav)
c = macro(🆕)

[nav]

"#,
        {
            press!(0, 2, true);

            assert_read!(E KeyEvent::PendingModifiers(3, true));
            assert_read!(KEY_DOWN, "u");
            assert_read!(KEY_UP, "u");
            assert_read!(E KeyEvent::Modifiers(3, false));

            for c in b"1f195" {
                t.check_time();
                let c = [*c];
                let c = str::from_utf8(&c).unwrap();
                assert_read!(KEY_DOWN, c);
                assert_read!(KEY_UP, c);
            }

            assert_read!(KEY_DOWN, "return");
            assert_read!(KEY_UP, "return");

            assert_read!(E KeyEvent::Delay(20));

            assert_read!(NONE);
        }
    );
}

#[test]
fn tap_macros() {
    setup!(
        t,
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

c = macro(a macro(B macro(cd) e) f)
"#,
        {
            macro_rules! timer {
                () => {{
                    t.report_channel
                        .timer()
                        .at_sig
                        .try_take()
                        .unwrap()
                        .as_millis()
                }};
            }

            t.now = 123_000;
            press!(0, 2, true);

            assert_eq!(timer!(), t.now);

            assert_read!(KEY_DOWN, "a");
            assert_read!(KEY_UP, "a");

            t.now = 123_100;

            assert_read!(E KeyEvent::PendingModifiers(2, true));
            assert_read!(KEY_DOWN, "b");
            assert_read!(KEY_UP, "b");
            assert_read!(KEY_UP, "leftshift");

            assert_read!(KEY_DOWN, "c");
            assert_read!(KEY_UP, "c");

            assert_read!(NONE); // out of room

            t.check_time();

            assert_read!(KEY_DOWN, "d");
            assert_read!(KEY_UP, "d");

            t.check_time();

            assert_read!(KEY_DOWN, "e");
            assert_read!(KEY_UP, "e");

            t.check_time();

            assert_read!(KEY_DOWN, "f");
            assert_read!(KEY_UP, "f");

            assert_eq!(timer!(), u64::MAX / 1000);

            press!(0, 2, false);
            t.check_time();
            assert_read!(NONE);
        }
    );
}

#[test]
fn hold_release_macros() {
    setup!(
        t,
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

c = macro(hold(aab) release(ba))
"#,
        {
            press!(0, 2, true);

            assert_read!(KEY_DOWN, "a");
            assert_read!(KEY_DOWN, "a");
            assert_read!(KEY_DOWN, "b");

            t.check_time();
            assert_read!(NONE);
            assert!(matches!(t.macro_running, Macro::Noop));

            press!(0, 2, false);

            assert_read!(KEY_UP, "b");
            assert_read!(KEY_UP, "a");

            t.check_time();
            assert_read!(NONE);
        }
    );
}

#[test]
fn layer_with_mods() {
    setup!(
        press,
        assert_read,
        r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

a = layer(shift)
b = rightshift
c = rightalt

[shift]

f = 1

[altgr]

e = 2
"#,
        {
            // generic layer change
            press!(0, 0, true);
            assert_read!(KEY_DOWN, "leftshift");

            // not mapped shift unchanged
            press!(1, 2, true);
            assert_read!(KEY_DOWN, "g");
            press!(1, 2, false);
            assert_read!(KEY_UP, "g");

            // f mapped to 1 shift temporary released
            press!(1, 1, true);
            assert_read!(E KeyEvent::PendingModifiers(2, false));
            assert_read!(KEY_DOWN, "1");
            press!(1, 1, false);
            assert_read!(KEY_UP, "1");
            assert_read!(KEY_DOWN, "leftshift");

            press!(0, 0, false);
            assert_read!(KEY_UP, "leftshift");

            press!(1, 1, true);

            assert_read!(KEY_DOWN, "f");

            // modifier rightshift key press change
            press!(0, 1, true);
            assert_read!(KEY_DOWN, "rightshift");

            press!(1, 1, true);

            assert_read!(KEY_DOWN, "1");

            press!(0, 1, false);
            assert_read!(KEY_UP, "rightshift");

            // modifier altgr key press change
            press!(0, 2, true);
            assert_read!(KEY_DOWN, "rightalt");
            press!(1, 0, true);

            assert_read!(E KeyEvent::PendingModifiers(64, false));
            assert_read!(KEY_DOWN, "2");

            press!(0, 2, false);
            assert_read!(NONE);
            press!(1, 0, false);
            assert_read!(KEY_UP, "2");
            assert_read!(NONE);
        }
    );
}

#[test]
fn run_loop() {
    block_on(async {
        setup!(
            t,
            press,
            assert_read,
            r#"
[matrix:2x3]

0x00 = a b c
0x10 = e f g

[main]

c = macro(hold(aabbccddeeff) release(fedcba))
"#,
            {
                extern crate alloc;
                use alloc::vec;

                let ksc = KeyScannerChannel::<NoopRawMutex, 32>::default();
                press!(0, 2, true);
                ksc.try_send(ScanKey::new(0, 2, false));
                t.report_channel.timer().signal(ControlMessage::Exit);

                let reader = &t.report_channel.0;

                let f1 = async {
                    let mut v = std::vec::Vec::new();
                    loop {
                        let msg = reader.receive().await;
                        if matches!(msg, KeyEvent::Pending) {
                            return v;
                        }

                        v.push(msg);
                    }
                };

                let f2 = async {
                    t.run(&ksc).await;
                    t.report_channel.0.send(KeyEvent::Pending).await;
                };

                let msgs = join(f1, f2).await.0;

                assert_read!(NONE);
                assert_eq!(msgs.len(), 18);

                let msgs: vec::Vec<u8> = msgs
                    .iter()
                    .map(|e| {
                        let KeyEvent::Basic(k, s) = e else {
                            panic!("unexpected event {e:?}");
                        };
                        if *s {
                            *k
                        } else {
                            *k | 0x80
                        }
                    })
                    .collect();

                assert_eq!(
                    msgs,
                    vec![4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 137, 136, 135, 134, 133, 132]
                );

                press!(1, 2, true);
                t.report_channel
                    .timer()
                    .signal(ControlMessage::LoadLayout { file_location: 123 });

                assert!(matches!(
                    t.run(&ksc).await,
                    ControlMessage::LoadLayout { file_location: 123 }
                ));

                assert_read!(E KeyEvent::Clear);
            }
        );
    });
}

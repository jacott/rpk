use std::collections::HashMap;

use lazy_static::lazy_static;
use rpk_common::keycodes::key_range;

const DASH_USCORE: [char; 2] = ['_', '-'];

const MODIFIER_BITS: [&str; 8] = ["C", "S", "A", "M", "RC", "RS", "RA", "G"];
lazy_static! {
    static ref MODIFIER_BIT_MAP: HashMap<&'static str, u8> = {
        let mut m = HashMap::new();
        for (i, c) in MODIFIER_BITS.iter().enumerate() {
            m.insert(*c, 1 << i);
        }
        m
    };
    static ref ACTION_NAMES: HashMap<&'static str, u16> = {
        let mut m = HashMap::new();
        m.insert("layer", key_range::LAYER);
        m.insert("toggle", key_range::TOGGLE);
        m.insert("setlayout", key_range::SET_LAYOUT);
        m.insert("oneshot", key_range::ONESHOT);
        m.insert("overload", key_range::MACROS_MIN);
        m.insert("dualaction", key_range::MACROS_MIN);
        m.insert("macro", key_range::MACROS_MIN);
        m.insert("hold", key_range::MACROS_MIN);
        m.insert("release", key_range::MACROS_MIN);
        m.insert("unicode", key_range::MACROS_MIN);
        m.insert("delay", key_range::MACROS_MIN);
        m
    };
    static ref SHIFT_KEY_NAMES : HashMap<char, char> = {
        let mut m = HashMap::new();
        let mut n = '\0';
        for c in r#"`~-_=+[{]}\|;:'",<.>/?1!2@3#4$5%6^7&8*9(0)"#.chars() {
            if n == '\0' {
                n = c;
            } else {
                m.insert(c, n);
                n = '\0';
            }
        }
        m
    };
    static ref KEY_NAMES: HashMap<String, u16> = {
        let mut m = HashMap::new();
        m.insert("noop".into(), 0);
        m.insert("/".into(), 0x38);
        let mut ins = |a: &str, b: u16| {
            for a in a.split('/') {
                let k = a.replace(DASH_USCORE, "").to_lowercase();
                if m.contains_key(k.as_str()) {
                    panic!("key already added {a}");
                }
                m.insert(k, b);
            }
        };
        for (i, name) in r#"
A B C D E F G H I J K L M N O P Q R S T U V W X Y Z
1 2 3 4 5 6 7 8 9 0
Return/Enter/ent
Escape/esc
backspace/bksp
Tab
Spacebar/space/spc
Dash/-/minus
Equals/=/equal
LeftBrace/[/leftsquarebracket
RightBrace/]/rightsquarebracket
Backslash/\
NonUsHash
Semicolon/;
LeftApos/'/apostrophe
GraveAccent/`/grave
Comma/,
Period/./dot
Forwardslash/slash
CapsLock
F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12
Printscreen/print
ScrollLock
Pause
Insert
Home Pageup/pgup Delete/del End Pagedown/pgdn
Right Left Down Up
KpNumLock
KpForwardslash KpStar KpDash KpPlus KpEnter
Kp1 Kp2 Kp3 Kp4 Kp5 Kp6 Kp7 Kp8 Kp9 Kp0
KpPeriod
NonUsBackslash
Application/app
Power
KpEquals
F13 F14 F15 F16 F17 F18 F19 F20 F21 F22 F23 F24
Execute
Help
Menu/mnu
Select
Stop
Again Undo
Cut Copy Paste
Find
Mute
VolumeUp VolumeDown
LockingCapsLock LockingNumLock LockingScrollLock
KpComma KpEqualSign
International1 International2 International3 International4
International5 International6 International7 International8 International9
Lang1 Lang2 Lang3 Lang4 Lang5 Lang6 Lang7 Lang8 Lang9
AlternateErase
SysreqAttention
Cancel
Clear
Prior
KeyboardReturn
Separator
Out
Oper
ClearAgain
CrselProps
Exsel
A5 A6 A7 A8 A9 AA AB AC AD AE AF
KpDouble0 KpTriple0
ThousandsSeparator DecimalSeparator CurrencyUnit CurrencySubUnit
KpLeftBracket KpRightBracket KpLeftBrace KpRightBrace
KpTab KpBackspace
KpA KpB KpC KpD KpE KpF
KpXor
KpCaret
KpPercentage
KpLess KpGreater
KpAmpersand KpDoubleAmpersand KpBar KpDoubleBar
KpColon KpHash KpSpace KpAt KpBang
KpMemoryStore KpMemoryRecall KpMemoryClear
KpMemoryAdd KpMemorySubtract KpMemoryMultiply KpMemoryDivide KpPlusMinus
KpClear KpClearEntry
KpBinary KpOctal KpDecimal KpHexadecimal
DE DF
Leftcontrol/lctrl/lc/lctl
Leftshift/lshift/ls
Leftalt/lalt/la
LeftGui/leftmeta/lgui/lg
Rightcontrol/rctrl/rc/rctl
Rightshift/rshift/rs
Rightalt/altgr/ralt/ra
RightGui/rightmeta/rgui/rg
MEDIA_PLAYPAUSE
MEDIA_STOPCD
MEDIA_PREVIOUSSONG
MEDIA_NEXTSONG
MEDIA_EJECTCD
MEDIA_VOLUMEUP
MEDIA_VOLUMEDOWN
MEDIA_MUTE
MEDIA_WWW
MEDIA_BACK
MEDIA_FORWARD
MEDIA_STOP
MEDIA_FIND
MEDIA_SCROLLUP
MEDIA_SCROLLDOWN
MEDIA_EDIT
MEDIA_SLEEP
MEDIA_COFFEE
MEDIA_REFRESH
MEDIA_CALC
"#
        .split_whitespace().enumerate()
        {
            ins(name, (i + 4) as u16);
        }

        for (i, name) in r#"
1 2 3 4 5 6 7 8
Left Right Up Down
ScrollDown/WheelDown
ScrollUp/WheelUp
ScrollRight/WheelRight
ScrollLeft/WheelLeft
Accel1 Accel2 Accel3
"#
        .split_whitespace()
        .enumerate()
        {
            for name in name.split('/') {
                ins(format!("mouse{}", name).as_str(), i as u16 + key_range::MOUSE_MIN);
            }
        }

        let mut i = |a: &str, b: u16| {
            ins(a, b+key_range::CONSUMER_MIN);
            if a.starts_with("al_") || a.starts_with("ac_") {
                ins(&a[3..], b+key_range::CONSUMER_MIN);
            }
        };

        // 15.5 Display Controls
        i("snapshot"        ,0x065);
        i("brightness_up"   ,0x06F);
        i("brightness_down" ,0x070);
        // 15.7 Transport Controls
        i("record"       ,0x0B2);
        i("fast_forward" ,0x0B3);
        i("rewind"       ,0x0B4);
        i("next_track"   ,0x0B5);
        i("prev_track"   ,0x0B6);
        i("tc_stop"         ,0x0B7);
        i("eject"        ,0x0B8);
        i("random_play"  ,0x0B9);
        i("stop_eject"   ,0x0CC);
        i("play_pause"   ,0x0CD);
        // 15.9.1 Audio Controls - Volume
        i("audio_mute"     ,0x0E2);
        i("audio_vol_up"   ,0x0E9);
        i("audio_vol_down" ,0x0EA);
        // 15.15 Application Launch Buttons
        i("al_cc_config"       ,0x183);
        i("al_email"           ,0x18A);
        i("al_calculator"      ,0x192);
        i("al_local_browser"   ,0x194);
        i("al_lock"            ,0x19E);
        i("al_control_panel"   ,0x19F);
        i("al_assistant"       ,0x1CB);
        i("al_keyboard_layout" ,0x1AE);
        // 15.16 Generic GUI Application Controls
        i("ac_new"                         ,0x201);
        i("ac_open"                        ,0x202);
        i("ac_close"                       ,0x203);
        i("ac_exit"                        ,0x204);
        i("ac_maximize"                    ,0x205);
        i("ac_minimize"                    ,0x206);
        i("ac_save"                        ,0x207);
        i("_ac_print"                       ,0x208);
        i("ac_properties"                  ,0x209);
        i("_ac_undo"                        ,0x21A);
        i("_ac_copy"                        ,0x21B);
        i("_ac_cut"                         ,0x21C);
        i("_ac_paste"                       ,0x21D);
        i("ac_select_all"                  ,0x21E);
        i("_ac_find"                        ,0x21F);
        i("ac_search"                      ,0x221);
        i("ac_homepage"                    ,0x223);
        i("ac_back"                        ,0x224);
        i("ac_forward"                     ,0x225);
        i("_ac_cancel"                      ,0x226);
        i("ac_refresh"                     ,0x227);
        i("ac_bookmarks"                   ,0x22A);
        i("ac_next_keyboard_layout_select" ,0x29D);
        i("ac_desktop_show_all_windows"    ,0x29F);
        i("ac_soft_key_left"               ,0x2A);

        // SYS_CTL
        i("system_power_down", 0x2a1);
        i("system_sleep/sleep", 0x2a2);
        i("system_wake_up/wakeup", 0x2a3);
        i("system_restart", 0x2af);
        i("system_display_toggle_int_ext", 0x2d);

        // FIRMWARE

        ins("reset_keyboard", key_range::FW_RESET_KEYBOARD);
        ins("clear_all", key_range::FW_CLEAR_ALL);
        ins("clear_layers", key_range::FW_CLEAR_LAYERS);
        ins("stop_active", key_range::FW_STOP_ACTIVE);
        ins("reset_to_usb_boot", key_range::FW_RESET_TO_USB_BOOT);
// clear_layers clear_input clear_all

        m
    };
}

pub const SHIFT_MOD: u8 = 2;

pub fn unshifted_char_code(c: char) -> char {
    match c {
        'A'..='Z' => c.to_ascii_lowercase(),
        c => *SHIFT_KEY_NAMES.get(&c).unwrap_or(&c),
    }
}

pub fn char_to_code(c: char) -> u16 {
    match c {
        'a'..='z' => ((c as u8) - b'a' + 4) as u16,
        '1'..='9' => ((c as u8) - b'1' + 30) as u16,
        '0' => 39,
        c => *KEY_NAMES.get(c.to_string().as_str()).unwrap_or(&0),
    }
}

pub fn key_code(name: &str) -> Option<u16> {
    let name = if name.len() > 1 && name.starts_with('\\') {
        &name[1..]
    } else {
        name
    };
    if name.contains(DASH_USCORE) {
        let name = name.replace(DASH_USCORE, "");
        KEY_NAMES.get(name.as_str()).copied()
    } else {
        KEY_NAMES.get(name).copied()
    }
}

pub fn action_code(name: &str) -> Option<u16> {
    ACTION_NAMES.get(name).copied()
}

pub fn modifier_macro(_name: &str) -> Option<u16> {
    Some(key_range::MACROS_MIN)
}

pub fn modifiers_to_bit_map(text: &str) -> Option<u8> {
    if text.is_empty() {
        return Some(0);
    }
    let mut bm = 0;
    for s in text.split('-') {
        match MODIFIER_BIT_MAP.get(s) {
            Some(bit) => bm |= bit,
            None => return None,
        }
    }

    Some(bm)
}

pub fn modifiers_to_string(mut modifiers: u8) -> String {
    let mut ans = String::new();

    for m in MODIFIER_BITS {
        if modifiers == 0 {
            return ans;
        }
        if modifiers & 1 == 1 {
            if !ans.is_empty() {
                ans += "-";
            }
            ans += m;
        }

        modifiers >>= 1;
    }
    ans
}

#[cfg(test)]
#[path = "keycodes_test.rs"]
mod test;

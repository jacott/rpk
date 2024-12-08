use std::collections::HashMap;

use lazy_static::lazy_static;
use rpk_common::keycodes::key_range;

const DASH_USCORE: [char; 2] = ['_', '-'];

const MODIFIER_BITS: [&str; 8] = ["C", "S", "A", "G", "RC", "RS", "RA", "RG"];
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
    static ref FULL_KEY_NAMES: HashMap<&'static str, u16> = {
        let mut m = HashMap::new();
        m.insert("Transparent", 0);
        m.insert("No_op", 1);
        m.insert("Nop", 1);
        m.insert("/", 0x38);
        let mut ins = |a: &'static str, b: u16| {
            for k in a.split('/') {
                if m.contains_key(k) {
                    panic!("key already added {a}");
                }
                m.insert(k, b);
            }
        };
        for (i, name) in r#"
A B C D E F G H I J K L M N O P Q R S T U V W X Y Z
1 2 3 4 5 6 7 8 9 0
Enter/Return/ent/⏎
Escape/Esc/⎋
backspace/bksp/⌫
Tab/⇥
Spacebar/Space/spc/␣
Dash/minus/-
Equals/equal/=
LeftBrace/leftsquarebracket/[
RightBrace/rightsquarebracket\]
Backslash/\
Hash/NonUsHash/#
Semicolon/;
LeftApos/apostrophe/'
GraveAccent/grave/`
Comma/,
Period/dot/.
Forwardslash/slash/
CapsLock/caps/⇪
F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12
Printscreen/print
ScrollLock
Pause
Insert
Home/⇱
PageUp/PgUp/⇞
Delete/del/⌦
End/⇲
PageDown/PgDn/⇟
Right/→ Left/← Down/↓ Up/↑
NumLock/KpNumLock/⇭
KpForwardslash KpStar KpDash KpPlus KpEnter
Kp1 Kp2 Kp3 Kp4 Kp5 Kp6 Kp7 Kp8 Kp9 Kp0
KpPeriod
NonUsBackslash
Application/App
Power
KpEquals
F13 F14 F15 F16 F17 F18 F19 F20 F21 F22 F23 F24
Execute
Help
Menu
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
LeftControl/LCtrl/LCTL/⌃
LeftShift/LShift/LSFT/⇧
LeftAlt/LAlt
LeftGui/LeftMeta/LGUI
RightControl/RCtrl/RCtl
RightShift/RShift/RSFT
RightAlt/AltGr/RAlt
RightGui/RightMeta/RGUI
Media_Play_Pause,
Media_Stop_CD
Media_Previous_Song
Media_Next_Song
Media_Eject_CD
Media_Volume_Up
Media_Volume_Down
Media_Mute
Media_WWW
Media_Back
Media_Forward
Media_Stop
Media_Find
Media_Scroll_Up
Media_Scroll_Down
Media_Edit
Media_Sleep
Media_Coffee
Media_Refresh
Media_Calc
"#
        .split_whitespace().enumerate()
        {
            ins(name, (i + 4) as u16);
        }

        for (i, name) in r#"
Mouse1 Mouse2 Mouse3 Mouse4 Mouse5 Mouse6 Mouse7 Mouse8
MouseLeft MouseRight MouseUp MouseDown
MouseScrollDown/MouseWheelDown
MouseScrollUp/MouseWheelUp
MouseScrollRight/MouseWheelRight
MouseScrollLeft/MouseWheelLeft
MouseAccel1 MouseAccel2 MouseAccel3
"#
        .split_whitespace()
        .enumerate()
        {
            for name in name.split('/') {
                ins(name, i as u16 + key_range::MOUSE_MIN);
            }
        }

        let mut i = |a: &'static str, b: u16| {
            ins(a, b+key_range::CONSUMER_MIN);
        };

        // 15.5 Display Controls
        i("Snapshot"        ,0x065);
        i("Brightness_Up"   ,0x06F);
        i("Brightness_Down" ,0x070);
        // 15.7 Transport Controls
        i("TC_Record"       ,0x0b2);
        i("TC_Fast_Forward" ,0x0B3);
        i("TC_Rewind"       ,0x0B4);
        i("TC_Next_Track"   ,0x0B5);
        i("TC_Prev_Track"   ,0x0B6);
        i("TC_Stop"         ,0x0B7);
        i("TC_Eject"        ,0x0B8);
        i("TC_Random_Play"  ,0x0B9);
        i("TC_Stop_Eject"   ,0x0CC);
        i("TC_Play_Pause"   ,0x0CD);
        // 15.9.1 Audio Controls - Volume
        i("Audio_Mute"     ,0x0E2);
        i("Audio_Vol_Up"   ,0x0E9);
        i("Audio_Vol_Down" ,0x0EA);
        // 15.15 Application Launch Buttons
        i("AL_Cc_Config"       ,0x183);
        i("AL_Email"           ,0x18A);
        i("AL_Calculator"      ,0x192);
        i("AL_Local_Browser"   ,0x194);
        i("AL_Lock"            ,0x19E);
        i("AL_Control_Panel"   ,0x19F);
        i("AL_Assistant"       ,0x1CB);
        i("AL_Keyboard_Layout" ,0x1AE);
        // 15.16 Generic GUI Application Controls
        i("AC_New"                         ,0x201);
        i("AC_Open"                        ,0x202);
        i("AC_Close"                       ,0x203);
        i("AC_Exit"                        ,0x204);
        i("AC_Maximize"                    ,0x205);
        i("AC_Minimize"                    ,0x206);
        i("AC_Save"                        ,0x207);
        i("AC_Print"                       ,0x208);
        i("AC_Properties"                  ,0x209);
        i("AC_Undo"                        ,0x21A);
        i("AC_Copy"                        ,0x21B);
        i("AC_Cut"                         ,0x21C);
        i("AC_Paste"                       ,0x21D);
        i("AC_Select_All"                  ,0x21E);
        i("AC_Find"                        ,0x21F);
        i("AC_Search"                      ,0x221);
        i("AC_Homepage"                    ,0x223);
        i("AC_Back"                        ,0x224);
        i("AC_Forward"                     ,0x225);
        i("AC_Cancel"                      ,0x226);
        i("AC_Refresh"                     ,0x227);
        i("AC_Bookmarks"                   ,0x22A);
        i("AC_Next_Keyboard_Layout_Select" ,0x29D);
        i("AC_Desktop_Show_All_Windows"    ,0x29F);
        i("AC_Soft_Key_Left"               ,0x2A);

        // SYS_CTL
        i("System_Power_Down", 0x2a1);
        i("System_Sleep/Sleep", 0x2a2);
        i("System_Wake_Up/Wakeup", 0x2a3);
        i("System_Restart", 0x2af);
        i("System_Display_Toggle_Int_Ext", 0x2d);

        // FIRMWARE

        ins("Reset_Keyboard", key_range::FW_RESET_KEYBOARD);
        ins("Clear_All", key_range::FW_CLEAR_ALL);
        ins("Clear_Layers", key_range::FW_CLEAR_LAYERS);
        ins("Stop_Active", key_range::FW_STOP_ACTIVE);
        ins("Reset_To_Usb_Boot", key_range::FW_RESET_TO_USB_BOOT);
// clear_layers clear_input clear_all

        m
    };
        static ref KEY_NAMES: HashMap<String, u16> = {
            let mut m = HashMap::new();
            for (r, v) in FULL_KEY_NAMES.iter() {
                let k = r.replace(DASH_USCORE, "").to_lowercase();
                if m.contains_key(k.as_str()) {
                    m.insert(r.to_string(), *v);
                }
                m.insert(k, *v);
            }
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

pub struct KeycodeDescriptor {
    pub name: &'static str,
    pub code: u16,
}

pub fn keycodes_iter() -> impl Iterator<Item = KeycodeDescriptor> {
    let iter = FULL_KEY_NAMES.iter();
    iter.map(|(name, code)| KeycodeDescriptor { name, code: *code })
}

#[cfg(test)]
#[path = "keycodes_test.rs"]
mod test;

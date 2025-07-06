use crate::test::kc;

use super::*;

#[test]
fn modifiers_convert() {
    assert_eq!(modifiers_to_bit_map("S").unwrap(), 2);
}

#[test]
fn char_to_code_test() {
    fn ccode(c: char) -> u16 {
        let a = char_to_code(unshifted_char_code(c));
        assert!(a != 0, "invalid char {c}");
        a
    }

    assert_eq!(ccode('a'), kc("a"));
    assert_eq!(ccode('0'), kc("0"));
    assert_eq!(ccode('1'), kc("1"));
    assert_eq!(ccode('A'), kc("a"));
    assert_eq!(ccode('5'), kc("5"));
    assert_eq!(ccode('%'), kc("5"));
    assert_eq!(ccode('['), kc("["));
    assert_eq!(ccode('['), kc("\\["));
    assert_eq!(ccode('\\'), kc("\\"));
    assert_eq!(ccode('<'), kc(","));
    assert_eq!(ccode('{'), kc("["));
}

#[test]
fn test_key_code() {
    assert_eq!(key_code("mediaplaypause"), Some(232));
    assert_eq!(key_code("Media_Play_Pause"), Some(232));
    assert_eq!(key_code("a"), Some(4));
    assert_eq!(key_code("A"), Some(4));
    assert_eq!(key_code("-"), Some(45));
}

#[test]
fn test_list_keycodes() {
    let m = keycodes_iter().filter(|l| l.name.starts_with("Mouse"));

    assert_eq!(m.count(), 23);

    let k = keycodes_iter().find(|d| d.code == 0xb5).unwrap();
    assert_eq!(k.name, "CurrencySubUnit");
}

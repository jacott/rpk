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
        assert!(a != 0, "invalid char {}", c);
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
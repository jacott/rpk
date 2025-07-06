//use super::*;

use rpk_common::f32_from_u16;

use crate::{f32_to_u16, keycodes};

pub fn kc(c: &str) -> u16 {
    match keycodes::key_code(c) {
        Some(kc) => kc,
        None => panic!("Unknown key mnemonic: {c:?}"),
    }
}

#[test]
fn f32_to_from_u16() {
    let x = 0.2;

    let a: Vec<u16> = f32_to_u16(x).collect();

    let x2 = f32_from_u16(a[0], a[1]);
    assert_eq!(x, x2);
}

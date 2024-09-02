use super::*;

#[test]
fn f32_to_from_u16() {
    let x2 = f32_from_u16(123, 456);
    assert_eq!(7.3469086e-38, x2);
}

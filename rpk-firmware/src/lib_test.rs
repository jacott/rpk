extern crate std;

#[cfg(feature = "defmt")]
#[defmt::global_logger]
struct Logger;

//struct MyDecoder {}

//static DECODER: Mutex<Option<MyDecoder>> = Mutex::new(None);

#[cfg(feature = "defmt")]
unsafe impl defmt::Logger for Logger {
    fn acquire() {}

    unsafe fn release() {}

    unsafe fn write(_bytes: &[u8]) {}

    unsafe fn flush() {}
}

#[cfg(all(not(test), feature = "defmt"))]
#[defmt::panic_handler] // defmt's attribute
fn defmt_panic() -> ! {
    // leave out the printing part here
    std::unimplemented!()
}

#[test]
fn key_bits() {
    let mut bits = [0; crate::KEY_BITS_SIZE];
    assert!(crate::add_key_bit(&mut bits, 0x06));
    assert!(crate::add_key_bit(&mut bits, 0x07));
    assert!(!crate::add_key_bit(&mut bits, 0x07));
    assert!(crate::add_key_bit(&mut bits, 0x08));
    assert!(crate::add_key_bit(&mut bits, 0xe7));
    assert_eq!(&bits[..3], &[0b1100_0000, 0x01, 0]);
    assert_eq!(bits[crate::KEY_BITS_SIZE - 4], 128);

    assert!(crate::del_key_bit(&mut bits, 0x07));
    assert_eq!(&bits[..3], &[0b0100_0000, 0x01, 0]);
    assert!(!crate::del_key_bit(&mut bits, 0x07));
}

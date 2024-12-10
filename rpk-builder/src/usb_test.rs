use super::*;

#[test]
fn config_builder() {
    let mut b = ConfigBuilder {
        manufacturer: "Jacott",
        product: "Macropad",
        vendor_id: 0xba5e,
        product_id: 0xfade,
        serial_number: "rpk:123",
        max_power: 150,
    };

    b.manufacturer = "Jacott";

    assert_eq!(b.manufacturer, "Jacott");
}

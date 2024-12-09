use super::*;

use tempfile::tempdir;

#[test]
fn build() -> anyhow::Result<()> {
    let tmp_dir = tempdir()?;
    let root = tmp_dir.into_path();
    let mut builder = KeyboardBuilder::new(root.join("my-keeb"));

    builder.chip(ChipType::Rp2040);

    builder.build()?;

    let v = fs::read_to_string(root.join("my-keeb/Cargo.toml"))?;

    assert!(v.contains(
        r#"[package]
name = "my-keeb""#
    ));
    assert!(v.contains(r#"rpk-builder = {version = "0.1", features = ["rp", "reset-on-panic"]}"#));
    assert!(v.contains(r#"rpk-builder/defmt"#));

    let v = fs::read_to_string(root.join("my-keeb/build.rs"))?;

    assert_eq!(
        v,
        r#"fn main() {
    rpk_config::builder::build_rs();
}
"#
    );

    let v = fs::read_to_string(root.join("my-keeb/src/main.rs"))?;

    assert!(v.contains("rpk_builder::configure_keyboard!()"));
    assert!(v.contains("run_keyboard!(spawner, driver, input_pins, output_pins, flash)"));

    let v = fs::read_to_string(root.join("my-keeb/.cargo/config.toml"))?;

    assert!(v.contains(
        r#"[target.thumbv6m-none-eabi]
runner = "elf2uf2-rs -d""#
    ));
    assert!(v.contains(
        r#"
linker = "flip-link"
"#
    ));

    assert!(v.contains(
        r#"
rustflags = ["-C", "linker=flip-link"]
"#
    ));

    let v = fs::read_to_string(root.join("my-keeb/memory.x"))?;

    assert!(v.starts_with(
        r#"MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
"#
    ));

    assert!(v.ends_with("\n}\n"));

    let v = fs::read_to_string(root.join("my-keeb/default-layout.rpk.conf"))?;

    assert!(v.starts_with(
        r#"[firmware]

vendor_id     = 0xfeed
product_id    = 0xkeeb
serial_number = rpk:123456

manufacturer  = MISSING
product       = MISSING
max_power     = 100

chip          = rp2040
output_pins   = [MISSING, PINS]
input_pins    = [MISSING, PINS]
row_is_output = true

max_layout_size     = 8 * 1024

# Flash ring file system
flash_size          = 2 * 1024 * 1024
fs_base             = 0x100000
fs_size             = flash_size - fs_base

[matrix:rxc]

0x00 = q w e r t y
"#
    ));

    let v = fs::read_to_string(root.join("my-keeb/.gitignore"))?;

    assert!(v.starts_with(
        r#"target
"#
    ));

    Ok(())
}

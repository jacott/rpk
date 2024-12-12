use std::{fs, io::Write, path::PathBuf};

pub enum ChipType {
    Rp2040,
}

pub struct KeyboardBuilder {
    root: PathBuf,
    name: String,
    chip: ChipType,
}
impl KeyboardBuilder {
    pub fn new(root: PathBuf) -> Self {
        let name = root.file_name().unwrap().to_string_lossy().to_string();
        Self {
            root,
            name,
            chip: ChipType::Rp2040,
        }
    }

    pub fn chip(&mut self, chip: ChipType) -> &mut Self {
        self.chip = chip;
        self
    }

    pub fn build(&self) -> anyhow::Result<()> {
        fs::create_dir_all(self.root.join("src"))?;
        fs::create_dir_all(self.root.join(".cargo"))?;

        self.create_gitignore()?;
        self.create_build_rs()?;
        self.create_main_rs()?;
        self.create_memory_x()?;
        self.create_cargo_toml()?;
        self.create_config_toml()?;
        self.create_rpk_conf()?;

        Ok(())
    }

    fn create_cargo_toml(&self) -> anyhow::Result<()> {
        let s = String::from(
            r##"[package]
name = "{{NAME}}"
version = "0.1.0"
edition = "2021"
publish = false

[workspace]

[dependencies]
cortex-m = { version = "0.7", features = ["inline-asm"] }
cortex-m-rt = "0.7"
embassy-executor = { version = "0.6", features = ["task-arena-size-32768"] }
embassy-usb = { version = "0.3", features = [
  "max-interface-count-8",
  "max-handler-count-2",
] }
rpk-builder = {version = "0.1", features = ["rp", "reset-on-panic"]}

[build-dependencies.rpk-config]
version = "0.1"

[[bin]]
name = "{{NAME}}"
test = false
doctest = false
bench = false

[profile.release]
debug = 0
opt-level = 'z'
lto = true
panic = "abort"

[profile.dev]
debug = 2
opt-level = 'z'
lto = true

[features]
defmt = ["rpk-builder/defmt"]
"##,
        );

        let s = s.replace("{{NAME}}", &self.name);
        self.create_file("Cargo.toml", &s)
    }

    fn create_main_rs(&self) -> anyhow::Result<()> {
        let s = r##"#![no_std]
#![no_main]

rpk_builder::rp_run_keyboard! {}
"##;

        self.create_file("src/main.rs", s)
    }

    fn create_gitignore(&self) -> anyhow::Result<()> {
        let s = r##"target
"##;

        self.create_file(".gitignore", s)
    }

    fn create_memory_x(&self) -> anyhow::Result<()> {
        let s = r##"MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100

    /* Pick one of the two options for RAM layout     */

    /* OPTION A: Use all RAM banks as one big block   */
    /* Reasonable, unless you are doing something     */
    /* really particular with DMA or other concurrent */
    /* access that would benefit from striping        */
    RAM   : ORIGIN = 0x20000000, LENGTH = 264K

    /* OPTION B: Keep the unstriped sections separate */
    /* RAM: ORIGIN = 0x20000000, LENGTH = 256K        */
    /* SCRATCH_A: ORIGIN = 0x20040000, LENGTH = 4K    */
    /* SCRATCH_B: ORIGIN = 0x20041000, LENGTH = 4K    */
}
"##;

        self.create_file("memory.x", s)
    }

    fn create_config_toml(&self) -> anyhow::Result<()> {
        let s = r##"[target.thumbv6m-none-eabi]
runner = "elf2uf2-rs -d"

linker = "flip-link"

rustflags = ["-C", "linker=flip-link"]

[build]
target = "thumbv6m-none-eabi"        # Cortex-M0 and Cortex-M0+

[env]
DEFMT_LOG = "debug"
"##;

        self.create_file(".cargo/config.toml", s)
    }

    fn create_build_rs(&self) -> anyhow::Result<()> {
        let s = r##"fn main() {
    rpk_config::builder::build_rs();
}
"##;

        self.create_file("build.rs", s)
    }

    fn create_rpk_conf(&self) -> anyhow::Result<()> {
        let s = r#"[firmware]

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
"#;

        self.create_file("default-layout.rpk.conf", s)
    }

    fn create_file(&self, name: &str, arg: &str) -> anyhow::Result<()> {
        fs::File::create(self.root.join(name)).and_then(|mut f| f.write_all(arg.as_bytes()))?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "init_builder_test.rs"]
mod test;

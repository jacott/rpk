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
        let s = include_str!("../../../../keyboards/rp2040/sk84/Cargo.toml");
        let s = &s[s.find("version =").expect("should contain 'version ='")
            ..s.rfind("[patch.crates-io]")
                .expect("should find [patch.crates-io]")];

        let s = String::from(
            r#"[package]
name = "sk84"
"#,
        ) + s;
        let s = s.replace("sk84", &self.name);

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
        self.create_file(".gitignore", "target\n")
    }

    fn create_memory_x(&self) -> anyhow::Result<()> {
        self.create_file(
            "memory.x",
            include_str!("../../../../keyboards/rp2040/sk84/memory.x"),
        )
    }

    fn create_config_toml(&self) -> anyhow::Result<()> {
        self.create_file(
            ".cargo/config.toml",
            include_str!("../../../../keyboards/rp2040/sk84/.cargo/config.toml"),
        )
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

vendor_id     = 0xfeed # REPLACE THIS
product_id    = 0xceeb # REPLACE THIS
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

[matrix:rxc] # matches the number of pins above

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

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use rpk_config::{
    compiler::{compile, KeyboardConfig, SourceRange},
    ConfigError,
};
use std::{
    env,
    fmt::Display,
    fs,
    path::{Path, PathBuf},
};
use syn::{visit::Visit, visit_mut::VisitMut, Lit};

#[derive(Debug)]
struct BuildError(String);
impl Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for BuildError {}
impl BuildError {
    fn compile_err(err: ConfigError, source_file: &Path, source: &str) -> Self {
        Self(err.long_format(source_file, source))
    }
    fn from_str(msg: &str) -> Self {
        Self(msg.to_owned())
    }
    fn from_error(msg: &impl std::error::Error) -> Self {
        Self(msg.to_string())
    }
}
impl From<std::io::Error> for BuildError {
    fn from(err: std::io::Error) -> Self {
        Self(err.to_string())
    }
}
impl From<&str> for BuildError {
    fn from(err: &str) -> Self {
        Self(err.to_string())
    }
}

type Result<T> = std::result::Result<T, BuildError>;

fn compile_error(message: &str) -> TokenStream {
    quote! {
        compile_error!(#message);

        macro_rules! config_pins {
            (peripherals: $p:ident) => {
                ([],[])
            };
        }

        const LAYOUT_MAPPING: &[u16] = &[];

        const INPUT_N: usize = 0;
        const OUTPUT_N: usize = 0;
        const ROW_COUNT: usize = 0;
        const COL_COUNT: usize = 0;
        const ROW_IS_OUTPUT: bool = true;
        const LAYOUT_MAX: usize = 0;

        const FLASH_SIZE: usize = 0;
        const FS_BASE: usize = 0;
        const FS_SIZE: usize = 0;
        const FS_MAX_FILES: u32 = 0;

        static CONFIG_BUILDER: rpk_builder::usb::ConfigBuilder = rpk_builder::usb::ConfigBuilder {
            vendor_id: 0,
            product_id: 0,
            manufacturer: "",
            product: "",
            serial_number: "",
            max_power: 0,
        };

        const SCANNER_BUFFER_SIZE: usize = 32;
        const REPORT_BUFFER_SIZE: usize = 32;

        type Flash = flash::Flash<'static, FLASH, Async, 4096>;
        type Rfs = NorflashRingFs<'static, Flash, 0, 4096,
          { flash::ERASE_SIZE as u32 }, { flash::PAGE_SIZE }, FS_MAX_FILES >;
    }
}

pub(crate) fn configure_keyboard(input: TokenStream) -> TokenStream {
    match get_config_filename(input) {
        Ok(source_file) => match quote_conf(&source_file) {
            Ok(s) => s,
            Err(err) => compile_error(err.0.as_str()),
        },

        Err(err) => compile_error(err.0.as_str()),
    }
}

fn get_config_filename(input: TokenStream) -> Result<PathBuf> {
    const LAYOUT: &str = "default-layout.rpk.conf";
    let cargo = cargo_dir()?;

    if input.is_empty() {
        return Ok(cargo.join(LAYOUT));
    }

    let ast: syn::Expr = syn::parse2(input.clone()).map_err(|e| BuildError::from_error(&e))?;

    if let syn::Expr::Lit(expr) = &ast {
        if let Lit::Str(lit) = &expr.lit {
            return Ok(cargo.join(lit.value()));
        }
    }

    Err(BuildError(
        "Expected filename or nothing as argument".to_owned(),
    ))
}

fn cargo_dir() -> Result<PathBuf> {
    Ok(PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .ok_or_else(|| BuildError::from_str("CARGO_MANIFEST_DIR not found"))?,
    ))
}

fn quote_conf(source_file: &Path) -> Result<TokenStream> {
    let source = read_conf(source_file)?;

    let config = compile(PathBuf::from(source_file), source.as_str())
        .map_err(|e| BuildError::compile_err(e, source_file, source.as_str()))?;

    let (defs, input_pins, output_pins) = parse_firmware(&config)?;

    let macros = quote! {
        macro_rules! config_matrix_pins_rp {
            (peripherals: $p:ident, input: [$($in_pin:ident), *], output: [$($out_pin:ident), +]) => {
                {
                    let mut output_pins = [$(Output::new(AnyPin::from($p.$out_pin), gpio::Level::High)), +];
                    let input_pins = [$(Input::new(AnyPin::from($p.$in_pin), gpio::Pull::Up)), +];
                    output_pins.iter_mut().for_each(|p| {
                        p.set_high();
                    });
                    (input_pins, output_pins)
                }
            };
        }

        macro_rules! config_pins {
            (peripherals: $p:ident) => {
                config_matrix_pins_rp!(peripherals: $p,
                    input: #input_pins, output: #output_pins)
            };
        }
    };

    let source_file = source_file.display().to_string();

    let result = quote! {
        #defs
        #macros

        const fn max32(a: u32, b: u32) -> u32 {
            if a < b {
                b
            } else {
                a
            }
        }

        const _: &[u8] = include_bytes!(#source_file);
        const ERASE_SIZE: u32 = max32(1, (flash::ERASE_SIZE as u32) >> 2) << 2;
        const DIR_SIZE: u32 = (max32(FS_MAX_FILES * 4 + 20, ERASE_SIZE)/ERASE_SIZE)*ERASE_SIZE;
        const PAGE_SIZE: usize = max32(4, ((flash::PAGE_SIZE as u32) >> 2) << 2) as usize;

        type Flash = flash::Flash<'static, FLASH, Async, FLASH_SIZE>;
        type Rfs = NorflashRingFs<'static, Flash, FS_BASE, FS_SIZE, DIR_SIZE, PAGE_SIZE, FS_MAX_FILES>;
    };

    Ok(result)
}

fn read_conf(source_file: &Path) -> Result<String> {
    let source = fs::read_to_string(source_file).map_err(|e| {
        BuildError(format!(
            "Can't read conf file {}, {e:?}",
            &source_file.display()
        ))
    })?;

    Ok(source)
}

fn syn_array_len(input: &TokenStream) -> Result<usize> {
    struct SynArrayLen(usize);
    impl<'ast> Visit<'ast> for SynArrayLen {
        fn visit_path(&mut self, _: &'ast syn::Path) {
            self.0 += 1;
        }
    }

    let ast: syn::Expr = syn::parse2(input.clone()).map_err(|e| BuildError::from_error(&e))?;
    let mut v = SynArrayLen(0);
    v.visit_expr(&ast);

    Ok(v.0)
}

fn syn_bool(input: &TokenStream) -> Result<bool> {
    struct SynBool(bool, usize);
    impl<'ast> Visit<'ast> for SynBool {
        fn visit_lit_bool(&mut self, i: &'ast syn::LitBool) {
            self.0 = i.value;
            self.1 += 1;
        }
    }

    let ast: syn::Expr = syn::parse2(input.clone()).map_err(|e| BuildError::from_error(&e))?;
    let mut v = SynBool(false, 0);
    v.visit_expr(&ast);

    if v.1 == 1 {
        Ok(v.0)
    } else {
        Err(BuildError::from_str("Expected true or false"))
    }
}

fn parse_firmware(config: &KeyboardConfig) -> Result<(TokenStream, TokenStream, TokenStream)> {
    struct SynIdent<'a>(&'a KeyboardConfig<'a>, bool);
    impl<'a> SynIdent<'a> {
        fn get_range(&mut self, key: &str) -> Result<SourceRange> {
            self.0
                .firmware_get(key)
                .ok_or_else(|| BuildError(format!("Missing required firmware config: {key}")))
        }

        fn get_var(&mut self, key: &str) -> Result<&'a str> {
            let v = self.get_range(key)?;
            Ok(self.0.trim_value(&v))
        }

        fn parse_var(&mut self, key: &str) -> Result<TokenStream> {
            let vr = self.get_range(key)?;
            let text = self.0.trim_value(&vr);
            let mut expr: syn::Expr = syn::parse_str(text).map_err(|e| {
                BuildError::compile_err(
                    ConfigError::new(e.to_string(), vr.start..vr.end),
                    self.0.path.as_path(),
                    self.0.source,
                )
            })?;

            let ss = self.1;

            self.visit_expr_mut(&mut expr);

            if self.1 != ss {
                Err(BuildError::compile_err(
                    ConfigError::new("Unknown identifier".into(), vr.start..vr.end),
                    self.0.path.as_path(),
                    self.0.source,
                ))
            } else {
                Ok(expr.to_token_stream())
            }
        }
    }
    impl VisitMut for SynIdent<'_> {
        fn visit_ident_mut(&mut self, i: &mut proc_macro2::Ident) {
            let key = i.to_string();
            if self.0.firmware_get(&key).is_some() {
                *i = proc_macro2::Ident::new(key.to_uppercase().as_str(), i.span());
            } else {
                self.1 = true;
            }
        }
    }

    macro_rules! get {
        ($f:ident) => {
            let $f = SynIdent(config, false).get_var(stringify!($f))?;
        };
    }

    macro_rules! parse {
        ($f:ident) => {
            let $f = SynIdent(config, false).parse_var(stringify!($f))?;
        };
        (PIN: $f:ident) => {
            let $f = SynIdent(config, true).parse_var(stringify!($f))?;
        };
    }

    let bin = config.serialize();
    let layout_mapping = format!("{{ const M: [u16; {}] = {:?}; &M }}", bin.len(), bin)
        .parse::<TokenStream>()
        .map_err(|e| BuildError(e.to_string()))?;
    parse!(vendor_id);
    parse!(product_id);
    parse!(row_is_output);
    parse!(max_layout_size);

    parse!(flash_size);
    parse!(fs_base);
    parse!(fs_size);
    parse!(fs_max_files);

    get!(chip);
    parse!(PIN: input_pins);
    parse!(PIN: output_pins);

    if chip != "rp2040" {
        let vr = config.firmware_get("chip").unwrap();
        return Err(BuildError::compile_err(
            ConfigError::new("Unknown/Unsupported chipset".into(), vr.start..vr.end),
            config.path.as_path(),
            config.source,
        ));
    }

    get!(manufacturer);
    get!(product);
    get!(serial_number);
    parse!(max_power);

    parse!(scanner_buffer_size);
    parse!(report_buffer_size);

    let input_n = syn_array_len(&input_pins)?;
    let output_n = syn_array_len(&output_pins)?;

    let (row_count, col_count) = if syn_bool(&row_is_output)? {
        (output_n, input_n)
    } else {
        (input_n, output_n)
    };

    Ok((
        quote! {
            const LAYOUT_MAPPING: &[u16] = #layout_mapping;

            const INPUT_N: usize = #input_n;
            const OUTPUT_N: usize = #output_n;
            const ROW_COUNT: usize = #row_count;
            const COL_COUNT: usize = #col_count;
            const ROW_IS_OUTPUT: bool = #row_is_output;
            const LAYOUT_MAX: usize = #max_layout_size;

            const FLASH_SIZE: usize = #flash_size;
            const FS_BASE: usize = #fs_base;
            const FS_SIZE: usize = #fs_size;
            const FS_MAX_FILES: u32 = #fs_max_files;

            static CONFIG_BUILDER: rpk_builder::usb::ConfigBuilder = rpk_builder::usb::ConfigBuilder {
                vendor_id: #vendor_id,
                product_id: #product_id,
                manufacturer: #manufacturer,
                product: #product,
                serial_number: #serial_number,
                max_power: #max_power,
            };
            const SCANNER_BUFFER_SIZE: usize = #scanner_buffer_size;
            const REPORT_BUFFER_SIZE: usize = #report_buffer_size;
        },
        input_pins,
        output_pins,
    ))
}

#[cfg(test)]
#[path = "build_test.rs"]
mod test;

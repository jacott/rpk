use proc_macro2::TokenStream;
use quote::quote;
use regex::{Captures, Regex};
use rpk_config::{
    compiler::{compile, KeyboardConfig},
    ConfigError,
};
use std::{
    borrow::Cow,
    env,
    fmt::Display,
    fs,
    path::{Path, PathBuf},
};
use syn::{visit::Visit, Lit};

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

    let config = compile(source.as_str())
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

        const _: &[u8] = include_bytes!(#source_file);

        type Flash = flash::Flash<'static, FLASH, Async, FLASH_SIZE>;
        type Rfs = NorflashRingFs<'static, Flash, FS_BASE, FS_SIZE,
          { flash::ERASE_SIZE as u32 }, { flash::PAGE_SIZE }, >;
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
    let vre = Regex::new(r"([a-zA-Z_]+)").map_err(|e| BuildError(e.to_string()))?;

    let get_var = |v: &str| {
        let v = config
            .firmware_get(v)
            .ok_or_else(|| BuildError(format!("Missing required firmware config: {}", v)))?;

        let v = vre.replace_all(v, |caps: &Captures| {
            if let Some(v) = config.firmware_get(&caps[1]) {
                v.to_owned()
            } else {
                caps[0].to_owned()
            }
        });

        Ok::<Cow<'_, str>, BuildError>(v)
    };

    macro_rules! get {
        ($f:tt) => {
            let $f = get_var(stringify!($f))?;
        };
    }

    macro_rules! parse {
        ($f:tt) => {
            get!($f);
            let $f = $f
                .parse::<TokenStream>()
                .map_err(|e| BuildError(format!("Error parsing {}: {e}", $f)))?;
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

    parse!(input_pins);
    parse!(output_pins);

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

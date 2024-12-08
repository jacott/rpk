use clap::{Args, Parser, Subcommand, ValueEnum};
use rpk_common::keycodes::key_range;
use rpk_config::{compiler::KeyboardConfig, keycodes, pretty_compile, vendor_coms::KeyboardCtl};
use std::{
    fs,
    path::{Path, PathBuf},
    process,
};

fn parse_hex(v: &Option<&str>) -> Result<Option<u16>, &'static str> {
    if let Some(v) = v {
        u16::from_str_radix(
            if v.to_lowercase().starts_with("0x") {
                &v[2..]
            } else {
                v
            },
            16,
        )
        .map(Some)
        .map_err(|_| "Invalid hex number")
    } else {
        Ok(None)
    }
}

trait DeviceLookup {
    fn vendor_id(&self) -> Option<u16>;
    fn product_id(&self) -> Option<u16>;
    fn serial_number(&self) -> &str;
    fn no_found(&self) -> String {
        static ANY: &str = "any";
        fn u16_to_hex(a: &Option<u16>) -> String {
            match a {
                Some(n) => format!("{n:04x}"),
                None => ANY.to_owned(),
            }
        }
        let vendor_id = self.vendor_id();
        let product_id = self.product_id();
        let serial_number = self.serial_number();
        format!(
            "No matching RPK usb device found!\n  vendor_id: {}, product_id: {}, serial_number: {}",
            u16_to_hex(&vendor_id),
            u16_to_hex(&product_id),
            if serial_number.is_empty() {
                ANY
            } else {
                serial_number
            }
        )
    }
}

/// Configure RPK keyboards
#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// The USB vendor_id of the device to find in 4 hex digits
    #[clap(long, short)]
    vendor_id: Option<String>,
    /// The USB product_id of the device to find in 4 hex digits
    #[clap(long, short)]
    product_id: Option<String>,
    /// The USB serial_number of the device to find. Must start with rpk:
    #[clap(long, short)]
    serial_number: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// List keycode names
    ListKeycodes(ListKeycodesArgs),
    /// List RPK devices
    ListUSB,
    /// Reset (restart) the keyboard
    Reset(ResetArgs),
    /// Validate a keyboard configuation file
    Validate(ValidateArgs),
    /// Upload keyboard configuation
    Upload(UploadArgs),
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum CodeType {
    Basic,
    Consumer,
    System,
    Mouse,
    Custom,
}

#[derive(Args)]
struct ListKeycodesArgs {
    /// Include extra information including the keycode hex value
    #[clap(long, short)]
    verbose: bool,

    /// Sort results by keycode; Defaults to sorting by name
    #[clap(long, short)]
    sort_by_keycode: bool,

    /// Limit to keycode type
    #[clap(long, short)]
    code_type: Option<CodeType>,

    /// Only list key names than contains pattern (case insensitive) if pattern starts with 0x then
    /// key names matching the key code will be shown.
    #[clap()]
    pattern: Option<String>,
}

#[derive(Args)]
struct ResetArgs {
    /// Reset keyboard in to usb boot mode
    #[clap(long, short)]
    usb_boot: bool,
}

struct ConfigFinder {
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    serial_number: String,
}
impl DeviceLookup for ConfigFinder {
    fn vendor_id(&self) -> Option<u16> {
        self.vendor_id
    }

    fn product_id(&self) -> Option<u16> {
        self.product_id
    }

    fn serial_number(&self) -> &str {
        self.serial_number.as_str()
    }
}
impl ConfigFinder {
    pub fn new(config: &KeyboardConfig, args: &impl DeviceLookup) -> Result<Self, &'static str> {
        let vendor_id = parse_hex(&config.firmware_get("vendor_id"))?;
        let product_id = parse_hex(&config.firmware_get("product_id"))?;
        Ok(Self {
            vendor_id: args.vendor_id().or(vendor_id),
            product_id: args.product_id().or(product_id),
            serial_number: if args.serial_number().is_empty() {
                config
                    .firmware_get("serial_number")
                    .unwrap_or_default()
                    .to_string()
            } else {
                args.serial_number().to_string()
            },
        })
    }

    pub fn from_cli(cli: &Cli) -> Result<Self, &'static str> {
        let vendor_id = parse_hex(&cli.vendor_id.as_deref())?;
        let product_id = parse_hex(&cli.product_id.as_deref())?;
        Ok(Self {
            vendor_id,
            product_id,
            serial_number: if cli.serial_number.is_none() {
                String::new()
            } else {
                cli.serial_number.as_ref().unwrap().to_owned()
            },
        })
    }
}

#[derive(Args)]
struct UploadArgs {
    /// keyboard config description file
    file: PathBuf,
}

#[derive(Args)]
struct ValidateArgs {
    /// keyboard config description file
    file: PathBuf,
}

fn iter_keyboards<T: DeviceLookup>(
    lookup: &T,
) -> Result<impl Iterator<Item = nusb::DeviceInfo> + use<'_, T>, String> {
    let vendor_id = lookup.vendor_id();
    let product_id = lookup.product_id();
    let serial_number = lookup.serial_number();
    nusb::list_devices()
        .map(|i| {
            i.filter(move |d| {
                d.serial_number().unwrap_or_default().starts_with("rpk:")
                    && vendor_id.is_none_or(|id| d.vendor_id() == id)
                    && product_id.is_none_or(|id| d.product_id() == id)
                    && (serial_number.is_empty()
                        || d.serial_number().is_some_and(|d| d == serial_number))
            })
        })
        .map_err(|err| err.to_string())
}

fn list_usb(lookup: &impl DeviceLookup) -> Result<(), String> {
    println!("RPK keyboards:");
    for dev in iter_keyboards(lookup)? {
        println!(
            "Device: {:03}.{:03}, Id: {:04x}:{:04x}, Name: {} - {}, Serial: {} ",
            dev.bus_number(),
            dev.device_address(),
            dev.vendor_id(),
            dev.product_id(),
            dev.manufacturer_string().unwrap_or(""),
            dev.product_string().unwrap_or(""),
            dev.serial_number().unwrap_or(""),
        );
    }

    println!();
    Ok(())
}

fn get_keyboard(lookup: &impl DeviceLookup) -> Result<KeyboardCtl, String> {
    if let Some(dev) = iter_keyboards(lookup)?.next() {
        let dev = dev.open().unwrap();
        KeyboardCtl::find_vendor_interface(&dev)
    } else {
        Err(lookup.no_found())
    }
}

fn reset_keyboard(args: &ResetArgs, lookup: &impl DeviceLookup) -> Result<(), String> {
    let ctl = get_keyboard(lookup)?;
    if args.usb_boot {
        ctl.reset_to_usb_boot_from_usb()
    } else {
        ctl.reset_keyboard()
    }
}

fn upload(args: &UploadArgs, lookup: &impl DeviceLookup) -> Result<(), String> {
    let file = &args.file;
    let err = match fs::read_to_string(file) {
        Ok(src) => {
            let config = compile_file(file, src.as_str())?;
            let bin = config.serialize();
            let finder = ConfigFinder::new(&config, lookup)?;
            let ctl = get_keyboard(&finder)?;
            return ctl.save_config(bin.as_slice());
        }

        Err(err) => err.to_string(),
    };
    Err(format!(
        "Failed to compile \"{}\"!\n    {}",
        file.to_str().unwrap(),
        &err
    ))
}

fn compile_file<'s>(file: &Path, src: &'s str) -> Result<KeyboardConfig<'s>, String> {
    pretty_compile(file, src).map_err(|err| {
        if err.span.is_none() {
            err.to_string()
        } else {
            "".into()
        }
    })
}

fn validate(args: &ValidateArgs) -> Result<(), String> {
    let file = &args.file;

    match fs::read_to_string(file) {
        Ok(src) => {
            compile_file(file, &src)?;
            Ok(())
        }

        Err(err) => Err(format!(
            "Failed to compile \"{}\"!\n    {}",
            file.to_str().unwrap(),
            &err
        )),
    }
}

fn main() {
    let cli = Cli::parse();

    let result = run(&cli);

    if let Err(message) = result {
        eprintln!("{}", message);
        process::exit(1);
    };
}

fn run(cli: &Cli) -> Result<(), String> {
    let lookup = ConfigFinder::from_cli(cli)?;

    match &cli.command {
        Commands::Upload(args) => upload(args, &lookup),
        Commands::Validate(args) => validate(args),
        Commands::ListUSB => list_usb(&lookup),
        Commands::Reset(args) => reset_keyboard(args, &lookup),
        Commands::ListKeycodes(args) => list_keycodes(args),
    }
}

fn list_keycodes(args: &ListKeycodesArgs) -> Result<(), String> {
    let iter = keycodes::keycodes_iter().filter(|d| match args.code_type {
        Some(t) => match t {
            CodeType::Basic => d.code <= key_range::BASIC_MAX,
            CodeType::Consumer => {
                (key_range::CONSUMER_MIN..=key_range::CONSUMER_MAX).contains(&d.code)
            }
            CodeType::System => (key_range::SYS_CTL_MIN..=key_range::SYS_CTL_MAX).contains(&d.code),
            CodeType::Mouse => (key_range::MOUSE_MIN..=key_range::MOUSE_MAX).contains(&d.code),
            CodeType::Custom => d.code >= key_range::FIRMWARE_MIN,
        },
        None => true,
    });
    let mut codes = if let Some(ref pattern) = &args.pattern {
        let pattern = pattern.to_lowercase();
        if let Some(hex) = pattern.strip_prefix("0x") {
            let pattern = u16::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
            iter.filter(|p| p.code == pattern).collect::<Vec<_>>()
        } else {
            let pattern = pattern.as_str();
            iter.filter(|p| p.name.to_lowercase().contains(pattern))
                .collect::<Vec<_>>()
        }
    } else {
        iter.collect::<Vec<_>>()
    };
    if args.sort_by_keycode {
        codes.sort_by(|a, b| match a.code.cmp(&b.code) {
            std::cmp::Ordering::Equal => a.name.cmp(b.name),
            i => i,
        });
    } else {
        codes.sort_by_key(|k| k.name);
    }
    if args.verbose {
        let mut prev_code = 0;
        let mut names = vec![];
        if args.sort_by_keycode {
            for d in codes {
                if prev_code != d.code {
                    if !names.is_empty() {
                        verbose_print(prev_code, &names.join(", "));
                        names = vec![];
                    }
                    prev_code = d.code
                }
                names.push(d.name);
            }
            if !names.is_empty() {
                verbose_print(prev_code, &names.join(", "));
            }
        } else {
            for d in codes {
                verbose_print(d.code, d.name);
            }
        }
    } else {
        for d in codes {
            println!("{}", d.name);
        }
    }
    Ok(())
}

fn verbose_print(code: u16, name: &str) {
    println!("{code:02X}: {name}");
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn validate_cmd() {
        let args = ValidateArgs {
            file: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/default.conf"),
        };

        validate(&args).expect("to be valid");
    }
}

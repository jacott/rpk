use clap::{Args, Parser, Subcommand};
use rpk_config::{compiler::KeyboardConfig, pretty_compile, vendor_coms::KeyboardCtl};
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
    /// Upload keyboard configuation
    Upload(UploadArgs),
    /// List RPK devices
    List,
    /// Reset (restart) the keyboard
    Reset(ResetArgs),
    /// Flash new software to keyboard
    Validate(ValidateArgs),
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

fn list(lookup: &impl DeviceLookup) -> Result<(), String> {
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
        Commands::List => list(&lookup),
        Commands::Reset(args) => reset_keyboard(args, &lookup),
    }
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

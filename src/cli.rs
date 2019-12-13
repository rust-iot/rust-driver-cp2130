
use std::num::ParseIntError;

extern crate structopt;
use structopt::StructOpt;

#[macro_use] extern crate log;
extern crate simplelog;
use simplelog::{TermLogger, LevelFilter, TerminalMode};

extern crate driver_cp2130;
use driver_cp2130::manager::Manager;
use driver_cp2130::device::{Cp2130, GpioMode, GpioLevel};

extern crate hex;

#[derive(Debug, StructOpt)]
#[structopt(name = "cp2130-util")]
/// CP2130 Utility
pub struct Options {

    #[structopt(subcommand)]
    pub command: Command,

    #[structopt(long, default_value="0")]
    /// Device index (to select from multiple devices)
    pub index: usize,

    #[structopt(long, default_value="10c4", parse(try_from_str=parse_hex))]
    /// Device Vendor ID (VID) in hex
    pub vid: u16,

    #[structopt(long, default_value="87a0", parse(try_from_str=parse_hex))]
    /// Device Product ID (PID) in hex
    pub pid: u16,


    #[structopt(long = "log-level", default_value="debug")]
    /// Enable verbose logging
    pub level: LevelFilter,
}

#[derive(Debug, StructOpt)]
pub enum Command {
    /// Fetch the chip version
    Version,
    /// Set a GPIO output
    SetOutput {
        #[structopt(long, default_value="6")]
        /// GPIO pin index
        pin: u8,

        #[structopt(long, default_value="push-pull")]
        /// GPIO pin mode to set (input, open drain, push-pull)
        mode: GpioMode,

        #[structopt(default_value="high")]
        /// GPIO pin state (high, low)
        state: GpioLevel,
    },
    /// Read a GPIO input
    GetInput {
        #[structopt(long, default_value="6")]
        /// GPIO pin index
        pin: u8,

        #[structopt(long)]
        /// GPIO pin mode to set
        mode: Option<GpioMode>,
    },
    SpiTransfer {
        #[structopt(parse(try_from_str=parse_hex_str))]
        /// Data to transfer out
        data: String,

        #[structopt(long, default_value="6")]
        /// SPI CS pin index
        cs_pin: u8,
    }
}

fn parse_hex(src: &str) -> Result<u16, ParseIntError> {
    u16::from_str_radix(src, 16)
}

fn parse_hex_str(src: &str) -> Result<String, hex::FromHexError> {
    hex::decode(src)
}


fn main() {
    let opts = Options::from_args();

    // Setup logging
    TermLogger::init(opts.level, simplelog::Config::default(), TerminalMode::Mixed).unwrap();

    // Create new CP2130 manager
    let mut m = Manager::new().unwrap();

    // Find matching devices
    let devices = m.devices().unwrap();

    let mut matches = vec![];

    for device in devices.iter() {
        // Fetch descriptor
        let device_desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue
        };

        trace!("Device: {:?}", device_desc);

        // Check for VID/PID match
        if device_desc.vendor_id() == opts.vid && device_desc.product_id() == opts.pid {
            matches.push((device, device_desc));
        }
    }

    debug!("Found {} matching devices", matches.len());

    if matches.len() < opts.index {
        error!("Device index ({}) exceeds number of discovered devices ({})",
            opts.index, matches.len());
        return
    }

    debug!("Connecting to device (index: {})", opts.index);
    
    let (device, descriptor) = matches.remove(opts.index);
    let mut cp2130 = Cp2130::new(device, descriptor).unwrap();

    debug!("Device connected");

    match opts.command {
        Command::Version => {
            let v = cp2130.version().unwrap();
            info!("Device version: {}", v);
        },
        Command::SetOutput{pin, mode, state} => {
            cp2130.set_gpio_mode_level(pin, mode, state).unwrap()
        },
        Command::GetInput{pin, mode} => {
            if let Some(m) = mode {
                cp2130.set_gpio_mode_level(pin, m, GpioLevel::Low).unwrap();
            }
            let v = cp2130.get_gpio_level(pin).unwrap();
            info!("Pin: {} value: {}", pin, v);
        },
        Command::SpiTransfer{data, cs_pin} => {
            cp2130.set_gpio_mode_level(pin, GpioMode::PushPull, GpioLevel::Low).unwrap();

            let cmd = data.as_bytes();
            let mut buff = vec![0u8; cmd.len()];
            cp2130.spi_write_read(data.as_bytes(), &mut buff).unwrap();

            
        }
    }



}


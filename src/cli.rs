//! CP2130 Driver CLI
//! 
//! 
//! Copyright 2019 Ryan Kurte

extern crate clap;
use clap::Parser;

#[macro_use] extern crate log;
extern crate simplelog;
use simplelog::{TermLogger, LevelFilter, TerminalMode};

use driver_cp2130::prelude::*;

extern crate embedded_hal;
use embedded_hal::spi::*;

extern crate hex;
extern crate rand;
use crate::rand::Rng;


#[derive(Debug, Parser)]
#[clap(name = "cp2130-util")]
/// CP2130 Utility
pub struct Options {

    #[clap(subcommand)]
    pub command: Command,

    #[clap(flatten)]
    pub filter: Filter,

    #[clap(flatten)]
    pub options: UsbOptions,

    #[clap(long, default_value="0")]
    /// Device index (to select from multiple devices)
    pub index: usize,

    #[clap(long = "log-level", default_value="info")]
    /// Enable verbose logging
    pub level: LevelFilter,
}

#[derive(Debug, Parser)]
pub enum Command {
    /// Fetch the chip version
    Version,
    /// Fetch chip info
    Info,
    /// Set a GPIO output
    SetOutput {
        #[clap(long, default_value="6")]
        /// GPIO pin index
        pin: u8,

        #[clap(long, default_value="push-pull")]
        /// GPIO pin mode to set (input, open drain, push-pull)
        mode: GpioMode,

        #[clap(default_value="high")]
        /// GPIO pin state (high, low)
        state: GpioLevel,
    },
    /// Read a GPIO input
    ReadInput {
        #[clap(long, default_value="6")]
        /// GPIO pin index
        pin: u8,

        #[clap(long)]
        /// GPIO pin mode to set
        mode: Option<GpioMode>,
    },
    /// Transfer (write-read) to an attached SPI device
    SpiTransfer {
        #[clap(value_parser=parse_hex_str)]
        /// Data to write (in hex)
        data: Data,

        #[clap(flatten)]
        spi_opts: SpiOpts,
    },
    /// Write to an attached SPI device
    SpiWrite {
        #[clap(value_parser=parse_hex_str)]
        /// Data to write (in hex)
        data: Data,

        #[clap(flatten)]
        spi_opts: SpiOpts,
    },
    /// Test interaction with the CP2130 device
    Test(TestOpts)
}

#[derive(Clone, Debug, PartialEq, Parser)]
pub struct SpiOpts {
    #[clap(long, default_value="0")]
    /// SPI Channel
    channel: u8,

    #[clap(long, default_value="0")]
    /// SPI CS gpio index
    cs_pin: u8,
}

#[derive(Debug, Parser)]
pub struct TestOpts {
    #[clap(long, default_value="0")]
    /// Pin for GPIO write
    write_pin: u8,

    #[clap(long, default_value="1")]
    /// Pin for GPIO read
    read_pin: u8,
}

type Data = Vec<u8>;

fn parse_hex_str(src: &str) -> Result<Vec<u8>, hex::FromHexError> {
    hex::decode(src)
}


fn main() {
    let opts = Options::parse();

    // Setup logging
    TermLogger::init(opts.level, simplelog::Config::default(), TerminalMode::Mixed).unwrap();

    // Find matching devices
    let (device, descriptor) = Manager::device(opts.filter, opts.index).unwrap();

    // Create CP2130 connection
    let mut cp2130 = Cp2130::new(device, descriptor, opts.options).unwrap();

    debug!("Device connected");

    match opts.command {
        Command::Info => {
            let i = cp2130.info();
            info!("Device info: {:?}", i);
        }
        Command::Version => {
            let v = cp2130.version().unwrap();
            info!("Device version: {}", v);
        },
        Command::SetOutput{pin, mode, state} => {
            cp2130.set_gpio_mode_level(pin, mode, state).unwrap()
        },
        Command::ReadInput{pin, mode} => {
            if let Some(m) = mode {
                cp2130.set_gpio_mode_level(pin, m, GpioLevel::Low).unwrap();
            }
            let v = cp2130.get_gpio_level(pin).unwrap();
            info!("Pin: {} value: {}", pin, v);
        },
        Command::SpiTransfer{data, spi_opts} => {
            info!("Transmit: {}", hex::encode(&data));

            let mut spi = cp2130.spi(spi_opts.channel, SpiConfig::default()).unwrap();

            cp2130.set_gpio_mode_level(spi_opts.cs_pin, GpioMode::PushPull, GpioLevel::Low).unwrap();

            let mut buff = data.clone();
            
            spi.transfer_in_place(&mut buff).unwrap();

            cp2130.set_gpio_mode_level(spi_opts.cs_pin, GpioMode::PushPull, GpioLevel::High).unwrap();

            info!("Received: {}", hex::encode(buff));
        },
        Command::SpiWrite{data, spi_opts} => {
            info!("Transmit: {}", hex::encode(&data));

            let mut spi = cp2130.spi(spi_opts.channel, SpiConfig::default()).unwrap();

            cp2130.set_gpio_mode_level(spi_opts.cs_pin, GpioMode::PushPull, GpioLevel::Low).unwrap();

            spi.write(&data).unwrap();

            cp2130.set_gpio_mode_level(spi_opts.cs_pin, GpioMode::PushPull, GpioLevel::High).unwrap();
        },
        Command::Test(opts) => {
            run_tests(&mut cp2130, &opts);
        }
    }

}


fn run_tests(cp2130: &mut Cp2130, opts: &TestOpts) {
    info!("Testing GPIO read/write");

    cp2130.set_gpio_mode_level(opts.read_pin, GpioMode::Input, GpioLevel::Low).unwrap();

    cp2130.set_gpio_mode_level(opts.write_pin, GpioMode::PushPull, GpioLevel::Low).unwrap();
    let v = cp2130.get_gpio_level(opts.read_pin).unwrap();
    if v != false {
        error!("GPIO read error");
    }

    cp2130.set_gpio_mode_level(opts.write_pin, GpioMode::PushPull, GpioLevel::High).unwrap();
    let v = cp2130.get_gpio_level(opts.read_pin).unwrap();
    if v != true {
        error!("GPIO read error");
    }

    info!("GPIO read/write okay");


    info!("Testing SPI write (short)");

    let mut rng = rand::thread_rng();
    let data: Vec<u8> = (0..34).map(|_| rng.gen() ).collect();

    cp2130.spi_write(&data).unwrap();

    info!("SPI write (short) okay");


    info!("Testing SPI write (long)");

    let mut rng = rand::thread_rng();
    let data: Vec<u8> = (0..300).map(|_| rng.gen() ).collect();

    cp2130.spi_write(&data).unwrap();

    info!("SPI write (long) okay");


    info!("Testing SPI transfer (short)");

    let mut rng = rand::thread_rng();
    let data: Vec<u8> = (0..34).map(|_| rng.gen() ).collect();
    let mut buff = vec![0u8; data.len()];

    cp2130.spi_write_read(&data, &mut buff).unwrap();

    if &data != &buff {
        error!("SPI transfer (short) error ({:?} vs. {:?})", data, buff);
    }

    info!("SPI transfer (short) okay");


    info!("Testing SPI transfer (long)");

    let mut rng = rand::thread_rng();
    let data: Vec<u8> = (0..300).map(|_| rng.gen() ).collect();
    let mut buff = vec![0u8; data.len()];

    cp2130.spi_write_read(&data, &mut buff).unwrap();

    if &data != &buff {
        error!("SPI transfer (long) error ({:?} vs. {:?})", data, buff);
    }

    info!("SPI transfer (long) okay");

}

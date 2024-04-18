//! CP2130 Driver
//! 
//! 
//! Copyright 2019 Ryan Kurte

use std::{sync::{Arc, Mutex}, time::{Instant, Duration}};

pub use embedded_hal::spi::{Mode as SpiMode};
use rusb::{Device as UsbDevice, Context as UsbContext, DeviceDescriptor};

pub mod device;
pub mod manager;
pub mod prelude;

pub use crate::device::{UsbOptions, GpioMode, GpioLevel, SpiConfig, SpiClock};
use crate::device::*;


#[derive(Debug, thiserror::Error)]
pub enum Error {
//    Io(IoError),
    #[error("USB error: {0}")]
    Usb(rusb::Error),

    #[error("No matching endpoint languages found")]
    NoLanguages,

    #[error("No valid endpoint configuration found")]
    Configurations,
    #[error("No matching endpoint found")]
    Endpoint,
    #[error("GPIO pin already in use")]
    GpioInUse,
    #[error("Invalid SPI index")]
    InvalidIndex,
    #[error("Invalid SPI baud rate")]
    InvalidBaud,
}

impl From<rusb::Error> for Error {
    fn from(e: rusb::Error) -> Self {
        Error::Usb(e)
    }
}



/// CP2130 provides methods to interact with the device, as well as create new spi and gpio connectors.
pub struct Cp2130 {
    inner: Arc<Mutex<Inner>>,
    info: Info,
}

/// Device trait provides methods directly on the CP2130
pub trait Device {
    /// Read from the SPI device
    fn spi_read(&self, buff: &mut [u8]) -> Result<usize, Error>;
    
    /// Write to the SPI device
    fn spi_write(&self, buff: &[u8]) -> Result<(), Error>;

    // Transfer (write-read) to and from the SPI device
    fn spi_write_read(&self, buff_out: &[u8], buff_in: &mut [u8]) -> Result<usize, Error>;
    
    /// Fetch the CP2130 chip version
    fn version(&self) -> Result<u16, Error> ;

    /// Set the mode and level for a given GPIO pin
    fn set_gpio_mode_level(&self, pin: u8, mode: GpioMode, level: GpioLevel) -> Result<(), Error>;
    
    /// Fetch the values for all GPIO pins
    fn get_gpio_values(&self) -> Result<GpioLevels, Error>;
    
    /// Fetch the value for a given GPIO pin
    fn get_gpio_level(&self, pin: u8) -> Result<bool, Error>;
}

impl Cp2130 {
    /// Create a new CP2130 instance from a libusb device and descriptor
    pub fn new(device: UsbDevice<UsbContext>, descriptor: DeviceDescriptor, options: UsbOptions) -> Result<Self, Error> {
        
        // Connect to device
        let (inner, info) = Inner::new(device, descriptor, options)?;
        let inner = Arc::new(Mutex::new(inner));

        // Create wrapper object
        Ok(Self{info, inner})
    }

    /// Fetch information for the connected device
    pub fn info(&self) -> Info {
        self.info.clone()
    }

    pub fn reset(&self) -> Result<(), Error> {
        self.inner.lock().unwrap().reset()
    }

    /// Create an SPI connector with an optional CS pin
    pub fn spi(&self, channel: u8, config: SpiConfig, cs_pin: Option<u8>) -> Result<Spi, Error> {
        let mut inner = self.inner.lock().unwrap();

        // Configure CS pin if provided
        if let Some(cs) = cs_pin {
            inner.set_gpio_mode_level(cs, GpioMode::PushPull, GpioLevel::High)?;
        }

        // Configure SPI
        inner.spi_configure(channel, config)?;

        Ok(Spi{inner: self.inner.clone(), _channel: channel, cs: cs_pin})
    }

    /// Create a GPIO OutputPin
    pub fn gpio_out(&self, index: u8, mode: GpioMode, level: GpioLevel) -> Result<OutputPin, Error> {
        let mut inner = self.inner.lock().unwrap();

        if inner.gpio_allocated[index as usize] {
            return Err(Error::GpioInUse)
        }

        inner.set_gpio_mode_level(index, mode, level)?;
        inner.gpio_allocated[index as usize] = true;

        Ok(OutputPin{index, mode, inner: self.inner.clone()})
    }

    /// Create a GPIO InputPin
    pub fn gpio_in(&self, index: u8) -> Result<InputPin, Error> {
        let mut inner = self.inner.lock().unwrap();

        if inner.gpio_allocated[index as usize] {
            return Err(Error::GpioInUse)
        }

        inner.set_gpio_mode_level(index, GpioMode::Input, GpioLevel::Low)?;
        inner.gpio_allocated[index as usize] = true;

        Ok(InputPin{index, inner: self.inner.clone()})
    }

}

/// Underlying device functions
impl Device for Cp2130 {
    fn spi_read(&self, buff: &mut [u8]) -> Result<usize, Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.spi_read(buff)
    }

    fn spi_write(&self, buff: &[u8]) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.spi_write(buff)
    }

    fn spi_write_read(&self, buff_out: &[u8], buff_in: &mut [u8]) -> Result<usize, Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.spi_write_read(buff_out, buff_in)
    }

    fn version(&self) -> Result<u16, Error>  {
        let mut inner = self.inner.lock().unwrap();
        inner.version()
    }

    fn set_gpio_mode_level(&self, pin: u8, mode: GpioMode, level: GpioLevel) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.set_gpio_mode_level(pin, mode, level)
    }

    fn get_gpio_values(&self) -> Result<GpioLevels, Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.get_gpio_values()
    }

    fn get_gpio_level(&self, pin: u8) -> Result<bool, Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.get_gpio_level(pin)
    }
}

/// Spi object implements embedded-hal SPI traits for the CP2130
pub struct Spi {
    // TODO: use channel configuration
    _channel: u8,
    // Handle for device singleton
    inner: Arc<Mutex<Inner>>,
    // CS pin index
    cs: Option<u8>,
}

use embedded_hal::spi::Operation as SpiOp;

impl embedded_hal::spi::SpiDevice<u8> for Spi {

    fn transaction(&mut self, operations: &mut [SpiOp<'_, u8>]) -> Result<(), Self::Error> {
        let mut i = self.inner.lock().unwrap();

        // Assert CS if available
        if let Some(cs) = self.cs {
            i.set_gpio_mode_level(cs, GpioMode::PushPull, GpioLevel::Low)?;
        }

        for o in operations {
            // Run operation and collect errors
            let err = match o {
                SpiOp::Write(w) => {
                    i.spi_write(w).err()
                },
                SpiOp::Transfer(r, w) => { 
                    i.spi_write_read(w, r).err()
                },
                SpiOp::TransferInPlace(b) => {
                    let out = b.to_vec();
                    i.spi_write_read(&out, b).err()
                },
                SpiOp::Read(r) => {
                    let out = vec![0u8; r.len()];
                    i.spi_write_read(&out, r).err()
                },
                SpiOp::DelayNs(ns) => {
                    let now = Instant::now();
                    while now.elapsed() < Duration::from_nanos(*ns as u64) {}
                    None
                }
            };

            // Check for errors
            if let Some(e) = err {
                // Deassert CS on failure
                if let Some(cs) = self.cs {
                    i.set_gpio_mode_level(cs, GpioMode::PushPull, GpioLevel::High)?;
                }

                // Return error
                return Err(e)
            }
        }

        // Clear CS if enabled
        if let Some(cs) = self.cs {
            i.set_gpio_mode_level(cs, GpioMode::PushPull, GpioLevel::Low)?;
        }

        Ok(())
    }
}


impl embedded_hal::spi::ErrorType for Spi {
    type Error = Error;
}

impl embedded_hal::spi::Error for Error {
    fn kind(&self) -> embedded_hal::spi::ErrorKind {
        embedded_hal::spi::ErrorKind::Other
    }
}
/// InputPin object implements embedded-hal InputPin traits for the CP2130
pub struct InputPin {
    index: u8,
    inner: Arc<Mutex<Inner>>,
}

impl  embedded_hal::digital::InputPin for InputPin {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        self.inner.lock().unwrap().get_gpio_level(self.index)
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        let v = self.is_high()?;
        Ok(!v)
    }
}

impl embedded_hal::digital::ErrorType for InputPin {
    type Error = Error;
}

impl embedded_hal::digital::Error for Error {
    fn kind(&self) -> embedded_hal::digital::ErrorKind {
        embedded_hal::digital::ErrorKind::Other
    }
}

/// OutputPin object implements embedded-hal OutputPin traits for the CP2130
pub struct OutputPin {
    index: u8,
    mode: GpioMode,
    inner: Arc<Mutex<Inner>>,
}

impl embedded_hal::digital::OutputPin for OutputPin {
    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.inner.lock().unwrap().set_gpio_mode_level(self.index, self.mode, GpioLevel::High)
    }

    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.inner.lock().unwrap().set_gpio_mode_level(self.index, self.mode, GpioLevel::Low)
    }
}


impl embedded_hal::digital::ErrorType for OutputPin {
    type Error = Error;
}
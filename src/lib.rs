//! CP2130 Driver
//! 
//! 
//! Copyright 2019 Ryan Kurte

use std::sync::{Arc, Mutex};

#[macro_use]
extern crate log;

#[macro_use]
extern crate bitflags;
extern crate byteorder;

extern crate embedded_hal;
extern crate libusb;
use libusb::{Device as UsbDevice, DeviceDescriptor};

pub mod device;
pub use crate::device::{Device, GpioMode, GpioLevel};

use crate::device::*;

pub mod manager;

#[derive(Debug)]
pub enum Error {
//    Io(IoError),
    Usb(libusb::Error),
    NoLanguages,
    Configurations,
    Endpoint,
    GpioInUse,
}

impl From<libusb::Error> for Error {
    fn from(e: libusb::Error) -> Self {
        Error::Usb(e)
    }
}

/// CP2130 provides methods to interact with the device, as well as create new spi and gpio connectors.
pub struct Cp2130<'a> {
    inner: Arc<Mutex<Inner<'a>>>,
    info: Info,
    gpio_allocated: [bool; 11],
}

impl <'a> Cp2130<'a> {
    /// Create a new CP2130 instance from a libusb device and descriptor
    pub fn new(device: UsbDevice<'a>, descriptor: DeviceDescriptor) -> Result<Self, Error> {
        
        // Connect to device
        let (inner, info) = Inner::new(device, descriptor)?;
        let inner = Arc::new(Mutex::new(inner));

        // Create wrapper object
        Ok(Self{info, inner, gpio_allocated: [false; 11]})
    }

    /// Fetch information for the connected device
    pub fn info(&self) -> Info {
        self.info.clone()
    }

    /// Create an SPI connector
    pub fn spi(&'a mut self) -> Spi<'a> {
        Spi{inner: self.inner.clone()}
    }

    /// Create a GPIO OutputPin
    pub fn gpio_out(&'a mut self, index: u8, mode: GpioMode, level: GpioLevel) -> Result<OutputPin<'a>, Error> {
        if self.gpio_allocated[index as usize] {
            return Err(Error::GpioInUse)
        }

        self.set_gpio_mode_level(index, mode, level)?;

        self.gpio_allocated[index as usize] = true;

        Ok(OutputPin{index, mode, inner: self.inner.clone()})
    }

    /// Create a GPIO InputPin
    pub fn gpio_in(&'a mut self, index: u8) -> Result<InputPin<'a>, Error> {
        if self.gpio_allocated[index as usize] {
            return Err(Error::GpioInUse)
        }

        self.set_gpio_mode_level(index, GpioMode::Input, GpioLevel::Low)?;

        self.gpio_allocated[index as usize] = true;

        Ok(InputPin{index, inner: self.inner.clone()})
    }

}

/// Underlying device functions
impl <'a> Device for Cp2130<'a> {
    fn spi_read(&mut self, buff: &mut [u8]) -> Result<usize, Error> {
        self.inner.lock().unwrap().spi_read(buff)
    }

    fn spi_write(&mut self, buff: &[u8]) -> Result<(), Error> {
        self.inner.lock().unwrap().spi_write(buff)
    }

    fn spi_write_read(&mut self, buff_out: &[u8], buff_in: &mut [u8]) -> Result<usize, Error> {
        self.inner.lock().unwrap().spi_write_read(buff_out, buff_in)
    }

    fn version(&mut self) -> Result<u16, Error>  {
        self.inner.lock().unwrap().version()
    }

    fn set_gpio_mode_level(&mut self, pin: u8, mode: GpioMode, level: GpioLevel) -> Result<(), Error> {
        self.inner.lock().unwrap().set_gpio_mode_level(pin, mode, level)
    }

    fn get_gpio_values(&mut self) -> Result<GpioLevels, Error> {
        self.inner.lock().unwrap().get_gpio_values()
    }

    fn get_gpio_level(&mut self, pin: u8) -> Result<bool, Error> {
        self.inner.lock().unwrap().get_gpio_level(pin)
    }
}

/// Spi object implements embedded-hal SPI traits for the CP2130
pub struct Spi<'a> {
    inner: Arc<Mutex<Inner<'a>>>,
}

use embedded_hal::blocking::spi::{Write, Transfer};

impl <'a> Transfer<u8> for Spi<'a> {
    type Error = Error;

    fn transfer<'w>(&mut self, words: &'w mut [u8] ) -> Result<&'w [u8], Self::Error> {
        let out = words.to_vec();
        let _n = self.inner.lock().unwrap().spi_write_read(&out, words)?;
        Ok(words)
    }
}

impl <'a> Write<u8> for Spi<'a> {
    type Error = Error;

    fn write(&mut self, words: &[u8] ) -> Result<(), Self::Error> {
        let _n = self.inner.lock().unwrap().spi_write(words)?;
        Ok(())
    }
}

/// InputPin object implements embedded-hal InputPin traits for the CP2130
pub struct InputPin<'a> {
    index: u8,
    inner: Arc<Mutex<Inner<'a>>>,
}

impl <'a> embedded_hal::digital::v2::InputPin for InputPin<'a> {
    type Error = Error;

    fn is_high(&self) -> Result<bool, Self::Error> {
        self.inner.lock().unwrap().get_gpio_level(self.index)
    }

    fn is_low(&self) -> Result<bool, Self::Error> {
        let v = self.is_high()?;
        Ok(!v)
    }
}

/// OutputPin object implements embedded-hal OutputPin traits for the CP2130
pub struct OutputPin<'a> {
    index: u8,
    mode: GpioMode,
    inner: Arc<Mutex<Inner<'a>>>,
}

impl <'a> embedded_hal::digital::v2::OutputPin for OutputPin<'a> {
    type Error = Error;

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.inner.lock().unwrap().set_gpio_mode_level(self.index, self.mode, GpioLevel::High)
    }

    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.inner.lock().unwrap().set_gpio_mode_level(self.index, self.mode, GpioLevel::Low)
    }
}

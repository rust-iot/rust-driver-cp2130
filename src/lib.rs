
use std::io::Error as IoError;
use std::time::Duration;

#[macro_use]
extern crate log;

extern crate bitflags;

extern crate embedded_hal;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use embedded_hal::blocking::spi::{Transfer, Write};

extern crate libusb;
use libusb::{Device, DeviceDescriptor, DeviceHandle, DeviceList};

pub mod device;
pub mod manager;


pub enum Error {
    Io(IoError),
    Usb(libusb::Error),
    NoLanguages,
}

impl From<libusb::Error> for Error {
    fn from(e: libusb::Error) -> Self {
        Error::Usb(e)
    }
}




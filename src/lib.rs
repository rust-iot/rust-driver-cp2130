
use std::io::Error as IoError;

#[macro_use]
extern crate log;

extern crate bitflags;
extern crate embedded_hal;
extern crate libusb;


pub mod device;
pub mod manager;

pub enum Error {
    Io(IoError),
    Usb(libusb::Error),
    NoLanguages,
    Configurations,
    Endpoint,
}

impl From<libusb::Error> for Error {
    fn from(e: libusb::Error) -> Self {
        Error::Usb(e)
    }
}




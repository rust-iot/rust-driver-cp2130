
use std::time::Duration;

use embedded_hal::digital::v2::{InputPin, OutputPin};
use embedded_hal::blocking::spi::{Transfer, Write};

use libusb::{Device, DeviceDescriptor, DeviceHandle, DeviceList};

use crate::Error;


/// Manager object maintains libusb context and provides
/// methods for connecting to matching devices
pub struct Manager {
    context: libusb::Context,
}

impl Manager {
    /// Initialise the CP2130 manager (and underlying libusb context)
    /// This must be kept in scope until all CP2130 instances are disposed of
    pub fn init() -> Result<Manager, Error> {
        // Attempt to initialise context
        let context = match libusb::Context::new() {
            Ok(v) => v,
            Err(e) => {
                error!("Initialising libusb context: {}", e);
                return Err(Error::Usb(e))
            }
        };

        Ok(Manager{context})
    }

    /// Fetch a libusb device list (for filtering and connecting to devices)
    pub fn devices<'b>(&'b mut self) -> Result<DeviceList<'b>, Error> {
        // Attempt to fetch device list
        let devices = match self.context.devices() {
            Ok(v) => v,
            Err(e) => {
                error!("Fetching devices: {}", e);
                return Err(Error::Usb(e))
            }
        };

        Ok(devices)
    }
}

//! CP2130 Driver Device Manager
//! 
//! 
//! Copyright 2019 Ryan Kurte

use std::num::ParseIntError;

use libusb::{Device as UsbDevice, DeviceList, DeviceDescriptor};
use structopt::StructOpt;

use crate::{Error};
use crate::device::{VID, PID};

/// Manager object maintains libusb context and provides
/// methods for connecting to matching devices
pub struct Manager {
    context: libusb::Context,
}

#[derive(Debug, Clone, PartialEq, StructOpt)]
pub struct Filter {
    #[structopt(long, default_value="10c4", parse(try_from_str=parse_hex))]
    /// Device Vendor ID (VID) in hex
    pub vid: u16,

    #[structopt(long, default_value="87a0", parse(try_from_str=parse_hex))]
    /// Device Product ID (PID) in hex
    pub pid: u16,
}

fn parse_hex(src: &str) -> Result<u16, ParseIntError> {
    u16::from_str_radix(src, 16)
}

impl Default for Filter {
    fn default() -> Self {
        Filter{vid: VID, pid: PID}
    }
}

impl Manager {
    /// Initialise the CP2130 manager (and underlying libusb context)
    /// This must be kept in scope until all CP2130 instances are disposed of
    pub fn new() -> Result<Manager, Error> {
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

    pub fn devices_filtered<'b>(&'b mut self, filter: Filter) -> Result<Vec<(UsbDevice, DeviceDescriptor)>, Error> {
        let devices = self.devices()?;

        let mut matches = vec![];

        for device in devices.iter() {
            // Fetch descriptor
            let device_desc = match device.device_descriptor() {
                Ok(d) => d,
                Err(_) => continue
            };
    
            trace!("Device: {:?}", device_desc);
    
            // Check for VID/PID match
            if device_desc.vendor_id() == filter.vid && device_desc.product_id() == filter.pid {
                matches.push((device, device_desc));
            }
        }
    
        debug!("Found {} matching devices", matches.len());
    
        Ok(matches)
    }

}

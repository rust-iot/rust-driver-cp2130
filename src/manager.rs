//! CP2130 Driver Device Manager
//! 
//! 
//! Copyright 2019 Ryan Kurte

pub use libusb::{Device as UsbDevice, DeviceList, DeviceDescriptor};

#[cfg(feature = "structopt")]
use std::num::ParseIntError;

#[cfg(feature = "structopt")]
use structopt::StructOpt;

use crate::{Error};
use crate::device::{VID, PID};

lazy_static!{
    // LibUSB context created automagically
    static ref CONTEXT: libusb::Context = {
        libusb::Context::new().unwrap()
    };
}

/// Manager object maintains libusb context and provides
/// methods for connecting to matching devices
pub struct Manager {
    //context: libusb::Context,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "structopt", derive(StructOpt))]
pub struct Filter {
    #[cfg_attr(feature = "structopt", structopt(long, default_value="10c4", parse(try_from_str=parse_hex)))]
    /// Device Vendor ID (VID) in hex
    pub vid: u16,

    #[cfg_attr(feature = "structopt", structopt(long, default_value="87a0", parse(try_from_str=parse_hex)))]
    /// Device Product ID (PID) in hex
    pub pid: u16,
}

#[cfg(feature = "structopt")]
fn parse_hex(src: &str) -> Result<u16, ParseIntError> {
    u16::from_str_radix(src, 16)
}

impl Default for Filter {
    fn default() -> Self {
        Filter{vid: VID, pid: PID}
    }
}

impl Manager {
    /// Fetch a libusb device list (for filtering and connecting to devices)
    pub fn devices() -> Result<DeviceList<'static>, Error> {
        // Attempt to fetch device list
        let devices = match CONTEXT.devices() {
            Ok(v) => v,
            Err(e) => {
                error!("Fetching devices: {}", e);
                return Err(Error::Usb(e))
            }
        };

        Ok(devices)
    }

    pub fn devices_filtered(filter: Filter) -> Result<Vec<(UsbDevice<'static>, DeviceDescriptor)>, Error> {
        let devices = Self::devices()?;

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

    pub fn device(filter: Filter, index: usize) -> Result<(UsbDevice<'static>, DeviceDescriptor), Error> {
        // Find matching devices
        let mut matches = Self::devices_filtered(filter)?;

        // Check index is valid
        if matches.len() < index {
            error!("Device index ({}) exceeds number of discovered devices ({})",
                index, matches.len());
            return Err(Error::InvalidIndex)
        }

        // Return match
        Ok(matches.remove(index))
    }
}

//! CP2130 Driver Device Manager
//!
//!
//! Copyright 2019 Ryan Kurte

pub use rusb::{
    Context as UsbContext, Device as UsbDevice, DeviceDescriptor, DeviceList, UsbContext as _,
};

#[cfg(feature = "clap")]
use std::num::ParseIntError;

#[cfg(feature = "clap")]
use clap::Parser;

use log::{debug, error, trace};

use crate::device::{PID, VID};
use crate::Error;

lazy_static::lazy_static! {
    // LibUSB context created automagically
    static ref CONTEXT: UsbContext = {
        UsbContext::new().unwrap()
    };
}

/// Manager object maintains libusb context and provides
/// methods for connecting to matching devices
pub struct Manager {
    //context: rusb::Context,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "clap", derive(Parser))]
pub struct Filter {
    #[cfg_attr(feature = "clap", clap(long, default_value="10c4", value_parser=parse_hex))]
    /// Device Vendor ID (VID) in hex
    pub vid: u16,

    #[cfg_attr(feature = "clap", clap(long, default_value="87a0", value_parser=parse_hex))]
    /// Device Product ID (PID) in hex
    pub pid: u16,
}

#[cfg(feature = "clap")]
fn parse_hex(src: &str) -> Result<u16, ParseIntError> {
    u16::from_str_radix(src, 16)
}

impl Default for Filter {
    fn default() -> Self {
        Filter { vid: VID, pid: PID }
    }
}

impl Manager {
    /// Fetch a libusb device list (for filtering and connecting to devices)
    pub fn devices() -> Result<DeviceList<UsbContext>, Error> {
        debug!("Fetching available USB devices");

        // Attempt to fetch device list
        let devices = match CONTEXT.devices() {
            Ok(v) => v,
            Err(e) => {
                error!("Fetching devices: {}", e);
                return Err(Error::Usb(e));
            }
        };

        Ok(devices)
    }

    pub fn devices_filtered(
        filter: Filter,
    ) -> Result<Vec<(UsbDevice<UsbContext>, DeviceDescriptor)>, Error> {
        let devices = Self::devices()?;

        let mut matches = vec![];

        for device in devices.iter() {
            // Fetch descriptor
            let device_desc = match device.device_descriptor() {
                Ok(d) => d,
                Err(_) => continue,
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

    pub fn device(
        filter: Filter,
        index: usize,
    ) -> Result<(UsbDevice<UsbContext>, DeviceDescriptor), Error> {
        // Find matching devices
        let mut matches = Self::devices_filtered(filter)?;

        // Check index is valid
        if matches.len() < index || matches.len() == 0 {
            error!(
                "Device index ({}) exceeds number of discovered devices ({})",
                index,
                matches.len()
            );
            return Err(Error::InvalidIndex);
        }

        // Return match
        Ok(matches.remove(index))
    }
}

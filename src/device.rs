
use std::time::Duration;
use std::str::FromStr;


use byteorder::{LE, BE, ByteOrder, ReadBytesExt, WriteBytesExt};

use embedded_hal::digital::v2::{InputPin, OutputPin};
use embedded_hal::blocking::spi::{Transfer, Write};

use libusb::{Device, DeviceDescriptor, DeviceHandle, Direction, TransferType};

use crate::Error;

pub struct Cp2130<'a> {
    _device: Device<'a>,
    handle: DeviceHandle<'a>,
    info: Info,
    endpoints: Endpoints,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    manufacturer: String,
    product: String,
    serial: String,
}

#[derive(Debug)]
pub struct Endpoints {
    control: Endpoint,
    read: Endpoint,
    write: Endpoint,
}

#[derive(Debug, PartialEq, Clone)]
struct Endpoint {
    config: u8,
    iface: u8,
    setting: u8,
    address: u8
}

#[derive(Debug, PartialEq, Clone)]
pub enum Commands {
    GetClockDivider = 0x46,
    GetEventCounter = 0x44,
    GetFullThreshold = 0x34,
    GetGpioChipSelect = 0x24,
    GetGpioModeAndLevel = 0x22,
    GetGpioValues = 0x20,
    GetRtrState = 0x36,
    GetSpiWord = 0x30,
    GetSpiDelay = 0x32,
    GetReadOnlyVersion = 0x11,
    ResetDevice = 0x10,
    SetClockDivider = 0x47,
    SetEventCOunter = 0x45,
    SetFullThreshold = 0x35,
    SetGpioChipSelect = 0x25,
    SetGpioModeAndLevel = 0x23,
    SetGpioValues = 0x21,
    SetRtrStop = 0x37,
    SetSpiWord = 0x31,
    SetSpiDelay = 0x33,
}

pub const VID: u16 = 0x10c4;
pub const PID: u16 = 0x87a0;

bitflags!(
    struct RequestType: u8 {
        const HOST_TO_DEVICE = 0b0000_0000;
        const DEVICE_TO_HOST = 0b1000_0000;

        const TYPE_STANDARD = 0b0000_0000;
        const TYPE_CLASS =    0b0010_0000;
        const TYPE_VENDOR =   0b0100_0000;

        const RECIPIENT_DEVICE =    0b0000_0000;
        const RECIPIENT_INTERFACE = 0b0000_0001;
        const RECIPIENT_ENDPOINT =  0b0000_0010;
        const RECIPIENT_OTHER =     0b0000_0011;
    }
);


bitflags!(
    /// Gpio PIN masks for multiple pin operations
    /// The endianness of this varies depending on where it is used...
    pub struct GpioLevels: u16 {
        const GPIO_10 = (1 << 14);
        const GPIO_9  = (1 << 13);
        const GPIO_8  = (1 << 12);
        const GPIO_7  = (1 << 11);
        const GPIO_6  = (1 << 10);
        const GPIO_5  = (1 << 8);

        const GPIO_4  = (1 << 7);
        const GPIO_3  = (1 << 6);
        const GPIO_2  = (1 << 5);
        const GPIO_1  = (1 << 4);
        const GPIO_0  = (1 << 3);
    }
);

#[derive(Debug, PartialEq, Clone)]
pub enum GpioMode {
    Input = 0x00,
    OpenDrain = 0x01,
    PushPull = 0x02,
}

impl FromStr for GpioMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "input" => Ok(Self::Input),
            "open-drain" => Ok(Self::OpenDrain),
            "push-pull" => Ok(Self::PushPull),
            _ => Err(format!("Unrecognised GPIO mode, try 'input', 'open-drain', or 'push-pull'")),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum GpioLevel {
    Low = 0x00,
    High = 0x01,
}

impl FromStr for GpioLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1" | "true" | "high" => Ok(Self::High),
            "0" | "false" | "low" => Ok(Self::Low),
            _ => Err(format!("Unrecognised GPIO level, try 'high' or 'low'")),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum TransferCommand {
    Read        = 0x00,
    Write       = 0x01,
    WriteRead   = 0x02,
    ReadWithRTR = 0x04,
}


impl <'a> Cp2130<'a> {
    /// Create a new CP2130 instance from a libusb device and descriptor
    pub fn new(device: Device<'a>, descriptor: DeviceDescriptor) -> Result<Self, Error> {
        let timeout = Duration::from_millis(200);
        
        // Fetch device handle
        let mut handle = match device.open() {
            Ok(v) => v,
            Err(e) => {
                error!("Opening device: {}", e);
                return Err(Error::Usb(e))
            }
        };


        // Reset device
        handle.reset()?;

        // Fetch base configuration
        let languages = handle.read_languages(timeout)?;
        let active_config = handle.active_configuration()?;

        trace!("Active configuration: {}", active_config);
        trace!("Languages: {:?}", languages);

        // Check a language is available
        if languages.len() == 0 {
            return Err(Error::NoLanguages)
        }

        // Fetch information
        let language = languages[0];
        let manufacturer = handle.read_manufacturer_string(language, &descriptor, timeout)?;
        let product = handle.read_product_string(language, &descriptor, timeout)?;
        let serial = handle.read_serial_number_string(language, &descriptor, timeout)?;
        let info = Info{manufacturer, product, serial};

        // Check at least one configuration exists
        if descriptor.num_configurations() != 1 {
            error!("Unexpected number of configurations");
            return Err(Error::Configurations)
        }

        // Connect to endpoints
        let config_desc = device.config_descriptor(0)?;
        
        let (mut control, mut write, mut read) = (None, None, None);

        for interface in config_desc.interfaces() {
            for interface_desc in interface.descriptors() {
                for endpoint_desc in interface_desc.endpoint_descriptors() {

                    // Create an endpoint container
                    let e = Endpoint {
                        config: config_desc.number(),
                        iface: interface_desc.interface_number(),
                        setting: interface_desc.setting_number(),
                        address: endpoint_desc.address(),
                    };

                    trace!("Endpoint: {:?}", e);

                    // Find the relevant endpoints
                    match (endpoint_desc.transfer_type(), endpoint_desc.direction()) {
                        (TransferType::Control, _) => control = Some(e),
                        (TransferType::Bulk, Direction::In) => read = Some(e),
                        (TransferType::Bulk, Direction::Out) => write = Some(e),
                        (_, _) => continue,
                    }
                }
            }
        }

        // Configure endpoints
        let control = Endpoint {
            config: 1,
            iface: 0,
            setting: 0,
            address: 0,
        };
        //control.configure(&mut handle)?;

        // Detach kernel driver if required
        if handle.kernel_driver_active(control.iface)? {
            debug!("Detaching kernel driver");
            handle.detach_kernel_driver(control.iface)?;
            // TODO: track this and re-enable on closing?
        }

        let write = match write {
            Some(c) => c,
            None => {
                error!("No write endpoint found");
                return Err(Error::Endpoint)
            }
        };
        handle.set_active_configuration(write.config)?;

        let read = match read {
            Some(c) => c,
            None => {
                error!("No read endpoint found");
                return Err(Error::Endpoint)
            }
        };
        handle.set_active_configuration(read.config)?;

        let endpoints = Endpoints{control, write, read};

        // Create device
        Ok(Self{_device: device, handle, info, endpoints})
    }

    /// Fetch information for the connected device
    pub fn info(&self) -> Info {
        self.info.clone()
    }

    pub fn spi_read(&mut self, buff: &mut [u8]) -> Result<usize, Error> {
        let mut cmd = [0u8; 8];
        cmd[2] = TransferCommand::Read as u8;
        LE::write_u32(&mut cmd[4..], buff.len() as u32);


        trace!("SPI read (cmd: {:?})", cmd);

        self.handle.write_bulk(
            self.endpoints.write.address,
            &cmd,
            Duration::from_millis(200),
        )?;

        // TODO: loop for > 64-byte packets
        let mut index = 0;

        while index < buff.len() {
            let remainder = if buff.len() > index + 64 {
                64
            } else {
                buff.len() - index
            };

            debug!("SPI read (i: {}, rem: {})", index, remainder);

            let n = self.handle.read_bulk(
                self.endpoints.read.address,
                &mut buff[index..index+remainder],
                Duration::from_millis(200),
            )?;

            index += n;
        }

        trace!("SPI read done");

        Ok(index)
    }

    pub fn spi_write(&mut self, buff: &[u8]) -> Result<(), Error> {

        let mut cmd = vec![0u8; buff.len() + 8];

        cmd[2] = TransferCommand::Write as u8;
        LE::write_u32(&mut cmd[4..], buff.len() as u32);
        (&mut cmd[8..]).copy_from_slice(buff);

        trace!("SPI write (cmd: {:?})", cmd);

        self.handle.write_bulk(
            self.endpoints.write.address,
            &cmd,
            Duration::from_millis(200),
        )?;


        trace!("SPI write done");

        Ok(())
    }

    pub fn spi_write_read(&mut self, buff_out: &[u8], buff_in: &mut [u8]) -> Result<usize, Error> {

        let mut cmd = vec![0u8; buff_out.len() + 8];

        cmd[2] = TransferCommand::WriteRead as u8;
        LE::write_u32(&mut cmd[4..], buff_out.len() as u32);
        (&mut cmd[8..]).copy_from_slice(buff_out);

        trace!("SPI transfer (cmd: {:?})", cmd);

        self.handle.write_bulk(
            self.endpoints.write.address,
            &cmd,
            Duration::from_millis(200),
        )?;

        trace!("SPI transfer await resp");

        // TODO: loop for > 64-byte packets
        let mut index = 0;

        while index < buff_in.len() {
            let remainder = if buff_in.len() > index + 64 {
                64
            } else {
                buff_in.len() - index
            };

            trace!("SPI read (len: {}, index: {}, rem: {})", buff_in.len(), index, remainder);

            let n = self.handle.read_bulk(
                self.endpoints.read.address,
                &mut buff_in[index..index+remainder],
                Duration::from_millis(200),
            )?;

            index += n;
        }

        trace!("SPI transfer done");

        Ok(index)
    }

    /// Fetch the chip version
    pub fn version(&mut self) -> Result<u16, Error> {
        let mut buff = [0u8; 2];

        self.handle.read_control(
            (RequestType::DEVICE_TO_HOST | RequestType::TYPE_VENDOR).bits(), 
            Commands::GetReadOnlyVersion as u8,
            0, 0,
            &mut buff,
            Duration::from_millis(200)
        )?;

        let version = LE::read_u16(&buff);

        Ok(version)
    }

    pub fn set_gpio_mode_level(&mut self, pin: u8, mode: GpioMode, level: GpioLevel) -> Result<(), Error> {
        assert!(pin <= 10);
        
        let cmd = [
            pin,
            mode as u8,
            level as u8,
        ];

        self.handle.write_control(
            (RequestType::HOST_TO_DEVICE | RequestType::TYPE_VENDOR).bits(), 
            Commands::SetGpioModeAndLevel as u8,
            0, 0,
            &cmd,
            Duration::from_millis(200)
        )?;

        Ok(())
    }

    pub fn get_gpio_values(&mut self) -> Result<GpioLevels, Error> {
        let mut buff = [0u8; 2];

        self.handle.read_control(
            (RequestType::DEVICE_TO_HOST | RequestType::TYPE_VENDOR).bits(), 
            Commands::GetGpioValues as u8,
            0, 0,
            &mut buff,
            Duration::from_millis(200)
        )?;

        // Inexplicably big endian here
        let values = BE::read_u16(&buff);

        Ok(GpioLevels::from_bits_truncate(values))
    }

    pub fn get_gpio_level(&mut self, pin: u8) -> Result<bool, Error> {
        assert!(pin <= 10);

        let levels = self.get_gpio_values()?;

        let v = match pin {
            0 => levels.contains(GpioLevels::GPIO_0),
            1 => levels.contains(GpioLevels::GPIO_1),
            2 => levels.contains(GpioLevels::GPIO_2),
            3 => levels.contains(GpioLevels::GPIO_3),
            4 => levels.contains(GpioLevels::GPIO_4),
            5 => levels.contains(GpioLevels::GPIO_5),
            6 => levels.contains(GpioLevels::GPIO_6),
            7 => levels.contains(GpioLevels::GPIO_7),
            8 => levels.contains(GpioLevels::GPIO_8),
            9 => levels.contains(GpioLevels::GPIO_9),
            10 => levels.contains(GpioLevels::GPIO_10),
            _ => panic!("invalid pin {}", pin),
        };

        Ok(v)
    }

}


impl <'a> Transfer<u8> for Cp2130<'a> {
    type Error = Error;

    fn transfer<'w>(&mut self, words: &'w mut [u8] ) -> Result<&'w [u8], Self::Error> {
        let out = words.to_vec();
        let _n = self.spi_write_read(&out, words)?;
        Ok(words)
    }
}

impl <'a> Write<u8> for Cp2130<'a> {
    type Error = Error;

    fn write(&mut self, words: &[u8] ) -> Result<(), Self::Error> {
        let _n = self.spi_write(words)?;
        Ok(())
    }
}


pub struct Gpio {

}

impl InputPin for Gpio {
    type Error = Error;

    fn is_high(&self) -> Result<bool, Self::Error> {
        unimplemented!()
    }

    fn is_low(&self) -> Result<bool, Self::Error> {
        unimplemented!()
    }
}


impl OutputPin for Gpio {
    type Error = Error;

    fn set_high(&mut self) -> Result<(), Self::Error> {
        unimplemented!()
    }

    fn set_low(&mut self) -> Result<(), Self::Error> {
        unimplemented!()
    }
}

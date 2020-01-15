//! CP2130 Driver Device Definitions
//! 
//! 
//! Copyright 2019 Ryan Kurte

use std::time::Duration;
use std::str::FromStr;

use byteorder::{LE, BE, ByteOrder};

use libusb::{Device as UsbDevice, DeviceDescriptor, DeviceHandle, Direction, TransferType};

use embedded_hal::spi::{Mode as SpiMode, Phase, Polarity, MODE_0};

use crate::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    manufacturer: String,
    product: String,
    serial: String,
}


/// CP2130 command enumeration
#[derive(Debug, PartialEq, Clone, Copy)]
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

/// Default CP2130 VID
pub const VID: u16 = 0x10c4;

/// Default CP2130 PID
pub const PID: u16 = 0x87a0;

bitflags!(
    /// USB request type flags
    pub struct RequestType: u8 {
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

/// GPIO mode enumeration
#[derive(Debug, PartialEq, Clone, Copy)]
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

/// GPIO level enumeration
#[derive(Debug, PartialEq, Clone, Copy)]
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

/// Transfer command enumeration
#[derive(Debug, PartialEq, Clone)]
pub enum TransferCommand {
    Read        = 0x00,
    Write       = 0x01,
    WriteRead   = 0x02,
    ReadWithRTR = 0x04,
}

/// Inner struct contains CP2130 IO functions
/// This is used to split SPI and GPIO components
pub(crate) struct Inner<'a> {
    _device: UsbDevice<'a>,
    handle: DeviceHandle<'a>,
    endpoints: Endpoints,

    pub(crate) gpio_allocated: [bool; 11],
    spi_clock: SpiClock,
}

/// Device specific endpoints
/// TODO: given it's one device this could all be hard-coded
#[derive(Debug)]
struct Endpoints {
    control: Endpoint,
    read: Endpoint,
    write: Endpoint,
}

/// Internal endpoint representations
#[derive(Debug, PartialEq, Clone)]
struct Endpoint {
    config: u8,
    iface: u8,
    setting: u8,
    address: u8
}

impl <'a> Inner<'a> {
    /// Create a new CP2130 instance from a libusb device and descriptor
    pub fn new(device: UsbDevice<'a>, descriptor: DeviceDescriptor) -> Result<(Self, Info), Error> {
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
        
        let (mut write, mut read) = (None, None);

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

        Ok((Inner{_device: device, handle, endpoints, gpio_allocated: [false; 11], spi_clock: SpiClock::Clock12Mhz}, info))
    }
}

/// SPI clock configuration
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum SpiClock {
    Clock12Mhz,
    Clock6MHz,
    Clock3MHz,
    Clock1_5MHz,
    Clock750KHz,
    Clock375MHz,
}

impl SpiClock {
    pub fn freq(&self) -> u64 {
        match self {
            SpiClock::Clock12Mhz  => 12_000_000,
            SpiClock::Clock6MHz   => 6_000_000,
            SpiClock::Clock3MHz   => 3_000_000,
            SpiClock::Clock1_5MHz => 1_500_000,
            SpiClock::Clock750KHz => 750_000,
            SpiClock::Clock375MHz => 375_000,
        }
    }

    pub fn transfer_time(&self, len_bytes: u64) -> std::time::Duration {
        let micros = len_bytes * 8 * 1_000_000 / self.freq();
        Duration::from_micros(micros)
    }
}

/// Chip select mode
#[derive(Debug, PartialEq, Clone)]
pub enum CsMode {
    /// Auto chip select is disabled for the specified channel
    Disabled = 0x00,
    /// Auto chip select is enabled for the specified channel
    Enabled = 0x01,
    /// Auto chip select is enabled for the specified channel,
    /// all other chip selects are disabled
    Exclusive = 0x02,
}

pub const CPOL_TRAILING: u8 = (0 << 5);

bitflags!(
    /// Mask for delay configuration
    pub struct DelayMask: u8 {
        const CS_TOGGLE      = 1 << 3;
        const PRE_DEASSERT   = 1 << 2;
        const POST_ASSERT    = 1 << 1;
        const INTER_BYE      = 1 << 0;
    }
);

#[derive(Debug, PartialEq, Clone)]
pub struct SpiDelays {
    mask: DelayMask,
    pre_deassert: u8,
    post_assert: u8,
    inter_byte: u8,
}

#[derive(PartialEq, Clone)]
pub struct SpiConfig {
    pub clock: SpiClock, 
    pub spi_mode: SpiMode, 
    pub cs_mode: CsMode,
    pub cs_pin_mode: GpioMode,
    pub delays: SpiDelays,
}

impl Default for SpiConfig {
    fn default() -> Self {
        Self {
            clock: SpiClock::Clock3MHz,
            spi_mode: MODE_0,
            cs_mode: CsMode::Disabled,
            cs_pin_mode: GpioMode::PushPull,
            delays: SpiDelays {
                mask: DelayMask::empty(),
                pre_deassert: 0,
                post_assert: 0,
                inter_byte: 0,
            }
        }
    }
}



impl <'a> Inner<'a> {

    pub(crate) fn spi_configure(&mut self, channel: u8, config: SpiConfig) -> Result<(), Error> {
        debug!("Setting SPI channel: {:?} clock: {:?} cs mode: {:?}", channel, config.clock, config.cs_mode);

        // Set SPI channel configuration
        self.set_spi_word(channel, config.clock, config.spi_mode, config.cs_pin_mode)?;

        // Configure chip select
        self.set_gpio_chip_select(channel, config.cs_mode)?;

        // Configure delays
        self.set_spi_delay(channel, config.delays)?;

        Ok(())
    }

    pub(crate) fn set_spi_word(&mut self, channel: u8, clock: SpiClock, spi_mode: SpiMode, cs_pin_mode: GpioMode) -> Result<(), Error> {

        let mut flags = 0;

        if let Phase::CaptureOnSecondTransition = spi_mode.phase {
            flags |= 1 << 5;
        }

        if let Polarity::IdleHigh = spi_mode.polarity {
            flags |= 1 << 4;
        };

        if let GpioMode::PushPull = cs_pin_mode {
            flags |= 1 << 3
        }

        flags |= (clock as u8) & 0b0111;

        debug!("Set SPI word: 0x{:02x?}", flags);

        let cmd = [
            channel,
            flags
        ];

        self.handle.write_control(
            (RequestType::HOST_TO_DEVICE | RequestType::TYPE_VENDOR).bits(), 
            Commands::SetSpiWord as u8,
            0, 0,
            &cmd,
            Duration::from_millis(200)
        )?;

        self.spi_clock = clock;

        Ok(())
    }

    pub(crate) fn reset(&mut self) -> Result<(), Error> {

        self.handle.write_control(
            (RequestType::HOST_TO_DEVICE | RequestType::TYPE_VENDOR).bits(), 
            Commands::ResetDevice as u8,
            0, 0,
            &[],
            Duration::from_millis(200)
        )?;

        Ok(())
    }

    pub(crate) fn set_spi_delay(&mut self, channel: u8, delays: SpiDelays) -> Result<(), Error> {

        let cmd = [
            channel,
            delays.mask.bits(),
            delays.inter_byte,
            delays.post_assert,
            delays.pre_deassert,
        ];

        self.handle.write_control(
            (RequestType::HOST_TO_DEVICE | RequestType::TYPE_VENDOR).bits(), 
            Commands::SetSpiDelay as u8,
            0, 0,
            &cmd,
            Duration::from_millis(200)
        )?;

        Ok(())
    }

    pub(crate) fn set_gpio_chip_select(&mut self, channel: u8, cs_mode: CsMode) -> Result<(), Error> {

        let cmd = [
            channel,
            cs_mode as u8,
        ];

        self.handle.write_control(
            (RequestType::HOST_TO_DEVICE | RequestType::TYPE_VENDOR).bits(), 
            Commands::SetGpioChipSelect as u8,
            0, 0,
            &cmd,
            Duration::from_millis(200)
        )?;

        Ok(())
    }

    /// Read from the SPI device
    pub(crate) fn spi_read(&mut self, buff: &mut [u8]) -> Result<usize, Error> {
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

    /// Write to the SPI device
    pub(crate) fn spi_write(&mut self, buff: &[u8]) -> Result<(), Error> {

        let mut cmd = vec![0u8; buff.len() + 8];

        cmd[2] = TransferCommand::Write as u8;
        LE::write_u32(&mut cmd[4..], buff.len() as u32);
        (&mut cmd[8..]).copy_from_slice(buff);

        let t = self.spi_clock.transfer_time(buff.len() as u64);
        debug!("SPI write (cmd: {:?} time: {} us)", cmd, t.as_micros());

        self.handle.write_bulk(
            self.endpoints.write.address,
            &cmd,
            Duration::from_millis(200),
        )?;

        // Wait for operation to complete so we don't confuse the device
        std::thread::sleep(t);

        trace!("SPI write done");

        Ok(())
    }

    // Transfer (write-read) to and from the SPI device
    pub(crate) fn spi_write_read(&mut self, buff_out: &[u8], buff_in: &mut [u8]) -> Result<usize, Error> {

        let mut cmd = vec![0u8; buff_out.len() + 8];

        // TODO: split this into while loop so long packet writes work correctly
        // At the moment the read buffer will probably be overwritten
        cmd[2] = TransferCommand::WriteRead as u8;
        LE::write_u32(&mut cmd[4..], buff_out.len() as u32);
        (&mut cmd[8..]).copy_from_slice(buff_out);

        let total_time = self.spi_clock.transfer_time(buff_out.len() as u64);
        debug!("SPI transfer (cmd: {:?} time: {} us)", cmd, total_time.as_micros());

        self.handle.write_bulk(
            self.endpoints.write.address,
            &cmd,
            Duration::from_millis(200),
        )?;

        trace!("SPI transfer await resp");

        let mut index = 0;

        while index < buff_in.len() {
            let remainder = if buff_in.len() > index + 64 {
                64
            } else {
                buff_in.len() - index
            };

            let t = self.spi_clock.transfer_time(buff_out.len() as u64);
            
            trace!("SPI read (len: {}, index: {}, rem: {}, time: {} us)", 
                    buff_in.len(), index, remainder, t.as_micros());

            let n = self.handle.read_bulk(
                self.endpoints.read.address,
                &mut buff_in[index..index+remainder],
                Duration::from_millis(200),
            )?;

            index += n;

            // Wait for operation to complete before we continue
            std::thread::sleep(t);
        }

        trace!("SPI transfer done");

        Ok(index)
    }

    /// Fetch the CP2130 chip version
    pub(crate) fn version(&mut self) -> Result<u16, Error> {
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

    /// Set the mode and level for a given GPIO pin
    pub(crate) fn set_gpio_mode_level(&mut self, pin: u8, mode: GpioMode, level: GpioLevel) -> Result<(), Error> {
        assert!(pin <= 10);
        
        let cmd = [
            pin,
            mode as u8,
            level as u8,
        ];

        trace!("GPIO set pin: {} mode: {:?} level: {:?} (cmd: {:?})", pin, mode, level, cmd);

        self.handle.write_control(
            (RequestType::HOST_TO_DEVICE | RequestType::TYPE_VENDOR).bits(), 
            Commands::SetGpioModeAndLevel as u8,
            0, 0,
            &cmd,
            Duration::from_millis(200)
        )?;

        Ok(())
    }

    /// Fetch the values for all GPIO pins
    pub(crate) fn get_gpio_values(&mut self) -> Result<GpioLevels, Error> {
        let mut buff = [0u8; 2];

        self.handle.read_control(
            (RequestType::DEVICE_TO_HOST | RequestType::TYPE_VENDOR).bits(), 
            Commands::GetGpioValues as u8,
            0, 0,
            &mut buff,
            Duration::from_millis(200)
        )?;

        // Inexplicably big endian here
        let values = GpioLevels::from_bits_truncate(BE::read_u16(&buff));

        trace!("GPIO get pins (values: {:?})", values);

        Ok(values)
    }

    /// Fetch the value for a given GPIO pin
    pub (crate) fn get_gpio_level(&mut self, pin: u8) -> Result<bool, Error> {
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



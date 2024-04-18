//! Test ssd1306 on linux with cp2130 usb to spi bridge.

use driver_cp2130::prelude::*;

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text,
};
use linux_embedded_hal::Delay;

use embedded_hal_compat::ReverseCompat as _;
use ssd1306::{prelude::*, Ssd1306};

fn main() {
    // Find matching devices
    let (device, descriptor) = Manager::device(Filter::default(), 0).unwrap();

    // Create CP2130 connection
    let cp2130 = Cp2130::new(device, descriptor, UsbOptions::default()).unwrap();

    let spi = cp2130.spi(1, SpiConfig::default()).unwrap();

    let dc = cp2130
        .gpio_out(0, GpioMode::PushPull, GpioLevel::Low)
        .unwrap();

    let rst = cp2130
        .gpio_out(1, GpioMode::PushPull, GpioLevel::Low)
        .unwrap();

    let mut delay = Delay {};

    let interface = SPIInterfaceNoCS::new(spi.reverse(), dc.reverse());
    let mut disp = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    disp.reset(&mut rst.reverse(), &mut delay).unwrap();
    disp.init().unwrap();

    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    Text::new("Hello world!", Point::zero(), text_style)
        .draw(&mut disp)
        .unwrap();

    Text::new("Hello Rust!", Point::new(0, 16), text_style)
        .draw(&mut disp)
        .unwrap();

    disp.flush().unwrap();

    loop {}
}

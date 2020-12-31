//! Test ssd1306 on linux with cp2130 usb to spi bridge.

use driver_cp2130::prelude::*;

use embedded_graphics::{
    fonts::{Font6x8, Text},
    pixelcolor::BinaryColor,
    prelude::*,
    style::TextStyleBuilder,
};
use linux_embedded_hal::Delay;

use ssd1306::{prelude::*, Builder};

fn main() {
    // Find matching devices
    let (device, descriptor) = Manager::device(Filter::default(), 0).unwrap();

    // Create CP2130 connection
    let cp2130 = Cp2130::new(device, descriptor, UsbOptions::default()).unwrap();

    let spi = cp2130.spi(1, SpiConfig::default()).unwrap();

    let dc = cp2130
        .gpio_out(0, GpioMode::PushPull, GpioLevel::Low)
        .unwrap();

    let mut rst = cp2130
        .gpio_out(1, GpioMode::PushPull, GpioLevel::Low)
        .unwrap();

    let mut delay = Delay {};

    let mut disp: GraphicsMode<_, _> = Builder::new().connect_spi(spi, dc).into();

    disp.reset(&mut rst, &mut delay).unwrap();
    disp.init().unwrap();

    let text_style = TextStyleBuilder::new(Font6x8)
        .text_color(BinaryColor::On)
        .build();

    Text::new("Hello world!", Point::zero())
        .into_styled(text_style)
        .draw(&mut disp)
        .unwrap();

    Text::new("Hello Rust!", Point::new(0, 16))
        .into_styled(text_style)
        .draw(&mut disp)
        .unwrap();

    disp.flush().unwrap();

    loop {}
}


extern crate driver_cp2130;
use driver_cp2130::prelude::*;

#[test]
#[ignore]
fn integration() {

    // Find matching devices
    let (device, descriptor) = Manager::device(Filter::default(), 0).unwrap();

    // Create CP2130 connection
    let mut cp2130 = Cp2130::new(device, descriptor, UsbOptions::default()).unwrap();

    // TODO

    let _ = &mut cp2130;
}



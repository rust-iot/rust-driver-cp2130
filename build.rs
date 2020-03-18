use std::{env, fs};
fn main() {
    
    let target = env::var("TARGET").expect("Missing TARGET variable");
    let host = env::var("HOST").expect("Missing HOST variable");

    let current_dir = env::var("CARGO_MANIFEST_DIR").expect("Missing CARGO_MANIFEST_DIR variable");
    let out_dir = env::var("OUT_DIR").expect("Missing OUT_DIR variable");


    let target_dir = format!("{}/../../../", out_dir);

    println!("CP2130 Build ({}, {})", current_dir, target_dir);

    match target.as_ref() {
        "i686-pc-windows-gnu" if host != "i686-pc-windows-gnu" => {
            // Copy libusb win to output dir
            fs::copy(
                format!("{}/cross/win/libusb-1.0.dll", current_dir), 
                format!("{}/libusb-1.0.dll", target_dir)).unwrap();

            print!("cargo:rustc-link-search={}\r\n", target_dir);
            print!("cargo:rustc-link-lib=dylib=libusb-1.0\r\n");
        }
        _ => (),
    }
}

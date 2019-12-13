# Rust CP2130 Driver

A driver for the Silicon Labs [CP2130](https://www.silabs.com/interface/usb-bridges/classic/device.cp2130) USB->SPI bridge IC, exposing an [embedded-hal](https://github.com/rust-embedded/embedded-hal) compatible interface as well as a command line utility for interacting with (and testing interaction with) CP2130 devices.

## Status

WIP. Basic functionality working, PRs for extended features (non-volatile programming, alternate pin modes, etc.) are absolutely welcome.

[![GitHub tag](https://img.shields.io/github/tag/ryankurte/rust-driver-cp2130.svg)](https://github.com/ryankurte/rust-driver-cp2130)
[![Travis Build Status](https://travis-ci.com/ryankurte/rust-driver-cp2130.svg?branch=master)](https://travis-ci.com/ryankurte/rust-driver-cp2130)
[![Crates.io](https://img.shields.io/crates/v/driver-cp2130.svg)](https://crates.io/crates/driver-cp2130)
[![Docs.rs](https://docs.rs/driver-cp2130/badge.svg)](https://docs.rs/driver-cp2130)

[Open Issues](https://github.com/ryankurte/rust-driver-cp2130/issues)

## Getting started

You can install the utility with `cargo install driver-cp2130` or grab a pre-compiled release from [here]()

You may wish to copy [40-cp2130.rules](40-cp2130.rules) to `/etc/udev/rules.d` to allow all users with `plugdev` permissions to interact with the CP2130 device.



## References

- Datasheet: https://www.silabs.com/documents/public/data-sheets/CP2130.pdf
- Interface specification: https://www.silabs.com/documents/public/application-notes/AN792.pdf


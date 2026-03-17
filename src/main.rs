//! Blinks the LED on a Pico board
//!
#![no_std]
#![no_main]

use bsp::entry;
use defmt::*;
use defmt_rtt as _;
use embedded_hal::digital::{InputPin, OutputPin};
use panic_probe as _;

// Provide an alias for our BSP so we can switch targets quickly.
// Uncomment the BSP you included in Cargo.toml, the rest of the code does not need to change.
use rp_pico::{
    self as bsp,
    hal::gpio::{DynPinId, FunctionSioInput, FunctionSioOutput, Pin, PullDown, PullNone},
};
// use sparkfun_pro_micro_rp2040 as bsp;

use bsp::hal::{
    clocks::{init_clocks_and_plls, Clock},
    pac,
    sio::Sio,
    watchdog::Watchdog,
};

#[entry]
fn main() -> ! {
    info!("Program start");
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    // External high-speed crystal on the pico board is 12Mhz
    let external_xtal_freq_hz = 12_000_000u32;
    let clocks = init_clocks_and_plls(
        external_xtal_freq_hz,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // This is the correct pin on the Raspberry Pico board. On other boards, even if they have an
    // on-board LED, it might need to be changed.
    //
    // Notably, on the Pico W, the LED is not connected to any of the RP2040 GPIOs but to the cyw43 module instead.
    // One way to do that is by using [embassy](https://github.com/embassy-rs/embassy/blob/main/examples/rp/src/bin/wifi_blinky.rs)
    //
    // If you have a Pico W and want to toggle a LED with a simple GPIO output pin, you can connect an external
    // LED to one of the GPIO pins, and reference that pin here. Don't forget adding an appropriate resistor
    // in series with the LED.
    let mut led_pin = pins.gpio17.into_push_pull_output();

    // let mut cols = (
    //     pins.gpio27.into_push_pull_output(),
    //     pins.gpio26.into_push_pull_output(),
    //     pins.gpio15.into_push_pull_output(),
    //     pins.gpio14.into_push_pull_output(),
    //     pins.gpio16.into_push_pull_output(),
    // );

    let mut cols: [Pin<DynPinId, FunctionSioOutput, PullNone>; 5] = [
        pins.gpio27
            .into_push_pull_output()
            .into_pull_type()
            .into_dyn_pin(),
        pins.gpio26
            .into_push_pull_output()
            .into_pull_type()
            .into_dyn_pin(),
        pins.gpio15
            .into_push_pull_output()
            .into_pull_type()
            .into_dyn_pin(),
        pins.gpio14
            .into_push_pull_output()
            .into_pull_type()
            .into_dyn_pin(),
        pins.b_power_save
            .into_push_pull_output()
            .into_pull_type()
            .into_dyn_pin(),
    ];

    let mut rows: [Pin<DynPinId, FunctionSioInput, PullDown>; 4] = [
        pins.gpio5.into_pull_down_input().into_dyn_pin(),
        pins.gpio6.into_pull_down_input().into_dyn_pin(),
        pins.gpio7.into_pull_down_input().into_dyn_pin(),
        pins.gpio8.into_pull_down_input().into_dyn_pin(),
    ];

    let mut counter: i32;
    loop {
        counter = 0;

        cols.iter_mut().enumerate().for_each(|(key, col)| {
            col.set_high().unwrap();
            delay.delay_us(30);
            rows.iter_mut().enumerate().for_each(|(key, row)| {
                if row.is_high().unwrap() {
                    counter += 1;
                }
            });
            col.set_low().unwrap();
        });

        if counter % 2 == 0 {
            led_pin.set_high().unwrap();
        } else {
            led_pin.set_low().unwrap();
        }
    }
}

// End of file

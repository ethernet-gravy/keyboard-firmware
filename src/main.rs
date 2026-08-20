//! Blinks the LED on a Pico board
//!
#![no_std]
#![no_main]

#[macro_use]
extern crate fixedvec;
use cortex_m::prelude::_embedded_hal_timer_CountDown;
use defmt::*;
use defmt_rtt as _;
use embedded_hal::digital::{InputPin, OutputPin, StatefulOutputPin};
use fixedvec::FixedVec;
use panic_probe as _;

// Provide an alias for our BSP so we can switch targets quickly.
// Uncomment the BSP you included in Cargo.toml, the rest of the code does not need to change.
use rp2040_hal::gpio::{
    DynPinId, FunctionSioInput, FunctionSioOutput, Pin, PinState, PullDown, PullNone,
};
// use sparkfun_pro_micro_rp2040 as bsp;

use core::fmt::Write;
use rp2040_hal::uart::{DataBits, StopBits, UartConfig};
use rp2040_hal::{
    self as hal,
    clocks::{init_clocks_and_plls, Clock},
    entry,
    fugit::{ExtU32, RateExtU32},
    pac,
    sio::Sio,
    watchdog::Watchdog,
};
use usb_device::bus::UsbBusAllocator;
use usb_device::device::{StringDescriptors, UsbDeviceBuilder, UsbVidPid};
use usb_device::UsbError;
use usbd_human_interface_device::page::Keyboard;
use usbd_human_interface_device::prelude::UsbHidClassBuilder;
use usbd_human_interface_device::UsbHidError;

#[link_section = ".boot_loader"]
#[used]
pub static BOOT2_FIRMWARE: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

const LAYOUT: [[usbd_human_interface_device::page::Keyboard; 5]; 4] = [
    [
        Keyboard::Q,
        Keyboard::W,
        Keyboard::E,
        Keyboard::R,
        Keyboard::T,
    ],
    [
        Keyboard::A,
        Keyboard::S,
        Keyboard::D,
        Keyboard::F,
        Keyboard::G,
    ],
    [
        Keyboard::Z,
        Keyboard::X,
        Keyboard::C,
        Keyboard::V,
        Keyboard::B,
    ],
    [
        Keyboard::NoEventIndicated,
        Keyboard::NoEventIndicated,
        Keyboard::E,
        Keyboard::R,
        Keyboard::T,
    ],
];

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
    let timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    let pins = rp2040_hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let mut led_pin = pins.gpio17.into_push_pull_output();

    let uart_pins = (pins.gpio12.into_function(), pins.gpio13.into_function());

    let mut uart = hal::uart::UartPeripheral::new(pac.UART0, uart_pins, &mut pac.RESETS)
        .enable(
            UartConfig::new(115200.Hz(), DataBits::Eight, None, StopBits::One),
            clocks.peripheral_clock.freq(),
        )
        .unwrap();

    uart.write_full_blocking(b"Hello! Welcome to the keyboard firmware.\r\n");
    // let mut cols = (
    //     pins.gpio27.into_push_pull_output(),
    //     pins.gpio26.into_push_pull_output(),
    //     pins.gpio15.into_push_pull_output(),
    //     pins.gpio14.into_push_pull_output(),
    //     pins.gpio16.into_push_pull_output(),
    // );

    let mut prealloc_space = alloc_stack!([Keyboard; 12]);
    let mut report = FixedVec::new(&mut prealloc_space);
    let mut cols: [Pin<DynPinId, FunctionSioOutput, PullNone>; 5] = [
        pins.gpio26
            .into_push_pull_output()
            .into_pull_type()
            .into_dyn_pin(),
        pins.gpio22
            .into_push_pull_output()
            .into_pull_type()
            .into_dyn_pin(),
        pins.gpio20
            .into_push_pull_output()
            .into_pull_type()
            .into_dyn_pin(),
        pins.gpio23
            .into_push_pull_output()
            .into_pull_type()
            .into_dyn_pin(),
        pins.gpio21
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

    let usb_bus = UsbBusAllocator::new(hal::usb::UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));

    let mut keyboard = UsbHidClassBuilder::new()
        .add_device(
            usbd_human_interface_device::device::keyboard::NKROBootKeyboardConfig::default(),
        )
        .build(&usb_bus);

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0xfeed, 0x8813))
        .strings(&[StringDescriptors::default()
            .manufacturer("keyboard-firmware")
            .product("Split Keyboard")
            .serial_number("Winter-Harvest")])
        .unwrap()
        .build();
    let mut input_count_down = timer.count_down();
    input_count_down.start(1000.millis());
    let mut led_count_down = timer.count_down();
    led_count_down.start(1000.millis());

    let mut tick_count_down = timer.count_down();
    tick_count_down.start(1.millis());

    let mut raw_state: i32;
    // keyboard.device().write_report(keys).unwrap();
    // let _ = keyboard.tick();
    loop {
        raw_state = 0;

        // cols.iter_mut().enumerate().for_each(|(col_num, col)| {
        //     col.set_high().unwrap();
        //     delay.delay_us(30);
        //     rows.iter_mut().enumerate().for_each(|(row_num, row)| {
        //         if row.is_high().unwrap() {
        //             raw_state |= 1 << (row_num * 5 + col_num);
        //         }
        //     });
        //     col.set_low().unwrap();
        // });

        read_input_state(&mut cols, &mut rows, &mut delay, &mut report);
        //Poll the keys every 10ms
        if input_count_down.wait().is_ok() {
            match keyboard
                .device()
                .write_report(report.as_slice().iter().cloned())
            {
                Err(UsbHidError::WouldBlock) => {}
                Err(UsbHidError::Duplicate) => {}
                Ok(_) => {}
                Err(e) => {
                    core::panic!("Failed to write keyboard report: {:?}", e)
                }
            };
        }

        //Tick once per ms
        if tick_count_down.wait().is_ok() {
            match keyboard.tick() {
                Err(UsbHidError::WouldBlock) => {}
                Ok(_) => {}
                Err(e) => {
                    core::panic!("Failed to process keyboard tick: {:?}", e)
                }
            };
        }

        if usb_dev.poll(&mut [&mut keyboard]) {
            match keyboard.device().read_report() {
                Err(UsbError::WouldBlock) => {
                    //do nothing
                }
                Err(e) => {
                    core::panic!("Failed to read keyboard report: {:?}", e)
                }
                Ok(_) => {}
            }
        }

        if led_count_down.wait().is_ok() {
            writeln!(uart, "The state is {:#b}\r", raw_state).unwrap();
            match led_pin.is_set_high().unwrap() {
                true => led_pin.set_low().unwrap(),
                false => led_pin.set_high().unwrap(),
            };
        }
    }
}

fn read_input_state(
    cols: &mut [Pin<DynPinId, FunctionSioOutput, PullNone>; 5],
    rows: &mut [Pin<DynPinId, FunctionSioInput, PullDown>; 4],
    delay: &mut cortex_m::delay::Delay,
    report: &mut FixedVec<Keyboard>,
) {
    let mut raw_state = 0;
    cols.iter_mut().enumerate().for_each(|(col_num, col)| {
        col.set_high().unwrap();
        delay.delay_us(30);
        rows.iter_mut().enumerate().for_each(|(row_num, row)| {
            if row.is_high().unwrap() {
                raw_state |= 1 << (row_num * 5 + col_num);
            }
        });
        col.set_low().unwrap();
    });
    report.clear();
    for i in 0..32 {
        if report.iter().count() < 12 {
            if ((raw_state >> i) & 0x1) == 1 {
                report.push(LAYOUT[i / 4][i % 4]).unwrap();
            }
        }
    }
}
// End of file

//! # Multicore Blinking Example
//!
//! This application blinks two LEDs on GPIOs 2 and 3 at different rates (3Hz
//! and 4Hz respectively.)
//!
//! Derived from rp2040-hal multicore example.
#![no_std]
//#![cfg(feature = "thread_local")]
#![feature(thread_local)]
#![no_main]

// Make sure crate is present so it gets linked in to define `__aeabi_read_tp`
extern crate rp2040_multicore_per_cpu;

use core::cell::RefCell;

use cortex_m::delay::Delay;

use hal::clocks::Clock;
use hal::gpio::{DynPinId, FunctionSio, Pin, Pins, PullDown, SioOutput};
use hal::multicore::{Multicore, Stack};
use hal::sio::Sio;
// Ensure we halt the program on panic (if we don't mention this crate it won't
// be linked)
use panic_halt as _;

// Alias for our HAL crate
use rp2040_hal as hal;

// A shorter alias for the Peripheral Access Crate, which provides low-level
// register access
use hal::pac;

// Some traits we need
use embedded_hal::digital::StatefulOutputPin;

/// The linker will place this boot block at the start of our program image. We
/// need this to help the ROM bootloader get our code up and running.
/// Note: This boot block is not necessary when using a rp-hal based BSP
/// as the BSPs already perform this step.
#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_GENERIC_03H;

/// External high-speed crystal on the Raspberry Pi Pico board is 12 MHz. Adjust
/// if your board has a different frequency
const XTAL_FREQ_HZ: u32 = 12_000_000u32;

/// The frequency at which core 0 will blink its LED (Hz).
const CORE0_FREQ: u32 = 3;
/// The frequency at which core 1 will blink its LED (Hz).
const CORE1_FREQ: u32 = 4;
/// The delay between each toggle of core 0's LED (us).
const CORE0_DELAY: u32 = 1_000_000 / CORE0_FREQ;
/// The delay between each toggle of core 1's LED (us).
const CORE1_DELAY: u32 = 1_000_000 / CORE1_FREQ;

/// Stack for core 1
///
/// Core 0 gets its stack via the normal route - any memory not used by static
/// values is reserved for stack and initialised by cortex-m-rt.
/// To get the same for Core 1, we would need to compile everything separately
/// and modify the linker file for both programs, and that's quite annoying.
/// So instead, core1.spawn takes a [usize] which gets used for the stack.
/// NOTE: We use the `Stack` struct here to ensure that it has 32-byte
/// alignment, which allows the stack guard to take up the least amount of
/// usable RAM.
static mut CORE1_STACK: Stack<4096> = Stack::new();

/// State for the blinker
struct BlinkState {
    led: Pin<DynPinId, FunctionSio<SioOutput>, PullDown>,
    delay: Delay,
    delay_time: u32,
}

/// Per core blinker state
#[thread_local]
static STATE: RefCell<Option<BlinkState>> = RefCell::new(None);

/// Blink which ever LED with whatever delay, according to the per-core state.
fn blinker() -> ! {
    let mut state = STATE.borrow_mut();
    let BlinkState {
        led,
        delay,
        delay_time,
    } = state.as_mut().unwrap();
    loop {
        led.toggle().unwrap();
        delay.delay_us(*delay_time);
    }
}

/// Entry point to our bare-metal application.
///
/// The `#[rp2040_hal::entry]` macro ensures the Cortex-M start-up code calls this function
/// as soon as all global variables and the spinlock are initialised.
#[rp2040_hal::entry]
fn main() -> ! {
    // Grab our singleton objects
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();

    // Set up the watchdog driver - needed by the clock setup code
    let mut watchdog = hal::watchdog::Watchdog::new(pac.WATCHDOG);

    // Configure the clocks
    let clocks = hal::clocks::init_clocks_and_plls(
        XTAL_FREQ_HZ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .unwrap();

    let sys_freq = clocks.system_clock.freq().to_Hz();

    // Set up the GPIO pins
    let mut sio = Sio::new(pac.SIO);
    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );
    let led1 = pins.gpio2.into_push_pull_output();
    let led2 = pins.gpio3.into_push_pull_output();

    // Start up the second core to blink the second LED
    let mut mc = Multicore::new(&mut pac.PSM, &mut pac.PPB, &mut sio.fifo);
    let cores = mc.cores();
    let core1 = &mut cores[1];
    core1
        .spawn(unsafe { &mut CORE1_STACK.mem }, move || {
            // Get the second core's copy of the `CorePeripherals`, which are per-core.
            // Unfortunately, `cortex-m` doesn't support this properly right now,
            // so we have to use `steal`.
            let core = unsafe { pac::CorePeripherals::steal() };
            // Set up the delay for the second core.
            let delay = Delay::new(core.SYST, sys_freq);

            STATE.borrow_mut().replace(BlinkState {
                led: led2.into_dyn_pin(),
                delay,
                delay_time: CORE1_DELAY,
            });

            // Blink the second LED.
            blinker();
        })
        .unwrap();

    // Set up the delay for the first core.
    let delay = Delay::new(core.SYST, sys_freq);

    // Blink the first LED.
    STATE.borrow_mut().replace(BlinkState {
        led: led1.into_dyn_pin(),
        delay,
        delay_time: CORE0_DELAY,
    });
    blinker();
}

// End of file

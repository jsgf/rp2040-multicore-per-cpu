//! ## RP2040 Per-core static data
//!
//! The RP2040 has two cores. The rp2040-hal crate's [`multicore`][multicore]
//! module makes it easy to start code on the second core. However, this brings
//! with it the same challenges as multi-threaded programming: state in shared
//! memory between the cores.
//!
//! This crate allows you to make use of the ([unstable][]) `#[thread_local]`
//! attribute on static variables to make them per-core state. This allows the
//! same code to run on both cores but with its own core-specific static state,
//! such maintaining program state, or for things like DMA buffers.
//!
//! For example:
//! ```rust,ignore
//! #![feature(thread_local)]
//! # use core::cell::RefCell;
//! extern crate rp2040_multicore_per_cpu;
//!
//! #[thread_local]
//! static MY_COUNTER: RefCell<usize> = RefCell::new(0);
//!
//! fn next_id() -> usize {
//!   MY_COUNTER.replace_with(|c| *c + 1)
//! }
//! ```
//!
//! Each core will get its own instance of the `MY_COUNTER` variable. Since
//! these are not shared, they do not need atomic operations to update.
//!
//! These core-local variables are initialized on program startup and retain
//! their value from there on, even between invocations of
//! [`Core::spawn`][spawn].
//!
//! If the variables are zero-initialized then they will be reserved space in
//! the `.tbss` section in the executable, and then space in `.bss` for each
//! core. Similarly, variables initialized with non-zero constants will be in
//! the executable's `.tdata` section, and have space reserved in `.bss`; the
//! initial values are copied at program startup. Note that this uses the
//! `__pre_init` hook to do this, so it won't be available for other uses.
//!
//! ## Build setup
//!
//! Note that this requires some setup in the linker script to allocate space
//! for the static data. See memory.x for details. You also need to make you
//! explicitly depend on this crate with `extern crate
//! rp2040_multicore_per_cpu;`. This crate has no Rust API of its own, but must
//! still be included in the linker line to ensure the `__aeabi_read_tp`
//! function is defined.
//!
//! Also note this uses the `__pre_init` hook from [`cortex-m-rt`] to set up the
//! per-core state at reset time, making it unavailable for other uses (this is
//! rare however).
//!
//! [multicore]:
//!     https://docs.rs/rp2040-hal/latest/rp2040_hal/multicore/index.html
//! [unstable]:
//!     https://doc.rust-lang.org/unstable-book/language-features/thread-local.html
//! [spawn]:
//!     https://docs.rs/rp2040-hal/latest/rp2040_hal/multicore/struct.Core.html#method.spawn
#![no_std]

use core::arch::global_asm;

extern "C" {
    static mut TLS_CORE_0: u8;
    static mut TLS_CORE_1: u8;
}

// Define `__aeabi_read_tp` called by the compiler to get access to
// thread-local storage.
global_asm! {
    ".pushsection .text.__aeabi_read_tp",
    ".align 4",
    ".p2align 4,,15",
    ".global __aeabi_read_tp",
    ".type __aeabi_read_tp,%function",

    "__aeabi_read_tp:",
    "    ldr r0, =0xd0000000",      // Load SIO CPUID addr
    "    ldr r0, [r0]",             // Load CPUID
    "    cmp r0, #0",               // Check core 0
    "    ldr r0, ={core_0}",        // Set TLS_CORE_0
    "    beq 1f",                   // skip if done
    "    ldr r0, ={core_1}",        // Set TLS_CORE_1
    "1:  bx lr",

    ".popsection",
    core_0 = sym TLS_CORE_0,
    core_1 = sym TLS_CORE_1,
}

// This must be pub for linkage but isn't a public API.
mod inner {
    use super::{TLS_CORE_0, TLS_CORE_1};
    use core::ptr::{addr_of, addr_of_mut};

    extern "C" {
        static __tdata_start: u8;
        static __tdata_end: u8;
        static __tbss_start: u8;
        static __tbss_end: u8;
    }

    /// Intercept __pre_init to hook into the startup code to copy the tdata into
    /// TLS_CORE_0/1. TLS_CORE_0/1 are outside of the regular .bss segment, so we
    /// also need to zero out the bss parts.
    ///
    /// NB: Run as the very first thing, nothing has been initialized and memory
    /// could be in arbitrary state, so we only deal with things via raw pointers.
    /// Assumes a stack has been set up.
    #[cortex_m_rt::pre_init]
    unsafe fn tls_pre_init_hook() {
        for dst in [addr_of_mut!(TLS_CORE_0), addr_of_mut!(TLS_CORE_1)] {
            let datalen = addr_of!(__tdata_end).offset_from(addr_of!(__tdata_start)) as usize;
            core::ptr::copy(addr_of!(__tdata_start), dst, datalen);
            let bsslen = addr_of!(__tbss_end).offset_from(addr_of!(__tbss_start)) as usize;
            dst.add(datalen).write_bytes(0, bsslen);
        }
    }
}

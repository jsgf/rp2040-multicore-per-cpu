#![no_std]

use core::arch::global_asm;
use core::ptr::{addr_of, addr_of_mut};

extern "C" {
    static mut TLS_CORE_0: u8;
    static mut TLS_CORE_1: u8;
    static __tdata_start: u8;
    static __tdata_end: u8;
    static __tbss_start: u8;
    static __tbss_end: u8;
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

// Intercept __pre_init to hook into the startup code to copy the tdata into
// TLS_CORE_[01]. TLS_CORE[01] are outside of the regular .bss segment, so we also
// need to zero out the bss parts.
//
// NB: Run as the very first thing, nothing has been initialized and memory
// could be in arbitrary state, so we only deal with things via raw pointers.
// Assumes a stack has been set up.
#[cortex_m_rt::pre_init]
unsafe fn tls_pre_init_hook() {
    for dst in [addr_of_mut!(TLS_CORE_0), addr_of_mut!(TLS_CORE_1)] {
        let datalen = addr_of!(__tdata_end).offset_from(addr_of!(__tdata_start)) as usize;
        core::ptr::copy(addr_of!(__tdata_start), dst, datalen);
        let bsslen = addr_of!(__tbss_end).offset_from(addr_of!(__tbss_start)) as usize;
        dst.add(datalen).write_bytes(0, bsslen);
    }
}
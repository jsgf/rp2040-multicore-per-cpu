# Per-core static stat on RP2040

The RP2040 is a dual-core microcontroller. The rp2040-hal `multicore` module
makes it easy to start code running on the second core, but doing so adds some
complications. Any `static` variables are shared in memory, causing the same
difficulties that static variables do in multithreaded systems.

This package enables the use of the unstable `#[thread_local]` attribute to mark
a `static` variable as core-local. Each core gets its own instance of any such
variables and can use and manipulate them completely independently.

Because these variables are not shared, their contents need not be `Send` or
`Sync`, so you can, for example, use a `RefCell`'s interior mutability to manage
them.

Unlike thread-local variables, their states are initialized once on system
reset, not when code is spawned onto a core. Therefore if you spawn multiple
times onto core 1, then later spawns will see the state left behind by the
previous runs.

## Notes on Usage

The implementation needs the linker script to set up the layout of per-core
state in memory. You therefore need to copy the relevant portions of `memory.x`
into your own linker script.

It also needs this crate to define the `__aeabi_read_tp` function to read the
per-"thread" state. You need to add
```rust
extern crate rp2040_multicore_per_cpu;
```
to make sure this crate is pulled in so that this symbols will be linked in.
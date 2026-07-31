#![no_std]
#![no_main]

extern crate thermite_probe;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let _observed = thermite_probe::probe_shell::boot_observation();
    loop {}
}

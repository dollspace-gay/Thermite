#![no_std]
#![no_main]

extern crate kernel_identity;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let _answer = kernel_identity::identity(41);
    loop {}
}

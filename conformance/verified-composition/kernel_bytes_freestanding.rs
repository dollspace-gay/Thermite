#![no_std]
#![no_main]

extern crate thermite_kernel_bytes;

use core::panic::PanicInfo;

static BYTES: [u8; 12] = [
    0x78, 0x56, 0x34, 0x12,
    0xef, 0xcd, 0xab, 0x90, 0x78, 0x56, 0x34, 0x12,
];

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let _u32_value = thermite_kernel_bytes::kernel_bytes_shell::read_u32_le(&BYTES, 0);
    let _u64_value = thermite_kernel_bytes::kernel_bytes_shell::read_u64_le(&BYTES, 4);
    loop {}
}

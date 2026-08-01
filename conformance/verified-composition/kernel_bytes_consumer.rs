extern crate thermite_kernel_bytes;

fn main() {
    let bytes = [
        0x78_u8, 0x56, 0x34, 0x12,
        0xef, 0xcd, 0xab, 0x90, 0x78, 0x56, 0x34, 0x12,
    ];
    assert_eq!(
        thermite_kernel_bytes::kernel_bytes_shell::read_u32_le(&bytes, 0),
        0x1234_5678,
    );
    assert_eq!(
        thermite_kernel_bytes::kernel_bytes_shell::read_u64_le(&bytes, 4),
        0x1234_5678_90ab_cdef,
    );
}

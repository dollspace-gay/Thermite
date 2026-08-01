use vstd::prelude::*;

pub open spec fn spec_read_u32_be(bytes: &[u8], offset: int) -> u32 {
    ((bytes@[offset] as u32) << 24)
        | ((bytes@[offset + 1] as u32) << 16)
        | ((bytes@[offset + 2] as u32) << 8)
        | (bytes@[offset + 3] as u32)
}

pub fn wrong_endian(bytes: &[u8], offset: usize) -> (result: u32)
    requires
        offset + 4 <= bytes.len(),
    ensures
        result == spec_read_u32_be(bytes, offset as int),
{
    (bytes[offset] as u32)
        | ((bytes[offset + 1] as u32) << 8)
        | ((bytes[offset + 2] as u32) << 16)
        | ((bytes[offset + 3] as u32) << 24)
}

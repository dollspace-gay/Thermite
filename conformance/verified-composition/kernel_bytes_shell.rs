use vstd::prelude::*;

pub open spec fn spec_read_u32_le(bytes: &[u8], offset: int) -> u32 {
    (bytes@[offset] as u32)
        | ((bytes@[offset + 1] as u32) << 8)
        | ((bytes@[offset + 2] as u32) << 16)
        | ((bytes@[offset + 3] as u32) << 24)
}

pub open spec fn spec_read_u64_le(bytes: &[u8], offset: int) -> u64 {
    (bytes@[offset] as u64)
        | ((bytes@[offset + 1] as u64) << 8)
        | ((bytes@[offset + 2] as u64) << 16)
        | ((bytes@[offset + 3] as u64) << 24)
        | ((bytes@[offset + 4] as u64) << 32)
        | ((bytes@[offset + 5] as u64) << 40)
        | ((bytes@[offset + 6] as u64) << 48)
        | ((bytes@[offset + 7] as u64) << 56)
}

pub fn read_u32_le(bytes: &[u8], offset: usize) -> (result: u32)
    requires
        offset + 4 <= bytes.len(),
    ensures
        result == spec_read_u32_le(bytes, offset as int),
{
    (bytes[offset] as u32)
        | ((bytes[offset + 1] as u32) << 8)
        | ((bytes[offset + 2] as u32) << 16)
        | ((bytes[offset + 3] as u32) << 24)
}

pub fn read_u64_le(bytes: &[u8], offset: usize) -> (result: u64)
    requires
        offset + 8 <= bytes.len(),
    ensures
        result == spec_read_u64_le(bytes, offset as int),
{
    let decoded = (bytes[offset] as u64)
        | ((bytes[offset + 1] as u64) << 8)
        | ((bytes[offset + 2] as u64) << 16)
        | ((bytes[offset + 3] as u64) << 24)
        | ((bytes[offset + 4] as u64) << 32)
        | ((bytes[offset + 5] as u64) << 40)
        | ((bytes[offset + 6] as u64) << 48)
        | ((bytes[offset + 7] as u64) << 56);
    model_identity(decoded)
}

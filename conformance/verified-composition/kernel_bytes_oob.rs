use vstd::prelude::*;

pub open spec fn spec_first_u32(bytes: &[u8]) -> u32 {
    (bytes@[0] as u32)
        | ((bytes@[1] as u32) << 8)
        | ((bytes@[2] as u32) << 16)
        | ((bytes@[3] as u32) << 24)
}

pub fn read_first_u32(bytes: &[u8]) -> (result: u32)
    requires
        4 <= bytes.len(),
    ensures
        result == spec_first_u32(bytes),
{
    (bytes[0] as u32)
        | ((bytes[1] as u32) << 8)
        | ((bytes[2] as u32) << 16)
        | ((bytes[3] as u32) << 24)
}

pub fn rejected_oob_call(bytes: &[u8]) -> u32
    requires
        bytes.len() < 4,
{
    read_first_u32(bytes)
}

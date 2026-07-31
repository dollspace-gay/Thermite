#![no_std]
#![crate_type = "rlib"]

use verus_builtin::*;
use verus_builtin_macros::*;

verus! {
pub fn identity(x: u64) -> (r: u64)
    requires true,
    ensures r == x,
{
    x
}
}

//! Frozen implementation shell for `conformance/bootable_kernel.th`.
//!
//! Forge binds this exact source and the `tpl_clock_read` public symbol.  The
//! unsafe foreign call is target-platform-layer code; ordinary Thermite sees
//! only the sealed `Clock` and the exact `Instant` contract.

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Clock {
    pub slot: u64,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Instant {
    pub ticks: u64,
    pub scale_numerator: u64,
    pub scale_denominator: u64,
    pub error_ticks: u64,
}

unsafe extern "C" {
    fn tpl_clock_read(clock: Clock) -> Instant;
}

pub fn kernel_step(clock: Clock) -> u64 {
    // SAFETY: the image validator resolves this call to the one registry-owned
    // `tpl_clock_read` symbol and binds both source files and the PDB inventory.
    unsafe { tpl_clock_read(clock) }.scale_denominator
}

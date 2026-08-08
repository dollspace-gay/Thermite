// Freestanding consumer for the closed result-enum kernel bundle
// (`.design/build/l3-verified-artifact.md` AC-19). It matches every variant of
// the exported `RingOffer` and completes a real final link.
//
// The kernel profile promises a verified rlib, not a bootable image. The panic
// handler and the target intrinsics a record copy leaves for the linker belong
// to the platform, so this harness supplies the minimum a final link needs
// (`.design/build/l3-rich-composition.md`, "Kernel bounded-state
// representation"). Nothing here is a kernel: there is no scheduler, allocator,
// device, or boot protocol, and `_start` only exercises the exported ABI.

#![no_std]
#![no_main]

extern crate closed_result_enum;

use closed_result_enum::{ring_empty, ring_offer, RingOffer, RingState, RING_SLOTS};

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

/// # Safety
/// The linker calls this with a valid, non-overlapping destination and source
/// run of `count` bytes, which is the contract of the intrinsic it replaces.
#[no_mangle]
pub unsafe extern "C" fn memcpy(destination: *mut u8, source: *const u8, count: usize) -> *mut u8 {
    let mut index = 0;
    while index < count {
        // SAFETY: `index < count` and the caller guarantees both runs are valid
        // for `count` bytes.
        unsafe { *destination.add(index) = *source.add(index) };
        index += 1;
    }
    destination
}

/// # Safety
/// The linker calls this with a valid destination run of `count` bytes, which is
/// the contract of the intrinsic it replaces.
#[no_mangle]
pub unsafe extern "C" fn memset(destination: *mut u8, value: i32, count: usize) -> *mut u8 {
    let mut index = 0;
    while index < count {
        // SAFETY: `index < count` and the caller guarantees the run is valid for
        // `count` bytes.
        unsafe { *destination.add(index) = value as u8 };
        index += 1;
    }
    destination
}

fn observed(offer: RingOffer) -> u64 {
    match offer {
        RingOffer::Accepted { ring, slot } => ring.slots[slot],
        RingOffer::Rejected { ring, stamp } => ring.len as u64 + stamp,
        RingOffer::Closed => 0,
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let accepted = observed(ring_offer(ring_empty(), 41));
    let full = RingState {
        slots: [0; RING_SLOTS],
        head: 0,
        len: RING_SLOTS,
    };
    let rejected = observed(ring_offer(full, 7));
    let past_end = RingState {
        slots: [0; RING_SLOTS],
        head: RING_SLOTS,
        len: 0,
    };
    let closed = observed(ring_offer(past_end, 7));
    let _ = accepted + rejected + closed;
    loop {}
}

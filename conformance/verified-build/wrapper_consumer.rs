extern crate bounded_guard_tv;

use bounded_guard_tv::{thermite_export_bounded_inc_v1, ThermiteContractError};

fn main() {
    assert!(matches!(thermite_export_bounded_inc_v1(41), Ok(42)));
    assert!(matches!(
        thermite_export_bounded_inc_v1(100),
        Err(ThermiteContractError::Precondition)
    ));
}

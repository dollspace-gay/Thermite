pub fn identity_impl(value: u64) -> (result: u64)
    ensures result == value,
{
    value
}

pub fn observe_bound_primitive() -> (result: u64)
    ensures result == 7,
{
    primitive_observation(7)
}

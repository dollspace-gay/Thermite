pub fn observe_separate_primitive() -> (result: u64)
    ensures result == 11,
{
    primitive_observation(11)
}

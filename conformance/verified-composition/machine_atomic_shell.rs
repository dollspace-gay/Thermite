pub fn observe_machine_atomic() -> (result: u64)
    ensures result == 19,
{
    machine_atomic_observation(19)
}

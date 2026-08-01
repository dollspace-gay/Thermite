pub fn array_observation() -> (result: u64)
    ensures result == 1,
{
    let values = [1u64, 2u64];
    values[0]
}

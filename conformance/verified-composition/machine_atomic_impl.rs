pub fn atomic_roundtrip_impl(value: u64) -> (result: u64)
    ensures result == value,
{
    let (atomic, Tracked(permission)) = vstd::atomic::PAtomicU64::new(value);
    atomic.load(Tracked(&permission))
}

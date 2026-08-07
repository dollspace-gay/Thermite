pub fn syn_address_from_region_impl(region: &SynRegion, offset: usize) -> (result: SynAddress)
    requires
        offset <= region.capacity,
    ensures
        result.region == region.identity,
        result.offset == offset,
        result.capacity == region.capacity,
{
    SynAddress {
        region: region.identity,
        offset: offset,
        capacity: region.capacity,
    }
}

pub fn syn_address_advance_impl(address: SynAddress, length: usize) -> (result: SynAddress)
    requires
        address.offset <= address.capacity,
        length <= address.capacity - address.offset,
    ensures
        result.region == address.region,
        result.offset == address.offset + length,
        result.capacity == address.capacity,
{
    SynAddress {
        region: address.region,
        offset: address.offset + length,
        capacity: address.capacity,
    }
}

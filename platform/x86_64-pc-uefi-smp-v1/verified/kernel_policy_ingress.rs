use vstd::prelude::*;

pub fn thermite_kernel_policy_entry(
    cpus: u64,
    scheduler_counter: &super::Atomic,
) -> (result: u64)
    requires
        cpus > 0,
        cpus <= 8,
    ensures
        result == 127 * 10000000 + 1010100 + (cpus - 1) * 10 + 41,
{
    super::kernel_acceptance_slice(cpus, scheduler_counter)
}

pub fn thermite_kernel_signature_policy_flags(signature: u64) -> (result: u64)
    ensures
        result == signature / 10000000,
{
    super::kernel_signature_policy_flags(signature)
}

pub fn thermite_kernel_signature_task_base(signature: u64, cpus: u64) -> (result: u64)
    requires
        cpus > 0,
        cpus <= 8,
    ensures
        (((signature % 10000000 <= 1010100 + (cpus - 1) * 10)
            || (signature % 10000000 > 1010100 + (cpus - 1) * 10 + 1000))
            && result == 0)
            || (signature % 10000000 > 1010100 + (cpus - 1) * 10
                && signature % 10000000 <= 1010100 + (cpus - 1) * 10 + 1000
                && result
                    == signature % 10000000 - (1010100 + (cpus - 1) * 10)),
{
    super::kernel_signature_task_base(signature, cpus)
}

pub fn thermite_scheduler_task_value(task_base: u64, task_index: u64) -> (result: u64)
    requires
        task_base > 0,
        task_base <= 1000,
        task_index < 4096,
    ensures
        result == task_base + task_index,
{
    super::scheduler_task_value(task_base, task_index)
}

pub fn thermite_scheduler_task_available(task_index: u64) -> (result: bool)
    ensures
        result == (task_index < 4096),
{
    super::scheduler_task_available(task_index)
}

pub fn thermite_scheduler_seed_admitted(
    seed_index: u64,
    expected_workers: u64,
) -> (result: bool)
    requires
        expected_workers > 0,
        expected_workers <= 8,
    ensures
        result == (seed_index < expected_workers),
{
    super::scheduler_seed_admitted(seed_index, expected_workers)
}

pub fn thermite_scheduler_release_gate(
    seed_index: u64,
    expected_workers: u64,
) -> (result: bool)
    requires
        expected_workers > 0,
        expected_workers <= 8,
        seed_index < expected_workers,
    ensures
        result == (seed_index + 1 == expected_workers),
{
    super::scheduler_release_gate(seed_index, expected_workers)
}

pub fn thermite_scheduler_required_ap_workers(ap_count: u64) -> (result: u64)
    requires
        ap_count < 8,
    ensures
        (ap_count < 2 && result == ap_count) || (ap_count >= 2 && result == 2),
{
    super::scheduler_required_ap_workers(ap_count)
}

pub fn thermite_scheduler_required_parallel_cpus(ap_count: u64) -> (result: u64)
    requires
        ap_count < 8,
    ensures
        (ap_count < 2 && result == 0) || (ap_count >= 2 && result == 2),
{
    super::scheduler_required_parallel_cpus(ap_count)
}

pub fn thermite_scheduler_claim(
    counter: &super::atomic::ExactAtomicU64,
) -> (result: u64)
    ensures
        result < 4096 || result == 18446744073709551615,
{
    super::scheduler_claim(counter)
}

pub fn thermite_scheduler_observe_max(
    maximum: &super::atomic::ExactAtomicU64,
    candidate: u64,
) -> (result: u64)
    requires
        candidate <= 8,
    ensures
        result == candidate + 1,
{
    super::scheduler_observe_max(maximum, candidate)
}

pub fn thermite_scheduler_worker_enter(
    cpu: u64,
    ready: &super::atomic::ExactAtomicU64,
    expected_workers: &super::atomic::ExactAtomicU64,
    task_base: &super::atomic::ExactAtomicU64,
    task_sum: &super::atomic::ExactAtomicU64,
    worker_mask: &super::atomic::ExactAtomicU64,
    maximum_active: &super::atomic::ExactAtomicU64,
    release_gate: &super::atomic::ExactAtomicU64,
) -> (result: u64)
    requires
        cpu < 8,
    ensures
        result == cpu + 10,
{
    super::scheduler_worker_enter(
        cpu,
        ready,
        expected_workers,
        task_base,
        task_sum,
        worker_mask,
        maximum_active,
        release_gate,
    )
}

pub fn thermite_atomic_load(cell: &super::atomic::ExactAtomicU64) -> (result: u64) {
    super::atomic_boundary_load(cell, super::Ordering::SeqCst)
}

pub fn thermite_atomic_store(cell: &super::atomic::ExactAtomicU64, value: u64) {
    super::atomic_boundary_store(cell, value, super::Ordering::SeqCst)
}

pub fn thermite_atomic_fetch_add(
    cell: &super::atomic::ExactAtomicU64,
    value: u64,
) -> (result: u64) {
    super::atomic_boundary_fetch(
        cell,
        super::FetchOp::Add,
        value,
        super::Ordering::SeqCst,
    )
}

pub fn thermite_atomic_fetch_or(
    cell: &super::atomic::ExactAtomicU64,
    value: u64,
) -> (result: u64) {
    super::atomic_boundary_fetch(
        cell,
        super::FetchOp::Or,
        value,
        super::Ordering::SeqCst,
    )
}

pub fn thermite_atomic_fetch_sub(
    cell: &super::atomic::ExactAtomicU64,
    value: u64,
) -> (result: u64) {
    super::atomic_boundary_fetch(
        cell,
        super::FetchOp::Sub,
        value,
        super::Ordering::SeqCst,
    )
}

pub fn thermite_atomic_fetch_and(
    cell: &super::atomic::ExactAtomicU64,
    value: u64,
) -> (result: u64) {
    super::atomic_boundary_fetch(
        cell,
        super::FetchOp::And,
        value,
        super::Ordering::SeqCst,
    )
}

pub fn thermite_atomic_compare_exchange(
    cell: &super::atomic::ExactAtomicU64,
    current: u64,
    value: u64,
) -> (result: super::Cas)
    ensures
        result.exchanged == (result.previous == current),
{
    super::atomic_compare_exchange(cell, current, value)
}

pub fn thermite_atomic_claim_once(
    cell: &super::atomic::ExactAtomicU64,
    owner: u64,
) -> (result: u64)
    requires
        owner > 0,
    ensures
        result > 0,
{
    super::atomic_claim_once(cell, owner)
}

pub fn thermite_scheduler_expected_sum(task_base: u64) -> (result: u64)
    requires
        task_base > 0,
        task_base <= 1000,
    ensures
        result == 4096 * 4095 / 2 + 4096 * task_base,
{
    super::scheduler_expected_sum(task_base)
}

pub fn thermite_ipc_runtime_accept(cpu: u64, payload: u64) -> (result: bool)
    requires
        cpu < 8,
    ensures
        result == (payload == 5928228344835556676),
{
    super::ipc_runtime_accept(cpu, payload)
}

pub fn thermite_ipc_cell_accept(
    cpu: u64,
    payload: &super::atomic::ExactAtomicU64,
) -> (result: bool)
    requires
        cpu < 8,
{
    let loaded = super::atomic_boundary_load(payload, super::Ordering::SeqCst);
    super::ipc_runtime_accept(cpu, loaded)
}

pub fn thermite_ipc_worker_dispatch(
    cpu: u64,
    payload: &super::atomic::ExactAtomicU64,
    stale_count: &super::atomic::ExactAtomicU64,
    delivered_mask: &super::atomic::ExactAtomicU64,
) -> (result: u64)
    requires
        cpu < 8,
    ensures
        result == 1,
{
    super::ipc_worker_dispatch(cpu, payload, stale_count, delivered_mask)
}

pub fn thermite_ticket_lock_can_enter(owner_ticket: u64, issued_ticket: u64) -> (result: bool)
    ensures
        result == (owner_ticket == issued_ticket),
{
    super::ticket_lock_can_enter(owner_ticket, issued_ticket)
}

pub fn thermite_ticket_issue(counter: &super::atomic::ExactAtomicU64) -> (result: u64) {
    super::atomic_boundary_fetch(
        counter,
        super::FetchOp::Add,
        1,
        super::Ordering::SeqCst,
    )
}

pub fn thermite_ticket_lock_cell_can_enter(
    owner: &super::atomic::ExactAtomicU64,
    issued_ticket: u64,
) -> (result: bool) {
    let owner_ticket = super::atomic_boundary_load(owner, super::Ordering::SeqCst);
    super::ticket_lock_can_enter(owner_ticket, issued_ticket)
}

pub fn thermite_ticket_lock_cell_release(
    owner: &super::atomic::ExactAtomicU64,
    issued_ticket: u64,
)
    requires
        issued_ticket < 18446744073709551615,
{
    let next_owner = super::ticket_lock_release_value(issued_ticket);
    super::atomic_boundary_store(owner, next_owner, super::Ordering::SeqCst);
}

pub fn thermite_ticket_lock_release_value(issued_ticket: u64) -> (result: u64)
    requires
        issued_ticket < 18446744073709551615,
    ensures
        result == issued_ticket + 1,
{
    super::ticket_lock_release_value(issued_ticket)
}

pub fn thermite_allocator_request_pages(size: u64, alignment: u64) -> (result: u64)
    ensures
        result == if alignment > 4096 || size > 262144 {
            0
        } else if size == 0 {
            1
        } else {
            (size + 4095) / 4096
        },
{
    super::allocator_request_pages(size, alignment)
}

pub fn thermite_allocator_runtime_accept(
    heap_base: u64,
    first_address: u64,
    second_address: u64,
    heap_bytes: u64,
    bitmap: u64,
) -> (result: bool)
    requires
        heap_bytes > 0,
        heap_base <= 18446744073709551615 - heap_bytes,
    ensures
        result == (heap_base > 0
            && first_address >= heap_base
            && first_address < heap_base + heap_bytes
            && second_address >= heap_base
            && second_address < heap_base + heap_bytes
            && first_address != second_address
            && bitmap == 0),
{
    super::allocator_runtime_accept(
        heap_base,
        first_address,
        second_address,
        heap_bytes,
        bitmap,
    )
}

pub fn thermite_allocator_run_mask(
    first: u64,
    pages: u64,
    all_bits: u64,
) -> (result: u64)
    requires
        pages > 0,
        pages <= 64,
        first < 64,
        first + pages <= 64,
        all_bits == 18446744073709551615,
    ensures
        result == (all_bits >> (64u64 - pages)) << first,
{
    super::allocator_run_mask(first, pages, all_bits)
}

pub fn thermite_allocator_candidate_available(observed: u64, mask: u64) -> (result: bool)
    ensures
        result == (observed & mask == 0),
{
    super::allocator_candidate_available(observed, mask)
}

pub fn thermite_allocator_first_available(observed: u64, pages: u64) -> (result: u64)
    requires
        pages > 0,
        pages <= 64,
    ensures
        result == super::allocator_first_available_spec(observed, pages, 0),
{
    super::allocator_first_available(observed, pages)
}

pub fn thermite_allocator_claim_first(
    cell: &super::atomic::ExactAtomicU64,
    pages: u64,
) -> (result: u64)
    requires
        pages > 0,
        pages <= 64,
    ensures
        result <= 64,
        result == 64 || result + pages <= 64,
{
    super::allocator_claim_first(cell, pages)
}

pub fn thermite_allocator_claim_value(observed: u64, mask: u64) -> (result: u64)
    ensures
        result == (observed | mask),
{
    super::allocator_claim_value(observed, mask)
}

pub fn thermite_mapping_kernel_table_entry(physical: u64) -> (result: u64)
    ensures
        ((physical > 4294967295 || physical % 4096 != 0) && result == 0)
            || (physical <= 4294967295
                && physical % 4096 == 0
                && result == physical + 3),
{
    super::mapping_kernel_table_entry(physical)
}

pub fn thermite_mapping_identity_physical(
    pdpt_index: u64,
    entry_index: u64,
) -> (result: u64)
    requires
        pdpt_index < 4,
        entry_index < 512,
    ensures
        result == (pdpt_index * 512 + entry_index) * 2097152,
{
    super::mapping_identity_physical(pdpt_index, entry_index)
}

pub fn thermite_mapping_user_table_entry(physical: u64) -> (result: u64)
    ensures
        ((physical > 4294967295 || physical % 4096 != 0) && result == 0)
            || (physical <= 4294967295
                && physical % 4096 == 0
                && result == physical + 7),
{
    super::mapping_user_table_entry(physical)
}

pub fn thermite_mapping_identity_huge_entry(physical: u64) -> (result: u64)
    ensures
        ((physical > 4294967295 || physical % 4096 != 0) && result == 0)
            || (physical <= 4294967295
                && physical % 4096 == 0
                && result == physical + 131),
{
    super::mapping_identity_huge_entry(physical)
}

pub fn thermite_mapping_user_code_entry(physical: u64) -> (result: u64)
    ensures
        ((physical > 4294967295 || physical % 4096 != 0) && result == 0)
            || (physical <= 4294967295
                && physical % 4096 == 0
                && result == physical + 5),
{
    super::mapping_user_code_entry(physical)
}

pub fn thermite_mapping_user_stack_entry(physical: u64) -> (result: u64)
    ensures
        ((physical > 4294967295 || physical % 4096 != 0) && result == 0)
            || (physical <= 4294967295
                && physical % 4096 == 0
                && result == physical + 9223372036854775815),
{
    super::mapping_user_stack_entry(physical)
}

pub fn thermite_mapping_kernel_data_entry(physical: u64) -> (result: u64)
    ensures
        ((physical > 4294967295 || physical % 4096 != 0) && result == 0)
            || (physical <= 4294967295
                && physical % 4096 == 0
                && result == physical + 9223372036854775811),
{
    super::mapping_kernel_data_entry(physical)
}

pub fn thermite_ap_failure_selection_valid(
    cpu_count: u64,
    bsp_failed: bool,
    failed_outside_set: bool,
    effective_count: u64,
) -> (result: bool)
    ensures
        result == (cpu_count > 0
            && cpu_count <= 8
            && !bsp_failed
            && !failed_outside_set
            && effective_count > 0
            && effective_count <= cpu_count),
{
    super::ap_failure_selection_valid(
        cpu_count,
        bsp_failed,
        failed_outside_set,
        effective_count,
    )
}

pub fn thermite_ap_cpu_bit(cpu: u64) -> (result: u64)
    requires
        cpu < 64,
    ensures
        result == 1u64 << cpu,
{
    super::ap_cpu_bit(cpu)
}

pub fn thermite_ap_mask_insert(mask: u64, cpu: u64) -> (result: u64)
    requires
        cpu < 64,
    ensures
        result == mask | (1u64 << cpu),
{
    super::ap_mask_insert(mask, cpu)
}

pub fn thermite_ap_expected_mask(apic_ids: &[u32]) -> (result: u64)
    requires
        apic_ids.len() > 0,
        apic_ids.len() <= 8,
    ensures
        result == super::ap_expected_mask_spec(apic_ids@),
{
    super::ap_expected_mask(apic_ids)
}

pub fn thermite_apic_profile_supported(msr_value: u64) -> (result: bool)
    ensures
        result == (msr_value & 2048 != 0 && msr_value & 1024 == 0),
{
    super::apic_profile_supported(msr_value)
}

pub fn thermite_apic_physical_base(msr_value: u64) -> (result: u64)
    requires
        msr_value & 2048 != 0,
        msr_value & 1024 == 0,
    ensures
        result == msr_value & 4294963200,
{
    super::apic_physical_base(msr_value)
}

pub fn thermite_ap_should_start(
    apic_id: u64,
    failure_present: bool,
    failed_apic_id: u64,
) -> (result: bool)
    ensures
        result == (!failure_present || apic_id != failed_apic_id),
{
    super::ap_should_start(apic_id, failure_present, failed_apic_id)
}

pub fn thermite_ap_runtime_ready(
    online_cpus: u64,
    expected_cpus: u64,
    ready_aps: u64,
    expected_aps: u64,
    cpu_local_cpus: u64,
) -> (result: bool)
    requires
        expected_cpus > 0,
        expected_cpus <= 8,
        expected_aps < expected_cpus,
    ensures
        result == (online_cpus == expected_cpus
            && ready_aps >= expected_aps
            && cpu_local_cpus == expected_cpus),
{
    super::ap_runtime_ready(
        online_cpus,
        expected_cpus,
        ready_aps,
        expected_aps,
        cpu_local_cpus,
    )
}

pub fn thermite_scheduler_runtime_complete(
    task_sum: u64,
    expected_sum: u64,
    worker_cpus: u64,
    required_workers: u64,
    lock_entries: u64,
    expected_lock_entries: u64,
    once_owner: u64,
    parallel_cpus: u64,
    required_parallel_cpus: u64,
) -> (result: bool)
    ensures
        result == (task_sum == expected_sum
            && worker_cpus >= required_workers
            && lock_entries == expected_lock_entries
            && once_owner > 0
            && parallel_cpus >= required_parallel_cpus),
{
    super::scheduler_runtime_complete(
        task_sum,
        expected_sum,
        worker_cpus,
        required_workers,
        lock_entries,
        expected_lock_entries,
        once_owner,
        parallel_cpus,
        required_parallel_cpus,
    )
}

pub fn thermite_shootdown_runtime_complete(
    observed_cpus: u64,
    expected_cpus: u64,
    stale_cpus: u64,
    acknowledged_cpus: u64,
    expected_acknowledgements: u64,
    epoch: u64,
    expected_epoch: u64,
) -> (result: bool)
    requires
        expected_cpus > 0,
        expected_cpus <= 8,
        expected_acknowledgements < expected_cpus,
    ensures
        result == (observed_cpus == expected_cpus
            && stale_cpus == 0
            && acknowledged_cpus == expected_acknowledgements
            && epoch == expected_epoch),
{
    super::shootdown_runtime_complete(
        observed_cpus,
        expected_cpus,
        stale_cpus,
        acknowledged_cpus,
        expected_acknowledgements,
        epoch,
        expected_epoch,
    )
}

pub fn thermite_pci_config_address(
    bus: u64,
    device: u64,
    function: u64,
    offset: u64,
) -> (result: u64)
    requires
        bus <= 255,
        device < 32,
        function < 8,
        offset <= 255,
    ensures
        result
            == 2147483648 + bus * 65536 + device * 2048 + function * 256 + (offset / 4) * 4,
{
    super::pci_config_address(bus, device, function, offset)
}

pub fn thermite_pci_virtio_block_identity(identity: u64) -> (result: bool)
    requires
        identity <= 4294967295,
    ensures
        result
            == (identity % 65536 == 6900
                && (identity / 65536 == 4097 || identity / 65536 == 4162)),
{
    super::pci_virtio_block_identity(identity)
}

pub fn thermite_pci_legacy_io_bar_valid(bar: u64) -> (result: bool)
    requires
        bar <= 4294967295,
    ensures
        result == (bar % 2 == 1 && bar < 65536),
{
    super::pci_legacy_io_bar_valid(bar)
}

pub fn thermite_pci_legacy_io_base(bar: u64) -> (result: u64)
    requires
        bar <= 4294967295,
        bar % 2 == 1,
        bar < 65536,
    ensures
        result == (bar / 4) * 4,
{
    super::pci_legacy_io_base(bar)
}

pub fn thermite_pci_enable_io_bus_master(command: u64) -> (result: u64)
    requires
        command <= 4294967295,
    ensures
        result == command + (1 - command % 2) + (1 - (command / 4) % 2) * 4,
{
    super::pci_enable_io_bus_master(command)
}

pub fn thermite_dma_register_port(io_base: u64, offset: u64) -> (result: u64)
    requires
        io_base < 65536,
        io_base % 4 == 0,
        offset <= 18,
        io_base + offset < 65536,
    ensures
        result == io_base + offset,
{
    super::dma_register_port(io_base, offset)
}

pub fn thermite_dma_queue_pfn(queue_address: u64) -> (result: u64)
    requires
        queue_address <= 4294967295,
        queue_address % 4096 == 0,
    ensures
        result == queue_address / 4096,
{
    super::dma_queue_pfn(queue_address)
}

pub fn thermite_dma_descriptor_length(index: u64) -> (result: u64)
    requires
        index < 3,
    ensures
        (index == 0 && result == 16)
            || (index == 1 && result == 512)
            || (index == 2 && result == 1),
{
    super::dma_descriptor_length(index)
}

pub fn thermite_dma_descriptor_flags(index: u64) -> (result: u64)
    requires
        index < 3,
    ensures
        (index == 0 && result == 1)
            || (index == 1 && result == 3)
            || (index == 2 && result == 2),
{
    super::dma_descriptor_flags(index)
}

pub fn thermite_dma_descriptor_next(index: u64) -> (result: u64)
    requires
        index < 3,
    ensures
        (index == 0 && result == 1)
            || (index == 1 && result == 2)
            || (index == 2 && result == 0),
{
    super::dma_descriptor_next(index)
}

pub fn thermite_dma_device_status(step: u64) -> (result: u64)
    requires
        step < 4,
    ensures
        (step == 0 && result == 0)
            || (step == 1 && result == 1)
            || (step == 2 && result == 3)
            || (step == 3 && result == 7),
{
    super::dma_device_status(step)
}

pub fn thermite_dma_device_ready(status: u64) -> (result: bool)
    requires
        status <= 255,
    ensures
        result == (status % 8 == 7),
{
    super::dma_device_ready(status)
}

pub fn thermite_dma_publish_available(current_index: u64) -> (result: u64)
    requires
        current_index < 65535,
    ensures
        result == current_index + 1,
{
    super::dma_publish_available(current_index)
}

pub fn thermite_dma_runtime_complete(
    completion_seen: bool,
    status: u64,
    signature_low: u64,
    signature_high: u64,
    bytes: u64,
    generation: u64,
) -> (result: bool)
    ensures
        result == (completion_seen
            && status == 0
            && signature_low == 85
            && signature_high == 170
            && bytes == 512
            && generation == 1),
{
    super::dma_runtime_complete(
        completion_seen,
        status,
        signature_low,
        signature_high,
        bytes,
        generation,
    )
}

pub fn thermite_dma_queue_size_valid(queue_size: u64) -> (result: bool)
    ensures
        result == (queue_size >= 3 && queue_size <= 256),
{
    super::dma_queue_size_valid(queue_size)
}

pub fn thermite_dma_descriptor_bytes(queue_size: u64) -> (result: u64)
    requires
        queue_size >= 3,
        queue_size <= 256,
    ensures
        result == queue_size * 16,
{
    super::dma_descriptor_bytes(queue_size)
}

pub fn thermite_dma_used_offset(queue_size: u64) -> (result: u64)
    requires
        queue_size >= 3,
        queue_size <= 256,
    ensures
        result == ((queue_size * 18 + 4101) / 4096) * 4096,
{
    super::dma_used_offset(queue_size)
}

pub fn thermite_dma_queue_layout_valid(queue_size: u64, capacity: u64) -> (result: bool)
    requires
        queue_size >= 3,
        queue_size <= 256,
    ensures
        result == (((queue_size * 18 + 4101) / 4096) * 4096 + 6 + queue_size * 8
            <= capacity),
{
    super::dma_queue_layout_valid(queue_size, capacity)
}

pub fn thermite_service_user_base() -> (result: u64)
    ensures
        result == 70368744177664,
{
    super::service_user_base()
}

pub fn thermite_service_user_stack(user_base: u64) -> (result: u64)
    requires
        user_base <= 18446744073709547519,
    ensures
        result == user_base + 4096,
{
    super::service_user_stack(user_base)
}

pub fn thermite_service_user_fault_address(user_base: u64) -> (result: u64)
    requires
        user_base <= 18446744073709543423,
    ensures
        result == user_base + 8192,
{
    super::service_user_fault_address(user_base)
}

pub fn thermite_service_user_stack_pointer(user_stack: u64) -> (result: u64)
    requires
        user_stack <= 18446744073709547535,
    ensures
        result == user_stack + 4080,
{
    super::service_user_stack_pointer(user_stack)
}

pub fn thermite_service_syscall_value() -> (result: u64)
    ensures
        result == 4660,
{
    super::service_syscall_value()
}

pub fn thermite_service_finish_value() -> (result: u64)
    ensures
        result == 22136,
{
    super::service_finish_value()
}

pub fn thermite_service_write_user_byte(
    data: &mut [u8],
    at: usize,
    value: u8,
) -> (result: u8)
    requires
        at < data.len(),
    ensures
        result == value,
        final(data)@[at as int] == value,
{
    super::service_write_user_byte(data, at, value)
}

pub fn thermite_service_runtime_complete(
    syscalls: u64,
    faults: u64,
    finished: u64,
    syscall_value: u64,
    finish_value: u64,
    kernel_faults: u64,
) -> (result: bool)
    ensures
        result == (syscalls == 1
            && faults == 1
            && finished == 1
            && syscall_value == 4660
            && finish_value == 22136
            && kernel_faults == 0),
{
    super::service_runtime_complete(
        syscalls,
        faults,
        finished,
        syscall_value,
        finish_value,
        kernel_faults,
    )
}

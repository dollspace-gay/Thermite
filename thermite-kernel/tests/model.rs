use std::collections::BTreeSet;

use thermite_kernel::atomic::validate_compare_exchange;
use thermite_kernel::memory::is_canonical_x86_64;
use thermite_kernel::{
    lookup, Action, ActionExecutor, AddressSpace, AtomicCell, AtomicError, AtomicMemoryModel,
    AtomicOrdering, AtomicWidth, Barrier, Bitmap, BoundaryDeclaration, BoundaryInventory,
    BoundedMpsc, CapabilityError, CapabilityKind, CapabilityLedger, ClosureError, ContextError,
    CpuLifecycle, CpuSet, DeviceBarrier, DeviceBus, DeviceError, DeviceWidth, DmaDirection,
    DmaError, DmaOwnership, DmaPin, DmaState, EventSequencer, FenceKind, FixedMap, FixedVec,
    FrameAllocator, FrameError, IommuDomain, IommuMode, IrqController, IrqError, KernelPolicy,
    MapError, OnceCell, PagePermissions, PageSize, PlatformServices, PolicyError, Registers,
    Rights, RingBuffer, Scheduler, ServiceError, SmpError, SmpState, StorageError, SyncError,
    TemporaryMapWindow, TerminalState, TicketLock, TrapOrigin, UserContext, UserMemory,
    X86_64_PC_UEFI_SMP_V1, X86_64_PC_UEFI_SMP_V1_OPERATION_COUNT,
};

#[test]
fn stale_foreign_and_escalated_capabilities_fail_closed() {
    let mut ledger = CapabilityLedger::new();
    let root = ledger
        .mint_root(
            CapabilityKind::Frame,
            7,
            1,
            Rights::READ.union(Rights::WRITE).union(Rights::TRANSFER),
            0x20_0000,
            0x20_0000,
        )
        .expect("mint frame root");
    let moved = ledger
        .transfer(&root, 2, Rights::READ.union(Rights::WRITE))
        .expect("transfer frame authority");

    assert_eq!(
        ledger.validate(&root, 1, Rights::READ, root.base(), 1),
        Err(CapabilityError::StaleGeneration)
    );
    assert_eq!(
        ledger.validate(&moved, 1, Rights::READ, moved.base(), 1),
        Err(CapabilityError::ForeignOwner)
    );
    assert_eq!(
        ledger.transfer(&moved, 3, Rights::ALL),
        Err(CapabilityError::RightsEscalation)
    );
    ledger.release(&moved).expect("release live generation");
    assert_eq!(ledger.release(&moved), Err(CapabilityError::Released));
}

#[test]
fn action_executor_checks_kind_owner_rights_generation_and_exact_range() {
    let mut ledger = CapabilityLedger::new();
    let mmio = ledger
        .mint_root(
            CapabilityKind::Mmio,
            90,
            7,
            Rights::READ.union(Rights::WRITE).union(Rights::TRANSFER),
            0xfee0_0000,
            0x1000,
        )
        .expect("mint MMIO capability");
    let executor = ActionExecutor::new();
    let read = Action::MmioRead {
        address: 0xfee0_0010,
        width: 4,
    };
    let authorized = executor
        .authorize(&ledger, 7, &mmio, read.clone())
        .expect("exact authority");
    assert_eq!(authorized.capability_slot, 90);
    assert_eq!(authorized.action, read);
    assert_eq!(
        executor.authorize(&ledger, 8, &mmio, read.clone()),
        Err(thermite_kernel::PlatformError::Capability(
            CapabilityError::ForeignOwner
        ))
    );
    assert_eq!(
        executor.authorize(
            &ledger,
            7,
            &mmio,
            Action::MmioRead {
                address: 0xfee0_1000,
                width: 4,
            },
        ),
        Err(thermite_kernel::PlatformError::Capability(
            CapabilityError::OutOfRange
        ))
    );
    assert_eq!(
        executor.authorize(
            &ledger,
            7,
            &mmio,
            Action::PioRead {
                port: 0x3f8,
                width: 1,
            },
        ),
        Err(thermite_kernel::PlatformError::Capability(
            CapabilityError::WrongKind
        ))
    );

    let moved = ledger
        .transfer(&mmio, 9, Rights::READ.union(Rights::WRITE))
        .expect("advance generation");
    assert_eq!(
        executor.authorize(&ledger, 7, &mmio, read.clone()),
        Err(thermite_kernel::PlatformError::Capability(
            CapabilityError::StaleGeneration
        ))
    );
    assert!(executor.authorize(&ledger, 9, &moved, read).is_ok());
}

#[test]
fn atomics_validate_backing_alignment_ordering_and_modification_order() {
    let mut ledger = CapabilityLedger::new();
    let backing = ledger
        .mint_root(
            CapabilityKind::CpuLocal,
            1,
            0,
            Rights::READ.union(Rights::WRITE),
            0x1000,
            0x1000,
        )
        .expect("mint cpu-local backing");
    assert_eq!(
        AtomicCell::create(&backing, AtomicWidth::U64, 0x1004, 0),
        Err(AtomicError::Misaligned)
    );
    let mut cell =
        AtomicCell::create(&backing, AtomicWidth::U64, 0x1008, 0).expect("create aligned atomic");
    assert_eq!(
        cell.load(AtomicOrdering::Release),
        Err(AtomicError::IllegalLoadOrdering)
    );
    assert_eq!(
        validate_compare_exchange(AtomicOrdering::Relaxed, AtomicOrdering::Acquire),
        Err(AtomicError::FailureStrongerThanSuccess)
    );
    for _ in 0..64 {
        cell.fetch_add(1, AtomicOrdering::AcqRel)
            .expect("fetch_add");
    }
    assert_eq!(cell.load(AtomicOrdering::Acquire), Ok(64));
    assert_eq!(cell.modification_order(), 64);

    let weak = cell
        .compare_exchange_weak(
            64,
            65,
            AtomicOrdering::AcqRel,
            AtomicOrdering::Acquire,
            true,
        )
        .expect("legal weak CAS");
    assert!(!weak.exchanged);
    assert_eq!(cell.load(AtomicOrdering::Acquire), Ok(64));
}

#[test]
fn atomic_trace_records_reads_from_happens_before_modification_and_sc_orders() {
    let mut model = AtomicMemoryModel::new();
    model.register(1, 0).expect("register payload");
    model.register(2, 0).expect("register ready");
    let payload_write = model
        .store(0, 1, 0x55aa, AtomicOrdering::Relaxed)
        .expect("write payload");
    let ready_release = model
        .store(0, 2, 1, AtomicOrdering::Release)
        .expect("publish ready");
    let (ready, ready_acquire) = model
        .load(1, 2, AtomicOrdering::Acquire)
        .expect("observe ready");
    let (payload, _) = model
        .load(1, 1, AtomicOrdering::Relaxed)
        .expect("read payload after acquire");
    assert_eq!((ready, payload), (1, 0x55aa));
    let acquire_event = model
        .events()
        .iter()
        .find(|event| event.id == ready_acquire)
        .expect("acquire event");
    assert_eq!(acquire_event.reads_from, Some(ready_release));
    assert_eq!(acquire_event.happens_after_release, Some(ready_release));
    assert!(payload_write < ready_release);
    assert!(thermite_verified::kernel_release_acquire_visible(
        payload_write,
        ready_release,
        acquire_event.reads_from.expect("reads-from release"),
    ));

    let (_, rmw) = model
        .fetch_add(1, 1, 1, AtomicOrdering::SeqCst)
        .expect("SC RMW");
    let fence = model
        .fence(1, FenceKind::Hardware, AtomicOrdering::SeqCst)
        .expect("SC fence");
    let rmw_event = model
        .events()
        .iter()
        .find(|event| event.id == rmw)
        .expect("RMW event");
    let fence_event = model
        .events()
        .iter()
        .find(|event| event.id == fence)
        .expect("fence event");
    assert_eq!(rmw_event.modification_order, Some(2));
    assert!(rmw_event.sequentially_consistent_order < fence_event.sequentially_consistent_order);
    assert_eq!(
        model.fence(0, FenceKind::Compiler, AtomicOrdering::Relaxed),
        Err(AtomicError::IllegalFenceOrdering)
    );
}

#[test]
fn mappings_are_capability_bounded_aligned_and_epoch_ordered() {
    let mut ledger = CapabilityLedger::new();
    let space = ledger
        .mint_root(CapabilityKind::AddressSpace, 1, 0, Rights::MAP, 0, 0)
        .expect("mint address space");
    let frames = ledger
        .mint_root(
            CapabilityKind::Frame,
            2,
            0,
            Rights::READ.union(Rights::WRITE),
            0x20_0000,
            0x40_0000,
        )
        .expect("mint frames");
    let mut mappings = AddressSpace::new();
    let mapped = mappings
        .map(
            &space,
            &frames,
            0xffff_8000_0000_0000,
            PageSize::Size2M,
            1,
            PagePermissions::READ.union(PagePermissions::WRITE),
        )
        .expect("map canonical aligned page");
    assert_eq!(mapped.epoch, 1);
    assert_eq!(
        mappings.map(
            &space,
            &frames,
            0xffff_8000_0000_1000,
            PageSize::Size2M,
            1,
            PagePermissions::READ,
        ),
        Err(MapError::Misaligned)
    );
    mappings
        .unmap(0xffff_8000_0000_0000)
        .expect("unmap exact mapping");
    assert_eq!(mappings.epoch(), 2);
    assert!(is_canonical_x86_64(0xffff_8000_0000_0000));
    assert!(!is_canonical_x86_64(0x0000_8000_0000_0000));
}

#[test]
fn ap_lifecycle_ipi_and_shootdown_reject_stale_or_missing_acknowledgements() {
    let mut smp = SmpState::new();
    for cpu in 0..4 {
        smp.discover(cpu).expect("unique CPU");
        smp.transition(cpu, CpuLifecycle::Prepared)
            .expect("prepare");
        smp.transition(cpu, CpuLifecycle::Starting).expect("start");
        smp.transition(cpu, CpuLifecycle::Online).expect("online");
    }
    assert_eq!(smp.online().len(), 4);
    let ipi = smp
        .begin_ipi(CpuSet::from_ids([1, 2, 3]))
        .expect("begin IPI");
    assert_eq!(smp.acknowledge_ipi(ipi, 0), Err(SmpError::ForeignCpu));
    assert_eq!(smp.acknowledge_ipi(ipi, 1), Ok(false));
    assert_eq!(
        smp.acknowledge_ipi(ipi, 1),
        Err(SmpError::DuplicateAcknowledgement)
    );

    let shootdown = smp.begin_shootdown().expect("begin shootdown");
    for cpu in 0..3 {
        assert_eq!(smp.acknowledge_shootdown(shootdown, cpu), Ok(false));
    }
    assert_eq!(smp.complete_shootdown(shootdown), Err(SmpError::Incomplete));
    assert_eq!(smp.acknowledge_shootdown(shootdown, 3), Ok(true));
    smp.complete_shootdown(shootdown)
        .expect("all online CPUs acknowledged current epoch");
}

#[test]
fn dma_generation_binds_device_and_domain() {
    let mut ledger = CapabilityLedger::new();
    let region = ledger
        .mint_root(
            CapabilityKind::Dma,
            9,
            0,
            Rights::READ.union(Rights::WRITE).union(Rights::TRANSFER),
            0x4000,
            0x1000,
        )
        .expect("mint DMA region");
    let mut dma = DmaState::new();
    let mapping = dma
        .pin(
            &region,
            DmaPin {
                slot: 4,
                device: 10,
                domain: 2,
                base: 0x4000,
                len: 0x800,
                direction: DmaDirection::FromDevice,
            },
        )
        .expect("pin DMA mapping");
    assert_eq!(
        dma.validate(4, &mapping, 11, 2),
        Err(DmaError::ForeignDevice)
    );
    assert_eq!(
        dma.validate(4, &mapping, 10, 3),
        Err(DmaError::ForeignDomain)
    );
    dma.unpin(4, &mapping).expect("unpin current generation");
    assert_eq!(
        dma.validate(4, &mapping, 10, 2),
        Err(DmaError::UnknownMapping)
    );
}

#[test]
fn registry_names_are_canonical_unique_and_exact_match_is_fail_closed() {
    let mut names = BTreeSet::new();
    for entry in X86_64_PC_UEFI_SMP_V1 {
        assert!(entry.name().starts_with("kernel::"));
        assert!(entry.name().ends_with("@v1"));
        assert!(names.insert(entry.name()), "duplicate {}", entry.name());
        assert_eq!(lookup(entry.name()), Ok(entry));
        assert_eq!(entry.exact_match(entry), Ok(()));
    }
    assert_eq!(
        X86_64_PC_UEFI_SMP_V1.len(),
        X86_64_PC_UEFI_SMP_V1_OPERATION_COUNT
    );
    assert!(lookup("kernel::memory::fabricated@v1").is_err());
}

#[test]
fn bootstrap_storage_is_bounded_content_preserving_and_allocation_free() {
    let mut values = FixedVec::<u16, 3>::new();
    values.push(7).expect("push 7");
    values.push(9).expect("push 9");
    values.set(1, 11).expect("replace element");
    assert_eq!(values.as_slice(), &[7, 11]);
    values.push(13).expect("fill capacity");
    assert_eq!(values.push(17), Err(StorageError::Full));

    let mut map = FixedMap::<u16, u64, 2>::new();
    map.insert(1, 10).expect("insert one");
    map.insert(2, 20).expect("insert two");
    assert_eq!(map.insert(1, 30), Err(StorageError::DuplicateKey));
    assert_eq!(map.remove(1), Ok(10));
    assert_eq!(map.get(2), Ok(20));

    let mut bits = Bitmap::<2>::new();
    bits.set(65, true).expect("set in-range bit");
    assert_eq!(bits.get(65), Ok(true));
    assert_eq!(bits.get(128), Err(StorageError::OutOfBounds));

    let mut ring = RingBuffer::<u32, 2>::new();
    ring.push(1).expect("enqueue one");
    ring.push(2).expect("enqueue two");
    assert_eq!(ring.push(3), Err(StorageError::Full));
    assert_eq!(ring.pop(), Ok(1));
    ring.push(3).expect("wrap tail");
    assert_eq!(ring.pop(), Ok(2));
    assert_eq!(ring.pop(), Ok(3));
}

#[test]
fn frame_allocator_preserves_disjointness_alignment_and_generation() {
    let mut allocator = FrameAllocator::new();
    allocator.add_region(0x10_0000, 32).expect("add region");
    let first = allocator.allocate(3, 4).expect("aligned allocation");
    assert_eq!(first.base % (4 * FrameAllocator::PAGE_SIZE), 0);
    let zeroed = allocator.mark_zeroed(first).expect("zero run");
    assert!(zeroed.zeroed);
    allocator
        .release(zeroed)
        .expect("release current generation");
    assert_eq!(allocator.release(zeroed), Err(FrameError::UnknownRun));
    assert_eq!(allocator.free_pages(), 32);

    let run = thermite_kernel::FrameRun {
        base: 0x20_0000,
        pages: 8,
        generation: 4,
        zeroed: true,
    };
    let (left, right) = FrameAllocator::split(run, 3).expect("split");
    assert_eq!(FrameAllocator::join(left, right), Ok(run));
}

#[test]
fn frame_split_lend_reclaim_and_join_consume_exact_generations() {
    let mut allocator = FrameAllocator::new();
    allocator.add_region(0x40_0000, 16).expect("add region");
    let whole = allocator.allocate(8, 1).expect("allocate exact run");
    let (left, right) = allocator.split_allocated(whole, 3).expect("stateful split");
    assert!(thermite_verified::kernel_frame_partition(
        whole.pages,
        left.pages,
        right.pages
    ));
    assert_eq!(allocator.release(whole), Err(FrameError::UnknownRun));
    let joined = allocator
        .join_allocated(left, right)
        .expect("stateful join");
    let lent = allocator.lend(joined, 17).expect("lend to device");
    assert!(thermite_verified::kernel_generation_transition(
        joined.generation,
        lent.generation
    ));
    assert_eq!(allocator.release(joined), Err(FrameError::StaleGeneration));
    assert_eq!(
        allocator.reclaim(lent, 18),
        Err(FrameError::ForeignBorrower)
    );
    let reclaimed = allocator.reclaim(lent, 17).expect("matching reclaim");
    assert!(thermite_verified::kernel_generation_transition(
        lent.generation,
        reclaimed.generation
    ));
    assert_eq!(
        allocator.reclaim(lent, 17),
        Err(FrameError::StaleGeneration)
    );
    allocator.release(reclaimed).expect("release reclaimed run");
    assert_eq!(allocator.free_pages(), 16);
}

#[test]
fn temporary_maps_and_user_copies_are_generation_safe_and_all_or_nothing() {
    let mut ledger = CapabilityLedger::new();
    let space = ledger
        .mint_root(CapabilityKind::AddressSpace, 1, 0, Rights::MAP, 0, 0)
        .expect("address space");
    let frame = ledger
        .mint_root(
            CapabilityKind::Frame,
            2,
            0,
            Rights::READ.union(Rights::WRITE),
            0x80_0000,
            4096,
        )
        .expect("frame");
    let window_cap = ledger
        .mint_root(
            CapabilityKind::VirtRegion,
            3,
            0,
            Rights::MAP,
            0xffff_9000_0000_0000,
            4096,
        )
        .expect("temporary window");
    let mut window = TemporaryMapWindow::new(0xffff_9000_0000_0000, 4096);
    let temporary = window.map(&window_cap, &frame).expect("temporary map");
    window.revoke(temporary).expect("revoke current generation");
    assert_eq!(window.revoke(temporary), Err(MapError::StaleGeneration));
    let replacement = window
        .map(&window_cap, &frame)
        .expect("map next generation");
    assert!(replacement.generation > temporary.generation);
    assert!(thermite_verified::kernel_generation_transition(
        temporary.generation,
        replacement.generation
    ));

    let mut address_space = AddressSpace::new();
    address_space
        .map(
            &space,
            &frame,
            0x4000,
            PageSize::Size4K,
            1,
            PagePermissions::READ
                .union(PagePermissions::WRITE)
                .union(PagePermissions::USER),
        )
        .expect("user mapping");
    let mut memory = UserMemory::new();
    memory
        .initialize(frame.base(), &[1, 2, 3, 4])
        .expect("initialize user bytes");
    let mut copied = [0_u8; 4];
    address_space
        .copy_from_user(&memory, 0x4000, &mut copied)
        .expect("copy all bytes");
    assert_eq!(copied, [1, 2, 3, 4]);
    assert!(thermite_verified::kernel_user_range_legal(
        0x4000,
        copied.len() as u64,
        0x0000_8000_0000_0000,
    ));
    let mut unchanged = [0xaa_u8; 5];
    assert_eq!(
        address_space.copy_from_user(&memory, 0x4000, &mut unchanged),
        Err(MapError::UserFault)
    );
    assert_eq!(unchanged, [0xaa; 5]);
    address_space
        .copy_to_user(&mut memory, 0x4001, &[9, 8])
        .expect("copy out atomically");
    assert_eq!(memory.byte(frame.base() + 1), Some(9));
    address_space.activate(&space, 0).expect("activate");
    assert_eq!(address_space.destroy(&space), Err(MapError::Active));
    address_space.deactivate(&space, 0).expect("deactivate");
    address_space.unmap(0x4000).expect("unmap");
    address_space
        .destroy(&space)
        .expect("destroy inactive root");
    assert_eq!(address_space.translate(0x4000), Err(MapError::Destroyed));
}

#[test]
fn iommu_mapping_binds_device_domain_aperture_and_dma_ownership() {
    let mut ledger = CapabilityLedger::new();
    let region = ledger
        .mint_root(
            CapabilityKind::Dma,
            8,
            0,
            Rights::READ.union(Rights::WRITE),
            0x20_0000,
            0x4000,
        )
        .expect("DMA region");
    let domain_cap = ledger
        .mint_root(
            CapabilityKind::IommuDomain,
            7,
            0,
            Rights::MAP,
            0x1_0000_0000,
            0x10_0000,
        )
        .expect("IOMMU domain");
    let mut dma = DmaState::new();
    dma.register_domain(IommuDomain {
        id: 7,
        device: 42,
        aperture_base: 0x1_0000_0000,
        aperture_len: 0x10_0000,
        mode: IommuMode::Present,
    })
    .expect("register present IOMMU");
    let mapping = dma
        .pin(
            &region,
            DmaPin {
                slot: 5,
                device: 42,
                domain: 7,
                base: 0x20_0000,
                len: 0x2000,
                direction: DmaDirection::Bidirectional,
            },
        )
        .expect("pin");
    assert_eq!(
        dma.map_domain(5, &mapping, None, 0x1_0000_1000),
        Err(DmaError::DomainCapability)
    );
    let segments = dma
        .map_domain(5, &mapping, Some(&domain_cap), 0x1_0000_1000)
        .expect("map into IOMMU");
    assert_eq!(segments[0].device_address, 0x1_0000_1000);
    let device_owned = dma.sync_for_device(5, &mapping).expect("publish DMA");
    assert_eq!(
        dma.unmap_domain(5, &device_owned),
        Err(DmaError::WrongOwnership)
    );
    let cpu_owned = dma.sync_for_cpu(5, &device_owned).expect("reclaim DMA");
    dma.unmap_domain(5, &cpu_owned).expect("unmap IOMMU");
    dma.unpin(5, &cpu_owned).expect("unpin current generation");
}

#[test]
fn volatile_device_models_reject_width_alignment_range_and_rights_errors() {
    let mut ledger = CapabilityLedger::new();
    let mmio = ledger
        .mint_root(
            CapabilityKind::Mmio,
            1,
            0,
            Rights::READ.union(Rights::WRITE),
            0xf000_0000,
            0x100,
        )
        .expect("mint MMIO");
    let pio = ledger
        .mint_root(
            CapabilityKind::IoPort,
            2,
            0,
            Rights::READ.union(Rights::WRITE),
            0x3f8,
            8,
        )
        .expect("mint PIO");
    let mut bus = DeviceBus::new();
    bus.mmio_write(&mmio, 0xf000_0004, DeviceWidth::U32, 0xfeed_beef)
        .expect("write MMIO");
    assert_eq!(
        bus.mmio_read(&mmio, 0xf000_0004, DeviceWidth::U32),
        Ok(0xfeed_beef)
    );
    assert_eq!(
        bus.mmio_read(&mmio, 0xf000_0001, DeviceWidth::U32),
        Err(DeviceError::Misaligned)
    );
    assert_eq!(
        bus.mmio_read(&mmio, 0xf000_0100, DeviceWidth::U8),
        Err(DeviceError::OutOfRange)
    );
    bus.pio_write(&pio, 0x3f8, DeviceWidth::U8, 0x54)
        .expect("write PIO");
    assert_eq!(bus.pio_read(&pio, 0x3f8, DeviceWidth::U8), Ok(0x54));
    assert_eq!(bus.barrier(DeviceBarrier::Full), Ok(1));
}

#[test]
fn interrupt_tokens_are_cpu_local_single_use_and_generation_checked() {
    let mut ledger = CapabilityLedger::new();
    let local = ledger
        .mint_root(CapabilityKind::CpuLocal, 1, 0, Rights::CONTROL, 0, 0)
        .expect("mint CPU-local authority");
    let irq = ledger
        .mint_root(
            CapabilityKind::Irq,
            2,
            0,
            Rights::ROUTE.union(Rights::CONTROL),
            32,
            224,
        )
        .expect("mint IRQ authority");
    let mut controller = IrqController::new();
    controller.add_cpu(0).expect("add BSP");
    controller.add_cpu(1).expect("add AP");
    let token = controller
        .save_disable(0, &local)
        .expect("disable BSP IRQs");
    assert_eq!(controller.interrupts_enabled(0), Some(false));
    assert_eq!(controller.interrupts_enabled(1), Some(true));
    assert_eq!(
        controller.restore(1, &local, token),
        Err(IrqError::ForeignCpu)
    );
    controller
        .restore(0, &local, token)
        .expect("restore BSP IRQs");
    assert_eq!(
        controller.restore(0, &local, token),
        Err(IrqError::AlreadyRestored)
    );
    controller.route(&irq, 48, 1).expect("route device IRQ");
    assert_eq!(controller.deliver(48), Ok(1));
    controller.end_of_interrupt(1, 48).expect("EOI");
}

#[test]
fn scheduler_moves_unique_task_ownership_and_steals_without_global_lock() {
    let cpus = CpuSet::from_ids([0, 1, 2, 3]);
    let mut scheduler = Scheduler::with_online_cpus(&cpus);
    for task in 0..16 {
        scheduler.create_task(task, 0).expect("create task");
    }
    for cpu in 0..4 {
        assert!(scheduler.dispatch(cpu).expect("dispatch").is_some());
    }
    assert_eq!(scheduler.steals(), 3);
    for cpu in 1..4 {
        assert!(thermite_verified::kernel_task_owner_transition(
            0,
            cpu,
            cpus.contains(cpu),
        ));
    }
    assert!(scheduler.invariant_holds());
    let blocked = scheduler.block_current(1).expect("block AP task");
    scheduler.wake(blocked, 3).expect("cross-CPU wakeup");
    scheduler.yield_current(0).expect("yield BSP task");
    scheduler.exit_current(2, 0).expect("exit task");
    assert!(scheduler.invariant_holds());
}

#[test]
fn synchronization_models_reject_duplicate_stale_and_capacity_failures() {
    let mut lock = TicketLock::new();
    let first = lock.enqueue().expect("ticket zero");
    let second = lock.enqueue().expect("ticket one");
    assert_eq!(lock.acquire(second), Err(SyncError::OutOfOrderRelease));
    let guard = lock.acquire(first).expect("acquire first");
    lock.release(guard).expect("release first");
    assert!(lock.acquire(second).is_ok());

    let mut once = OnceCell::new();
    assert_eq!(once.initialize(7), Ok(&7));
    assert_eq!(once.initialize(9), Err(SyncError::AlreadyInitialized));

    let mut queue = BoundedMpsc::new(2);
    queue.push(0, 10).expect("producer zero");
    queue.push(1, 11).expect("producer one");
    assert_eq!(queue.push(2, 12), Err(SyncError::QueueFull));
    assert_eq!(queue.pop(), Ok((0, 10)));
    assert!(queue.accounting_holds());

    let mut barrier = Barrier::new(CpuSet::from_ids([0, 1]));
    assert_eq!(barrier.arrive(0, 0), Ok(false));
    assert_eq!(barrier.arrive(0, 0), Err(SyncError::DuplicateParticipant));
    assert_eq!(barrier.arrive(1, 0), Ok(true));
    assert_eq!(barrier.advance(), Ok(1));
    assert_eq!(barrier.arrive(0, 0), Err(SyncError::StaleGeneration));
}

#[test]
fn user_context_syscall_and_fault_frames_preserve_context_identity() {
    let mut ledger = CapabilityLedger::new();
    let context_cap = ledger
        .mint_root(CapabilityKind::UserContext, 1, 0, Rights::CONTROL, 0, 0)
        .expect("mint user context");
    let trap_cap = ledger
        .mint_root(CapabilityKind::TrapFrame, 2, 0, Rights::CONTROL, 0, 0)
        .expect("mint trap frame");
    let mut context = UserContext::create(&context_cap, 7, 3, 0x400000, 0x7fff_ff00)
        .expect("create user context");
    let registers = Registers {
        instruction_pointer: 0x400010,
        stack_pointer: 0x7fff_ff00,
        flags: 0x202,
        argument0: 41,
        result: 0,
    };
    let syscall = context
        .trap(&trap_cap, TrapOrigin::Syscall(1), registers)
        .expect("construct syscall frame");
    context
        .resume(&context_cap, syscall, 42)
        .expect("resume syscall");
    assert_eq!(context.generation(), 1);
    assert_eq!(
        UserContext::create(&context_cap, 8, 3, 0x400000, 0x7fff_ff01),
        Err(ContextError::MisalignedStack)
    );
}

#[test]
fn clock_entropy_and_power_authority_fail_closed() {
    let mut ledger = CapabilityLedger::new();
    let clock = ledger
        .mint_root(
            CapabilityKind::Clock,
            1,
            0,
            Rights::READ.union(Rights::CONTROL),
            0,
            0,
        )
        .expect("mint clock");
    let entropy = ledger
        .mint_root(CapabilityKind::Entropy, 2, 0, Rights::READ, 0, 0)
        .expect("mint entropy");
    let power = ledger
        .mint_root(CapabilityKind::Power, 3, 0, Rights::CONTROL, 0, 0)
        .expect("mint power");
    let mut services = PlatformServices::new(1, 1_000_000).expect("clock scale");
    services.update_clock(100, 2).expect("advance clock");
    assert_eq!(services.read_clock(&clock).expect("read clock").ticks, 100);
    assert_eq!(
        services.arm_deadline(&clock, 1, 99),
        Err(ServiceError::DeadlineInPast)
    );
    services.arm_deadline(&clock, 1, 150).expect("arm timer");
    let mut bytes = [0; 32];
    services
        .fill_entropy(&entropy, &mut bytes)
        .expect("fill exact entropy bytes");
    assert!(bytes.iter().any(|byte| *byte != 0));
    services
        .terminal(&power, TerminalState::PowerOff)
        .expect("terminal transition");
    assert_eq!(
        services.terminal(&power, TerminalState::Reboot),
        Err(ServiceError::AlreadyTerminal)
    );
}

#[test]
fn event_action_ids_are_monotonic_correlated_and_payload_checked() {
    let mut sequencer = EventSequencer::new();
    let issued = sequencer
        .issue(Action::ArmTimer { deadline: 100 })
        .expect("issue timer");
    let (completion, event) = sequencer
        .complete(&issued, 0, Ok(()))
        .expect("complete timer");
    assert_eq!(completion.action, issued.id);
    assert_eq!(event.correlation, Some(issued.id));
    assert!(thermite_verified::kernel_id_transition(
        issued.id,
        issued.id + 1,
    ));
    assert_eq!(
        sequencer.issue(Action::MmioRead {
            address: 0,
            width: 3,
        }),
        Err(thermite_kernel::PlatformError::InvalidWidth)
    );
}

#[test]
fn strict_event_ingress_checks_cpu_vector_dma_syscall_and_fault_origin() {
    let mut ingress = EventSequencer::with_topology(CpuSet::from_ids([0, 1]), 7);
    ingress.assign_irq(48, 1).expect("assign owned vector");
    ingress.allow_dma_slot(3).expect("allow DMA slot");
    assert_eq!(
        ingress.ingress(2, thermite_kernel::EventKind::Timer, None),
        Err(thermite_kernel::PlatformError::UnknownCpu)
    );
    assert_eq!(
        ingress.ingress(0, thermite_kernel::EventKind::Irq { vector: 48 }, None),
        Err(thermite_kernel::PlatformError::UnownedVector)
    );
    assert!(ingress
        .ingress(1, thermite_kernel::EventKind::Irq { vector: 48 }, None)
        .is_ok());
    assert_eq!(
        ingress.ingress(0, thermite_kernel::EventKind::DmaComplete { slot: 4 }, None,),
        Err(thermite_kernel::PlatformError::UnownedDmaSlot)
    );
    assert!(ingress
        .ingress(0, thermite_kernel::EventKind::DmaComplete { slot: 3 }, None,)
        .is_ok());
    assert_eq!(
        ingress.ingress(0, thermite_kernel::EventKind::Syscall { number: 8 }, None,),
        Err(thermite_kernel::PlatformError::InvalidPayload)
    );
    assert_eq!(
        ingress.ingress(
            0,
            thermite_kernel::EventKind::UserFault {
                address: 0x0000_8000_0000_0000,
                code: 0,
            },
            None,
        ),
        Err(thermite_kernel::PlatformError::InvalidTrapOrigin)
    );
}

#[test]
fn boundary_closure_exactly_matches_every_declared_registry_field() {
    let entry = &X86_64_PC_UEFI_SMP_V1[0];
    let declaration = BoundaryDeclaration {
        name: entry.name(),
        signature: entry.signature,
        contract: entry.contract,
        domain: entry.domain,
        capability: entry.capability,
        rights: entry.rights,
        symbol: entry.symbol,
        source_contract_sha256: entry.source_contract_sha256,
        source_reachable: entry.source_reachable,
    };
    let declarations = [declaration.clone()];
    let inventory = BoundaryInventory::close(&declarations, false).expect("exact match");
    assert_eq!(inventory.names(), &[entry.name()]);
    let mut drift = declaration;
    drift.contract = "weaker";
    assert!(BoundaryInventory::close(&[drift], false).is_err());
    assert_eq!(
        BoundaryInventory::close(&[], false),
        Err(ClosureError::MissingReachableEntry)
    );
}

#[test]
fn dma_requires_explicit_cpu_device_ownership_transitions() {
    let mut ledger = CapabilityLedger::new();
    let region = ledger
        .mint_root(
            CapabilityKind::Dma,
            20,
            0,
            Rights::READ.union(Rights::WRITE),
            0x8000,
            0x1000,
        )
        .expect("mint DMA region");
    let mut dma = DmaState::new();
    let mapping = dma
        .pin(
            &region,
            DmaPin {
                slot: 8,
                device: 4,
                domain: 1,
                base: 0x8000,
                len: 512,
                direction: DmaDirection::ToDevice,
            },
        )
        .expect("pin");
    assert!(thermite_verified::kernel_dma_mapping_legal(
        region.base(),
        region.len(),
        mapping.base,
        mapping.len,
        (4_u64 << 32) | 1,
        (u64::from(mapping.device) << 32) | u64::from(mapping.domain),
    ));
    let device = dma.sync_for_device(8, &mapping).expect("give to device");
    assert_eq!(device.ownership, DmaOwnership::Device);
    assert_eq!(dma.unpin(8, &device), Err(DmaError::WrongOwnership));
    let cpu = dma.sync_for_cpu(8, &device).expect("return to CPU");
    dma.unpin(8, &cpu).expect("unpin CPU-owned generation");
}

#[test]
fn kernel_decisions_match_the_verus_verified_transition_predicates() {
    let orderings = [
        AtomicOrdering::Relaxed,
        AtomicOrdering::Acquire,
        AtomicOrdering::Release,
        AtomicOrdering::AcqRel,
        AtomicOrdering::SeqCst,
    ];
    for (success_tag, success) in orderings.iter().copied().enumerate() {
        for (failure_tag, failure) in orderings.iter().copied().enumerate() {
            assert_eq!(
                validate_compare_exchange(success, failure).is_ok(),
                thermite_verified::kernel_cas_order_legal(success_tag as u8, failure_tag as u8)
            );
        }
    }

    let states = [
        CpuLifecycle::Discovered,
        CpuLifecycle::Prepared,
        CpuLifecycle::Starting,
        CpuLifecycle::Online,
        CpuLifecycle::Failed(thermite_kernel::smp::CpuFailure::StartupTimeout),
    ];
    for (current_tag, current) in states.iter().copied().enumerate() {
        for (next_tag, next) in states.iter().copied().enumerate() {
            let mut smp = SmpState::new();
            smp.discover(0).expect("discover");
            match current {
                CpuLifecycle::Discovered => {}
                CpuLifecycle::Prepared => {
                    smp.transition(0, CpuLifecycle::Prepared).expect("prepare");
                }
                CpuLifecycle::Starting => {
                    smp.transition(0, CpuLifecycle::Prepared).expect("prepare");
                    smp.transition(0, CpuLifecycle::Starting).expect("start");
                }
                CpuLifecycle::Online => {
                    smp.transition(0, CpuLifecycle::Prepared).expect("prepare");
                    smp.transition(0, CpuLifecycle::Starting).expect("start");
                    smp.transition(0, CpuLifecycle::Online).expect("online");
                }
                CpuLifecycle::Failed(reason) => {
                    smp.transition(0, CpuLifecycle::Prepared).expect("prepare");
                    smp.transition(0, CpuLifecycle::Failed(reason))
                        .expect("fail from prepared");
                }
            }
            let production = smp.transition(0, next).is_ok();
            assert_eq!(
                production,
                thermite_verified::kernel_cpu_transition_legal(current_tag as u8, next_tag as u8),
                "lifecycle edge {current_tag}->{next_tag}"
            );
        }
    }

    for base in 0_u64..8 {
        for len in 0_u64..8 {
            for query in 0_u64..16 {
                for query_len in 0_u64..8 {
                    let reference =
                        thermite_verified::kernel_range_contains(base, len, query, query_len);
                    let end = base.checked_add(len);
                    let query_end = query.checked_add(query_len);
                    let model = matches!((end, query_end), (Some(end), Some(query_end)) if query >= base && query_end <= end);
                    assert_eq!(model, reference);
                }
            }
        }
    }

    for targets in 0_u64..16 {
        for acknowledgements in 0_u64..16 {
            for epoch in 0_u64..3 {
                assert_eq!(
                    thermite_verified::kernel_epoch_complete(targets, acknowledgements, epoch, 2,),
                    targets != 0 && targets == acknowledgements && epoch == 2
                );
            }
        }
    }
}

#[test]
fn kernel_policy_steps_typed_events_into_bounded_actions() {
    let mut sequencer = EventSequencer::new();
    let mut policy = KernelPolicy::default();
    policy.add_cpu(0).expect("add BSP shard");
    let boot = sequencer
        .ingress(0, thermite_kernel::EventKind::Boot, None)
        .expect("boot event");
    let actions = policy.step(boot).expect("boot step");
    assert_eq!(actions.len(), 1);
    assert!(matches!(
        actions.iter().next(),
        Some(Action::ArmTimer { deadline: 1 })
    ));

    let timer = sequencer
        .ingress(0, thermite_kernel::EventKind::Timer, None)
        .expect("timer event");
    policy.step(timer).expect("timer step");
    assert_eq!(policy.shard(0).expect("BSP shard").ticks, 1);

    let shutdown = sequencer
        .ingress(0, thermite_kernel::EventKind::ShutdownRequest, None)
        .expect("shutdown request");
    let actions = policy.step(shutdown).expect("terminal step");
    assert!(matches!(actions.iter().next(), Some(Action::PowerOff)));
    let later = sequencer
        .ingress(0, thermite_kernel::EventKind::Timer, None)
        .expect("later timer");
    assert_eq!(policy.step(later), Err(PolicyError::Terminal));
}

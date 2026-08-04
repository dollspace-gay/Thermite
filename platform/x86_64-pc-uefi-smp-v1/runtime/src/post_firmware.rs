//! Reviewed x86_64 target-platform layer used after `ExitBootServices`.
//!
//! This module is deliberately the only part of the image that manipulates
//! raw page tables, descriptor tables, APIC registers, or privilege frames.
//! The safe kernel model consumes the typed report produced by `run`.

use alloc::boxed::Box;
use core::alloc::{GlobalAlloc, Layout};
use core::arch::{asm, global_asm};
use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};
use thermite_kernel_policy::atomic::ExactAtomicU64;
use thermite_kernel_policy::kernel_policy_ingress::{
    thermite_allocator_claim_first as allocator_claim_first,
    thermite_allocator_run_mask as allocator_run_mask, thermite_ap_cpu_bit as ap_cpu_bit,
    thermite_ap_expected_mask as ap_expected_mask,
    thermite_ap_should_start as ap_should_start,
    thermite_ap_worker_online as ap_worker_online,
    thermite_ap_worker_task_complete as ap_worker_task_complete,
    thermite_apic_physical_base as apic_physical_base,
    thermite_apic_profile_supported as apic_profile_supported,
    thermite_atomic_fetch_add as exact_fetch_add, thermite_atomic_fetch_and as exact_fetch_and,
    thermite_atomic_fetch_or as exact_fetch_or, thermite_atomic_load as exact_load,
    thermite_atomic_store as exact_store,
    thermite_dma_descriptor_bytes as dma_descriptor_bytes,
    thermite_dma_descriptor_flags as dma_descriptor_flags,
    thermite_dma_descriptor_length as dma_descriptor_length,
    thermite_dma_descriptor_next as dma_descriptor_next,
    thermite_dma_device_ready as dma_device_ready, thermite_dma_device_status as dma_device_status,
    thermite_dma_publish_available as dma_publish_available,
    thermite_dma_queue_layout_valid as dma_queue_layout_valid,
    thermite_dma_queue_pfn as dma_queue_pfn, thermite_dma_queue_size_valid as dma_queue_size_valid,
    thermite_dma_register_port as dma_register_port, thermite_dma_used_offset as dma_used_offset,
    thermite_kernel_signature_policy_flags as signature_policy_flags,
    thermite_kernel_signature_task_base as signature_task_base,
    thermite_ipc_worker_dispatch as ipc_worker_dispatch,
    thermite_mapping_identity_huge_entry as mapping_identity_huge_entry,
    thermite_mapping_identity_physical as mapping_identity_physical,
    thermite_mapping_kernel_data_entry as mapping_kernel_data_entry,
    thermite_mapping_kernel_table_entry as mapping_kernel_table_entry,
    thermite_mapping_user_code_entry as mapping_user_code_entry,
    thermite_mapping_user_stack_entry as mapping_user_stack_entry,
    thermite_mapping_user_table_entry as mapping_user_table_entry,
    thermite_pci_config_address as pci_config_address,
    thermite_pci_enable_io_bus_master as pci_enable_io_bus_master,
    thermite_pci_legacy_io_bar_valid as pci_legacy_io_bar_valid,
    thermite_pci_legacy_io_base as pci_legacy_io_base,
    thermite_pci_virtio_block_identity as pci_virtio_block_identity,
    thermite_scheduler_required_ap_workers as scheduler_required_ap_workers,
    thermite_scheduler_required_parallel_cpus as scheduler_required_parallel_cpus,
    thermite_scheduler_worker_drain as scheduler_worker_drain,
    thermite_scheduler_worker_enter as scheduler_worker_enter,
    thermite_shootdown_worker_report as shootdown_worker_report,
    thermite_synchronization_worker_can_enter as synchronization_worker_can_enter,
    thermite_synchronization_worker_complete as synchronization_worker_complete,
    thermite_synchronization_worker_issue as synchronization_worker_issue,
    thermite_service_finish_value as service_finish_value,
    thermite_service_syscall_value as service_syscall_value,
    thermite_service_user_base as service_user_base,
    thermite_service_user_fault_address as service_user_fault_address,
    thermite_service_user_stack as service_user_stack,
    thermite_service_user_stack_pointer as service_user_stack_pointer,
    thermite_service_write_user_byte as service_write_user_byte,
};

use crate::{BootServices, Handle, Serial, Status, SUCCESS};

global_asm!(include_str!("ap_trampoline.S"), options(att_syntax));
global_asm!(
    r#"
    .text
    .global thermite_ipi_handler
    .global thermite_timer_handler
    .global thermite_page_fault_handler
    .global thermite_user_syscall_handler
    .global thermite_syscall_entry
    .global thermite_user_finish_handler
    .global thermite_enter_user
    .global memmove

/* Compiler-runtime memmove for the target's Win64 C ABI:
 * rcx=destination, rdx=source, r8=length, rax=destination. */
memmove:
    movq %rcx, %rax
    testq %r8, %r8
    jz 4f
    cmpq %rdx, %rcx
    jbe 2f
    leaq (%rdx,%r8), %r9
    cmpq %r9, %rcx
    jae 2f
    movq %r8, %r10
1:
    decq %r10
    movb (%rdx,%r10), %r9b
    movb %r9b, (%rcx,%r10)
    testq %r10, %r10
    jnz 1b
    jmp 4f
2:
    xorq %r10, %r10
3:
    movb (%rdx,%r10), %r9b
    movb %r9b, (%rcx,%r10)
    incq %r10
    cmpq %r8, %r10
    jb 3b
4:
    retq

thermite_ipi_handler:
    pushq %rax
    pushq %rbx
    pushq %rcx
    pushq %rdx
    movl $1, %eax
    cpuid
    shrl $24, %ebx
    andl $63, %ebx
    movabsq $0x0000408000000000, %rax
    invlpg (%rax)
    lock btsq %rbx, POST_IPI_ACK_MASK(%rip)
    movabsq $0xfee000b0, %rax
    movl $0, (%rax)
    popq %rdx
    popq %rcx
    popq %rbx
    popq %rax
    iretq

thermite_timer_handler:
    pushq %rax
    pushq %rbx
    pushq %rcx
    pushq %rdx
    movl $1, %eax
    cpuid
    shrl $24, %ebx
    andl $63, %ebx
    lock btsq %rbx, POST_TIMER_MASK(%rip)
    movabsq $0xfee000b0, %rax
    movl $0, (%rax)
    popq %rdx
    popq %rcx
    popq %rbx
    popq %rax
    iretq

thermite_page_fault_handler:
    pushq %rax
    pushq %rcx
    movq %cr2, %rax
    movq USER_EXPECTED_FAULT_ADDRESS(%rip), %rcx
    cmpq %rcx, %rax
    jne thermite_unexpected_page_fault
    cmpq $0x23, 32(%rsp)
    jne thermite_unexpected_page_fault
    lock incq USER_FAULTS(%rip)
    popq %rcx
    popq %rax
    addq $10, 8(%rsp)
    addq $8, %rsp
    iretq

thermite_unexpected_page_fault:
    lock incq KERNEL_FAULTS(%rip)
    cli
1:
    hlt
    jmp 1b

thermite_user_syscall_handler:
    movq %rax, USER_SYSCALL_VALUE(%rip)
    lock incq USER_SYSCALLS(%rip)
    iretq

/* Architectural SYSCALL entry.  SYSCALL does not switch stacks, so the first
 * two instructions preserve the user RSP and select the TSS-owned kernel
 * syscall stack before touching memory through the stack.  RCX/R11 are the
 * architecturally supplied return RIP/RFLAGS consumed by SYSRETQ. */
thermite_syscall_entry:
    movq %rsp, USER_SYSCALL_RSP(%rip)
    movq KERNEL_SYSCALL_RSP(%rip), %rsp
    pushq %rcx
    pushq %r11
    movq %rax, USER_SYSCALL_VALUE(%rip)
    lock incq USER_SYSCALLS(%rip)
    popq %r11
    popq %rcx
    movq USER_SYSCALL_RSP(%rip), %rsp
    sysretq

thermite_user_finish_handler:
    movq %rax, USER_FINISH_VALUE(%rip)
    lock incq USER_FINISHED(%rip)
    movq KERNEL_RETURN_RSP(%rip), %rsp
    retq

/* rcx = user RIP, rdx = user RSP under the x86_64 UEFI/Win64 C ABI.  The
 * completion gate restores the exact
 * kernel call frame saved here, so returning from this routine is ordinary
 * SysV control flow rather than a synthetic marker. */
thermite_enter_user:
    movq %rsp, KERNEL_RETURN_RSP(%rip)
    pushq $0x1b
    pushq %rdx
    pushfq
    orq $0x200, (%rsp)
    pushq $0x23
    pushq %rcx
    iretq
"#,
    options(att_syntax)
);

const MAX_CPUS: usize = 64;
const STACK_BYTES: usize = 16 * 1024;
const TRAMPOLINE_BASE: usize = 0x0008_0000;
const TRAMPOLINE_PARAMETERS: usize = TRAMPOLINE_BASE + 0x0c00;
const APIC_BASE_MSR: u32 = 0x1b;
const EFER_MSR: u32 = 0xc000_0080;
const STAR_MSR: u32 = 0xc000_0081;
const LSTAR_MSR: u32 = 0xc000_0082;
const SFMASK_MSR: u32 = 0xc000_0084;
const GS_BASE_MSR: u32 = 0xc000_0101;
const TSC_DEADLINE_MSR: u32 = 0x6e0;
const APIC_EOI: usize = 0xb0;
const APIC_TPR: usize = 0x80;
const APIC_SPURIOUS: usize = 0xf0;
const APIC_ICR_LOW: usize = 0x300;
const APIC_ICR_HIGH: usize = 0x310;
const APIC_LVT_TIMER: usize = 0x320;
const APIC_TIMER_INITIAL: usize = 0x380;
const APIC_TIMER_DIVIDE: usize = 0x3e0;
const IPI_VECTOR: u8 = 0xf1;
const TIMER_VECTOR: u8 = 0xf2;
const SHOOTDOWN_ADDRESS: u64 = 0x0000_4080_0000_0000;
const EFI_ALLOCATE_ADDRESS: u32 = 2;
const EFI_ALLOCATE_MAX_ADDRESS: u32 = 1;
const EFI_LOADER_DATA: u32 = 2;
const HEAP_PAGES: usize = 64;
const HEAP_BYTES: usize = HEAP_PAGES * 4096;
const POLICY_ALLOCATOR: u64 = 1;
const POLICY_MAPPING: u64 = 2;
const POLICY_SYNCHRONIZATION: u64 = 4;
const POLICY_AP_LIFECYCLE: u64 = 8;
const POLICY_SHOOTDOWN: u64 = 16;
const POLICY_DMA: u64 = 32;
const POLICY_SERVICES: u64 = 64;
const POLICY_KNOWN: u64 = POLICY_ALLOCATOR
    | POLICY_MAPPING
    | POLICY_SYNCHRONIZATION
    | POLICY_AP_LIFECYCLE
    | POLICY_SHOOTDOWN
    | POLICY_DMA
    | POLICY_SERVICES;

type AllocatePages = unsafe extern "efiapi" fn(
    allocation_type: u32,
    memory_type: u32,
    pages: usize,
    memory: *mut u64,
) -> Status;
type ExitBootServices = unsafe extern "efiapi" fn(image: Handle, map_key: usize) -> Status;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Reboot,
    PowerOff,
}

#[derive(Debug, Clone, Copy)]
pub struct PostFirmwareReport {
    pub online: usize,
    pub failed: usize,
    pub failed_apic_id: u32,
    pub ap_workers: usize,
    pub worker_cpus: usize,
    pub parallel_cpus: usize,
    pub task_sum: u64,
    pub thermite_policy_signature: u64,
    pub thermite_policy_flags: u64,
    pub thermite_task_base: usize,
    pub heap_bytes: usize,
    pub heap_allocations: usize,
    pub heap_oom_rejected: usize,
    pub atomic_message_cpus: usize,
    pub atomic_message_stale: usize,
    pub device_negative_checks: usize,
    pub cpu_local_cpus: usize,
    pub power_action: PowerAction,
    pub lock_entries: usize,
    pub ipi_acks: usize,
    pub timer_cpus: usize,
    pub timer_ipi_fallbacks: usize,
    pub tlb_cpus: usize,
    pub dma_bytes: usize,
    pub dma_generation: u64,
    pub entropy_bytes: usize,
    pub syscalls: u64,
    pub faults: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum PostFirmwareError {
    UnsupportedApic,
    InvalidCpuId,
    TrampolineTooLarge,
    TrampolineAllocation(Status),
    MemoryMap(Status),
    ExitBootServices(Status),
    ApStartup,
    Scheduler,
    Ipi,
    Timer,
    Tlb,
    Dma,
    Entropy,
    Injection,
    UserMode,
    Heap(Status),
    Device,
    Memory,
}

impl PostFirmwareError {
    pub const fn code(self) -> usize {
        match self {
            Self::UnsupportedApic => 1,
            Self::InvalidCpuId => 2,
            Self::TrampolineTooLarge => 3,
            Self::TrampolineAllocation(status) => status,
            Self::MemoryMap(status) => status,
            Self::ExitBootServices(status) => status,
            Self::ApStartup => 7,
            Self::Scheduler => 8,
            Self::Ipi => 9,
            Self::Timer => 10,
            Self::Tlb => 11,
            Self::Dma => 12,
            Self::Entropy => 13,
            Self::Injection => 14,
            Self::UserMode => 15,
            Self::Heap(status) => status,
            Self::Device => 16,
            Self::Memory => 17,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C, align(4096))]
struct Page([u64; 512]);

#[repr(C, align(4096))]
struct PageSet([Page; 4]);

#[repr(C, align(4096))]
struct BytePage([u8; 4096]);

#[repr(C, align(16))]
struct CpuStacks([[u8; STACK_BYTES]; MAX_CPUS]);

#[repr(C, align(16))]
struct TssBytes([u8; 104]);

#[repr(C, align(4096))]
struct VirtioQueue([u8; 16 * 1024]);

#[repr(C, align(16))]
struct VirtioRequest {
    request_type: u32,
    reserved: u32,
    sector: u64,
    data: [u8; 512],
    status: u8,
    padding: [u8; 15],
}

#[derive(Clone, Copy)]
#[repr(C)]
struct VirtqDescriptor {
    address: u64,
    length: u32,
    flags: u16,
    next: u16,
}

#[repr(C, align(16))]
struct Gdt([u64; 7]);

#[derive(Clone, Copy)]
#[repr(C, align(64))]
struct CpuLocalRecord {
    logical_id: u64,
    generation: u64,
    stack_top: u64,
    interrupt_depth: u64,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_middle: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const MISSING: Self = Self {
        offset_low: 0,
        selector: 0,
        ist: 0,
        attributes: 0,
        offset_middle: 0,
        offset_high: 0,
        reserved: 0,
    };

    fn interrupt(handler: usize, user: bool) -> Self {
        Self {
            offset_low: handler as u16,
            selector: 0x08,
            ist: 0,
            attributes: if user { 0xee } else { 0x8e },
            offset_middle: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }
}

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

static mut PML4: Page = Page([0; 512]);
static mut IDENTITY_PDPT: Page = Page([0; 512]);
static mut IDENTITY_PDS: PageSet = PageSet([Page([0; 512]); 4]);
static mut USER_PDPT: Page = Page([0; 512]);
static mut USER_PD: Page = Page([0; 512]);
static mut USER_PT: Page = Page([0; 512]);
static mut SHOOT_PDPT: Page = Page([0; 512]);
static mut SHOOT_PD: Page = Page([0; 512]);
static mut SHOOT_PT: Page = Page([0; 512]);
static mut USER_CODE_PAGE: BytePage = BytePage([0; 4096]);
static mut USER_STACK_PAGE: BytePage = BytePage([0; 4096]);
static mut TEST_PAGE_A: BytePage = BytePage([0; 4096]);
static mut TEST_PAGE_B: BytePage = BytePage([0; 4096]);
static mut AP_STACKS: CpuStacks = CpuStacks([[0; STACK_BYTES]; MAX_CPUS]);
static mut RING0_STACK: [u8; 32 * 1024] = [0; 32 * 1024];
static mut TSS: TssBytes = TssBytes([0; 104]);
static mut GDT: Gdt = Gdt([0; 7]);
static mut IDT: [IdtEntry; 256] = [IdtEntry::MISSING; 256];
static mut MEMORY_MAP: [u8; 128 * 1024] = [0; 128 * 1024];
static mut VIRTIO_QUEUE: VirtioQueue = VirtioQueue([0; 16 * 1024]);
static mut VIRTIO_REQUEST: VirtioRequest = VirtioRequest {
    request_type: 0,
    reserved: 0,
    sector: 0,
    data: [0; 512],
    status: 0xff,
    padding: [0; 15],
};
static mut MMIO_PROBE: [u64; 2] = [0; 2];
static mut CPU_LOCALS: [CpuLocalRecord; MAX_CPUS] = [CpuLocalRecord {
    logical_id: 0,
    generation: 0,
    stack_top: 0,
    interrupt_depth: 0,
}; MAX_CPUS];

static POST_ONLINE_MASK: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_PHASE: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_READY: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_NEXT_TASK: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_TASK_BASE: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_TASK_READY: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_TASK_GATE: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_EXPECTED_WORKERS: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_TASK_SUM: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_TASK_WORKERS: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_TASK_DONE: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_MAX_ACTIVE: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_LOCK_NEXT: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_LOCK_OWNER: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_LOCK_ENTRIES: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_ONCE: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_TLB_PRE_MASK: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_TLB_POST_MASK: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_TLB_STALE: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_TLB_OBSERVED: [ExactAtomicU64; MAX_CPUS] = [const { ExactAtomicU64::new(0) }; MAX_CPUS];
static POST_IPI_EPOCH: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_TIMER_IPI_FALLBACKS: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_MESSAGE_PAYLOAD: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_MESSAGE_READY: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_MESSAGE_MASK: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_MESSAGE_STALE: ExactAtomicU64 = ExactAtomicU64::new(0);
static POST_CPU_LOCAL_MASK: ExactAtomicU64 = ExactAtomicU64::new(0);
static HEAP_BASE: ExactAtomicU64 = ExactAtomicU64::new(0);
static HEAP_BITMAP: ExactAtomicU64 = ExactAtomicU64::new(0);
static HEAP_ALLOCATIONS: ExactAtomicU64 = ExactAtomicU64::new(0);

#[no_mangle]
static POST_IPI_ACK_MASK: AtomicU64 = AtomicU64::new(0);
#[no_mangle]
static POST_TIMER_MASK: AtomicU64 = AtomicU64::new(0);
#[no_mangle]
static USER_SYSCALLS: AtomicU64 = AtomicU64::new(0);
#[no_mangle]
static USER_FAULTS: AtomicU64 = AtomicU64::new(0);
#[no_mangle]
static USER_FINISHED: AtomicU64 = AtomicU64::new(0);
#[no_mangle]
static USER_SYSCALL_VALUE: AtomicU64 = AtomicU64::new(0);
#[no_mangle]
static USER_FINISH_VALUE: AtomicU64 = AtomicU64::new(0);
#[no_mangle]
static USER_EXPECTED_FAULT_ADDRESS: AtomicU64 = AtomicU64::new(0);
#[no_mangle]
static KERNEL_RETURN_RSP: AtomicU64 = AtomicU64::new(0);
#[no_mangle]
static KERNEL_SYSCALL_RSP: AtomicU64 = AtomicU64::new(0);
#[no_mangle]
static USER_SYSCALL_RSP: AtomicU64 = AtomicU64::new(0);
#[no_mangle]
static KERNEL_FAULTS: AtomicU64 = AtomicU64::new(0);

unsafe extern "C" {
    static thermite_ap_trampoline_start: u8;
    static thermite_ap_trampoline_end: u8;
    fn thermite_ipi_handler();
    fn thermite_timer_handler();
    fn thermite_page_fault_handler();
    fn thermite_user_syscall_handler();
    fn thermite_syscall_entry();
    fn thermite_user_finish_handler();
    fn thermite_enter_user(entry: u64, stack: u64);
}

struct KernelAllocator;

#[global_allocator]
static KERNEL_ALLOCATOR: KernelAllocator = KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let base = exact_load(&HEAP_BASE) as usize;
        if base == 0 {
            return ptr::null_mut();
        }
        let pages = thermite_kernel_policy::kernel_policy_ingress::thermite_allocator_request_pages(
            layout.size() as u64,
            layout.align() as u64,
        ) as usize;
        if pages == 0 {
            return ptr::null_mut();
        }
        let first = allocator_claim_first(&HEAP_BITMAP, pages as u64) as usize;
        if first >= HEAP_PAGES {
            return ptr::null_mut();
        }
        let allocation = (base + first * 4096) as *mut u8;
        // SAFETY: the generated bounded CAS state machine grants this caller
        // exclusive ownership of the returned first-fit page run.
        unsafe { ptr::write_bytes(allocation, 0, pages * 4096) };
        exact_fetch_add(&HEAP_ALLOCATIONS, 1);
        allocation
    }

    unsafe fn dealloc(&self, allocation: *mut u8, layout: Layout) {
        let base = exact_load(&HEAP_BASE) as usize;
        let address = allocation as usize;
        if base == 0
            || address < base
            || address >= base + HEAP_BYTES
            || !address.is_multiple_of(4096)
        {
            return;
        }
        let pages = thermite_kernel_policy::kernel_policy_ingress::thermite_allocator_request_pages(
            layout.size() as u64,
            layout.align() as u64,
        ) as usize;
        let first = (address - base) / 4096;
        if pages == 0 || first + pages > HEAP_PAGES {
            return;
        }
        let mask = allocator_run_mask(first as u64, pages as u64, u64::MAX);
        exact_fetch_and(&HEAP_BITMAP, !mask);
    }
}

#[inline]
fn bit(cpu: usize) -> Result<u64, PostFirmwareError> {
    if cpu >= 64 {
        return Err(PostFirmwareError::InvalidCpuId);
    }
    Ok(ap_cpu_bit(cpu as u64))
}

fn cpu_id() -> usize {
    let ebx: u32;
    // SAFETY: CPUID leaf 1 is mandatory on x86_64 and has no memory operands.
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov {saved:e}, ebx",
            "pop rbx",
            inout("eax") 1_u32 => _,
            out("ecx") _,
            out("edx") _,
            saved = lateout(reg) ebx,
            options(nomem)
        );
    }
    ((ebx >> 24) & 0xff) as usize
}

unsafe fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    // SAFETY: callers use MSRs frozen by the x86_64 profile.
    unsafe { asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high) };
    (u64::from(high) << 32) | u64::from(low)
}

unsafe fn wrmsr(msr: u32, value: u64) {
    // SAFETY: callers use validated values for profile-owned MSRs.
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") value as u32,
            in("edx") (value >> 32) as u32
        )
    };
}

unsafe fn apic_base() -> Result<usize, PostFirmwareError> {
    // SAFETY: APIC_BASE_MSR is architectural and available after CPUID APIC
    // validation performed by `run`.
    let value = unsafe { rdmsr(APIC_BASE_MSR) };
    if !apic_profile_supported(value) {
        return Err(PostFirmwareError::UnsupportedApic);
    }
    Ok(apic_physical_base(value) as usize)
}

unsafe fn apic_read(offset: usize) -> Result<u32, PostFirmwareError> {
    // SAFETY: the profile identity-maps the capability-owned LAPIC page.
    Ok(unsafe { ptr::read_volatile((apic_base()? + offset) as *const u32) })
}

unsafe fn apic_write(offset: usize, value: u32) -> Result<(), PostFirmwareError> {
    // SAFETY: the offset is one of the frozen aligned LAPIC registers.
    unsafe { ptr::write_volatile((apic_base()? + offset) as *mut u32, value) };
    Ok(())
}

fn checked_mmio_address(
    base: usize,
    len: usize,
    offset: usize,
    width: usize,
) -> Result<usize, PostFirmwareError> {
    let end = offset.checked_add(width).ok_or(PostFirmwareError::Device)?;
    if width == 0 || !offset.is_multiple_of(width) || end > len {
        return Err(PostFirmwareError::Device);
    }
    base.checked_add(offset).ok_or(PostFirmwareError::Device)
}

unsafe fn mmio_read8(base: usize, len: usize, offset: usize) -> Result<u8, PostFirmwareError> {
    let address = checked_mmio_address(base, len, offset, 1)?;
    Ok(unsafe { ptr::read_volatile(address as *const u8) })
}

unsafe fn mmio_read16(base: usize, len: usize, offset: usize) -> Result<u16, PostFirmwareError> {
    let address = checked_mmio_address(base, len, offset, 2)?;
    Ok(unsafe { ptr::read_volatile(address as *const u16) })
}

unsafe fn mmio_read32(base: usize, len: usize, offset: usize) -> Result<u32, PostFirmwareError> {
    let address = checked_mmio_address(base, len, offset, 4)?;
    Ok(unsafe { ptr::read_volatile(address as *const u32) })
}

unsafe fn mmio_read64(base: usize, len: usize, offset: usize) -> Result<u64, PostFirmwareError> {
    let address = checked_mmio_address(base, len, offset, 8)?;
    Ok(unsafe { ptr::read_volatile(address as *const u64) })
}

unsafe fn mmio_write8(
    base: usize,
    len: usize,
    offset: usize,
    value: u8,
) -> Result<(), PostFirmwareError> {
    let address = checked_mmio_address(base, len, offset, 1)?;
    unsafe { ptr::write_volatile(address as *mut u8, value) };
    Ok(())
}

unsafe fn mmio_write16(
    base: usize,
    len: usize,
    offset: usize,
    value: u16,
) -> Result<(), PostFirmwareError> {
    let address = checked_mmio_address(base, len, offset, 2)?;
    unsafe { ptr::write_volatile(address as *mut u16, value) };
    Ok(())
}

unsafe fn mmio_write32(
    base: usize,
    len: usize,
    offset: usize,
    value: u32,
) -> Result<(), PostFirmwareError> {
    let address = checked_mmio_address(base, len, offset, 4)?;
    unsafe { ptr::write_volatile(address as *mut u32, value) };
    Ok(())
}

unsafe fn mmio_write64(
    base: usize,
    len: usize,
    offset: usize,
    value: u64,
) -> Result<(), PostFirmwareError> {
    let address = checked_mmio_address(base, len, offset, 8)?;
    unsafe { ptr::write_volatile(address as *mut u64, value) };
    Ok(())
}

unsafe fn volatile_device_probe() -> Result<usize, PostFirmwareError> {
    let base = ptr::addr_of_mut!(MMIO_PROBE).cast::<u8>() as usize;
    let len = core::mem::size_of::<[u64; 2]>();
    unsafe {
        mmio_write8(base, len, 0, 0x12)?;
        mmio_write16(base, len, 2, 0x3456)?;
        mmio_write32(base, len, 4, 0x789a_bcde)?;
        mmio_write64(base, len, 8, 0xfedc_ba98_7654_3210)?;
        core::sync::atomic::compiler_fence(Ordering::SeqCst);
        asm!("lfence", options(nostack));
        asm!("sfence", options(nostack));
        asm!("mfence", options(nostack));
        if mmio_read8(base, len, 0)? != 0x12
            || mmio_read16(base, len, 2)? != 0x3456
            || mmio_read32(base, len, 4)? != 0x789a_bcde
            || mmio_read64(base, len, 8)? != 0xfedc_ba98_7654_3210
        {
            return Err(PostFirmwareError::Device);
        }
        if mmio_read16(base, len, 1).is_ok() || mmio_read64(base, len, 12).is_ok() {
            return Err(PostFirmwareError::Device);
        }
    }
    Ok(2)
}

unsafe fn local_apic_enable() -> Result<(), PostFirmwareError> {
    // SAFETY: this CPU owns its local spurious-vector and EOI registers.
    unsafe {
        asm!("mov cr8, {}", in(reg) 0_usize, options(nomem, nostack));
        apic_write(APIC_TPR, 0)?;
        apic_write(APIC_SPURIOUS, 0x100 | 0xff)?;
        apic_write(APIC_EOI, 0)?;
    }
    Ok(())
}

unsafe fn install_cpu_local(logical_id: usize) -> Result<(), PostFirmwareError> {
    if logical_id >= MAX_CPUS {
        return Err(PostFirmwareError::InvalidCpuId);
    }
    let record = unsafe {
        ptr::addr_of_mut!(CPU_LOCALS)
            .cast::<CpuLocalRecord>()
            .add(logical_id)
    };
    let stack_base = unsafe { ptr::addr_of_mut!(AP_STACKS.0).cast::<u8>() };
    let stack_top = unsafe { stack_base.add((logical_id + 1) * STACK_BYTES) as u64 };
    unsafe {
        ptr::write(
            record,
            CpuLocalRecord {
                logical_id: logical_id as u64,
                generation: 1,
                stack_top,
                interrupt_depth: 0,
            },
        );
        wrmsr(GS_BASE_MSR, record as u64);
    }
    let observed: u64;
    unsafe { asm!("mov {}, gs:[0]", out(reg) observed, options(readonly, nostack)) };
    if observed != logical_id as u64 {
        return Err(PostFirmwareError::InvalidCpuId);
    }
    exact_fetch_or(&POST_CPU_LOCAL_MASK, bit(logical_id)?);
    Ok(())
}

fn delay() {
    for _ in 0..200_000 {
        core::hint::spin_loop();
    }
}

unsafe fn wait_icr() -> Result<(), PostFirmwareError> {
    for _ in 0..10_000_000 {
        // SAFETY: the BSP owns ICR serialization during startup and tests.
        if unsafe { apic_read(APIC_ICR_LOW)? } & (1 << 12) == 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(PostFirmwareError::Ipi)
}

unsafe fn send_raw_ipi(apic_id: u32, low: u32) -> Result<(), PostFirmwareError> {
    if apic_id >= 64 {
        return Err(PostFirmwareError::InvalidCpuId);
    }
    // SAFETY: the BSP serializes ICR access and `apic_id` is from the frozen
    // processor inventory.
    unsafe {
        wait_icr()?;
        apic_write(APIC_ICR_HIGH, apic_id << 24)?;
        apic_write(APIC_ICR_LOW, low)?;
        wait_icr()?;
    }
    Ok(())
}

unsafe fn start_ap(apic_id: u32) -> Result<(), PostFirmwareError> {
    // INIT (level-triggered assert), deassert, then two SIPIs for vector 0x80.
    unsafe {
        send_raw_ipi(apic_id, (5 << 8) | (1 << 14) | (1 << 15))?;
    }
    delay();
    unsafe { send_raw_ipi(apic_id, (5 << 8) | (1 << 15))? };
    delay();
    unsafe { send_raw_ipi(apic_id, (6 << 8) | 0x80)? };
    delay();
    unsafe { send_raw_ipi(apic_id, (6 << 8) | 0x80)? };
    Ok(())
}

unsafe fn send_fixed_ipi(apic_id: u32) -> Result<(), PostFirmwareError> {
    // SAFETY: vector IPI_VECTOR is installed in every online CPU's IDT.
    unsafe { send_raw_ipi(apic_id, u32::from(IPI_VECTOR)) }
}

unsafe fn arm_timer() -> Result<(), PostFirmwareError> {
    let low: u32;
    let high: u32;
    // SAFETY: each CPU owns its local timer registers and TSC-deadline MSR.
    unsafe {
        asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack));
        apic_write(APIC_TIMER_DIVIDE, 0x0b)?;
        apic_write(APIC_TIMER_INITIAL, 0)?;
        apic_write(APIC_LVT_TIMER, (1 << 18) | u32::from(TIMER_VECTOR))?;
        let now = (u64::from(high) << 32) | u64::from(low);
        let deadline = now.saturating_add(100_000);
        wrmsr(TSC_DEADLINE_MSR, deadline);
        loop {
            let next_low: u32;
            let next_high: u32;
            asm!(
                "rdtsc",
                out("eax") next_low,
                out("edx") next_high,
                options(nomem, nostack)
            );
            if (u64::from(next_high) << 32) | u64::from(next_low) >= deadline {
                break;
            }
            core::hint::spin_loop();
        }
        let cpu = cpu_id();
        let cpu_bit = bit(cpu)?;
        if POST_TIMER_MASK.load(Ordering::Acquire) & cpu_bit == 0 {
            // QEMU's TCG LAPIC timer can remain quiescent after firmware exits.
            // Preserve deadline semantics by delivering the expired per-CPU
            // timer through the same hardware APIC gate as a self-directed IPI.
            exact_fetch_add(&POST_TIMER_IPI_FALLBACKS, 1);
            wait_icr()?;
            apic_write(APIC_ICR_LOW, (1 << 18) | u32::from(TIMER_VECTOR))?;
            wait_icr()?;
        }
    }
    Ok(())
}

fn run_tasks(cpu: usize) {
    let _ = scheduler_worker_enter(
        cpu as u64,
        &POST_TASK_READY,
        &POST_EXPECTED_WORKERS,
        &POST_TASK_BASE,
        &POST_TASK_SUM,
        &POST_TASK_WORKERS,
        &POST_MAX_ACTIVE,
        &POST_TASK_GATE,
    );
    while exact_load(&POST_TASK_GATE) == 0 {
        core::hint::spin_loop();
    }
    let _ = scheduler_worker_drain(&POST_TASK_BASE, &POST_NEXT_TASK, &POST_TASK_SUM);
}

fn message_probe(cpu: usize) {
    let _ = ipc_worker_dispatch(
        cpu as u64,
        &POST_MESSAGE_PAYLOAD,
        &POST_MESSAGE_STALE,
        &POST_MESSAGE_MASK,
    );
}

fn lock_once(cpu: usize) {
    let issued = synchronization_worker_issue(&POST_LOCK_NEXT);
    while !synchronization_worker_can_enter(&POST_LOCK_OWNER, issued.ticket).can_enter {
        core::hint::spin_loop();
    }
    let _ = synchronization_worker_complete(
        cpu as u64,
        issued.ticket,
        &POST_LOCK_OWNER,
        &POST_LOCK_ENTRIES,
        &POST_ONCE,
    );
}

fn shootdown_probe(cpu: usize, expected: u64, mask: &ExactAtomicU64) {
    // SAFETY: SHOOTDOWN_ADDRESS is a profile-owned test mapping present in the
    // active page table and points to one immutable test word in this phase.
    let observed = unsafe { ptr::read_volatile(SHOOTDOWN_ADDRESS as *const u64) };
    let _ = shootdown_worker_report(
        cpu as u64,
        observed,
        expected,
        &POST_TLB_OBSERVED[cpu],
        &POST_TLB_STALE,
        mask,
    );
}

#[no_mangle]
extern "C" fn thermite_ap_rust_entry(apic_id: usize) -> ! {
    // SAFETY: the trampoline selected this stack by the same bounded xAPIC ID.
    unsafe {
        install_gdt(false);
        install_idt();
        if install_cpu_local(apic_id).is_err() {
            loop {
                asm!("cli", "hlt", options(nomem, nostack));
            }
        }
        if local_apic_enable().is_err() {
            loop {
                asm!("cli", "hlt", options(nomem, nostack));
            }
        }
        asm!("sti", options(nomem, nostack));
    }
    let _ = ap_worker_online(apic_id as u64, &POST_ONLINE_MASK, &POST_READY);
    while exact_load(&POST_PHASE) < 1 {
        core::hint::spin_loop();
    }
    message_probe(apic_id);
    run_tasks(apic_id);
    lock_once(apic_id);
    shootdown_probe(apic_id, 0xaaaa_5555_1111_2222, &POST_TLB_PRE_MASK);
    let _ = ap_worker_task_complete(apic_id as u64, &POST_TASK_DONE);

    while exact_load(&POST_PHASE) < 2 {
        core::hint::spin_loop();
    }
    shootdown_probe(apic_id, 0xbbbb_6666_3333_4444, &POST_TLB_POST_MASK);
    // SAFETY: the AP owns its timer and has already loaded TIMER_VECTOR.
    let _ = unsafe { arm_timer() };
    loop {
        // SAFETY: interrupts are enabled; HLT is the profile's idle primitive.
        unsafe { asm!("hlt", options(nomem, nostack)) };
    }
}

unsafe fn setup_page_tables(user_base: u64) -> Result<u64, PostFirmwareError> {
    // SAFETY: only the BSP can reach setup and APs have not been released.
    let (
        pml4,
        identity_pdpt,
        identity_pds,
        user_pdpt,
        user_pd,
        user_pt,
        shoot_pdpt,
        shoot_pd,
        shoot_pt,
    ) = unsafe {
        (
            ptr::addr_of_mut!(PML4.0).cast::<u64>(),
            ptr::addr_of_mut!(IDENTITY_PDPT.0).cast::<u64>(),
            ptr::addr_of_mut!(IDENTITY_PDS.0).cast::<Page>(),
            ptr::addr_of_mut!(USER_PDPT.0).cast::<u64>(),
            ptr::addr_of_mut!(USER_PD.0).cast::<u64>(),
            ptr::addr_of_mut!(USER_PT.0).cast::<u64>(),
            ptr::addr_of_mut!(SHOOT_PDPT.0).cast::<u64>(),
            ptr::addr_of_mut!(SHOOT_PD.0).cast::<u64>(),
            ptr::addr_of_mut!(SHOOT_PT.0).cast::<u64>(),
        )
    };

    // SAFETY: all tables are uniquely initialized before any CPU can observe
    // the new CR3.  Static alignment supplies the architectural 4-KiB bound.
    unsafe {
        ptr::write(
            pml4.add(0),
            mapping_kernel_table_entry(identity_pdpt as u64),
        );
        for pdpt_index in 0..4 {
            let pd = ptr::addr_of_mut!((*identity_pds.add(pdpt_index)).0).cast::<u64>();
            ptr::write(
                identity_pdpt.add(pdpt_index),
                mapping_kernel_table_entry(pd as u64),
            );
            for entry in 0..512 {
                let physical = mapping_identity_physical(pdpt_index as u64, entry as u64);
                ptr::write(pd.add(entry), mapping_identity_huge_entry(physical));
            }
        }

        ptr::write(
            pml4.add(((user_base >> 39) & 0x1ff) as usize),
            mapping_user_table_entry(user_pdpt as u64),
        );
        ptr::write(user_pdpt, mapping_user_table_entry(user_pd as u64));
        ptr::write(user_pd, mapping_user_table_entry(user_pt as u64));
        ptr::write(
            user_pt,
            mapping_user_code_entry(ptr::addr_of!(USER_CODE_PAGE.0) as u64),
        );
        ptr::write(
            user_pt.add(1),
            mapping_user_stack_entry(ptr::addr_of!(USER_STACK_PAGE.0) as u64),
        );

        ptr::write(
            pml4.add(((SHOOTDOWN_ADDRESS >> 39) & 0x1ff) as usize),
            mapping_kernel_table_entry(shoot_pdpt as u64),
        );
        ptr::write(shoot_pdpt, mapping_kernel_table_entry(shoot_pd as u64));
        ptr::write(shoot_pd, mapping_kernel_table_entry(shoot_pt as u64));
        ptr::write(
            shoot_pt,
            mapping_kernel_data_entry(ptr::addr_of!(TEST_PAGE_A.0) as u64),
        );

        ptr::write_unaligned(
            ptr::addr_of_mut!(TEST_PAGE_A.0).cast::<u64>(),
            0xaaaa_5555_1111_2222,
        );
        ptr::write_unaligned(
            ptr::addr_of_mut!(TEST_PAGE_B.0).cast::<u64>(),
            0xbbbb_6666_3333_4444,
        );
    }
    Ok(pml4 as u64)
}

unsafe fn setup_user_code(fault_address: u64, syscall_value: u64, finish_value: u64) {
    // SAFETY: the BSP uniquely owns the page before it becomes reachable from
    // CR3. Raw provenance creation stays in the TPL; every bounded byte store
    // is performed by the generated Thermite mutable-slice operation.
    let bytes = unsafe {
        core::slice::from_raw_parts_mut(ptr::addr_of_mut!(USER_CODE_PAGE.0).cast::<u8>(), 32)
    };
    let syscall_bytes = syscall_value.to_le_bytes();
    let fault_bytes = fault_address.to_le_bytes();
    let finish_bytes = finish_value.to_le_bytes();
    service_write_user_byte(bytes, 0, 0xb8);
    for (offset, byte) in syscall_bytes[..4].iter().enumerate() {
        service_write_user_byte(bytes, 1 + offset, *byte);
    }
    service_write_user_byte(bytes, 5, 0x0f);
    service_write_user_byte(bytes, 6, 0x05);
    service_write_user_byte(bytes, 7, 0x48);
    service_write_user_byte(bytes, 8, 0xa1);
    for (offset, byte) in fault_bytes.iter().enumerate() {
        service_write_user_byte(bytes, 9 + offset, *byte);
    }
    service_write_user_byte(bytes, 17, 0xb8);
    for (offset, byte) in finish_bytes[..4].iter().enumerate() {
        service_write_user_byte(bytes, 18 + offset, *byte);
    }
    service_write_user_byte(bytes, 22, 0xcd);
    service_write_user_byte(bytes, 23, 0x81);
    service_write_user_byte(bytes, 24, 0xf4);
    service_write_user_byte(bytes, 25, 0xeb);
    service_write_user_byte(bytes, 26, 0xfd);
}

unsafe fn setup_descriptor_tables() {
    // SAFETY: only the BSP initializes the table before AP release.
    let gdt = unsafe { ptr::addr_of_mut!(GDT.0).cast::<u64>() };
    // SAFETY: the BSP exclusively constructs the static table before loading
    // it or releasing any AP.
    unsafe {
        ptr::write(gdt.add(0), 0);
        ptr::write(gdt.add(1), 0x00af_9a00_0000_ffff);
        ptr::write(gdt.add(2), 0x00cf_9200_0000_ffff);
        ptr::write(gdt.add(3), 0x00cf_f200_0000_ffff);
        ptr::write(gdt.add(4), 0x00af_fa00_0000_ffff);

        let ring0_top = ptr::addr_of_mut!(RING0_STACK).cast::<u8>().add(32 * 1024) as u64;
        ptr::write_unaligned(
            ptr::addr_of_mut!(TSS.0).cast::<u8>().add(4).cast::<u64>(),
            ring0_top,
        );
        ptr::write_unaligned(
            ptr::addr_of_mut!(TSS.0).cast::<u8>().add(102).cast::<u16>(),
            104,
        );
        let base = ptr::addr_of!(TSS.0) as u64;
        let limit = 103_u64;
        let tss_low = limit
            | ((base & 0x00ff_ffff) << 16)
            | (0x89_u64 << 40)
            | (((limit >> 16) & 0x0f) << 48)
            | (((base >> 24) & 0xff) << 56);
        ptr::write(gdt.add(5), tss_low);
        ptr::write(gdt.add(6), base >> 32);

        let idt = ptr::addr_of_mut!(IDT).cast::<IdtEntry>();
        ptr::write(
            idt.add(14),
            IdtEntry::interrupt(thermite_page_fault_handler as *const () as usize, false),
        );
        ptr::write(
            idt.add(IPI_VECTOR as usize),
            IdtEntry::interrupt(thermite_ipi_handler as *const () as usize, false),
        );
        ptr::write(
            idt.add(TIMER_VECTOR as usize),
            IdtEntry::interrupt(thermite_timer_handler as *const () as usize, false),
        );
        ptr::write(
            idt.add(0x80),
            IdtEntry::interrupt(thermite_user_syscall_handler as *const () as usize, true),
        );
        ptr::write(
            idt.add(0x81),
            IdtEntry::interrupt(thermite_user_finish_handler as *const () as usize, true),
        );
    }
}

unsafe fn install_syscall_entry() {
    // STAR selects kernel CS 0x08/SS 0x10 for SYSCALL and the synthetic base
    // 0x13 that SYSRET expands to user SS 0x1b and user CS 0x23.  LSTAR owns
    // the only architectural syscall ingress and SFMASK clears IF/DF until the
    // entry has moved from the user stack to the kernel stack.
    let ring0_top = unsafe { ptr::addr_of_mut!(RING0_STACK).cast::<u8>().add(32 * 1024) as u64 };
    KERNEL_SYSCALL_RSP.store(ring0_top, Ordering::Release);
    unsafe {
        wrmsr(EFER_MSR, rdmsr(EFER_MSR) | 1);
        wrmsr(STAR_MSR, (0x13_u64 << 48) | (0x08_u64 << 32));
        wrmsr(
            LSTAR_MSR,
            thermite_syscall_entry as *const () as usize as u64,
        );
        wrmsr(SFMASK_MSR, (1 << 9) | (1 << 10));
    }
}

unsafe fn install_gdt(load_tss: bool) {
    let descriptor = DescriptorTablePointer {
        limit: (7 * 8 - 1) as u16,
        // SAFETY: the static GDT was initialized before this CPU was released.
        base: unsafe { ptr::addr_of!(GDT.0) as u64 },
    };
    // SAFETY: the descriptor points to the immutable initialized GDT.  The far
    // return reloads CS immediately and the remaining segment selectors use
    // the matching kernel-data entry.
    unsafe {
        asm!(
            "lgdt [{table}]",
            "push 0x08",
            "lea rax, [rip + 2f]",
            "push rax",
            "retfq",
            "2:",
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov ss, ax",
            table = in(reg) &descriptor,
            out("rax") _,
        );
        if load_tss {
            asm!("mov ax, 0x28", "ltr ax", out("ax") _, options(nostack));
        }
    }
}

unsafe fn install_idt() {
    let descriptor = DescriptorTablePointer {
        limit: (256 * 16 - 1) as u16,
        base: ptr::addr_of!(IDT) as u64,
    };
    // SAFETY: every installed gate references the active kernel code selector.
    unsafe { asm!("lidt [{}]", in(reg) &descriptor, options(readonly, nostack)) };
}

unsafe fn switch_page_table(cr3: u64) {
    // SAFETY: `setup_page_tables` created an aligned hierarchy that identity
    // maps the executing image, stacks, LAPIC, and runtime-service code.
    unsafe {
        let efer = rdmsr(EFER_MSR);
        wrmsr(EFER_MSR, efer | (1 << 11));
        asm!("mov cr3, {}", in(reg) cr3, options(nostack));
    }
}

unsafe fn install_trampoline(
    boot_services: *mut BootServices,
    cr3: u64,
    apic_ids: &[u32],
) -> Result<(), PostFirmwareError> {
    let start = ptr::addr_of!(thermite_ap_trampoline_start);
    let end = ptr::addr_of!(thermite_ap_trampoline_end);
    let length = end as usize - start as usize;
    if length > 0x0c00 || cr3 > u64::from(u32::MAX) {
        return Err(PostFirmwareError::TrampolineTooLarge);
    }
    let mut address = TRAMPOLINE_BASE as u64;
    let allocate: AllocatePages = unsafe { core::mem::transmute((*boot_services).allocate_pages) };
    // SAFETY: the frozen address is a single conventional low-memory page; a
    // successful firmware allocation grants exclusive ownership to the image.
    let status = unsafe { allocate(EFI_ALLOCATE_ADDRESS, EFI_LOADER_DATA, 1, &mut address) };
    if status != SUCCESS || address != TRAMPOLINE_BASE as u64 {
        return Err(PostFirmwareError::TrampolineAllocation(status));
    }
    // SAFETY: the allocation is writable and the linker symbols bound one
    // relocation-free code interval smaller than its reserved prefix.
    unsafe {
        ptr::copy_nonoverlapping(start, TRAMPOLINE_BASE as *mut u8, length);
        ptr::write_volatile(TRAMPOLINE_PARAMETERS as *mut u64, cr3);
        ptr::write_volatile(
            (TRAMPOLINE_PARAMETERS + 8) as *mut u64,
            thermite_ap_rust_entry as *const () as usize as u64,
        );
        for &apic in apic_ids {
            let cpu = apic as usize;
            if cpu >= MAX_CPUS {
                return Err(PostFirmwareError::InvalidCpuId);
            }
            let stack_base = ptr::addr_of_mut!(AP_STACKS.0).cast::<u8>();
            let stack_top = stack_base.add((cpu + 1) * STACK_BYTES) as u64;
            ptr::write_volatile(
                (TRAMPOLINE_PARAMETERS + 16 + cpu * 8) as *mut u64,
                stack_top,
            );
        }
    }
    Ok(())
}

unsafe fn reserve_boot_heap(boot_services: *mut BootServices) -> Result<usize, PostFirmwareError> {
    let allocate: AllocatePages = unsafe { core::mem::transmute((*boot_services).allocate_pages) };
    let mut address = u64::from(u32::MAX);
    // SAFETY: AllocateMaxAddress reserves one page-aligned kernel-owned extent
    // below 4 GiB so the frozen identity map covers it after firmware exit.
    let status = unsafe {
        allocate(
            EFI_ALLOCATE_MAX_ADDRESS,
            EFI_LOADER_DATA,
            HEAP_PAGES,
            &mut address,
        )
    };
    if status != SUCCESS || address == 0 || address > u64::from(u32::MAX) || address & 0xfff != 0 {
        return Err(PostFirmwareError::Heap(status));
    }
    exact_store(&HEAP_BASE, address);
    exact_store(&HEAP_BITMAP, 0);
    exact_store(&HEAP_ALLOCATIONS, 0);
    Ok(address as usize)
}

fn allocator_probe() -> Result<(usize, usize, usize), PostFirmwareError> {
    let heap_base = exact_load(&HEAP_BASE) as usize;
    if heap_base == 0 {
        return Err(PostFirmwareError::Heap(usize::MAX));
    }
    let mut first = Box::new([0_u64; 128]);
    let mut second = Box::new([0_u64; 128]);
    let first_address = first.as_ptr() as usize;
    let second_address = second.as_ptr() as usize;
    if first.iter().any(|value| *value != 0) || second.iter().any(|value| *value != 0) {
        return Err(PostFirmwareError::Heap(usize::MAX));
    }
    first[0] = 0x5448_4552_4d49_5445;
    second[127] = 0x4845_4150_4252_4944;
    // SAFETY: volatile reads make the two independently owned allocations part
    // of the executable acceptance path before their ownership is reclaimed.
    if unsafe { ptr::read_volatile(first.as_ptr()) } != 0x5448_4552_4d49_5445
        || unsafe { ptr::read_volatile(second.as_ptr().add(127)) } != 0x4845_4150_4252_4944
    {
        return Err(PostFirmwareError::Heap(usize::MAX));
    }
    drop(first);
    drop(second);
    if !thermite_kernel_policy::kernel_policy_ingress::thermite_allocator_runtime_accept(
        heap_base as u64,
        first_address as u64,
        second_address as u64,
        HEAP_BYTES as u64,
        exact_load(&HEAP_BITMAP),
    ) {
        return Err(PostFirmwareError::Heap(usize::MAX));
    }
    let reclaimed = Box::new([0_u64; 128]);
    if reclaimed.as_ptr() as usize != first_address || reclaimed.iter().any(|value| *value != 0) {
        return Err(PostFirmwareError::Heap(usize::MAX));
    }
    drop(reclaimed);
    if exact_load(&HEAP_BITMAP) != 0 {
        return Err(PostFirmwareError::Heap(usize::MAX));
    }
    let oversized_layout = Layout::from_size_align(HEAP_BYTES + 4096, 4096)
        .map_err(|_| PostFirmwareError::Heap(usize::MAX))?;
    // SAFETY: this directly probes the frozen allocator's recoverable failure
    // contract. The requested 65-page run exceeds its 64-page arena, so it
    // must return null without changing allocator ownership state.
    let oversized = unsafe { KERNEL_ALLOCATOR.alloc(oversized_layout) };
    if !oversized.is_null() || exact_load(&HEAP_BITMAP) != 0 {
        return Err(PostFirmwareError::Heap(usize::MAX));
    }
    Ok((HEAP_BYTES, exact_load(&HEAP_ALLOCATIONS) as usize, 1))
}

unsafe fn exit_boot_services(
    image: Handle,
    boot_services: *mut BootServices,
) -> Result<(), PostFirmwareError> {
    let exit: ExitBootServices =
        unsafe { core::mem::transmute((*boot_services).exit_boot_services) };
    let mut last_status = usize::MAX;
    for _ in 0..4 {
        let mut bytes = 128 * 1024;
        let mut key = 0;
        let mut descriptor_size = 0;
        let mut descriptor_version = 0;
        // SAFETY: the static buffer remains writable and no allocations occur
        // between the successful map call and ExitBootServices.
        let map_status = unsafe {
            ((*boot_services).get_memory_map)(
                &mut bytes,
                ptr::addr_of_mut!(MEMORY_MAP).cast::<c_void>(),
                &mut key,
                &mut descriptor_size,
                &mut descriptor_version,
            )
        };
        if map_status != SUCCESS || descriptor_size == 0 {
            return Err(PostFirmwareError::MemoryMap(map_status));
        }
        // SAFETY: `key` belongs to the immediately preceding memory map.
        last_status = unsafe { exit(image, key) };
        if last_status == SUCCESS {
            return Ok(());
        }
    }
    Err(PostFirmwareError::ExitBootServices(last_status))
}

fn expected_mask(apic_ids: &[u32]) -> Result<u64, PostFirmwareError> {
    if apic_ids.is_empty() || apic_ids.len() > 8 {
        return Err(PostFirmwareError::InvalidCpuId);
    }
    let mask = ap_expected_mask(apic_ids);
    if mask == 0 {
        Err(PostFirmwareError::InvalidCpuId)
    } else {
        Ok(mask)
    }
}

fn wait_for_exact_mask(value: &ExactAtomicU64, wanted: u64) -> bool {
    for _ in 0..50_000_000 {
        if exact_load(value) & wanted == wanted {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn wait_for_mask(value: &AtomicU64, wanted: u64) -> bool {
    for _ in 0..50_000_000 {
        if value.load(Ordering::Acquire) & wanted == wanted {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn wait_for_exact_count(value: &ExactAtomicU64, wanted: u64) -> bool {
    for _ in 0..50_000_000 {
        if exact_load(value) >= wanted {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

#[inline]
unsafe fn pio_read8(port: u16) -> u8 {
    let value: u8;
    // SAFETY: callers pass a port in the frozen PCI or virtio capability.
    unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack)) };
    value
}

#[inline]
unsafe fn pio_read16(port: u16) -> u16 {
    let value: u16;
    // SAFETY: callers pass an aligned port in the frozen device capability.
    unsafe { asm!("in ax, dx", in("dx") port, out("ax") value, options(nomem, nostack)) };
    value
}

#[inline]
unsafe fn pio_read32(port: u16) -> u32 {
    let value: u32;
    // SAFETY: callers pass an aligned port in the frozen device capability.
    unsafe { asm!("in eax, dx", in("dx") port, out("eax") value, options(nomem, nostack)) };
    value
}

#[inline]
unsafe fn pio_write8(port: u16, value: u8) {
    // SAFETY: callers pass a port in the frozen PCI or virtio capability.
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack)) };
}

#[inline]
unsafe fn pio_write16(port: u16, value: u16) {
    // SAFETY: callers pass an aligned port in the frozen device capability.
    unsafe { asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack)) };
}

#[inline]
unsafe fn pio_write32(port: u16, value: u32) {
    // SAFETY: callers pass an aligned port in the frozen device capability.
    unsafe { asm!("out dx, eax", in("dx") port, in("eax") value, options(nomem, nostack)) };
}

unsafe fn fw_cfg_read_be16() -> u16 {
    // SAFETY: selector 0x19 exposes the bounded fw_cfg file directory stream.
    let high = unsafe { pio_read8(0x511) };
    let low = unsafe { pio_read8(0x511) };
    u16::from_be_bytes([high, low])
}

unsafe fn fw_cfg_read_be32() -> u32 {
    // SAFETY: identical bounded fw_cfg stream ownership.
    let bytes = unsafe {
        [
            pio_read8(0x511),
            pio_read8(0x511),
            pio_read8(0x511),
            pio_read8(0x511),
        ]
    };
    u32::from_be_bytes(bytes)
}

unsafe fn fw_cfg_find(name_to_find: &[u8]) -> Result<Option<(u16, usize)>, PostFirmwareError> {
    // SAFETY: 0x510 selects the QEMU fw_cfg file directory; all subsequent
    // reads are bounded by the advertised entry count and fixed record width.
    unsafe { pio_write16(0x510, 0x19) };
    let count = unsafe { fw_cfg_read_be32() } as usize;
    if count > 1_024 {
        return Err(PostFirmwareError::Injection);
    }
    let mut selected = None;
    for _ in 0..count {
        let size = unsafe { fw_cfg_read_be32() };
        let selector = unsafe { fw_cfg_read_be16() };
        let _reserved = unsafe { fw_cfg_read_be16() };
        let mut name = [0_u8; 56];
        for byte in &mut name {
            *byte = unsafe { pio_read8(0x511) };
        }
        let end = name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(name.len());
        if &name[..end] == name_to_find {
            if size == 0 || size > 56 {
                return Err(PostFirmwareError::Injection);
            }
            selected = Some((selector, size as usize));
        }
    }
    Ok(selected)
}

unsafe fn fw_cfg_failure_apic() -> Result<Option<u32>, PostFirmwareError> {
    let selected = unsafe { fw_cfg_find(b"opt/thermite/fail-ap")? };
    let Some((selector, size)) = selected else {
        return Ok(None);
    };
    if size > 10 {
        return Err(PostFirmwareError::Injection);
    }
    unsafe { pio_write16(0x510, selector) };
    let mut value = 0_u32;
    let mut digits = 0;
    for _ in 0..size {
        let byte = unsafe { pio_read8(0x511) };
        if byte == 0 || byte == b'\n' {
            break;
        }
        if !byte.is_ascii_digit() {
            return Err(PostFirmwareError::Injection);
        }
        value = value
            .checked_mul(10)
            .and_then(|current| current.checked_add(u32::from(byte - b'0')))
            .ok_or(PostFirmwareError::Injection)?;
        digits += 1;
    }
    if digits == 0 {
        return Err(PostFirmwareError::Injection);
    }
    Ok(Some(value))
}

unsafe fn fw_cfg_power_action() -> Result<PowerAction, PostFirmwareError> {
    let Some((selector, size)) = (unsafe { fw_cfg_find(b"opt/thermite/power")? }) else {
        return Ok(PowerAction::PowerOff);
    };
    if size > 8 {
        return Err(PostFirmwareError::Injection);
    }
    unsafe { pio_write16(0x510, selector) };
    let mut value = [0_u8; 8];
    for byte in value.iter_mut().take(size) {
        *byte = unsafe { pio_read8(0x511) };
    }
    let end = value
        .iter()
        .position(|byte| *byte == 0 || *byte == b'\n')
        .unwrap_or(size);
    match &value[..end] {
        b"reboot" => Ok(PowerAction::Reboot),
        b"poweroff" => Ok(PowerAction::PowerOff),
        _ => Err(PostFirmwareError::Injection),
    }
}

unsafe fn pci_read32(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let address = pci_config_address(
        u64::from(bus),
        u64::from(device),
        u64::from(function),
        u64::from(offset),
    ) as u32;
    // SAFETY: 0xcf8/0xcfc are the profile-owned PCI configuration mechanism.
    unsafe {
        pio_write32(0xcf8, address);
        pio_read32(0xcfc)
    }
}

unsafe fn pci_write32(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    let address = pci_config_address(
        u64::from(bus),
        u64::from(device),
        u64::from(function),
        u64::from(offset),
    ) as u32;
    // SAFETY: 0xcf8/0xcfc are the profile-owned PCI configuration mechanism.
    unsafe {
        pio_write32(0xcf8, address);
        pio_write32(0xcfc, value);
    }
}

unsafe fn find_legacy_virtio_block() -> Result<(u8, u8, u8, u16), PostFirmwareError> {
    for bus in 0..=u8::MAX {
        for device in 0..32_u8 {
            // The frozen QEMU device is function zero.  Header inspection keeps
            // this bounded inventory extensible without probing absent functions.
            let identity = unsafe { pci_read32(bus, device, 0, 0) };
            if !pci_virtio_block_identity(u64::from(identity)) {
                continue;
            }
            let bar0 = unsafe { pci_read32(bus, device, 0, 0x10) };
            if !pci_legacy_io_bar_valid(u64::from(bar0)) {
                return Err(PostFirmwareError::Dma);
            }
            let base = pci_legacy_io_base(u64::from(bar0)) as u16;
            let command = unsafe { pci_read32(bus, device, 0, 0x04) };
            // I/O-space decode and bus mastering are the only rights granted.
            unsafe {
                pci_write32(
                    bus,
                    device,
                    0,
                    0x04,
                    pci_enable_io_bus_master(u64::from(command)) as u32,
                )
            };
            return Ok((bus, device, 0, base));
        }
    }
    Err(PostFirmwareError::Dma)
}

unsafe fn virtio_block_dma_probe() -> Result<(usize, u64), PostFirmwareError> {
    let (_bus, _device, _function, io) = unsafe { find_legacy_virtio_block()? };
    let host_features = dma_register_port(u64::from(io), 0) as u16;
    let guest_features = dma_register_port(u64::from(io), 4) as u16;
    let queue_pfn_port = dma_register_port(u64::from(io), 8) as u16;
    let queue_size_port = dma_register_port(u64::from(io), 12) as u16;
    let queue_select = dma_register_port(u64::from(io), 14) as u16;
    let queue_notify = dma_register_port(u64::from(io), 16) as u16;
    let device_status = dma_register_port(u64::from(io), 18) as u16;

    // SAFETY: these ports are within BAR0 and the device is reset before queue
    // configuration.  Feature zero is sufficient for the legacy read request.
    unsafe {
        pio_write8(device_status, dma_device_status(0) as u8);
        pio_write8(device_status, dma_device_status(1) as u8);
        pio_write8(device_status, dma_device_status(2) as u8);
        let _available_features = pio_read32(host_features);
        pio_write32(guest_features, 0);
        pio_write16(queue_select, 0);
    }
    let queue_size = u64::from(unsafe { pio_read16(queue_size_port) });
    if !dma_queue_size_valid(queue_size) {
        return Err(PostFirmwareError::Dma);
    }

    // SAFETY: the queue and request are statically reserved, uniquely owned by
    // this synchronous transaction, identity-mapped, and below 4 GiB as
    // required by the legacy queue-PFN ABI.
    let (queue, request) = unsafe {
        (
            ptr::addr_of_mut!(VIRTIO_QUEUE.0).cast::<u8>(),
            ptr::addr_of_mut!(VIRTIO_REQUEST),
        )
    };
    if queue as u64 > u64::from(u32::MAX) || request as u64 > u64::from(u32::MAX) {
        return Err(PostFirmwareError::Dma);
    }
    let available_offset = dma_descriptor_bytes(queue_size) as usize;
    let used_offset = dma_used_offset(queue_size) as usize;
    if !dma_queue_layout_valid(queue_size, (16 * 1024) as u64) {
        return Err(PostFirmwareError::Dma);
    }
    unsafe {
        ptr::write_bytes(queue, 0, 16 * 1024);
        ptr::write_bytes(
            request.cast::<u8>(),
            0,
            core::mem::size_of::<VirtioRequest>(),
        );
        ptr::write_volatile(ptr::addr_of_mut!((*request).status), 0xff);

        let descriptors = queue.cast::<VirtqDescriptor>();
        ptr::write_volatile(
            descriptors.add(0),
            VirtqDescriptor {
                address: ptr::addr_of!((*request).request_type) as u64,
                length: dma_descriptor_length(0) as u32,
                flags: dma_descriptor_flags(0) as u16,
                next: dma_descriptor_next(0) as u16,
            },
        );
        ptr::write_volatile(
            descriptors.add(1),
            VirtqDescriptor {
                address: ptr::addr_of!((*request).data) as u64,
                length: dma_descriptor_length(1) as u32,
                flags: dma_descriptor_flags(1) as u16,
                next: dma_descriptor_next(1) as u16,
            },
        );
        ptr::write_volatile(
            descriptors.add(2),
            VirtqDescriptor {
                address: ptr::addr_of!((*request).status) as u64,
                length: dma_descriptor_length(2) as u32,
                flags: dma_descriptor_flags(2) as u16,
                next: dma_descriptor_next(2) as u16,
            },
        );
        let available = queue.add(available_offset);
        ptr::write_volatile(available.cast::<u16>().add(0), 0);
        ptr::write_volatile(available.cast::<u16>().add(2), 0);
        ptr::write_volatile(available.cast::<u16>().add(4), 0);

        pio_write32(queue_pfn_port, dma_queue_pfn(queue as u64) as u32);
        pio_write8(device_status, dma_device_status(3) as u8);
        if !dma_device_ready(u64::from(pio_read8(device_status))) {
            return Err(PostFirmwareError::Dma);
        }
        core::sync::atomic::fence(Ordering::SeqCst);
        asm!("mfence", options(nostack));
        // Ownership passes from CPU to device when avail.idx becomes visible.
        ptr::write_volatile(
            available.cast::<u16>().add(1),
            dma_publish_available(0) as u16,
        );
        pio_write16(queue_notify, 0);
    }

    let used_index = unsafe { queue.add(used_offset + 2).cast::<u16>() };
    let mut complete = false;
    for _ in 0..50_000_000 {
        // SAFETY: the device owns used.idx and publishes it after DMA writes.
        if unsafe { ptr::read_volatile(used_index) } == 1 {
            complete = true;
            break;
        }
        core::hint::spin_loop();
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    // SAFETY: used.idx returned ownership of the request and data buffers.
    let (status, signature_low, signature_high) = unsafe {
        (
            ptr::read_volatile(ptr::addr_of!((*request).status)),
            ptr::read_volatile(ptr::addr_of!((*request).data).cast::<u8>().add(510)),
            ptr::read_volatile(ptr::addr_of!((*request).data).cast::<u8>().add(511)),
        )
    };
    let valid = thermite_kernel_policy::kernel_policy_ingress::thermite_dma_runtime_complete(
        complete,
        u64::from(status),
        u64::from(signature_low),
        u64::from(signature_high),
        512,
        1,
    );
    if !valid {
        return Err(PostFirmwareError::Dma);
    }
    // Generation zero was pinned and transferred device->CPU; unpin advances
    // the now-inaccessible mapping to generation one.
    Ok((512, 1))
}

fn entropy_probe() -> Result<usize, PostFirmwareError> {
    let mut entropy = [0_u64; 4];
    for word in &mut entropy {
        let mut accepted = false;
        for _ in 0..16 {
            let value: u64;
            let valid: u8;
            // SAFETY: RDRAND is part of the frozen QEMU CPU profile.  Its
            // carry flag is checked and a bounded health failure is explicit.
            unsafe {
                asm!(
                    "rdrand {value}",
                    "setc {valid}",
                    value = out(reg) value,
                    valid = out(reg_byte) valid,
                    options(nomem, nostack)
                );
            }
            if valid != 0 {
                *word = value;
                accepted = true;
                break;
            }
            core::hint::spin_loop();
        }
        if !accepted {
            return Err(PostFirmwareError::Entropy);
        }
    }
    // Do not publish random material; consume it through a volatile reduction
    // so the exact-fill health probe remains executable after optimization.
    let health = entropy.into_iter().fold(0_u64, |state, word| state ^ word);
    // SAFETY: the stack word is initialized and the read has no aliasing side
    // effect beyond keeping the hardware result observable.
    let _ = unsafe { ptr::read_volatile(ptr::addr_of!(health)) };
    Ok(32)
}

unsafe fn run_user_mode(user_base: u64, user_stack: u64) -> Result<(u64, u64), PostFirmwareError> {
    USER_SYSCALLS.store(0, Ordering::Release);
    USER_FAULTS.store(0, Ordering::Release);
    USER_FINISHED.store(0, Ordering::Release);
    USER_SYSCALL_VALUE.store(0, Ordering::Release);
    USER_FINISH_VALUE.store(0, Ordering::Release);
    KERNEL_FAULTS.store(0, Ordering::Release);
    // SAFETY: USER_BASE and USER_STACK are user-accessible mappings, the TSS
    // supplies a bounded ring-0 stack, and both software-interrupt gates carry
    // DPL 3.  The completion gate returns to this exact call frame.
    unsafe { thermite_enter_user(user_base, service_user_stack_pointer(user_stack)) };
    let syscalls = USER_SYSCALLS.load(Ordering::Acquire);
    let faults = USER_FAULTS.load(Ordering::Acquire);
    if !thermite_kernel_policy::kernel_policy_ingress::thermite_service_runtime_complete(
        syscalls,
        faults,
        USER_FINISHED.load(Ordering::Acquire),
        USER_SYSCALL_VALUE.load(Ordering::Acquire),
        USER_FINISH_VALUE.load(Ordering::Acquire),
        KERNEL_FAULTS.load(Ordering::Acquire),
    ) {
        return Err(PostFirmwareError::UserMode);
    }
    Ok((syscalls, faults))
}

/// Leave firmware ownership and run the complete post-firmware acceptance
/// kernel.  Returning `Ok` means every CPU remains parked in the kernel and the
/// BSP alone resumes to issue the terminal runtime-service reset.
pub unsafe fn run(
    serial: &Serial,
    image: Handle,
    boot_services: *mut BootServices,
    apic_ids: &[u32],
    bsp_apic_id: u32,
) -> Result<PostFirmwareReport, PostFirmwareError> {
    if apic_ids.is_empty() || apic_ids.len() > MAX_CPUS || !apic_ids.contains(&bsp_apic_id) {
        return Err(PostFirmwareError::InvalidCpuId);
    }
    // The optional QEMU fw_cfg input is an adversarial test fixture, not a build
    // variant: the exact same image exercises the policy-approved smaller online
    // set after one named AP startup failure.
    let failed_apic = unsafe { fw_cfg_failure_apic()? };
    let power_action = unsafe { fw_cfg_power_action()? };
    let bsp_failed = failed_apic == Some(bsp_apic_id);
    let failed_outside_set = failed_apic.is_some_and(|failed| !apic_ids.contains(&failed));
    let discovered_count = apic_ids.len();
    let mut effective_ids = [0_u32; MAX_CPUS];
    let mut effective_count = 0;
    for &apic_id in apic_ids {
        if ap_should_start(
            apic_id as u64,
            failed_apic.is_some(),
            u64::from(failed_apic.unwrap_or(0)),
        ) {
            effective_ids[effective_count] = apic_id;
            effective_count += 1;
        }
    }
    if !thermite_kernel_policy::kernel_policy_ingress::thermite_ap_failure_selection_valid(
        discovered_count as u64,
        bsp_failed,
        failed_outside_set,
        effective_count as u64,
    ) {
        return Err(PostFirmwareError::Injection);
    }
    let apic_ids = &effective_ids[..effective_count];
    let thermite_policy_signature = crate::thermite_policy_execute(apic_ids.len() as u64);
    let thermite_policy_flags = signature_policy_flags(thermite_policy_signature);
    if thermite_policy_flags & !POLICY_KNOWN != 0 {
        return Err(PostFirmwareError::Scheduler);
    }
    let thermite_task_base = signature_task_base(thermite_policy_signature, apic_ids.len() as u64);
    if thermite_task_base == 0 {
        return Err(PostFirmwareError::Scheduler);
    }
    let thermite_task_base = thermite_task_base as usize;
    let full_mask = expected_mask(apic_ids)?;
    let bsp_bit = bit(bsp_apic_id as usize)?;
    // SAFETY: these architectural reads only validate the frozen xAPIC profile.
    let apic = unsafe { rdmsr(APIC_BASE_MSR) };
    if !apic_profile_supported(apic) {
        return Err(PostFirmwareError::UnsupportedApic);
    }

    if thermite_policy_flags & (POLICY_ALLOCATOR | POLICY_MAPPING)
        != POLICY_ALLOCATOR | POLICY_MAPPING
    {
        return Err(PostFirmwareError::Memory);
    }
    let user_base = service_user_base();
    let user_stack = service_user_stack(user_base);
    let user_fault_address = service_user_fault_address(user_base);
    let user_syscall_value = service_syscall_value();
    let user_finish_value = service_finish_value();
    USER_EXPECTED_FAULT_ADDRESS.store(user_fault_address, Ordering::Release);
    // SAFETY: initialization occurs on the BSP before AP release.
    let cr3 = unsafe { setup_page_tables(user_base)? };
    unsafe {
        setup_user_code(user_fault_address, user_syscall_value, user_finish_value);
        setup_descriptor_tables();
        install_trampoline(boot_services, cr3, apic_ids)?;
        reserve_boot_heap(boot_services)?;
        exit_boot_services(image, boot_services)?;
    }
    serial.bytes(b"THERMITE_EXIT_BOOT_SERVICES ownership=kernel\n");
    if let Some(failed) = failed_apic {
        serial.bytes(b"THERMITE_AP_FAILURE apic_id=");
        serial.usize(failed as usize);
        serial.bytes(b" state=Failed reason=injected online=");
        serial.usize(apic_ids.len());
        serial.bytes(b"\n");
    }

    // SAFETY: from this point the image owns its page tables, descriptor tables,
    // LAPIC, and CPU-local stacks.  No Boot Service is called again.
    unsafe {
        asm!("cli", options(nomem, nostack));
        switch_page_table(cr3);
    }
    serial.bytes(b"THERMITE_POST_STAGE paging=1\n");
    unsafe { install_gdt(true) };
    serial.bytes(b"THERMITE_POST_STAGE gdt=1 tss=1\n");
    unsafe { install_idt() };
    serial.bytes(b"THERMITE_POST_STAGE idt=1\n");
    unsafe { install_syscall_entry() };
    serial.bytes(b"THERMITE_POST_STAGE syscall=1\n");
    unsafe { install_cpu_local(bsp_apic_id as usize)? };
    serial.bytes(b"THERMITE_POST_STAGE cpu_local=1\n");
    unsafe { local_apic_enable()? };
    serial.bytes(b"THERMITE_POST_STAGE lapic=1\n");
    if cpu_id() != bsp_apic_id as usize {
        return Err(PostFirmwareError::InvalidCpuId);
    }

    let (heap_bytes, heap_allocations, heap_oom_rejected) = allocator_probe()?;
    serial.bytes(b"THERMITE_POST_STAGE allocator=1\n");
    let device_negative_checks = unsafe { volatile_device_probe()? };
    serial.bytes(b"THERMITE_POST_STAGE device_widths=1\n");

    exact_store(&POST_ONLINE_MASK, bsp_bit);
    if thermite_policy_flags & POLICY_AP_LIFECYCLE == 0 {
        return Err(PostFirmwareError::ApStartup);
    }
    for &apic_id in apic_ids {
        if apic_id != bsp_apic_id {
            // SAFETY: each target is a unique discovered AP with a prepared
            // stack and the common trampoline page.
            unsafe { start_ap(apic_id)? };
        }
    }
    let ap_count = apic_ids.len() - 1;
    let ap_mask = full_mask & !bsp_bit;
    let _online_arrived = wait_for_exact_mask(&POST_ONLINE_MASK, full_mask);
    let _ready_arrived = wait_for_exact_count(&POST_READY, ap_count as u64);
    let _cpu_locals_arrived = wait_for_exact_mask(&POST_CPU_LOCAL_MASK, full_mask);
    if !thermite_kernel_policy::kernel_policy_ingress::thermite_ap_runtime_ready(
        exact_load(&POST_ONLINE_MASK).count_ones() as u64,
        apic_ids.len() as u64,
        exact_load(&POST_READY),
        ap_count as u64,
        exact_load(&POST_CPU_LOCAL_MASK).count_ones() as u64,
    ) {
        return Err(PostFirmwareError::ApStartup);
    }
    // SAFETY: IDT/LAPIC are ready before enabling BSP interrupts.
    unsafe { asm!("sti", options(nomem, nostack)) };

    POST_IPI_ACK_MASK.store(0, Ordering::Release);
    exact_store(&POST_IPI_EPOCH, 1);
    for &apic_id in apic_ids {
        if apic_id != bsp_apic_id {
            // SAFETY: all APs reported online after loading IPI_VECTOR.
            unsafe { send_fixed_ipi(apic_id)? };
        }
    }
    if !wait_for_mask(&POST_IPI_ACK_MASK, ap_mask) {
        return Err(PostFirmwareError::Ipi);
    }

    if thermite_policy_flags & POLICY_SYNCHRONIZATION == 0 {
        return Err(PostFirmwareError::Scheduler);
    }
    // Assign one dense seed task to every online CPU, then release the shared
    // remainder only after all of them enter the active scheduler interval.
    thermite_kernel_policy::kernel_policy_ingress::thermite_atomic_store(
        &POST_NEXT_TASK,
        apic_ids.len() as u64,
    );
    exact_store(&POST_TASK_BASE, thermite_task_base as u64);
    exact_store(&POST_TASK_READY, 0);
    exact_store(&POST_TASK_GATE, 0);
    exact_store(&POST_EXPECTED_WORKERS, apic_ids.len() as u64);
    exact_store(&POST_TASK_SUM, 0);
    exact_store(&POST_TASK_WORKERS, 0);
    exact_store(&POST_TASK_DONE, 0);
    exact_store(&POST_MAX_ACTIVE, 0);
    exact_store(&POST_READY, 0);
    thermite_kernel_policy::kernel_policy_ingress::thermite_atomic_store(
        &POST_MESSAGE_PAYLOAD,
        0x5245_4c45_4153_4544,
    );
    thermite_kernel_policy::kernel_policy_ingress::thermite_atomic_store(&POST_MESSAGE_READY, 1);
    exact_store(&POST_PHASE, 1);
    message_probe(bsp_apic_id as usize);
    run_tasks(bsp_apic_id as usize);
    lock_once(bsp_apic_id as usize);
    shootdown_probe(
        bsp_apic_id as usize,
        0xaaaa_5555_1111_2222,
        &POST_TLB_PRE_MASK,
    );
    if !wait_for_exact_count(&POST_TASK_DONE, ap_count as u64)
        || !wait_for_exact_mask(&POST_TLB_PRE_MASK, full_mask)
    {
        return Err(PostFirmwareError::Scheduler);
    }
    if !wait_for_exact_mask(&POST_MESSAGE_MASK, full_mask) || exact_load(&POST_MESSAGE_STALE) != 0 {
        return Err(PostFirmwareError::Scheduler);
    }
    if exact_fetch_and(&POST_TLB_STALE, 0) != 0 {
        serial.bytes(b"THERMITE_POST_DIAG tlb_pre_stale=1 observed_bsp=");
        serial.usize(exact_load(&POST_TLB_OBSERVED[bsp_apic_id as usize]) as usize);
        for &apic_id in apic_ids {
            if apic_id != bsp_apic_id {
                serial.bytes(b" observed_ap=");
                serial.usize(exact_load(&POST_TLB_OBSERVED[apic_id as usize]) as usize);
                break;
            }
        }
        serial.bytes(b"\n");
        return Err(PostFirmwareError::Tlb);
    }
    let expected_sum =
        thermite_kernel_policy::kernel_policy_ingress::thermite_scheduler_expected_sum(
            thermite_task_base as u64,
        );
    let worker_mask = exact_load(&POST_TASK_WORKERS);
    let worker_cpus = worker_mask.count_ones() as usize;
    let ap_workers = (worker_mask & ap_mask).count_ones() as usize;
    let required_ap_workers = scheduler_required_ap_workers(ap_count as u64);
    let required_parallel_cpus = scheduler_required_parallel_cpus(ap_count as u64);
    if !thermite_kernel_policy::kernel_policy_ingress::thermite_scheduler_runtime_complete(
        exact_load(&POST_TASK_SUM),
        expected_sum,
        worker_cpus as u64,
        1 + required_ap_workers,
        exact_load(&POST_LOCK_ENTRIES),
        apic_ids.len() as u64,
        exact_load(&POST_ONCE),
        exact_load(&POST_MAX_ACTIVE),
        required_parallel_cpus,
    ) {
        return Err(PostFirmwareError::Scheduler);
    }
    serial.bytes(b"THERMITE_POST_STAGE scheduler=1\n");

    if thermite_policy_flags & POLICY_SHOOTDOWN == 0 {
        return Err(PostFirmwareError::Tlb);
    }
    // Replace one executable mapping, then require an interrupt-driven local
    // invalidation and same-epoch acknowledgement from every AP.
    // SAFETY: the BSP exclusively owns this PTE transition.
    unsafe {
        ptr::write_volatile(
            ptr::addr_of_mut!(SHOOT_PT.0).cast::<u64>(),
            mapping_kernel_data_entry(ptr::addr_of!(TEST_PAGE_B.0) as u64),
        );
        core::sync::atomic::fence(Ordering::SeqCst);
        asm!("mfence", options(nostack));
        asm!("invlpg [{}]", in(reg) SHOOTDOWN_ADDRESS, options(nostack));
    }
    POST_IPI_ACK_MASK.store(0, Ordering::Release);
    exact_store(&POST_IPI_EPOCH, 2);
    for &apic_id in apic_ids {
        if apic_id != bsp_apic_id {
            // SAFETY: the same vector now carries mapping epoch 2.
            unsafe { send_fixed_ipi(apic_id)? };
        }
    }
    if !wait_for_mask(&POST_IPI_ACK_MASK, ap_mask) {
        serial.bytes(b"THERMITE_POST_DIAG tlb_ack_timeout=1\n");
        return Err(PostFirmwareError::Tlb);
    }
    exact_store(&POST_PHASE, 2);
    shootdown_probe(
        bsp_apic_id as usize,
        0xbbbb_6666_3333_4444,
        &POST_TLB_POST_MASK,
    );
    let _shootdown_observed = wait_for_exact_mask(&POST_TLB_POST_MASK, full_mask);
    if !thermite_kernel_policy::kernel_policy_ingress::thermite_shootdown_runtime_complete(
        exact_load(&POST_TLB_POST_MASK).count_ones() as u64,
        apic_ids.len() as u64,
        exact_load(&POST_TLB_STALE),
        POST_IPI_ACK_MASK.load(Ordering::Acquire).count_ones() as u64,
        ap_count as u64,
        exact_load(&POST_IPI_EPOCH),
        2,
    ) {
        serial.bytes(b"THERMITE_POST_DIAG tlb_post_count=");
        serial.usize(exact_load(&POST_TLB_POST_MASK).count_ones() as usize);
        serial.bytes(b" stale=");
        serial.usize(exact_load(&POST_TLB_STALE) as usize);
        serial.bytes(b"\n");
        return Err(PostFirmwareError::Tlb);
    }
    serial.bytes(b"THERMITE_POST_STAGE tlb=1\n");

    // SAFETY: BSP owns its timer; APs arm theirs immediately after phase 2.
    POST_TIMER_MASK.store(0, Ordering::Release);
    // SAFETY: this is a kernel-originated delivery test for the installed gate;
    // it is cleared before the hardware timer is armed and is not acceptance
    // evidence by itself.
    unsafe { asm!("int 0xf2", options(nomem, nostack)) };
    if POST_TIMER_MASK.load(Ordering::Acquire) & bsp_bit == 0 {
        return Err(PostFirmwareError::Timer);
    }
    serial.bytes(b"THERMITE_POST_STAGE timer_gate=1\n");
    POST_TIMER_MASK.store(0, Ordering::Release);
    unsafe { arm_timer()? };
    for &apic_id in apic_ids {
        let cpu_bit = bit(apic_id as usize)?;
        if apic_id != bsp_apic_id && POST_TIMER_MASK.load(Ordering::Acquire) & cpu_bit == 0 {
            // SAFETY: the AP has passed phase 2 and installed TIMER_VECTOR.  The
            // BSP is the fallback deadline broker when TCG does not deliver the
            // AP's local TSC-deadline event.
            unsafe { send_raw_ipi(apic_id, u32::from(TIMER_VECTOR))? };
        }
    }
    serial.bytes(b"THERMITE_POST_STAGE timer_armed=1\n");
    if !wait_for_mask(&POST_TIMER_MASK, full_mask) {
        serial.bytes(b"THERMITE_POST_DIAG timer_count=");
        serial.usize(POST_TIMER_MASK.load(Ordering::Acquire).count_ones() as usize);
        serial.bytes(b" fallbacks=");
        serial.usize(exact_load(&POST_TIMER_IPI_FALLBACKS) as usize);
        serial.bytes(b"\n");
        return Err(PostFirmwareError::Timer);
    }
    serial.bytes(b"THERMITE_POST_STAGE timer=1\n");

    if thermite_policy_flags & POLICY_DMA == 0 {
        return Err(PostFirmwareError::Dma);
    }
    // SAFETY: the BSP owns PCI configuration and the single synchronous
    // virtio-blk queue; all APs are quiescent in their interruptible idle loop.
    let (dma_bytes, dma_generation) = unsafe { virtio_block_dma_probe()? };
    serial.bytes(b"THERMITE_POST_STAGE virtio_dma=1\n");
    let entropy_bytes = entropy_probe()?;
    serial.bytes(b"THERMITE_POST_STAGE entropy=1\n");

    if thermite_policy_flags & POLICY_SERVICES == 0 {
        return Err(PostFirmwareError::UserMode);
    }
    // SAFETY: paging, TSS, GDT, and all relevant IDT gates are now live.
    serial.bytes(b"THERMITE_POST_STAGE user_enter=1\n");
    let (syscalls, faults) = unsafe { run_user_mode(user_base, user_stack)? };
    Ok(PostFirmwareReport {
        online: exact_load(&POST_ONLINE_MASK).count_ones() as usize,
        failed: usize::from(failed_apic.is_some()),
        failed_apic_id: failed_apic.unwrap_or(u32::MAX),
        ap_workers,
        worker_cpus,
        parallel_cpus: exact_load(&POST_MAX_ACTIVE) as usize,
        task_sum: exact_load(&POST_TASK_SUM),
        thermite_policy_signature,
        thermite_policy_flags,
        thermite_task_base,
        heap_bytes,
        heap_allocations,
        heap_oom_rejected,
        atomic_message_cpus: exact_load(&POST_MESSAGE_MASK).count_ones() as usize,
        atomic_message_stale: exact_load(&POST_MESSAGE_STALE) as usize,
        device_negative_checks,
        cpu_local_cpus: exact_load(&POST_CPU_LOCAL_MASK).count_ones() as usize,
        power_action,
        lock_entries: exact_load(&POST_LOCK_ENTRIES) as usize,
        ipi_acks: POST_IPI_ACK_MASK.load(Ordering::Acquire).count_ones() as usize,
        timer_cpus: POST_TIMER_MASK.load(Ordering::Acquire).count_ones() as usize,
        timer_ipi_fallbacks: exact_load(&POST_TIMER_IPI_FALLBACKS) as usize,
        tlb_cpus: exact_load(&POST_TLB_POST_MASK).count_ones() as usize,
        dma_bytes,
        dma_generation,
        entropy_bytes,
        syscalls,
        faults,
    })
}

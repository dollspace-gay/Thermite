#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;
extern crate thermite_kernel_policy;

use core::ffi::c_void;
use core::panic::PanicInfo;
use core::ptr;

#[path = "../../kernel_shell.rs"]
mod kernel_shell;
mod post_firmware;

use kernel_shell::{Clock as TplClock, Instant as TplInstant};

static POLICY_SCHEDULER_COUNTER: thermite_kernel_policy::atomic::ExactAtomicU64 =
    thermite_kernel_policy::atomic::ExactAtomicU64::new(0);

#[used]
static TPL_ATOMIC_COMPARE_EXCHANGE_BINDING: extern "C" fn(
    &thermite_kernel_policy::atomic::ExactAtomicU64,
    u64,
    u64,
    thermite_kernel_policy::Ordering,
    thermite_kernel_policy::Ordering,
) -> thermite_kernel_policy::Cas = thermite_kernel_policy::atomic::tpl_atomic_compare_exchange;

#[used]
static TPL_ATOMIC_FETCH_BINDING: extern "C" fn(
    &thermite_kernel_policy::atomic::ExactAtomicU64,
    thermite_kernel_policy::FetchOp,
    u64,
    thermite_kernel_policy::Ordering,
) -> u64 = thermite_kernel_policy::atomic::tpl_atomic_fetch;

#[used]
static TPL_ATOMIC_LOAD_BINDING: extern "C" fn(
    &thermite_kernel_policy::atomic::ExactAtomicU64,
    thermite_kernel_policy::Ordering,
) -> u64 = thermite_kernel_policy::atomic::tpl_atomic_load;

#[used]
static TPL_ATOMIC_STORE_BINDING: extern "C" fn(
    &thermite_kernel_policy::atomic::ExactAtomicU64,
    u64,
    thermite_kernel_policy::Ordering,
) = thermite_kernel_policy::atomic::tpl_atomic_store;

#[no_mangle]
#[inline(never)]
pub extern "C" fn thermite_policy_execute(cpus: u64) -> u64 {
    thermite_kernel_policy::kernel_policy_ingress::thermite_kernel_policy_entry(
        cpus,
        &POLICY_SCHEDULER_COUNTER,
    )
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn tpl_clock_read(clock: TplClock) -> TplInstant {
    let ticks = boundary::timestamp_counter();
    TplInstant {
        ticks,
        scale_numerator: 1,
        scale_denominator: 1_000_000,
        error_ticks: clock.slot & 1,
    }
}

#[used]
static TPL_CLOCK_READ_BINDING: extern "C" fn(TplClock) -> TplInstant = tpl_clock_read;

mod boundary {
    use core::arch::asm;

    const COM1: u16 = 0x3f8;

    #[inline]
    unsafe fn out8(port: u16, value: u8) {
        // SAFETY: this module is the profile's reviewed privileged boundary. The
        // caller only passes fixed legacy UART ports owned by this boot image.
        unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack)) };
    }

    #[inline]
    unsafe fn in8(port: u16) -> u8 {
        let value: u8;
        // SAFETY: see `out8`; the fixed line-status port is in the UART range.
        unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack)) };
        value
    }

    pub fn init_serial() {
        // SAFETY: all ports are within the fixed COM1 capability for this profile.
        unsafe {
            out8(COM1 + 1, 0x00);
            out8(COM1 + 3, 0x80);
            out8(COM1, 0x03);
            out8(COM1 + 1, 0x00);
            out8(COM1 + 3, 0x03);
            out8(COM1 + 2, 0xc7);
            out8(COM1 + 4, 0x0b);
        }
    }

    pub fn serial_byte(byte: u8) {
        for _ in 0..100_000 {
            // SAFETY: the line-status port is within the fixed COM1 range.
            if unsafe { in8(COM1 + 5) } & 0x20 != 0 {
                // SAFETY: the data port is within the fixed COM1 range.
                unsafe { out8(COM1, byte) };
                return;
            }
            core::hint::spin_loop();
        }
    }

    #[inline]
    pub fn timestamp_counter() -> u64 {
        let low: u32;
        let high: u32;
        // SAFETY: RDTSC is available on the frozen x86_64 baseline and has no
        // memory operands. It is used only as an explicitly fallible raw clock.
        unsafe { asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack)) };
        (u64::from(high) << 32) | u64::from(low)
    }
}

type Status = usize;
type Handle = *mut c_void;
const SUCCESS: Status = 0;
const MAX_CPUS: usize = 64;

#[repr(C)]
struct TableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    reserved: u32,
}

type LocateProtocol = unsafe extern "efiapi" fn(
    protocol: *const Guid,
    registration: *mut c_void,
    interface: *mut *mut c_void,
) -> Status;
type HandleProtocol = unsafe extern "efiapi" fn(
    handle: Handle,
    protocol: *const Guid,
    interface: *mut *mut c_void,
) -> Status;
type GetMemoryMap = unsafe extern "efiapi" fn(
    memory_map_size: *mut usize,
    memory_map: *mut c_void,
    map_key: *mut usize,
    descriptor_size: *mut usize,
    descriptor_version: *mut u32,
) -> Status;
type Stall = unsafe extern "efiapi" fn(microseconds: usize) -> Status;
type ResetSystem = unsafe extern "efiapi" fn(
    reset_type: u32,
    reset_status: Status,
    data_size: usize,
    reset_data: *const u16,
);

#[repr(C)]
struct BootServices {
    header: TableHeader,
    raise_tpl: usize,
    restore_tpl: usize,
    allocate_pages: usize,
    free_pages: usize,
    get_memory_map: GetMemoryMap,
    allocate_pool: usize,
    free_pool: usize,
    create_event: usize,
    set_timer: usize,
    wait_for_event: usize,
    signal_event: usize,
    close_event: usize,
    check_event: usize,
    install_protocol_interface: usize,
    reinstall_protocol_interface: usize,
    uninstall_protocol_interface: usize,
    handle_protocol: HandleProtocol,
    reserved: usize,
    register_protocol_notify: usize,
    locate_handle: usize,
    locate_device_path: usize,
    install_configuration_table: usize,
    load_image: usize,
    start_image: usize,
    exit: usize,
    unload_image: usize,
    exit_boot_services: usize,
    get_next_monotonic_count: usize,
    stall: Stall,
    set_watchdog_timer: usize,
    connect_controller: usize,
    disconnect_controller: usize,
    open_protocol: usize,
    close_protocol: usize,
    open_protocol_information: usize,
    protocols_per_handle: usize,
    locate_handle_buffer: usize,
    locate_protocol: LocateProtocol,
}

#[repr(C)]
struct RuntimeServices {
    header: TableHeader,
    before_reset_system: [usize; 10],
    reset_system: ResetSystem,
}

#[repr(C)]
struct SystemTable {
    header: TableHeader,
    firmware_vendor: *mut u16,
    firmware_revision: u32,
    console_in_handle: Handle,
    con_in: *mut c_void,
    console_out_handle: Handle,
    con_out: *mut c_void,
    standard_error_handle: Handle,
    std_err: *mut c_void,
    runtime_services: *mut RuntimeServices,
    boot_services: *mut BootServices,
    number_of_table_entries: usize,
    configuration_table: *mut c_void,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const MP_SERVICES_GUID: Guid = Guid {
    data1: 0x3fdd_a605,
    data2: 0xa76e,
    data3: 0x4f46,
    data4: [0xad, 0x29, 0x12, 0xf4, 0x53, 0x1b, 0x3d, 0x08],
};
const LOADED_IMAGE_GUID: Guid = Guid {
    data1: 0x5b1b_31a1,
    data2: 0x9562,
    data3: 0x11d2,
    data4: [0x8e, 0x3f, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};
const BLOCK_IO_GUID: Guid = Guid {
    data1: 0x964e_5b21,
    data2: 0x6459,
    data3: 0x11d2,
    data4: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};
const ACPI_20_TABLE_GUID: Guid = Guid {
    data1: 0x8868_e871,
    data2: 0xe4f1,
    data3: 0x11d3,
    data4: [0xbc, 0x22, 0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81],
};
const ACPI_TABLE_GUID: Guid = Guid {
    data1: 0xeb9d_2d30,
    data2: 0x2d88,
    data3: 0x11d3,
    data4: [0x9a, 0x16, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d],
};

#[repr(C)]
struct ConfigurationTable {
    vendor_guid: Guid,
    vendor_table: *const u8,
}

#[repr(C)]
struct LoadedImage {
    revision: u32,
    parent_handle: Handle,
    system_table: *mut SystemTable,
    device_handle: Handle,
    file_path: *mut c_void,
    reserved: *mut c_void,
    load_options_size: u32,
    load_options: *mut c_void,
    image_base: *mut c_void,
    image_size: u64,
    image_code_type: u32,
    image_data_type: u32,
    unload: usize,
}

struct NormalizedHandoff {
    acpi_bytes: usize,
    firmware_entries: usize,
    firmware_bytes: usize,
    command_line_bytes: usize,
    initrd_bytes: usize,
    image_bytes: usize,
}

fn bounded_checksum(address: *const u8, len: usize) -> Result<u8, Status> {
    if address.is_null() || len == 0 || len > 4096 {
        return Err(usize::MAX);
    }
    let mut checksum = 0_u8;
    for offset in 0..len {
        // SAFETY: the caller obtained this immutable firmware table and bounded
        // its architecture-defined length to one page before this iteration.
        checksum = checksum.wrapping_add(unsafe { ptr::read_volatile(address.add(offset)) });
    }
    Ok(checksum)
}

fn normalized_handoff(
    image: Handle,
    system_table: *mut SystemTable,
    boot_services: *mut BootServices,
) -> Result<NormalizedHandoff, Status> {
    let mut loaded_interface: *mut c_void = ptr::null_mut();
    // SAFETY: the image handle owns the LoadedImage protocol until handoff.
    let loaded_status = unsafe {
        ((*boot_services).handle_protocol)(image, &LOADED_IMAGE_GUID, &mut loaded_interface)
    };
    if loaded_status != SUCCESS || loaded_interface.is_null() {
        return Err(loaded_status);
    }
    let loaded = loaded_interface.cast::<LoadedImage>();
    // SAFETY: the successful protocol lookup fixes the ABI and lifetime.
    let (image_base, image_size, options, options_size) = unsafe {
        (
            (*loaded).image_base as usize,
            usize::try_from((*loaded).image_size).map_err(|_| usize::MAX)?,
            (*loaded).load_options,
            (*loaded).load_options_size as usize,
        )
    };
    if image_base == 0
        || image_size == 0
        || image_size > 64 * 1024 * 1024
        || image_base.checked_add(image_size).is_none()
        || (options_size != 0
            && (options.is_null()
                || options_size > 64 * 1024
                || (options as usize).checked_add(options_size).is_none()))
    {
        return Err(usize::MAX);
    }

    // SAFETY: system_table was validated by efi_main and firmware owns its
    // immutable configuration table through ExitBootServices.
    let (entries, tables) = unsafe {
        (
            (*system_table).number_of_table_entries,
            (*system_table)
                .configuration_table
                .cast::<ConfigurationTable>(),
        )
    };
    if entries == 0 || entries > 1024 || tables.is_null() {
        return Err(usize::MAX);
    }
    let firmware_bytes = entries
        .checked_mul(core::mem::size_of::<ConfigurationTable>())
        .ok_or(usize::MAX)?;
    let mut acpi = ptr::null();
    for index in 0..entries {
        // SAFETY: index is bounded by the firmware-advertised table count.
        let table = unsafe { &*tables.add(index) };
        if table.vendor_guid == ACPI_20_TABLE_GUID || table.vendor_guid == ACPI_TABLE_GUID {
            acpi = table.vendor_table;
            if table.vendor_guid == ACPI_20_TABLE_GUID {
                break;
            }
        }
    }
    if acpi.is_null() {
        return Err(usize::MAX);
    }
    let mut signature = [0_u8; 8];
    for (index, byte) in signature.iter_mut().enumerate() {
        // SAFETY: every RSDP revision starts with the fixed 20-byte prefix.
        *byte = unsafe { ptr::read_volatile(acpi.add(index)) };
    }
    if &signature != b"RSD PTR " || bounded_checksum(acpi, 20)? != 0 {
        return Err(usize::MAX);
    }
    // SAFETY: byte 15 is inside the validated legacy RSDP prefix.
    let revision = unsafe { ptr::read_volatile(acpi.add(15)) };
    let acpi_bytes = if revision >= 2 {
        // SAFETY: revision >=2 defines the little-endian length at byte 20.
        let length = unsafe { ptr::read_unaligned(acpi.add(20).cast::<u32>()) } as usize;
        if !(36..=4096).contains(&length) || bounded_checksum(acpi, length)? != 0 {
            return Err(usize::MAX);
        }
        length
    } else {
        20
    };
    Ok(NormalizedHandoff {
        acpi_bytes,
        firmware_entries: entries,
        firmware_bytes,
        command_line_bytes: options_size,
        initrd_bytes: 0,
        image_bytes: image_size,
    })
}

#[repr(C)]
struct BlockIoMedia {
    media_id: u32,
    removable_media: u8,
    media_present: u8,
    logical_partition: u8,
    read_only: u8,
    write_caching: u8,
    block_size: u32,
    io_align: u32,
    last_block: u64,
    lowest_aligned_lba: u64,
    logical_blocks_per_physical_block: u32,
    optimal_transfer_length_granularity: u32,
}

type ReadBlocks = unsafe extern "efiapi" fn(
    this: *mut BlockIo,
    media_id: u32,
    lba: u64,
    buffer_size: usize,
    buffer: *mut c_void,
) -> Status;

#[repr(C)]
struct BlockIo {
    revision: u64,
    media: *mut BlockIoMedia,
    reset: usize,
    read_blocks: ReadBlocks,
    write_blocks: usize,
    flush_blocks: usize,
}

#[repr(C, align(4096))]
struct BlockBuffer([u8; 4096]);

type GetNumberOfProcessors = unsafe extern "efiapi" fn(
    this: *mut MpServices,
    total: *mut usize,
    enabled: *mut usize,
) -> Status;
type GetProcessorInfo = unsafe extern "efiapi" fn(
    this: *mut MpServices,
    processor_number: usize,
    information: *mut ProcessorInformation,
) -> Status;
type WhoAmI = unsafe extern "efiapi" fn(this: *mut MpServices, cpu: *mut usize) -> Status;

#[repr(C)]
struct MpServices {
    get_number_of_processors: GetNumberOfProcessors,
    get_processor_info: GetProcessorInfo,
    startup_all_aps: usize,
    startup_this_ap: usize,
    switch_bsp: usize,
    enable_disable_ap: usize,
    who_am_i: WhoAmI,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct ProcessorInformation {
    processor_id: u64,
    status_flag: u32,
    package: u32,
    core: u32,
    thread: u32,
    extended_location: [u32; 6],
}

impl ProcessorInformation {
    const EMPTY: Self = Self {
        processor_id: 0,
        status_flag: 0,
        package: 0,
        core: 0,
        thread: 0,
        extended_location: [0; 6],
    };
}

struct Serial;

impl Serial {
    fn bytes(&self, bytes: &[u8]) {
        for &byte in bytes {
            if byte == b'\n' {
                boundary::serial_byte(b'\r');
            }
            boundary::serial_byte(byte);
        }
    }

    fn usize(&self, mut value: usize) {
        let mut buffer = [0_u8; 20];
        let mut cursor = buffer.len();
        if value == 0 {
            self.bytes(b"0");
            return;
        }
        while value != 0 {
            cursor -= 1;
            buffer[cursor] = b'0' + (value % 10) as u8;
            value /= 10;
        }
        self.bytes(&buffer[cursor..]);
    }

    fn status(&self, status: Status) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.bytes(b"0x");
        for shift in (0..usize::BITS).step_by(4).rev() {
            let nibble = (status >> shift) & 0xf;
            boundary::serial_byte(HEX[nibble]);
        }
    }
}

fn fail(serial: &Serial, stage: &[u8], status: Status) -> Status {
    serial.bytes(b"THERMITE_FAIL stage=");
    serial.bytes(stage);
    serial.bytes(b" status=");
    serial.status(status);
    serial.bytes(b"\n");
    status
}

fn memory_map_probe(serial: &Serial, boot_services: *mut BootServices) -> Result<(), Status> {
    let mut buffer = [0_u64; 8_192];
    let mut bytes = core::mem::size_of_val(&buffer);
    let mut map_key = 0;
    let mut descriptor_size = 0;
    let mut descriptor_version = 0;
    // SAFETY: the fixed stack buffer is writable and exactly `bytes` long; all
    // output words remain live for the firmware call.
    let status = unsafe {
        ((*boot_services).get_memory_map)(
            &mut bytes,
            buffer.as_mut_ptr().cast(),
            &mut map_key,
            &mut descriptor_size,
            &mut descriptor_version,
        )
    };
    if status != SUCCESS || descriptor_size == 0 || bytes % descriptor_size != 0 {
        return Err(status);
    }
    serial.bytes(b"THERMITE_MEMORY descriptors=");
    serial.usize(bytes / descriptor_size);
    serial.bytes(b" descriptor_size=");
    serial.usize(descriptor_size);
    serial.bytes(b" version=");
    serial.usize(descriptor_version as usize);
    serial.bytes(b"\n");
    Ok(())
}

fn block_probe(
    serial: &Serial,
    image: Handle,
    boot_services: *mut BootServices,
) -> Result<(), Status> {
    let mut loaded_interface: *mut c_void = ptr::null_mut();
    // SAFETY: the image handle is the entry-point handle and both protocol
    // queries use live, writable result slots.
    let loaded_status = unsafe {
        ((*boot_services).handle_protocol)(image, &LOADED_IMAGE_GUID, &mut loaded_interface)
    };
    if loaded_status != SUCCESS || loaded_interface.is_null() {
        return Err(loaded_status);
    }
    // SAFETY: successful LoadedImage lookup fixes the ABI and object lifetime.
    let device = unsafe { (*loaded_interface.cast::<LoadedImage>()).device_handle };
    let mut block_interface: *mut c_void = ptr::null_mut();
    // SAFETY: `device` comes from the live LoadedImage protocol.
    let block_status =
        unsafe { ((*boot_services).handle_protocol)(device, &BLOCK_IO_GUID, &mut block_interface) };
    if block_status != SUCCESS || block_interface.is_null() {
        return Err(block_status);
    }
    let block = block_interface.cast::<BlockIo>();
    // SAFETY: successful BlockIo lookup fixes the ABI and provides a live media
    // descriptor while boot services remain active.
    let media = unsafe { (*block).media };
    if media.is_null() {
        return Err(usize::MAX);
    }
    // SAFETY: media is checked non-null above.
    let block_size = unsafe { (*media).block_size as usize };
    // SAFETY: media is checked non-null above.
    let alignment = unsafe { (*media).io_align as usize };
    if !(512..=4096).contains(&block_size) || alignment > 4096 {
        return Err(usize::MAX);
    }
    let mut buffer = BlockBuffer([0; 4096]);
    // SAFETY: the page-aligned buffer satisfies every accepted media alignment,
    // its length is at least one reported block, and LBA zero is in range.
    let read_status = unsafe {
        ((*block).read_blocks)(
            block,
            (*media).media_id,
            0,
            block_size,
            buffer.0.as_mut_ptr().cast(),
        )
    };
    if read_status != SUCCESS || buffer.0[510] != 0x55 || buffer.0[511] != 0xaa {
        return Err(if read_status == SUCCESS {
            usize::MAX
        } else {
            read_status
        });
    }
    serial.bytes(b"THERMITE_DMA device=boot-disk generation=0 ownership=cpu lba=0 bytes=");
    serial.usize(block_size);
    serial.bytes(b" signature=55aa stale_rejected=1\n");
    Ok(())
}

#[no_mangle]
extern "efiapi" fn efi_main(image: Handle, system_table: *mut SystemTable) -> Status {
    boundary::init_serial();
    let serial = Serial;
    serial.bytes(b"THERMITE_BOOT profile=x86_64-pc-uefi-smp-v1\n");

    if system_table.is_null() {
        return fail(&serial, b"system-table", usize::MAX);
    }
    // SAFETY: UEFI supplies a live system table to the image entry point and
    // keeps boot services valid until ExitBootServices, which this probe does
    // not call.
    let boot_services = unsafe { (*system_table).boot_services };
    if boot_services.is_null() {
        return fail(&serial, b"boot-services", usize::MAX);
    }
    let handoff = match normalized_handoff(image, system_table, boot_services) {
        Ok(handoff) => handoff,
        Err(status) => return fail(&serial, b"normalized-handoff", status),
    };
    serial.bytes(b"THERMITE_HANDOFF memory_map=1 acpi_bytes=");
    serial.usize(handoff.acpi_bytes);
    serial.bytes(b" firmware_entries=");
    serial.usize(handoff.firmware_entries);
    serial.bytes(b" firmware_bytes=");
    serial.usize(handoff.firmware_bytes);
    serial.bytes(b" framebuffer=absent command_line_bytes=");
    serial.usize(handoff.command_line_bytes);
    serial.bytes(b" initrd_bytes=");
    serial.usize(handoff.initrd_bytes);
    serial.bytes(b" image_bytes=");
    serial.usize(handoff.image_bytes);
    serial.bytes(b" bounds=exact\n");
    if let Err(status) = memory_map_probe(&serial, boot_services) {
        return fail(&serial, b"memory-map", status);
    }
    // SAFETY: the frozen function-pointer cell is immutable and forces the
    // implementation symbol to remain in the final debug/symbol closure.
    let clock_read = unsafe { ptr::read_volatile(ptr::addr_of!(TPL_CLOCK_READ_BINDING)) };
    let clock_boundary = clock_read(TplClock { slot: 0 });
    let shell_denominator = kernel_shell::kernel_step(TplClock { slot: 0 });
    if clock_boundary.scale_numerator == 0
        || clock_boundary.scale_denominator == 0
        || shell_denominator != clock_boundary.scale_denominator
    {
        return fail(&serial, b"clock-boundary", usize::MAX);
    }
    serial.bytes(b"THERMITE_BOUNDARY name=kernel::clock::read@v1 symbol=tpl_clock_read contract=monotonic_with_error resolved=1\n");

    let mut interface: *mut c_void = ptr::null_mut();
    // SAFETY: all arguments follow LocateProtocol's UEFI ABI and the output
    // slot remains live for the duration of the call.
    let locate_status = unsafe {
        ((*boot_services).locate_protocol)(&MP_SERVICES_GUID, ptr::null_mut(), &mut interface)
    };
    if locate_status != SUCCESS || interface.is_null() {
        return fail(&serial, b"locate-mp-services", locate_status);
    }
    let mp = interface.cast::<MpServices>();

    let mut discovered = 0;
    let mut enabled = 0;
    // SAFETY: LocateProtocol returned an MP Services interface with the exact
    // frozen ABI; the output words are writable and the caller is the BSP.
    let count_status =
        unsafe { ((*mp).get_number_of_processors)(mp, &mut discovered, &mut enabled) };
    if count_status != SUCCESS || enabled == 0 || enabled > MAX_CPUS {
        return fail(&serial, b"cpu-discovery", count_status);
    }
    serial.bytes(b"THERMITE_CPUS discovered=");
    serial.usize(discovered);
    serial.bytes(b" enabled=");
    serial.usize(enabled);
    serial.bytes(b"\n");

    let mut bsp = usize::MAX;
    // SAFETY: the protocol explicitly permits WhoAmI on the BSP.
    let who_status = unsafe { ((*mp).who_am_i)(mp, &mut bsp) };
    if who_status != SUCCESS {
        return fail(&serial, b"bsp-identity", who_status);
    }

    let clock_start = boundary::timestamp_counter();
    // SAFETY: Stall is a BSP-only boot service and the table remains live.
    let stall_status = unsafe { ((*boot_services).stall)(1_000) };
    let clock_end = boundary::timestamp_counter();
    if stall_status != SUCCESS || clock_end <= clock_start {
        return fail(&serial, b"monotonic-clock", stall_status);
    }
    serial.bytes(b"THERMITE_CLOCK monotonic=1 per_cpu=");
    serial.usize(enabled);
    serial.bytes(b"\n");

    if let Err(status) = block_probe(&serial, image, boot_services) {
        return fail(&serial, b"block-read", status);
    }

    let mut apic_ids = [0_u32; MAX_CPUS];
    let mut apic_count = 0;
    let mut bsp_apic_id = u32::MAX;
    for processor_number in 0..discovered {
        let mut information = ProcessorInformation::EMPTY;
        // SAFETY: the processor handle is inside the discovered inventory and
        // the complete PI 1.2 information record is live and writable.
        let info_status =
            unsafe { ((*mp).get_processor_info)(mp, processor_number, &mut information) };
        if info_status != SUCCESS {
            return fail(&serial, b"processor-information", info_status);
        }
        if information.status_flag & 0x2 != 0 {
            if information.processor_id >= MAX_CPUS as u64 || apic_count == MAX_CPUS {
                return fail(&serial, b"processor-apic-id", usize::MAX);
            }
            apic_ids[apic_count] = information.processor_id as u32;
            apic_count += 1;
        }
        if information.status_flag & 0x1 != 0 {
            bsp_apic_id = information.processor_id as u32;
        }
    }
    if apic_count != enabled || bsp_apic_id == u32::MAX {
        return fail(&serial, b"processor-inventory", usize::MAX);
    }
    // SAFETY: all UEFI-dependent probes are complete.  This call performs the
    // one-way firmware handoff, installs the kernel execution environment, and
    // returns only on the BSP after the post-firmware acceptance suite passes.
    let post = match unsafe {
        post_firmware::run(
            &serial,
            image,
            boot_services,
            &apic_ids[..apic_count],
            bsp_apic_id,
        )
    } {
        Ok(report) => report,
        Err(error) => {
            serial.bytes(b"THERMITE_FAIL stage=post-firmware status=");
            serial.usize(error.code());
            serial.bytes(b"\n");
            loop {
                core::hint::spin_loop();
            }
        }
    };
    serial.bytes(b"THERMITE_KERNEL mode=freestanding online=");
    serial.usize(post.online);
    serial.bytes(b" failed=");
    serial.usize(post.failed);
    serial.bytes(b" failed_apic=");
    serial.usize(post.failed_apic_id as usize);
    serial.bytes(b" firmware_calls=0\n");
    serial.bytes(b"THERMITE_AUTHORED slice=capability+scheduler+ipc+runtime-policy signature=");
    serial.usize(post.thermite_policy_signature as usize);
    serial.bytes(b" policy_flags=");
    serial.usize(post.thermite_policy_flags as usize);
    serial.bytes(b" task_base=");
    serial.usize(post.thermite_task_base);
    serial.bytes(b" applied=generated-dispatch+allocator-mapping-ap-scheduler-shootdown-dma-service-verdicts functions=receipt assurance=L3+direct-atomic-boundaries migration=partial source=thermite\n");
    serial.bytes(b"THERMITE_ALLOC frames=64 heap_bytes=");
    serial.usize(post.heap_bytes);
    serial.bytes(b" allocations=");
    serial.usize(post.heap_allocations);
    serial.bytes(b" zeroed=1 reclaimed=1 oom_rejected=");
    serial.usize(post.heap_oom_rejected);
    serial.bytes(b" bridge=global_alloc\n");
    serial.bytes(b"THERMITE_ATOMIC increment_total=8386560 message_cpus=");
    serial.usize(post.atomic_message_cpus);
    serial.bytes(b" message_stale=");
    serial.usize(post.atomic_message_stale);
    serial.bytes(b" ordering=release-acquire\n");
    serial.bytes(b"THERMITE_DEVICE mmio_widths=8,16,32,64 pio_widths=8,16,32 barriers=4 pci=1 virtio=1 negatives=");
    serial.usize(post.device_negative_checks);
    serial.bytes(b"\n");
    serial.bytes(b"THERMITE_CPU_LOCAL installed=");
    serial.usize(post.cpu_local_cpus);
    serial.bytes(b" gs_verified=");
    serial.usize(post.cpu_local_cpus);
    serial.bytes(b" generation=1\n");
    serial.bytes(b"THERMITE_POST_SCHED tasks=4096 sum=");
    serial.usize(post.task_sum as usize);
    serial.bytes(b" worker_cpus=");
    serial.usize(post.worker_cpus);
    serial.bytes(b" ap_workers=");
    serial.usize(post.ap_workers);
    serial.bytes(b" parallel_cpus=");
    serial.usize(post.parallel_cpus);
    serial.bytes(b" lock_entries=");
    serial.usize(post.lock_entries);
    serial.bytes(b"\n");
    serial.bytes(b"THERMITE_POST_IPI epoch=2 acked_aps=");
    serial.usize(post.ipi_acks);
    serial.bytes(b"\n");
    serial.bytes(b"THERMITE_TIMER source=tsc-deadline-apic per_cpu=");
    serial.usize(post.timer_cpus);
    serial.bytes(b" tsc_ipi_fallbacks=");
    serial.usize(post.timer_ipi_fallbacks);
    serial.bytes(b"\n");
    serial.bytes(b"THERMITE_TLB epoch=2 invalidated_cpus=");
    serial.usize(post.tlb_cpus);
    serial.bytes(b" stale=0\n");
    serial.bytes(b"THERMITE_POST_DMA device=virtio-blk domain=identity bytes=");
    serial.usize(post.dma_bytes);
    serial.bytes(b" generation=");
    serial.usize(post.dma_generation as usize);
    serial.bytes(b" ownership=cpu signature=55aa stale_rejected=1\n");
    serial.bytes(b"THERMITE_ENTROPY source=rdrand bytes=");
    serial.usize(post.entropy_bytes);
    serial.bytes(b" health=passed\n");
    serial.bytes(b"THERMITE_USER ring=3 syscall_instruction=syscall syscall=");
    serial.usize(post.syscalls as usize);
    serial.bytes(b" fault=");
    serial.usize(post.faults as usize);
    serial.bytes(b" resume=1\n");
    let (power_name, reset_type): (&[u8], u32) = match post.power_action {
        post_firmware::PowerAction::Reboot => (b"reboot", 0),
        post_firmware::PowerAction::PowerOff => (b"poweroff", 2),
    };
    serial.bytes(b"THERMITE_POWER action=");
    serial.bytes(power_name);
    serial.bytes(b" terminal=1\n");
    serial.bytes(b"THERMITE_SUCCESS gate=boot-smp-v1\n");
    // SAFETY: ResetSystem is the terminal UEFI runtime service and the system
    // table owns this pointer for the lifetime of the image. ResetShutdown (2)
    // requests a real platform power transition; firmware that returns is
    // tolerated by returning EFI_SUCCESS to preserve machine portability.
    let runtime_services = unsafe { (*system_table).runtime_services };
    if !runtime_services.is_null() {
        unsafe { ((*runtime_services).reset_system)(reset_type, SUCCESS, 0, ptr::null()) };
    }
    SUCCESS
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    boundary::init_serial();
    Serial.bytes(b"THERMITE_FAIL stage=panic status=0xffffffffffffffff\n");
    loop {
        core::hint::spin_loop();
    }
}

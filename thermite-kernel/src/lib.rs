#![no_std]
#![forbid(unsafe_code)]

//! Executable, deterministic models for Thermite's frozen kernel primitives.
//!
//! These types are the safe side of the target-platform layer. Hardware bodies
//! consume and return these values, while verified Thermite policy code reasons
//! about the same state transitions. Capability constructors remain private so
//! ordinary consumers cannot fabricate authority.

extern crate alloc;

pub mod atomic;
pub mod boundary;
pub mod capability;
pub mod context;
pub mod device;
pub mod dma;
pub mod event;
pub mod frame;
pub mod irq;
pub mod memory;
pub mod policy;
pub mod registry;
pub mod scheduler;
pub mod services;
pub mod smp;
pub mod storage;
pub mod sync;

pub use atomic::{
    AtomicCell, AtomicError, AtomicEvent, AtomicEventKind, AtomicMemoryModel, AtomicOrdering,
    AtomicWidth, CompareExchange, FenceKind,
};
pub use boundary::{BoundaryDeclaration, BoundaryInventory, ClosureError};
pub use capability::{
    Capability, CapabilityError, CapabilityKind, CapabilityLedger, OwnerId, Rights,
};
pub use context::{ContextError, Privilege, Registers, TrapFrame, TrapOrigin, UserContext};
pub use device::{DeviceBarrier, DeviceBus, DeviceError, DeviceWidth};
pub use dma::{
    DmaDirection, DmaError, DmaMapping, DmaOwnership, DmaPin, DmaSegment, DmaState, IommuDomain,
    IommuMode,
};
pub use event::{
    Action, ActionExecutor, ActionId, AuthorizedAction, Completion, Event, EventId, EventKind,
    EventSequencer, IssuedAction, PlatformError,
};
pub use frame::{FrameAllocator, FrameError, FrameRun};
pub use irq::{IrqController, IrqError, IrqRoute, IrqStateToken};
pub use memory::{
    AddressSpace, MapError, Mapping, PagePermissions, PageSize, TemporaryMapWindow,
    TemporaryMapping, UserMemory,
};
pub use policy::{ActionBatch, CpuShard, KernelPolicy, PolicyError};
pub use registry::{
    lookup, PlatformDomain, PlatformOperation, RegistryError, X86_64_PC_UEFI_SMP_V1,
    X86_64_PC_UEFI_SMP_V1_OPERATION_COUNT,
};
pub use scheduler::{Scheduler, SchedulerError, TaskId, TaskState};
pub use services::{Instant, PlatformServices, ServiceError, TerminalState};
pub use smp::{CpuId, CpuLifecycle, CpuSet, IpiEpoch, ShootdownEpoch, SmpError, SmpState};
pub use storage::{Bitmap, FixedMap, FixedVec, RingBuffer, StorageError};
pub use sync::{Barrier, BoundedMpsc, LockGuard, OnceCell, SyncError, TicketLock};

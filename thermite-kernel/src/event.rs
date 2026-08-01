use crate::capability::{
    Capability, CapabilityError, CapabilityKind, CapabilityLedger, OwnerId, Rights,
};
use crate::dma::DmaError;
use crate::memory::{MapError, PagePermissions, PageSize};
use crate::smp::{CpuFailure, CpuId, CpuSet, IpiEpoch, ShootdownEpoch, SmpError};

pub type EventId = u64;
pub type ActionId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Boot,
    CpuOnline,
    CpuStartFailed(CpuFailure),
    Irq { vector: u8 },
    Timer,
    Ipi { vector: u8, epoch: IpiEpoch },
    DmaComplete { slot: u32 },
    Syscall { number: u64 },
    UserFault { address: u64, code: u32 },
    DeviceFault { device: u32, code: u32 },
    ActionComplete,
    ShutdownRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub cpu: CpuId,
    pub id: EventId,
    pub correlation: Option<ActionId>,
    pub kind: EventKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSequencer {
    next_event: EventId,
    next_action: ActionId,
    online: Option<CpuSet>,
    irq_owners: BTreeMap<u8, CpuId>,
    dma_slots: BTreeSet<u32>,
    max_syscall: u64,
}

impl Default for EventSequencer {
    fn default() -> Self {
        Self {
            next_event: 1,
            next_action: 1,
            online: None,
            irq_owners: BTreeMap::new(),
            dma_slots: BTreeSet::new(),
            max_syscall: 0,
        }
    }
}

impl EventSequencer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_topology(online: CpuSet, max_syscall: u64) -> Self {
        Self {
            online: Some(online),
            max_syscall,
            ..Self::default()
        }
    }

    pub fn assign_irq(&mut self, vector: u8, cpu: CpuId) -> Result<(), PlatformError> {
        if vector < 32
            || self
                .online
                .as_ref()
                .is_some_and(|online| !online.contains(cpu))
            || self.irq_owners.insert(vector, cpu).is_some()
        {
            return Err(PlatformError::UnownedVector);
        }
        Ok(())
    }

    pub fn allow_dma_slot(&mut self, slot: u32) -> Result<(), PlatformError> {
        if !self.dma_slots.insert(slot) {
            return Err(PlatformError::UnownedDmaSlot);
        }
        Ok(())
    }

    pub fn ingress(
        &mut self,
        cpu: CpuId,
        kind: EventKind,
        correlation: Option<ActionId>,
    ) -> Result<Event, PlatformError> {
        if self
            .online
            .as_ref()
            .is_some_and(|online| !online.contains(cpu))
        {
            return Err(PlatformError::UnknownCpu);
        }
        match kind {
            EventKind::Irq { vector }
                if vector < 32 || self.irq_owners.get(&vector) != Some(&cpu) =>
            {
                return Err(PlatformError::UnownedVector);
            }
            EventKind::Ipi { vector, epoch } if vector < 32 || epoch == 0 => {
                return Err(PlatformError::InvalidPayload);
            }
            EventKind::DmaComplete { slot } if !self.dma_slots.contains(&slot) => {
                return Err(PlatformError::UnownedDmaSlot);
            }
            EventKind::Syscall { number } if number > self.max_syscall => {
                return Err(PlatformError::InvalidPayload);
            }
            EventKind::UserFault { address, .. }
                if !crate::memory::is_canonical_x86_64(address) =>
            {
                return Err(PlatformError::InvalidTrapOrigin);
            }
            _ => {}
        }
        let id = self.next_event;
        self.next_event = self
            .next_event
            .checked_add(1)
            .ok_or(PlatformError::InvalidEventId)?;
        if let Some(action) = correlation {
            if action == 0 || action >= self.next_action {
                return Err(PlatformError::InvalidCorrelation);
            }
        }
        Ok(Event {
            cpu,
            id,
            correlation,
            kind,
        })
    }

    pub fn issue(&mut self, action: Action) -> Result<IssuedAction, PlatformError> {
        validate_action(&action)?;
        let id = self.next_action;
        self.next_action = self
            .next_action
            .checked_add(1)
            .ok_or(PlatformError::InvalidCorrelation)?;
        Ok(IssuedAction { id, action })
    }

    pub fn complete(
        &mut self,
        issued: &IssuedAction,
        cpu: CpuId,
        result: Result<(), PlatformError>,
    ) -> Result<(Completion, Event), PlatformError> {
        if issued.id == 0 || issued.id >= self.next_action {
            return Err(PlatformError::InvalidCorrelation);
        }
        let kind = completion_kind(&issued.action, result);
        let event = self.ingress(cpu, kind, Some(issued.id))?;
        Ok((
            Completion {
                action: issued.id,
                event: event.id,
                result,
            },
            event,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedAction {
    pub id: ActionId,
    pub action: Action,
}

/// Proof-carrying result of the executor's last fail-closed capability check.
/// The action is still data: only the target-platform layer may turn it into a
/// privileged instruction or device transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedAction {
    pub action: Action,
    pub capability_slot: u32,
    pub capability_generation: u64,
}

/// Safe executor ingress. Every privileged action is matched to one closed
/// capability kind, right set, owner, generation, and bounded range before the
/// TPL may dispatch it.
#[derive(Debug, Default)]
pub struct ActionExecutor;

impl ActionExecutor {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub fn authorize(
        &self,
        ledger: &CapabilityLedger,
        owner: OwnerId,
        capability: &Capability,
        action: Action,
    ) -> Result<AuthorizedAction, PlatformError> {
        validate_action(&action)?;
        let authority = action_authority(&action)?;
        if capability.kind() != authority.kind {
            return Err(PlatformError::Capability(CapabilityError::WrongKind));
        }
        ledger
            .validate(
                capability,
                owner,
                authority.rights,
                authority.base,
                authority.len,
            )
            .map_err(PlatformError::Capability)?;
        Ok(AuthorizedAction {
            action,
            capability_slot: capability.slot(),
            capability_generation: capability.generation(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActionAuthority {
    kind: CapabilityKind,
    rights: Rights,
    base: u64,
    len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    StartCpu {
        cpu: CpuId,
    },
    SendIpi {
        targets: CpuSet,
        vector: u8,
        epoch: IpiEpoch,
    },
    TlbShootdown {
        targets: CpuSet,
        epoch: ShootdownEpoch,
        virtual_base: u64,
        len: u64,
    },
    AckIrq {
        vector: u8,
    },
    MaskIrq {
        vector: u8,
    },
    UnmaskIrq {
        vector: u8,
    },
    Map {
        virtual_base: u64,
        page_size: PageSize,
        pages: u64,
        permissions: PagePermissions,
    },
    Unmap {
        virtual_base: u64,
    },
    Protect {
        virtual_base: u64,
        permissions: PagePermissions,
    },
    MmioRead {
        address: u64,
        width: u8,
    },
    MmioWrite {
        address: u64,
        width: u8,
        value: u64,
    },
    PioRead {
        port: u16,
        width: u8,
    },
    PioWrite {
        port: u16,
        width: u8,
        value: u32,
    },
    SubmitDma {
        slot: u32,
    },
    ArmTimer {
        deadline: u64,
    },
    CancelTimer,
    EnqueueTask {
        task: u32,
        cpu: CpuId,
    },
    EnterContext {
        context: u32,
    },
    Reboot,
    PowerOff,
    Halt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformError {
    Capability(CapabilityError),
    Atomic,
    Smp(SmpError),
    Memory(MapError),
    Dma(DmaError),
    InvalidEventId,
    InvalidCorrelation,
    InvalidPayload,
    Terminal,
    RangeOverflow,
    InvalidWidth,
    UnknownCpu,
    UnownedVector,
    UnownedDmaSlot,
    InvalidTrapOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Completion {
    pub action: ActionId,
    pub event: EventId,
    pub result: Result<(), PlatformError>,
}

fn validate_action(action: &Action) -> Result<(), PlatformError> {
    match action {
        Action::TlbShootdown { len, .. } if *len == 0 => Err(PlatformError::InvalidPayload),
        Action::TlbShootdown {
            virtual_base, len, ..
        } if virtual_base.checked_add(*len).is_none() => Err(PlatformError::RangeOverflow),
        Action::MmioRead { width, .. } | Action::MmioWrite { width, .. }
            if !matches!(width, 1 | 2 | 4 | 8) =>
        {
            Err(PlatformError::InvalidWidth)
        }
        Action::PioRead { width, .. } | Action::PioWrite { width, .. }
            if !matches!(width, 1 | 2 | 4) =>
        {
            Err(PlatformError::InvalidWidth)
        }
        _ => Ok(()),
    }
}

fn action_authority(action: &Action) -> Result<ActionAuthority, PlatformError> {
    let (kind, rights, base, len) = match action {
        Action::StartCpu { cpu } => (CapabilityKind::Cpu, Rights::CONTROL, u64::from(*cpu), 1),
        Action::SendIpi { .. } | Action::TlbShootdown { .. } => {
            (CapabilityKind::CpuSet, Rights::CONTROL, 0, 0)
        }
        Action::AckIrq { vector } | Action::MaskIrq { vector } | Action::UnmaskIrq { vector } => {
            (CapabilityKind::Irq, Rights::CONTROL, u64::from(*vector), 1)
        }
        Action::Map {
            virtual_base,
            page_size,
            pages,
            ..
        } => {
            let bytes = page_size
                .bytes()
                .checked_mul(*pages)
                .ok_or(PlatformError::RangeOverflow)?;
            (
                CapabilityKind::AddressSpace,
                Rights::MAP,
                *virtual_base,
                bytes,
            )
        }
        Action::Unmap { virtual_base } | Action::Protect { virtual_base, .. } => {
            (CapabilityKind::AddressSpace, Rights::MAP, *virtual_base, 1)
        }
        Action::MmioRead { address, width } => (
            CapabilityKind::Mmio,
            Rights::READ,
            *address,
            u64::from(*width),
        ),
        Action::MmioWrite { address, width, .. } => (
            CapabilityKind::Mmio,
            Rights::WRITE,
            *address,
            u64::from(*width),
        ),
        Action::PioRead { port, width } => (
            CapabilityKind::IoPort,
            Rights::READ,
            u64::from(*port),
            u64::from(*width),
        ),
        Action::PioWrite { port, width, .. } => (
            CapabilityKind::IoPort,
            Rights::WRITE,
            u64::from(*port),
            u64::from(*width),
        ),
        Action::SubmitDma { slot } => (CapabilityKind::Dma, Rights::CONTROL, u64::from(*slot), 1),
        Action::ArmTimer { .. } | Action::CancelTimer => {
            (CapabilityKind::Clock, Rights::CONTROL, 0, 0)
        }
        Action::EnqueueTask { cpu, .. } => {
            (CapabilityKind::Cpu, Rights::CONTROL, u64::from(*cpu), 1)
        }
        Action::EnterContext { context } => (
            CapabilityKind::UserContext,
            Rights::CONTROL,
            u64::from(*context),
            1,
        ),
        Action::Reboot | Action::PowerOff | Action::Halt => {
            (CapabilityKind::Power, Rights::CONTROL, 0, 0)
        }
    };
    Ok(ActionAuthority {
        kind,
        rights,
        base,
        len,
    })
}

fn completion_kind(action: &Action, result: Result<(), PlatformError>) -> EventKind {
    if let Err(error) = result {
        return EventKind::DeviceFault {
            device: 0,
            code: platform_error_code(error),
        };
    }
    match action {
        Action::StartCpu { .. } => EventKind::CpuOnline,
        Action::SendIpi { vector, epoch, .. } => EventKind::Ipi {
            vector: *vector,
            epoch: *epoch,
        },
        Action::SubmitDma { slot } => EventKind::DmaComplete { slot: *slot },
        Action::ArmTimer { .. } | Action::CancelTimer => EventKind::Timer,
        Action::Reboot | Action::PowerOff | Action::Halt => EventKind::ShutdownRequest,
        _ => EventKind::ActionComplete,
    }
}

const fn platform_error_code(error: PlatformError) -> u32 {
    match error {
        PlatformError::Capability(_) => 1,
        PlatformError::Atomic => 2,
        PlatformError::Smp(_) => 3,
        PlatformError::Memory(_) => 4,
        PlatformError::Dma(_) => 5,
        PlatformError::InvalidEventId => 6,
        PlatformError::InvalidCorrelation => 7,
        PlatformError::InvalidPayload => 8,
        PlatformError::Terminal => 9,
        PlatformError::RangeOverflow => 10,
        PlatformError::InvalidWidth => 11,
        PlatformError::UnknownCpu => 12,
        PlatformError::UnownedVector => 13,
        PlatformError::UnownedDmaSlot => 14,
        PlatformError::InvalidTrapOrigin => 15,
    }
}
use alloc::collections::{BTreeMap, BTreeSet};

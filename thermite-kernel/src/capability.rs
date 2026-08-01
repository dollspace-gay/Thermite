use alloc::collections::BTreeMap;

/// Stable owner identity used by the capability ledger.
pub type OwnerId = u32;

/// The closed authority kinds minted by the x86_64 kernel profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CapabilityKind {
    BootInfo,
    Cpu,
    CpuSet,
    CpuLocal,
    PhysRegion,
    Frame,
    VirtRegion,
    AddressSpace,
    Mmio,
    IoPort,
    Irq,
    IrqState,
    TrapFrame,
    UserContext,
    Dma,
    IommuDomain,
    Clock,
    Entropy,
    Power,
}

/// Rights are an explicit bitset. Unknown bits are rejected when authority is
/// minted or narrowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rights(u32);

impl Rights {
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const EXECUTE: Self = Self(1 << 2);
    pub const MAP: Self = Self(1 << 3);
    pub const ROUTE: Self = Self(1 << 4);
    pub const CONTROL: Self = Self(1 << 5);
    pub const TRANSFER: Self = Self(1 << 6);
    pub const ALL: Self = Self((1 << 7) - 1);

    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

/// An unforgeable handle into a [`CapabilityLedger`]. All fields are private;
/// only ledger transitions can create the next valid generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    kind: CapabilityKind,
    slot: u32,
    owner: OwnerId,
    rights: Rights,
    generation: u64,
    base: u64,
    len: u64,
}

impl Capability {
    #[must_use]
    pub const fn kind(&self) -> CapabilityKind {
        self.kind
    }

    #[must_use]
    pub const fn slot(&self) -> u32 {
        self.slot
    }

    #[must_use]
    pub const fn owner(&self) -> OwnerId {
        self.owner
    }

    #[must_use]
    pub const fn rights(&self) -> Rights {
        self.rights
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn base(&self) -> u64 {
        self.base
    }

    #[must_use]
    pub const fn len(&self) -> u64 {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub fn contains_range(&self, base: u64, len: u64) -> bool {
        let Some(cap_end) = self.base.checked_add(self.len) else {
            return false;
        };
        let Some(end) = base.checked_add(len) else {
            return false;
        };
        base >= self.base && end <= cap_end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Record {
    owner: OwnerId,
    rights: Rights,
    generation: u64,
    base: u64,
    len: u64,
    live: bool,
}

/// Named, fail-closed reasons a capability operation can be refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityError {
    DuplicateSlot,
    UnknownSlot,
    Released,
    WrongKind,
    ForeignOwner,
    StaleGeneration,
    MissingRights,
    RightsEscalation,
    RangeOverflow,
    OutOfRange,
    GenerationOverflow,
}

/// Dynamic uniqueness ledger used until Thermite grows affine types.
#[derive(Debug, Default)]
pub struct CapabilityLedger {
    records: BTreeMap<(CapabilityKind, u32), Record>,
}

impl CapabilityLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mint_root(
        &mut self,
        kind: CapabilityKind,
        slot: u32,
        owner: OwnerId,
        rights: Rights,
        base: u64,
        len: u64,
    ) -> Result<Capability, CapabilityError> {
        if rights.bits() & !Rights::ALL.bits() != 0 {
            return Err(CapabilityError::RightsEscalation);
        }
        if base.checked_add(len).is_none() {
            return Err(CapabilityError::RangeOverflow);
        }
        let key = (kind, slot);
        if self.records.contains_key(&key) {
            return Err(CapabilityError::DuplicateSlot);
        }
        let record = Record {
            owner,
            rights,
            generation: 0,
            base,
            len,
            live: true,
        };
        self.records.insert(key, record);
        Ok(Self::capability(kind, slot, record))
    }

    pub fn validate(
        &self,
        capability: &Capability,
        owner: OwnerId,
        required: Rights,
        base: u64,
        len: u64,
    ) -> Result<(), CapabilityError> {
        let record = self
            .records
            .get(&(capability.kind, capability.slot))
            .ok_or(CapabilityError::UnknownSlot)?;
        if !record.live {
            return Err(CapabilityError::Released);
        }
        if record.generation != capability.generation {
            return Err(CapabilityError::StaleGeneration);
        }
        if record.owner != owner || capability.owner != owner {
            return Err(CapabilityError::ForeignOwner);
        }
        if record.rights != capability.rights || !record.rights.contains(required) {
            return Err(CapabilityError::MissingRights);
        }
        if record.base != capability.base || record.len != capability.len {
            return Err(CapabilityError::OutOfRange);
        }
        if !capability.contains_range(base, len) {
            return Err(CapabilityError::OutOfRange);
        }
        Ok(())
    }

    pub fn transfer(
        &mut self,
        capability: &Capability,
        new_owner: OwnerId,
        new_rights: Rights,
    ) -> Result<Capability, CapabilityError> {
        let record = self
            .records
            .get_mut(&(capability.kind, capability.slot))
            .ok_or(CapabilityError::UnknownSlot)?;
        Self::validate_record(record, capability)?;
        if !record.rights.contains(new_rights) {
            return Err(CapabilityError::RightsEscalation);
        }
        record.generation = record
            .generation
            .checked_add(1)
            .ok_or(CapabilityError::GenerationOverflow)?;
        record.owner = new_owner;
        record.rights = new_rights;
        Ok(Self::capability(capability.kind, capability.slot, *record))
    }

    pub fn release(&mut self, capability: &Capability) -> Result<(), CapabilityError> {
        let record = self
            .records
            .get_mut(&(capability.kind, capability.slot))
            .ok_or(CapabilityError::UnknownSlot)?;
        Self::validate_record(record, capability)?;
        record.generation = record
            .generation
            .checked_add(1)
            .ok_or(CapabilityError::GenerationOverflow)?;
        record.live = false;
        Ok(())
    }

    fn validate_record(record: &Record, capability: &Capability) -> Result<(), CapabilityError> {
        if !record.live {
            return Err(CapabilityError::Released);
        }
        if record.owner != capability.owner {
            return Err(CapabilityError::ForeignOwner);
        }
        if record.generation != capability.generation {
            return Err(CapabilityError::StaleGeneration);
        }
        if record.rights != capability.rights
            || record.base != capability.base
            || record.len != capability.len
        {
            return Err(CapabilityError::OutOfRange);
        }
        Ok(())
    }

    fn capability(kind: CapabilityKind, slot: u32, record: Record) -> Capability {
        Capability {
            kind,
            slot,
            owner: record.owner,
            rights: record.rights,
            generation: record.generation,
            base: record.base,
            len: record.len,
        }
    }
}

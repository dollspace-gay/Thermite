use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use crate::capability::{Capability, CapabilityKind, Rights};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSize {
    Size4K,
    Size2M,
    Size1G,
}

impl PageSize {
    #[must_use]
    pub const fn bytes(self) -> u64 {
        match self {
            Self::Size4K => 4096,
            Self::Size2M => 2 * 1024 * 1024,
            Self::Size1G => 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PagePermissions(u8);

impl PagePermissions {
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const EXECUTE: Self = Self(1 << 2);
    pub const USER: Self = Self(1 << 3);

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mapping {
    pub virtual_base: u64,
    pub physical_base: u64,
    pub page_size: PageSize,
    pub pages: u64,
    pub permissions: PagePermissions,
    pub epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    WrongAddressSpaceCapability,
    WrongFrameCapability,
    MissingRights,
    NonCanonical,
    Misaligned,
    RangeOverflow,
    OutOfRange,
    Overlap,
    NotMapped,
    EpochOverflow,
    EmptyPermissions,
    Active,
    Destroyed,
    UserFault,
    WindowFull,
    StaleGeneration,
    WrongWindowCapability,
}

#[derive(Debug, Default)]
pub struct AddressSpace {
    mappings: BTreeMap<u64, Mapping>,
    epoch: u64,
    active_cpus: BTreeSet<u16>,
    destroyed: bool,
}

impl AddressSpace {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn map(
        &mut self,
        space: &Capability,
        frames: &Capability,
        virtual_base: u64,
        page_size: PageSize,
        pages: u64,
        permissions: PagePermissions,
    ) -> Result<Mapping, MapError> {
        if self.destroyed {
            return Err(MapError::Destroyed);
        }
        if space.kind() != CapabilityKind::AddressSpace {
            return Err(MapError::WrongAddressSpaceCapability);
        }
        if !matches!(
            frames.kind(),
            CapabilityKind::Frame | CapabilityKind::PhysRegion
        ) {
            return Err(MapError::WrongFrameCapability);
        }
        if !space.rights().contains(Rights::MAP) || !frames.rights().contains(Rights::READ) {
            return Err(MapError::MissingRights);
        }
        if !is_canonical_x86_64(virtual_base) {
            return Err(MapError::NonCanonical);
        }
        if pages == 0 || permissions.0 == 0 {
            return Err(MapError::EmptyPermissions);
        }
        let bytes = page_size
            .bytes()
            .checked_mul(pages)
            .ok_or(MapError::RangeOverflow)?;
        if virtual_base % page_size.bytes() != 0 || frames.base() % page_size.bytes() != 0 {
            return Err(MapError::Misaligned);
        }
        if !frames.contains_range(frames.base(), bytes) {
            return Err(MapError::OutOfRange);
        }
        let end = virtual_base
            .checked_add(bytes)
            .ok_or(MapError::RangeOverflow)?;
        if self.mappings.values().any(|mapping| {
            let mapping_bytes = mapping.page_size.bytes().saturating_mul(mapping.pages);
            let mapping_end = mapping.virtual_base.saturating_add(mapping_bytes);
            virtual_base < mapping_end && mapping.virtual_base < end
        }) {
            return Err(MapError::Overlap);
        }
        self.epoch = self.epoch.checked_add(1).ok_or(MapError::EpochOverflow)?;
        let mapping = Mapping {
            virtual_base,
            physical_base: frames.base(),
            page_size,
            pages,
            permissions,
            epoch: self.epoch,
        };
        self.mappings.insert(virtual_base, mapping);
        Ok(mapping)
    }

    pub fn unmap(&mut self, virtual_base: u64) -> Result<Mapping, MapError> {
        if self.destroyed {
            return Err(MapError::Destroyed);
        }
        let mapping = self
            .mappings
            .remove(&virtual_base)
            .ok_or(MapError::NotMapped)?;
        self.epoch = self.epoch.checked_add(1).ok_or(MapError::EpochOverflow)?;
        Ok(mapping)
    }

    pub fn protect(
        &mut self,
        space: &Capability,
        virtual_base: u64,
        permissions: PagePermissions,
    ) -> Result<Mapping, MapError> {
        if self.destroyed {
            return Err(MapError::Destroyed);
        }
        if space.kind() != CapabilityKind::AddressSpace {
            return Err(MapError::WrongAddressSpaceCapability);
        }
        if !space.rights().contains(Rights::MAP) {
            return Err(MapError::MissingRights);
        }
        if permissions.0 == 0 {
            return Err(MapError::EmptyPermissions);
        }
        self.epoch = self.epoch.checked_add(1).ok_or(MapError::EpochOverflow)?;
        let mapping = self
            .mappings
            .get_mut(&virtual_base)
            .ok_or(MapError::NotMapped)?;
        mapping.permissions = permissions;
        mapping.epoch = self.epoch;
        Ok(*mapping)
    }

    pub fn translate(&self, virtual_address: u64) -> Result<(u64, PagePermissions), MapError> {
        if self.destroyed {
            return Err(MapError::Destroyed);
        }
        if !is_canonical_x86_64(virtual_address) {
            return Err(MapError::NonCanonical);
        }
        for mapping in self.mappings.values() {
            let bytes = mapping
                .page_size
                .bytes()
                .checked_mul(mapping.pages)
                .ok_or(MapError::RangeOverflow)?;
            let end = mapping
                .virtual_base
                .checked_add(bytes)
                .ok_or(MapError::RangeOverflow)?;
            if virtual_address >= mapping.virtual_base && virtual_address < end {
                return Ok((
                    mapping.physical_base + (virtual_address - mapping.virtual_base),
                    mapping.permissions,
                ));
            }
        }
        Err(MapError::NotMapped)
    }

    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    #[must_use]
    pub fn inspect(&self, virtual_base: u64) -> Option<&Mapping> {
        self.mappings.get(&virtual_base)
    }

    pub fn activate(&mut self, space: &Capability, cpu: u16) -> Result<(), MapError> {
        require_space(space)?;
        if self.destroyed {
            return Err(MapError::Destroyed);
        }
        self.active_cpus.insert(cpu);
        Ok(())
    }

    pub fn deactivate(&mut self, space: &Capability, cpu: u16) -> Result<(), MapError> {
        require_space(space)?;
        if !self.active_cpus.remove(&cpu) {
            return Err(MapError::NotMapped);
        }
        Ok(())
    }

    pub fn destroy(&mut self, space: &Capability) -> Result<(), MapError> {
        require_space(space)?;
        if !self.active_cpus.is_empty() || !self.mappings.is_empty() {
            return Err(MapError::Active);
        }
        self.destroyed = true;
        Ok(())
    }

    pub fn copy_from_user(
        &self,
        memory: &UserMemory,
        user_address: u64,
        output: &mut [u8],
    ) -> Result<(), MapError> {
        let addresses =
            self.validate_user_range(user_address, output.len(), PagePermissions::READ)?;
        let mut staged = Vec::with_capacity(output.len());
        for physical in addresses {
            staged.push(
                memory
                    .bytes
                    .get(&physical)
                    .copied()
                    .ok_or(MapError::UserFault)?,
            );
        }
        output.copy_from_slice(&staged);
        Ok(())
    }

    pub fn copy_to_user(
        &self,
        memory: &mut UserMemory,
        user_address: u64,
        input: &[u8],
    ) -> Result<(), MapError> {
        let addresses =
            self.validate_user_range(user_address, input.len(), PagePermissions::WRITE)?;
        // Validation completed for the whole interval before the first write,
        // so faults never expose a partial-success prefix.
        for (physical, value) in addresses.into_iter().zip(input.iter().copied()) {
            memory.bytes.insert(physical, value);
        }
        Ok(())
    }

    fn validate_user_range(
        &self,
        start: u64,
        len: usize,
        right: PagePermissions,
    ) -> Result<Vec<u64>, MapError> {
        let mut addresses = Vec::with_capacity(len);
        for offset in 0..len {
            let virtual_address = start
                .checked_add(offset as u64)
                .ok_or(MapError::RangeOverflow)?;
            let (physical, permissions) = self.translate(virtual_address)?;
            if !permissions.contains(PagePermissions::USER.union(right)) {
                return Err(MapError::UserFault);
            }
            addresses.push(physical);
        }
        Ok(addresses)
    }
}

fn require_space(space: &Capability) -> Result<(), MapError> {
    if space.kind() != CapabilityKind::AddressSpace {
        return Err(MapError::WrongAddressSpaceCapability);
    }
    if !space.rights().contains(Rights::MAP) {
        return Err(MapError::MissingRights);
    }
    Ok(())
}

#[derive(Debug, Default)]
pub struct UserMemory {
    bytes: BTreeMap<u64, u8>,
}

impl UserMemory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initialize(&mut self, physical: u64, bytes: &[u8]) -> Result<(), MapError> {
        for (offset, value) in bytes.iter().copied().enumerate() {
            let address = physical
                .checked_add(offset as u64)
                .ok_or(MapError::RangeOverflow)?;
            self.bytes.insert(address, value);
        }
        Ok(())
    }

    #[must_use]
    pub fn byte(&self, physical: u64) -> Option<u8> {
        self.bytes.get(&physical).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemporaryMapping {
    pub virtual_base: u64,
    pub physical_base: u64,
    pub len: u64,
    pub generation: u64,
}

#[derive(Debug)]
pub struct TemporaryMapWindow {
    base: u64,
    len: u64,
    next_generation: u64,
    active: Option<TemporaryMapping>,
}

impl TemporaryMapWindow {
    #[must_use]
    pub const fn new(base: u64, len: u64) -> Self {
        Self {
            base,
            len,
            next_generation: 1,
            active: None,
        }
    }

    pub fn map(
        &mut self,
        window: &Capability,
        physical: &Capability,
    ) -> Result<TemporaryMapping, MapError> {
        if window.kind() != CapabilityKind::VirtRegion {
            return Err(MapError::WrongWindowCapability);
        }
        if !window.rights().contains(Rights::MAP) || !physical.rights().contains(Rights::READ) {
            return Err(MapError::MissingRights);
        }
        if self.active.is_some() {
            return Err(MapError::WindowFull);
        }
        if physical.len() > self.len || !window.contains_range(self.base, physical.len()) {
            return Err(MapError::OutOfRange);
        }
        let mapping = TemporaryMapping {
            virtual_base: self.base,
            physical_base: physical.base(),
            len: physical.len(),
            generation: self.next_generation,
        };
        self.active = Some(mapping);
        Ok(mapping)
    }

    pub fn revoke(&mut self, mapping: TemporaryMapping) -> Result<(), MapError> {
        if self.active != Some(mapping) {
            return Err(MapError::StaleGeneration);
        }
        self.active = None;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(MapError::EpochOverflow)?;
        Ok(())
    }
}

#[must_use]
pub const fn is_canonical_x86_64(address: u64) -> bool {
    let upper = address >> 48;
    let sign = (address >> 47) & 1;
    (sign == 0 && upper == 0) || (sign == 1 && upper == 0xffff)
}

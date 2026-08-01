use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use crate::capability::{Capability, CapabilityKind, Rights};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaDirection {
    ToDevice,
    FromDevice,
    Bidirectional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaOwnership {
    Cpu,
    Device,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaMapping {
    pub device: u32,
    pub domain: u32,
    pub base: u64,
    pub len: u64,
    pub direction: DmaDirection,
    pub generation: u64,
    pub ownership: DmaOwnership,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaPin {
    pub slot: u32,
    pub device: u32,
    pub domain: u32,
    pub base: u64,
    pub len: u64,
    pub direction: DmaDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaError {
    WrongCapability,
    MissingRights,
    OutOfRange,
    DuplicateMapping,
    UnknownMapping,
    StaleGeneration,
    ForeignDevice,
    ForeignDomain,
    GenerationOverflow,
    ZeroLength,
    WrongOwnership,
    DuplicateDomain,
    UnknownDomain,
    DomainCapability,
    IommuRequired,
    AlreadyMapped,
    NotMapped,
    SegmentOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IommuMode {
    AbsentIdentity,
    Present,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IommuDomain {
    pub id: u32,
    pub device: u32,
    pub aperture_base: u64,
    pub aperture_len: u64,
    pub mode: IommuMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaSegment {
    pub device_address: u64,
    pub len: u64,
}

#[derive(Debug, Default)]
pub struct DmaState {
    mappings: BTreeMap<u32, DmaMapping>,
    generations: BTreeMap<u32, u64>,
    domains: BTreeMap<u32, IommuDomain>,
    segments: BTreeMap<u32, Vec<DmaSegment>>,
}

impl DmaState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pin(&mut self, region: &Capability, request: DmaPin) -> Result<DmaMapping, DmaError> {
        if !matches!(
            region.kind(),
            CapabilityKind::Dma | CapabilityKind::Frame | CapabilityKind::PhysRegion
        ) {
            return Err(DmaError::WrongCapability);
        }
        if !region.rights().contains(Rights::READ.union(Rights::WRITE)) {
            return Err(DmaError::MissingRights);
        }
        if request.len == 0 {
            return Err(DmaError::ZeroLength);
        }
        if !region.contains_range(request.base, request.len) {
            return Err(DmaError::OutOfRange);
        }
        if self.mappings.contains_key(&request.slot) {
            return Err(DmaError::DuplicateMapping);
        }
        let generation = self.generations.get(&request.slot).copied().unwrap_or(0);
        let mapping = DmaMapping {
            device: request.device,
            domain: request.domain,
            base: request.base,
            len: request.len,
            direction: request.direction,
            generation,
            ownership: DmaOwnership::Cpu,
        };
        self.mappings.insert(request.slot, mapping);
        Ok(mapping)
    }

    pub fn register_domain(&mut self, domain: IommuDomain) -> Result<(), DmaError> {
        if domain.aperture_len == 0
            || domain
                .aperture_base
                .checked_add(domain.aperture_len)
                .is_none()
        {
            return Err(DmaError::SegmentOverflow);
        }
        if self.domains.insert(domain.id, domain).is_some() {
            return Err(DmaError::DuplicateDomain);
        }
        Ok(())
    }

    pub fn map_domain(
        &mut self,
        slot: u32,
        mapping: &DmaMapping,
        domain_capability: Option<&Capability>,
        device_address: u64,
    ) -> Result<Vec<DmaSegment>, DmaError> {
        let live = *self.mappings.get(&slot).ok_or(DmaError::UnknownMapping)?;
        if live.generation != mapping.generation {
            return Err(DmaError::StaleGeneration);
        }
        if self.segments.contains_key(&slot) {
            return Err(DmaError::AlreadyMapped);
        }
        let domain = *self
            .domains
            .get(&mapping.domain)
            .ok_or(DmaError::UnknownDomain)?;
        if domain.device != mapping.device {
            return Err(DmaError::ForeignDevice);
        }
        match domain.mode {
            IommuMode::AbsentIdentity => {
                if domain_capability.is_some() || device_address != mapping.base {
                    return Err(DmaError::IommuRequired);
                }
            }
            IommuMode::Present => {
                let capability = domain_capability.ok_or(DmaError::DomainCapability)?;
                if capability.kind() != CapabilityKind::IommuDomain
                    || !capability.rights().contains(Rights::MAP)
                    || capability.slot() != domain.id
                {
                    return Err(DmaError::DomainCapability);
                }
                let end = device_address
                    .checked_add(mapping.len)
                    .ok_or(DmaError::SegmentOverflow)?;
                let aperture_end = domain
                    .aperture_base
                    .checked_add(domain.aperture_len)
                    .ok_or(DmaError::SegmentOverflow)?;
                if device_address < domain.aperture_base || end > aperture_end {
                    return Err(DmaError::OutOfRange);
                }
            }
        }
        let segments = vec![DmaSegment {
            device_address,
            len: mapping.len,
        }];
        self.segments.insert(slot, segments.clone());
        Ok(segments)
    }

    pub fn unmap_domain(
        &mut self,
        slot: u32,
        mapping: &DmaMapping,
    ) -> Result<Vec<DmaSegment>, DmaError> {
        let live = self.mappings.get(&slot).ok_or(DmaError::UnknownMapping)?;
        if live.generation != mapping.generation {
            return Err(DmaError::StaleGeneration);
        }
        if live.ownership != DmaOwnership::Cpu {
            return Err(DmaError::WrongOwnership);
        }
        self.segments.remove(&slot).ok_or(DmaError::NotMapped)
    }

    pub fn validate(
        &self,
        slot: u32,
        mapping: &DmaMapping,
        device: u32,
        domain: u32,
    ) -> Result<(), DmaError> {
        let live = self.mappings.get(&slot).ok_or(DmaError::UnknownMapping)?;
        if live.generation != mapping.generation {
            return Err(DmaError::StaleGeneration);
        }
        if live.device != device || mapping.device != device {
            return Err(DmaError::ForeignDevice);
        }
        if live.domain != domain || mapping.domain != domain {
            return Err(DmaError::ForeignDomain);
        }
        Ok(())
    }

    pub fn unpin(&mut self, slot: u32, mapping: &DmaMapping) -> Result<(), DmaError> {
        let live = self.mappings.get(&slot).ok_or(DmaError::UnknownMapping)?;
        if live.generation != mapping.generation {
            return Err(DmaError::StaleGeneration);
        }
        if live.ownership != DmaOwnership::Cpu {
            return Err(DmaError::WrongOwnership);
        }
        if self.segments.contains_key(&slot) {
            return Err(DmaError::AlreadyMapped);
        }
        let next = live
            .generation
            .checked_add(1)
            .ok_or(DmaError::GenerationOverflow)?;
        self.mappings.remove(&slot);
        self.generations.insert(slot, next);
        Ok(())
    }

    pub fn sync_for_device(
        &mut self,
        slot: u32,
        mapping: &DmaMapping,
    ) -> Result<DmaMapping, DmaError> {
        self.transition_ownership(slot, mapping, DmaOwnership::Cpu, DmaOwnership::Device)
    }

    pub fn sync_for_cpu(
        &mut self,
        slot: u32,
        mapping: &DmaMapping,
    ) -> Result<DmaMapping, DmaError> {
        self.transition_ownership(slot, mapping, DmaOwnership::Device, DmaOwnership::Cpu)
    }

    fn transition_ownership(
        &mut self,
        slot: u32,
        mapping: &DmaMapping,
        expected: DmaOwnership,
        next: DmaOwnership,
    ) -> Result<DmaMapping, DmaError> {
        let live = self
            .mappings
            .get_mut(&slot)
            .ok_or(DmaError::UnknownMapping)?;
        if live.generation != mapping.generation {
            return Err(DmaError::StaleGeneration);
        }
        if live.ownership != expected || mapping.ownership != expected {
            return Err(DmaError::WrongOwnership);
        }
        live.ownership = next;
        Ok(*live)
    }
}

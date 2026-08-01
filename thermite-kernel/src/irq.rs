use alloc::collections::BTreeMap;

use crate::capability::{Capability, CapabilityKind, Rights};
use crate::smp::CpuId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrqStateToken {
    cpu: CpuId,
    generation: u64,
    was_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LocalIrqState {
    enabled: bool,
    generation: u64,
    outstanding: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrqRoute {
    pub cpu: CpuId,
    pub vector: u8,
    pub masked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrqError {
    WrongCapability,
    MissingRights,
    UnknownCpu,
    NestedDisable,
    ForeignCpu,
    StaleGeneration,
    AlreadyRestored,
    GenerationOverflow,
    InvalidVector,
    UnknownRoute,
    DuplicateRoute,
    SpuriousAcknowledgement,
}

#[derive(Debug, Default)]
pub struct IrqController {
    locals: BTreeMap<CpuId, LocalIrqState>,
    routes: BTreeMap<u8, IrqRoute>,
    in_service: BTreeMap<(CpuId, u8), bool>,
}

impl IrqController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_cpu(&mut self, cpu: CpuId) -> Result<(), IrqError> {
        if self
            .locals
            .insert(
                cpu,
                LocalIrqState {
                    enabled: true,
                    generation: 0,
                    outstanding: false,
                },
            )
            .is_some()
        {
            return Err(IrqError::UnknownCpu);
        }
        Ok(())
    }

    pub fn save_disable(
        &mut self,
        cpu: CpuId,
        local: &Capability,
    ) -> Result<IrqStateToken, IrqError> {
        require(local, CapabilityKind::CpuLocal, Rights::CONTROL)?;
        let state = self.locals.get_mut(&cpu).ok_or(IrqError::UnknownCpu)?;
        if state.outstanding {
            return Err(IrqError::NestedDisable);
        }
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or(IrqError::GenerationOverflow)?;
        let token = IrqStateToken {
            cpu,
            generation: state.generation,
            was_enabled: state.enabled,
        };
        state.enabled = false;
        state.outstanding = true;
        Ok(token)
    }

    pub fn restore(
        &mut self,
        cpu: CpuId,
        local: &Capability,
        token: IrqStateToken,
    ) -> Result<(), IrqError> {
        require(local, CapabilityKind::CpuLocal, Rights::CONTROL)?;
        if token.cpu != cpu {
            return Err(IrqError::ForeignCpu);
        }
        let state = self.locals.get_mut(&cpu).ok_or(IrqError::UnknownCpu)?;
        if token.generation != state.generation {
            return Err(IrqError::StaleGeneration);
        }
        if !state.outstanding {
            return Err(IrqError::AlreadyRestored);
        }
        state.enabled = token.was_enabled;
        state.outstanding = false;
        Ok(())
    }

    pub fn route(
        &mut self,
        capability: &Capability,
        vector: u8,
        cpu: CpuId,
    ) -> Result<(), IrqError> {
        require(capability, CapabilityKind::Irq, Rights::ROUTE)?;
        if vector < 32 {
            return Err(IrqError::InvalidVector);
        }
        if !self.locals.contains_key(&cpu) {
            return Err(IrqError::UnknownCpu);
        }
        if self
            .routes
            .insert(
                vector,
                IrqRoute {
                    cpu,
                    vector,
                    masked: false,
                },
            )
            .is_some()
        {
            return Err(IrqError::DuplicateRoute);
        }
        Ok(())
    }

    pub fn set_masked(&mut self, vector: u8, masked: bool) -> Result<(), IrqError> {
        self.routes
            .get_mut(&vector)
            .ok_or(IrqError::UnknownRoute)?
            .masked = masked;
        Ok(())
    }

    pub fn deliver(&mut self, vector: u8) -> Result<CpuId, IrqError> {
        let route = self.routes.get(&vector).ok_or(IrqError::UnknownRoute)?;
        if route.masked {
            return Err(IrqError::UnknownRoute);
        }
        self.in_service.insert((route.cpu, vector), true);
        Ok(route.cpu)
    }

    pub fn end_of_interrupt(&mut self, cpu: CpuId, vector: u8) -> Result<(), IrqError> {
        if self.in_service.remove(&(cpu, vector)) != Some(true) {
            return Err(IrqError::SpuriousAcknowledgement);
        }
        Ok(())
    }

    #[must_use]
    pub fn interrupts_enabled(&self, cpu: CpuId) -> Option<bool> {
        self.locals.get(&cpu).map(|state| state.enabled)
    }
}

fn require(capability: &Capability, kind: CapabilityKind, rights: Rights) -> Result<(), IrqError> {
    if capability.kind() != kind {
        return Err(IrqError::WrongCapability);
    }
    if !capability.rights().contains(rights) {
        return Err(IrqError::MissingRights);
    }
    Ok(())
}

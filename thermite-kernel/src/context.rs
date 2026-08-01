use crate::capability::{Capability, CapabilityKind, Rights};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Privilege {
    Kernel,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapOrigin {
    Interrupt(u8),
    Exception(u8),
    Syscall(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registers {
    pub instruction_pointer: u64,
    pub stack_pointer: u64,
    pub flags: u64,
    pub argument0: u64,
    pub result: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserContext {
    id: u32,
    address_space: u32,
    registers: Registers,
    generation: u64,
    runnable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrapFrame {
    context: u32,
    origin: TrapOrigin,
    registers: Registers,
    privilege: Privilege,
    generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextError {
    WrongCapability,
    MissingRights,
    NonCanonical,
    MisalignedStack,
    KernelAddress,
    NotRunnable,
    WrongContext,
    WrongGeneration,
    WrongPrivilege,
    GenerationOverflow,
}

impl UserContext {
    pub fn create(
        capability: &Capability,
        id: u32,
        address_space: u32,
        instruction_pointer: u64,
        stack_pointer: u64,
    ) -> Result<Self, ContextError> {
        require(capability, CapabilityKind::UserContext, Rights::CONTROL)?;
        if !crate::memory::is_canonical_x86_64(instruction_pointer)
            || !crate::memory::is_canonical_x86_64(stack_pointer)
        {
            return Err(ContextError::NonCanonical);
        }
        if instruction_pointer >= 0x0000_8000_0000_0000 || stack_pointer >= 0x0000_8000_0000_0000 {
            return Err(ContextError::KernelAddress);
        }
        if stack_pointer & 0xf != 0 {
            return Err(ContextError::MisalignedStack);
        }
        Ok(Self {
            id,
            address_space,
            registers: Registers {
                instruction_pointer,
                stack_pointer,
                flags: 0x202,
                argument0: 0,
                result: 0,
            },
            generation: 0,
            runnable: true,
        })
    }

    pub fn enter(&mut self, capability: &Capability) -> Result<TrapFrame, ContextError> {
        require(capability, CapabilityKind::UserContext, Rights::CONTROL)?;
        if !self.runnable {
            return Err(ContextError::NotRunnable);
        }
        self.runnable = false;
        Ok(TrapFrame {
            context: self.id,
            origin: TrapOrigin::Syscall(0),
            registers: self.registers,
            privilege: Privilege::User,
            generation: self.generation,
        })
    }

    pub fn trap(
        &mut self,
        frame_capability: &Capability,
        origin: TrapOrigin,
        registers: Registers,
    ) -> Result<TrapFrame, ContextError> {
        require(frame_capability, CapabilityKind::TrapFrame, Rights::CONTROL)?;
        Ok(TrapFrame {
            context: self.id,
            origin,
            registers,
            privilege: Privilege::User,
            generation: self.generation,
        })
    }

    pub fn resume(
        &mut self,
        capability: &Capability,
        frame: TrapFrame,
        result: u64,
    ) -> Result<(), ContextError> {
        require(capability, CapabilityKind::UserContext, Rights::CONTROL)?;
        if frame.context != self.id {
            return Err(ContextError::WrongContext);
        }
        if frame.generation != self.generation {
            return Err(ContextError::WrongGeneration);
        }
        if frame.privilege != Privilege::User {
            return Err(ContextError::WrongPrivilege);
        }
        self.registers = frame.registers;
        self.registers.result = result;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(ContextError::GenerationOverflow)?;
        self.runnable = true;
        Ok(())
    }

    pub fn terminate(&mut self) {
        self.runnable = false;
    }

    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    #[must_use]
    pub const fn address_space(&self) -> u32 {
        self.address_space
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn is_runnable(&self) -> bool {
        self.runnable
    }
}

impl TrapFrame {
    #[must_use]
    pub const fn origin(&self) -> TrapOrigin {
        self.origin
    }
}

fn require(
    capability: &Capability,
    kind: CapabilityKind,
    rights: Rights,
) -> Result<(), ContextError> {
    if capability.kind() != kind {
        return Err(ContextError::WrongCapability);
    }
    if !capability.rights().contains(rights) {
        return Err(ContextError::MissingRights);
    }
    Ok(())
}

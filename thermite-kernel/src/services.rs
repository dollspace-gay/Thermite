use alloc::collections::BTreeMap;

use crate::capability::{Capability, CapabilityKind, Rights};
use crate::smp::CpuId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instant {
    pub ticks: u64,
    pub scale_numerator: u64,
    pub scale_denominator: u64,
    pub error_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalState {
    Running,
    Reboot,
    PowerOff,
    Halt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceError {
    WrongCapability,
    MissingRights,
    ClockWentBackward,
    InvalidScale,
    DeadlineInPast,
    NoDeadline,
    EntropyUnavailable,
    BufferTooLarge,
    AlreadyTerminal,
}

#[derive(Debug)]
pub struct PlatformServices {
    now: u64,
    scale_numerator: u64,
    scale_denominator: u64,
    error_ticks: u64,
    deadlines: BTreeMap<CpuId, u64>,
    entropy_state: u64,
    entropy_healthy: bool,
    terminal: TerminalState,
}

impl PlatformServices {
    pub fn new(scale_numerator: u64, scale_denominator: u64) -> Result<Self, ServiceError> {
        if scale_numerator == 0 || scale_denominator == 0 {
            return Err(ServiceError::InvalidScale);
        }
        Ok(Self {
            now: 0,
            scale_numerator,
            scale_denominator,
            error_ticks: 0,
            deadlines: BTreeMap::new(),
            entropy_state: 0x6a09_e667_f3bc_c909,
            entropy_healthy: true,
            terminal: TerminalState::Running,
        })
    }

    pub fn update_clock(&mut self, ticks: u64, error_ticks: u64) -> Result<(), ServiceError> {
        if ticks < self.now {
            return Err(ServiceError::ClockWentBackward);
        }
        self.now = ticks;
        self.error_ticks = error_ticks;
        Ok(())
    }

    pub fn read_clock(&self, capability: &Capability) -> Result<Instant, ServiceError> {
        require(capability, CapabilityKind::Clock, Rights::READ)?;
        Ok(Instant {
            ticks: self.now,
            scale_numerator: self.scale_numerator,
            scale_denominator: self.scale_denominator,
            error_ticks: self.error_ticks,
        })
    }

    pub fn arm_deadline(
        &mut self,
        capability: &Capability,
        cpu: CpuId,
        deadline: u64,
    ) -> Result<(), ServiceError> {
        require(capability, CapabilityKind::Clock, Rights::CONTROL)?;
        if deadline <= self.now {
            return Err(ServiceError::DeadlineInPast);
        }
        self.deadlines.insert(cpu, deadline);
        Ok(())
    }

    pub fn cancel_deadline(
        &mut self,
        capability: &Capability,
        cpu: CpuId,
    ) -> Result<u64, ServiceError> {
        require(capability, CapabilityKind::Clock, Rights::CONTROL)?;
        self.deadlines.remove(&cpu).ok_or(ServiceError::NoDeadline)
    }

    pub fn due(&self) -> impl Iterator<Item = CpuId> + '_ {
        self.deadlines
            .iter()
            .filter_map(|(cpu, deadline)| (*deadline <= self.now).then_some(*cpu))
    }

    pub fn set_entropy_health(&mut self, healthy: bool) {
        self.entropy_healthy = healthy;
    }

    pub fn fill_entropy(
        &mut self,
        capability: &Capability,
        output: &mut [u8],
    ) -> Result<(), ServiceError> {
        require(capability, CapabilityKind::Entropy, Rights::READ)?;
        if !self.entropy_healthy {
            return Err(ServiceError::EntropyUnavailable);
        }
        if output.len() > 4096 {
            return Err(ServiceError::BufferTooLarge);
        }
        for byte in output {
            self.entropy_state ^= self.entropy_state << 13;
            self.entropy_state ^= self.entropy_state >> 7;
            self.entropy_state ^= self.entropy_state << 17;
            *byte = self.entropy_state as u8;
        }
        Ok(())
    }

    pub fn terminal(
        &mut self,
        capability: &Capability,
        state: TerminalState,
    ) -> Result<(), ServiceError> {
        require(capability, CapabilityKind::Power, Rights::CONTROL)?;
        if self.terminal != TerminalState::Running || state == TerminalState::Running {
            return Err(ServiceError::AlreadyTerminal);
        }
        self.terminal = state;
        Ok(())
    }

    #[must_use]
    pub const fn terminal_state(&self) -> TerminalState {
        self.terminal
    }
}

fn require(
    capability: &Capability,
    kind: CapabilityKind,
    rights: Rights,
) -> Result<(), ServiceError> {
    if capability.kind() != kind {
        return Err(ServiceError::WrongCapability);
    }
    if !capability.rights().contains(rights) {
        return Err(ServiceError::MissingRights);
    }
    Ok(())
}

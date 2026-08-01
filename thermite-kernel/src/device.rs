use alloc::collections::BTreeMap;

use crate::capability::{Capability, CapabilityKind, Rights};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceWidth {
    U8,
    U16,
    U32,
    U64,
}

impl DeviceWidth {
    #[must_use]
    pub const fn bytes(self) -> u64 {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::U32 => 4,
            Self::U64 => 8,
        }
    }

    #[must_use]
    const fn mask(self) -> u64 {
        match self {
            Self::U8 => u8::MAX as u64,
            Self::U16 => u16::MAX as u64,
            Self::U32 => u32::MAX as u64,
            Self::U64 => u64::MAX,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceBarrier {
    Compiler,
    Read,
    Write,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceError {
    WrongCapability,
    MissingRights,
    OutOfRange,
    Misaligned,
    UnsupportedWidth,
    RangeOverflow,
}

#[derive(Debug, Default)]
pub struct DeviceBus {
    mmio: BTreeMap<u64, u8>,
    pio: BTreeMap<u16, u8>,
    barrier_epoch: u64,
}

impl DeviceBus {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mmio_read(
        &self,
        capability: &Capability,
        address: u64,
        width: DeviceWidth,
    ) -> Result<u64, DeviceError> {
        validate_range(
            capability,
            CapabilityKind::Mmio,
            Rights::READ,
            address,
            width,
        )?;
        Ok(read_bytes(&self.mmio, address, width) & width.mask())
    }

    pub fn mmio_write(
        &mut self,
        capability: &Capability,
        address: u64,
        width: DeviceWidth,
        value: u64,
    ) -> Result<(), DeviceError> {
        validate_range(
            capability,
            CapabilityKind::Mmio,
            Rights::WRITE,
            address,
            width,
        )?;
        write_bytes(&mut self.mmio, address, width, value);
        Ok(())
    }

    pub fn pio_read(
        &self,
        capability: &Capability,
        port: u16,
        width: DeviceWidth,
    ) -> Result<u32, DeviceError> {
        if matches!(width, DeviceWidth::U64) {
            return Err(DeviceError::UnsupportedWidth);
        }
        validate_range(
            capability,
            CapabilityKind::IoPort,
            Rights::READ,
            u64::from(port),
            width,
        )?;
        Ok(read_port_bytes(&self.pio, port, width) as u32)
    }

    pub fn pio_write(
        &mut self,
        capability: &Capability,
        port: u16,
        width: DeviceWidth,
        value: u32,
    ) -> Result<(), DeviceError> {
        if matches!(width, DeviceWidth::U64) {
            return Err(DeviceError::UnsupportedWidth);
        }
        validate_range(
            capability,
            CapabilityKind::IoPort,
            Rights::WRITE,
            u64::from(port),
            width,
        )?;
        for index in 0..width.bytes() {
            let Some(current) = port.checked_add(index as u16) else {
                return Err(DeviceError::RangeOverflow);
            };
            self.pio
                .insert(current, ((u64::from(value) >> (index * 8)) & 0xff) as u8);
        }
        Ok(())
    }

    pub fn barrier(&mut self, _kind: DeviceBarrier) -> Result<u64, DeviceError> {
        self.barrier_epoch = self
            .barrier_epoch
            .checked_add(1)
            .ok_or(DeviceError::RangeOverflow)?;
        Ok(self.barrier_epoch)
    }
}

fn validate_range(
    capability: &Capability,
    kind: CapabilityKind,
    rights: Rights,
    address: u64,
    width: DeviceWidth,
) -> Result<(), DeviceError> {
    if capability.kind() != kind {
        return Err(DeviceError::WrongCapability);
    }
    if !capability.rights().contains(rights) {
        return Err(DeviceError::MissingRights);
    }
    if address % width.bytes() != 0 {
        return Err(DeviceError::Misaligned);
    }
    if !capability.contains_range(address, width.bytes()) {
        return Err(DeviceError::OutOfRange);
    }
    Ok(())
}

fn read_bytes(values: &BTreeMap<u64, u8>, address: u64, width: DeviceWidth) -> u64 {
    (0..width.bytes()).fold(0, |value, index| {
        value | (u64::from(values.get(&(address + index)).copied().unwrap_or(0)) << (index * 8))
    })
}

fn write_bytes(values: &mut BTreeMap<u64, u8>, address: u64, width: DeviceWidth, value: u64) {
    for index in 0..width.bytes() {
        values.insert(address + index, ((value >> (index * 8)) & 0xff) as u8);
    }
}

fn read_port_bytes(values: &BTreeMap<u16, u8>, port: u16, width: DeviceWidth) -> u64 {
    (0..width.bytes()).fold(0, |value, index| {
        let current = port.saturating_add(index as u16);
        value | (u64::from(values.get(&current).copied().unwrap_or(0)) << (index * 8))
    })
}

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::capability::{Capability, CapabilityKind, Rights};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicOrdering {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicWidth {
    Bool,
    U32,
    U64,
    Usize,
}

impl AtomicWidth {
    #[must_use]
    pub const fn bytes(self) -> u64 {
        match self {
            Self::Bool => 1,
            Self::U32 => 4,
            Self::U64 | Self::Usize => 8,
        }
    }

    #[must_use]
    const fn mask(self) -> u64 {
        match self {
            Self::Bool => 1,
            Self::U32 => u32::MAX as u64,
            Self::U64 | Self::Usize => u64::MAX,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicError {
    WrongCapability,
    MissingRights,
    Misaligned,
    OutOfRange,
    IllegalLoadOrdering,
    IllegalStoreOrdering,
    IllegalFailureOrdering,
    FailureStrongerThanSuccess,
    UnknownCell,
    DuplicateCell,
    IllegalFenceOrdering,
    EventOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompareExchange {
    pub previous: u64,
    pub exchanged: bool,
}

/// A sealed sequential model of one hardware atomic cell. The target body must
/// refine these transitions and ordering checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicCell {
    width: AtomicWidth,
    address: u64,
    value: u64,
    modification_order: u64,
}

impl AtomicCell {
    pub fn create(
        backing: &Capability,
        width: AtomicWidth,
        address: u64,
        initial: u64,
    ) -> Result<Self, AtomicError> {
        if !matches!(
            backing.kind(),
            CapabilityKind::PhysRegion | CapabilityKind::VirtRegion | CapabilityKind::CpuLocal
        ) {
            return Err(AtomicError::WrongCapability);
        }
        if !backing.rights().contains(Rights::READ.union(Rights::WRITE)) {
            return Err(AtomicError::MissingRights);
        }
        if address % width.bytes() != 0 {
            return Err(AtomicError::Misaligned);
        }
        if !backing.contains_range(address, width.bytes()) {
            return Err(AtomicError::OutOfRange);
        }
        Ok(Self {
            width,
            address,
            value: initial & width.mask(),
            modification_order: 0,
        })
    }

    pub fn load(&self, ordering: AtomicOrdering) -> Result<u64, AtomicError> {
        if matches!(ordering, AtomicOrdering::Release | AtomicOrdering::AcqRel) {
            return Err(AtomicError::IllegalLoadOrdering);
        }
        Ok(self.value)
    }

    pub fn store(&mut self, value: u64, ordering: AtomicOrdering) -> Result<(), AtomicError> {
        if matches!(ordering, AtomicOrdering::Acquire | AtomicOrdering::AcqRel) {
            return Err(AtomicError::IllegalStoreOrdering);
        }
        self.write(value);
        Ok(())
    }

    pub fn swap(&mut self, value: u64, _ordering: AtomicOrdering) -> Result<u64, AtomicError> {
        let previous = self.value;
        self.write(value);
        Ok(previous)
    }

    pub fn compare_exchange(
        &mut self,
        current: u64,
        new: u64,
        success: AtomicOrdering,
        failure: AtomicOrdering,
    ) -> Result<CompareExchange, AtomicError> {
        validate_compare_exchange(success, failure)?;
        let previous = self.value;
        let exchanged = previous == current & self.width.mask();
        if exchanged {
            self.write(new);
        }
        Ok(CompareExchange {
            previous,
            exchanged,
        })
    }

    pub fn compare_exchange_weak(
        &mut self,
        current: u64,
        new: u64,
        success: AtomicOrdering,
        failure: AtomicOrdering,
        spurious_failure: bool,
    ) -> Result<CompareExchange, AtomicError> {
        validate_compare_exchange(success, failure)?;
        let previous = self.value;
        if spurious_failure && previous == current & self.width.mask() {
            return Ok(CompareExchange {
                previous,
                exchanged: false,
            });
        }
        self.compare_exchange(current, new, success, failure)
    }

    pub fn fetch_add(&mut self, value: u64, _ordering: AtomicOrdering) -> Result<u64, AtomicError> {
        let previous = self.value;
        self.write(previous.wrapping_add(value));
        Ok(previous)
    }

    pub fn fetch_sub(&mut self, value: u64, _ordering: AtomicOrdering) -> Result<u64, AtomicError> {
        let previous = self.value;
        self.write(previous.wrapping_sub(value));
        Ok(previous)
    }

    pub fn fetch_and(&mut self, value: u64, _ordering: AtomicOrdering) -> Result<u64, AtomicError> {
        let previous = self.value;
        self.write(previous & value);
        Ok(previous)
    }

    pub fn fetch_or(&mut self, value: u64, _ordering: AtomicOrdering) -> Result<u64, AtomicError> {
        let previous = self.value;
        self.write(previous | value);
        Ok(previous)
    }

    pub fn fetch_xor(&mut self, value: u64, _ordering: AtomicOrdering) -> Result<u64, AtomicError> {
        let previous = self.value;
        self.write(previous ^ value);
        Ok(previous)
    }

    #[must_use]
    pub const fn modification_order(&self) -> u64 {
        self.modification_order
    }

    #[must_use]
    pub const fn address(&self) -> u64 {
        self.address
    }

    fn write(&mut self, value: u64) {
        self.value = value & self.width.mask();
        self.modification_order = self.modification_order.wrapping_add(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceKind {
    Compiler,
    Hardware,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicEventKind {
    Load,
    Store,
    ReadModifyWrite,
    Fence(FenceKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtomicEvent {
    pub id: u64,
    pub cpu: u16,
    pub cell: Option<u32>,
    pub kind: AtomicEventKind,
    pub ordering: AtomicOrdering,
    pub value: u64,
    pub reads_from: Option<u64>,
    pub modification_order: Option<u64>,
    pub sequentially_consistent_order: Option<u64>,
    pub happens_after_release: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MemoryCell {
    value: u64,
    modification_order: u64,
    last_write: u64,
    last_write_ordering: AtomicOrdering,
}

/// A finite executable model of the frozen atomic memory interface.  Each read
/// records its reads-from edge, each write its per-cell modification-order
/// index, an acquire that observes a release records its synchronizes-with /
/// happens-before predecessor, and every SeqCst operation receives one total
/// order index.
#[derive(Debug, Default)]
pub struct AtomicMemoryModel {
    cells: BTreeMap<u32, MemoryCell>,
    events: Vec<AtomicEvent>,
    next_event: u64,
    next_sc: u64,
}

impl AtomicMemoryModel {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_event: 1,
            next_sc: 1,
            ..Self::default()
        }
    }

    pub fn register(&mut self, cell: u32, initial: u64) -> Result<(), AtomicError> {
        if self
            .cells
            .insert(
                cell,
                MemoryCell {
                    value: initial,
                    modification_order: 0,
                    last_write: 0,
                    last_write_ordering: AtomicOrdering::Relaxed,
                },
            )
            .is_some()
        {
            return Err(AtomicError::DuplicateCell);
        }
        Ok(())
    }

    pub fn store(
        &mut self,
        cpu: u16,
        cell: u32,
        value: u64,
        ordering: AtomicOrdering,
    ) -> Result<u64, AtomicError> {
        if matches!(ordering, AtomicOrdering::Acquire | AtomicOrdering::AcqRel) {
            return Err(AtomicError::IllegalStoreOrdering);
        }
        self.write_event(cpu, cell, value, ordering, AtomicEventKind::Store, None)
    }

    pub fn load(
        &mut self,
        cpu: u16,
        cell: u32,
        ordering: AtomicOrdering,
    ) -> Result<(u64, u64), AtomicError> {
        if matches!(ordering, AtomicOrdering::Release | AtomicOrdering::AcqRel) {
            return Err(AtomicError::IllegalLoadOrdering);
        }
        let state = *self.cells.get(&cell).ok_or(AtomicError::UnknownCell)?;
        let happens_after_release =
            (matches!(ordering, AtomicOrdering::Acquire | AtomicOrdering::SeqCst)
                && matches!(
                    state.last_write_ordering,
                    AtomicOrdering::Release | AtomicOrdering::AcqRel | AtomicOrdering::SeqCst
                )
                && state.last_write != 0)
                .then_some(state.last_write);
        let event = self.next_event_id()?;
        let sc = self.sc_index(ordering)?;
        self.events.push(AtomicEvent {
            id: event,
            cpu,
            cell: Some(cell),
            kind: AtomicEventKind::Load,
            ordering,
            value: state.value,
            reads_from: (state.last_write != 0).then_some(state.last_write),
            modification_order: None,
            sequentially_consistent_order: sc,
            happens_after_release,
        });
        Ok((state.value, event))
    }

    pub fn fetch_add(
        &mut self,
        cpu: u16,
        cell: u32,
        value: u64,
        ordering: AtomicOrdering,
    ) -> Result<(u64, u64), AtomicError> {
        let previous = self.cells.get(&cell).ok_or(AtomicError::UnknownCell)?.value;
        let event = self.write_event(
            cpu,
            cell,
            previous.wrapping_add(value),
            ordering,
            AtomicEventKind::ReadModifyWrite,
            None,
        )?;
        // The RMW reads from the immediately preceding modification.
        let prior = self.events.iter().rev().nth(1).and_then(|candidate| {
            (candidate.cell == Some(cell) && candidate.modification_order.is_some())
                .then_some(candidate.id)
        });
        let happens_after_release = if matches!(
            ordering,
            AtomicOrdering::Acquire | AtomicOrdering::AcqRel | AtomicOrdering::SeqCst
        ) {
            prior.filter(|id| {
                self.events.iter().any(|candidate| {
                    candidate.id == *id
                        && matches!(
                            candidate.ordering,
                            AtomicOrdering::Release
                                | AtomicOrdering::AcqRel
                                | AtomicOrdering::SeqCst
                        )
                })
            })
        } else {
            None
        };
        if let Some(last) = self.events.last_mut() {
            last.reads_from = prior;
            last.happens_after_release = happens_after_release;
        }
        Ok((previous, event))
    }

    pub fn fence(
        &mut self,
        cpu: u16,
        kind: FenceKind,
        ordering: AtomicOrdering,
    ) -> Result<u64, AtomicError> {
        if ordering == AtomicOrdering::Relaxed {
            return Err(AtomicError::IllegalFenceOrdering);
        }
        let event = self.next_event_id()?;
        let sc = self.sc_index(ordering)?;
        self.events.push(AtomicEvent {
            id: event,
            cpu,
            cell: None,
            kind: AtomicEventKind::Fence(kind),
            ordering,
            value: 0,
            reads_from: None,
            modification_order: None,
            sequentially_consistent_order: sc,
            happens_after_release: None,
        });
        Ok(event)
    }

    #[must_use]
    pub fn events(&self) -> &[AtomicEvent] {
        &self.events
    }

    fn write_event(
        &mut self,
        cpu: u16,
        cell: u32,
        value: u64,
        ordering: AtomicOrdering,
        kind: AtomicEventKind,
        reads_from: Option<u64>,
    ) -> Result<u64, AtomicError> {
        let event = self.next_event_id()?;
        let sc = self.sc_index(ordering)?;
        let state = self.cells.get_mut(&cell).ok_or(AtomicError::UnknownCell)?;
        state.modification_order = state
            .modification_order
            .checked_add(1)
            .ok_or(AtomicError::EventOverflow)?;
        state.value = value;
        state.last_write = event;
        state.last_write_ordering = ordering;
        self.events.push(AtomicEvent {
            id: event,
            cpu,
            cell: Some(cell),
            kind,
            ordering,
            value,
            reads_from,
            modification_order: Some(state.modification_order),
            sequentially_consistent_order: sc,
            happens_after_release: None,
        });
        Ok(event)
    }

    fn next_event_id(&mut self) -> Result<u64, AtomicError> {
        let event = self.next_event;
        self.next_event = self
            .next_event
            .checked_add(1)
            .ok_or(AtomicError::EventOverflow)?;
        Ok(event)
    }

    fn sc_index(&mut self, ordering: AtomicOrdering) -> Result<Option<u64>, AtomicError> {
        if ordering != AtomicOrdering::SeqCst {
            return Ok(None);
        }
        let index = self.next_sc;
        self.next_sc = self
            .next_sc
            .checked_add(1)
            .ok_or(AtomicError::EventOverflow)?;
        Ok(Some(index))
    }
}

pub fn validate_compare_exchange(
    success: AtomicOrdering,
    failure: AtomicOrdering,
) -> Result<(), AtomicError> {
    if matches!(failure, AtomicOrdering::Release | AtomicOrdering::AcqRel) {
        return Err(AtomicError::IllegalFailureOrdering);
    }
    let permitted = match success {
        AtomicOrdering::Relaxed => matches!(failure, AtomicOrdering::Relaxed),
        AtomicOrdering::Acquire => {
            matches!(failure, AtomicOrdering::Relaxed | AtomicOrdering::Acquire)
        }
        AtomicOrdering::Release => matches!(failure, AtomicOrdering::Relaxed),
        AtomicOrdering::AcqRel => {
            matches!(failure, AtomicOrdering::Relaxed | AtomicOrdering::Acquire)
        }
        AtomicOrdering::SeqCst => matches!(
            failure,
            AtomicOrdering::Relaxed | AtomicOrdering::Acquire | AtomicOrdering::SeqCst
        ),
    };
    if permitted {
        Ok(())
    } else {
        Err(AtomicError::FailureStrongerThanSuccess)
    }
}

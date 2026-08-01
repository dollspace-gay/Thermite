use alloc::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::smp::{CpuId, CpuSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncError {
    TicketOverflow,
    UnknownTicket,
    OutOfOrderRelease,
    AlreadyInitialized,
    Uninitialized,
    QueueFull,
    QueueEmpty,
    DuplicateParticipant,
    ForeignParticipant,
    StaleGeneration,
    GenerationOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockGuard {
    ticket: u64,
}

#[derive(Debug, Default)]
pub struct TicketLock {
    next: u64,
    owner: u64,
    outstanding: BTreeSet<u64>,
}

impl TicketLock {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&mut self) -> Result<u64, SyncError> {
        let ticket = self.next;
        self.next = self.next.checked_add(1).ok_or(SyncError::TicketOverflow)?;
        self.outstanding.insert(ticket);
        Ok(ticket)
    }

    pub fn acquire(&self, ticket: u64) -> Result<LockGuard, SyncError> {
        if !self.outstanding.contains(&ticket) {
            return Err(SyncError::UnknownTicket);
        }
        if ticket != self.owner {
            return Err(SyncError::OutOfOrderRelease);
        }
        Ok(LockGuard { ticket })
    }

    pub fn release(&mut self, guard: LockGuard) -> Result<(), SyncError> {
        if guard.ticket != self.owner || !self.outstanding.remove(&guard.ticket) {
            return Err(SyncError::OutOfOrderRelease);
        }
        self.owner = self.owner.checked_add(1).ok_or(SyncError::TicketOverflow)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnceCell<T> {
    value: Option<T>,
}

impl<T> Default for OnceCell<T> {
    fn default() -> Self {
        Self { value: None }
    }
}

impl<T> OnceCell<T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initialize(&mut self, value: T) -> Result<&T, SyncError> {
        if self.value.is_some() {
            return Err(SyncError::AlreadyInitialized);
        }
        self.value = Some(value);
        self.value.as_ref().ok_or(SyncError::Uninitialized)
    }

    pub fn get(&self) -> Result<&T, SyncError> {
        self.value.as_ref().ok_or(SyncError::Uninitialized)
    }
}

#[derive(Debug)]
pub struct BoundedMpsc<T> {
    capacity: usize,
    queue: VecDeque<(CpuId, T)>,
    pushes: BTreeMap<CpuId, u64>,
    pops: u64,
}

impl<T> BoundedMpsc<T> {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            queue: VecDeque::new(),
            pushes: BTreeMap::new(),
            pops: 0,
        }
    }

    pub fn push(&mut self, producer: CpuId, value: T) -> Result<(), SyncError> {
        if self.queue.len() == self.capacity {
            return Err(SyncError::QueueFull);
        }
        self.queue.push_back((producer, value));
        *self.pushes.entry(producer).or_insert(0) += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Result<(CpuId, T), SyncError> {
        let value = self.queue.pop_front().ok_or(SyncError::QueueEmpty)?;
        self.pops = self.pops.saturating_add(1);
        Ok(value)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    #[must_use]
    pub fn accounting_holds(&self) -> bool {
        let pushes = self.pushes.values().copied().sum::<u64>();
        pushes == self.pops.saturating_add(self.queue.len() as u64)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Barrier {
    participants: CpuSet,
    arrived: BTreeSet<CpuId>,
    generation: u64,
}

impl Barrier {
    #[must_use]
    pub fn new(participants: CpuSet) -> Self {
        Self {
            participants,
            arrived: BTreeSet::new(),
            generation: 0,
        }
    }

    pub fn arrive(&mut self, cpu: CpuId, generation: u64) -> Result<bool, SyncError> {
        if generation != self.generation {
            return Err(SyncError::StaleGeneration);
        }
        if !self.participants.contains(cpu) {
            return Err(SyncError::ForeignParticipant);
        }
        if !self.arrived.insert(cpu) {
            return Err(SyncError::DuplicateParticipant);
        }
        Ok(self.arrived.len() == self.participants.len())
    }

    pub fn advance(&mut self) -> Result<u64, SyncError> {
        if self.arrived.len() != self.participants.len() {
            return Err(SyncError::ForeignParticipant);
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(SyncError::GenerationOverflow)?;
        self.arrived.clear();
        Ok(self.generation)
    }
}

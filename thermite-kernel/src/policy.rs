use alloc::collections::BTreeMap;

use crate::event::{Action, Event, EventKind, PlatformError};
use crate::smp::{CpuId, CpuSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuShard {
    pub cpu: CpuId,
    pub last_event: u64,
    pub ticks: u64,
    pub runnable_tasks: u32,
    pub terminal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionBatch<const N: usize> {
    actions: [Option<Action>; N],
    len: usize,
}

impl<const N: usize> Default for ActionBatch<N> {
    fn default() -> Self {
        Self {
            actions: core::array::from_fn(|_| None),
            len: 0,
        }
    }
}

impl<const N: usize> ActionBatch<N> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, action: Action) -> Result<(), PolicyError> {
        if self.len == N {
            return Err(PolicyError::ActionOverflow);
        }
        self.actions[self.len] = Some(action);
        self.len += 1;
        Ok(())
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &Action> {
        self.actions[..self.len].iter().flatten()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    UnknownCpu,
    DuplicateCpu,
    StaleEvent,
    CorrelationRequired,
    UnexpectedCorrelation,
    ActionOverflow,
    TickOverflow,
    TaskOverflow,
    Terminal,
    Platform(PlatformError),
}

#[derive(Debug, Default)]
pub struct KernelPolicy {
    shards: BTreeMap<CpuId, CpuShard>,
    last_global_event: u64,
}

impl KernelPolicy {
    pub fn add_cpu(&mut self, cpu: CpuId) -> Result<(), PolicyError> {
        if self
            .shards
            .insert(
                cpu,
                CpuShard {
                    cpu,
                    last_event: 0,
                    ticks: 0,
                    runnable_tasks: 0,
                    terminal: false,
                },
            )
            .is_some()
        {
            return Err(PolicyError::DuplicateCpu);
        }
        Ok(())
    }

    pub fn step(&mut self, event: Event) -> Result<ActionBatch<4>, PolicyError> {
        if event.id <= self.last_global_event {
            return Err(PolicyError::StaleEvent);
        }
        let shard = self
            .shards
            .get_mut(&event.cpu)
            .ok_or(PolicyError::UnknownCpu)?;
        if shard.terminal {
            return Err(PolicyError::Terminal);
        }
        let completion_event = matches!(
            event.kind,
            EventKind::CpuOnline | EventKind::CpuStartFailed(_) | EventKind::DmaComplete { .. }
        );
        if completion_event && event.correlation.is_none() {
            return Err(PolicyError::CorrelationRequired);
        }
        if !completion_event && event.correlation.is_some() {
            return Err(PolicyError::UnexpectedCorrelation);
        }

        self.last_global_event = event.id;
        shard.last_event = event.id;
        let mut actions = ActionBatch::new();
        match event.kind {
            EventKind::Boot => actions.push(Action::ArmTimer { deadline: 1 })?,
            EventKind::CpuOnline => {
                shard.runnable_tasks = shard
                    .runnable_tasks
                    .checked_add(1)
                    .ok_or(PolicyError::TaskOverflow)?;
                actions.push(Action::EnqueueTask {
                    task: u32::from(event.cpu),
                    cpu: event.cpu,
                })?;
            }
            EventKind::CpuStartFailed(_) => {}
            EventKind::Irq { vector } => actions.push(Action::AckIrq { vector })?,
            EventKind::Timer => {
                shard.ticks = shard
                    .ticks
                    .checked_add(1)
                    .ok_or(PolicyError::TickOverflow)?;
                actions.push(Action::ArmTimer {
                    deadline: shard.ticks + 1,
                })?;
            }
            EventKind::Ipi { vector, epoch } => actions.push(Action::SendIpi {
                targets: CpuSet::from_ids([event.cpu]),
                vector,
                epoch,
            })?,
            EventKind::DmaComplete { .. } => {
                shard.runnable_tasks = shard
                    .runnable_tasks
                    .checked_add(1)
                    .ok_or(PolicyError::TaskOverflow)?;
            }
            EventKind::Syscall { number } => match number {
                0 => actions.push(Action::EnqueueTask {
                    task: shard.runnable_tasks,
                    cpu: event.cpu,
                })?,
                _ => actions.push(Action::EnterContext {
                    context: number as u32,
                })?,
            },
            EventKind::UserFault { .. } => {
                actions.push(Action::EnterContext { context: 0 })?;
            }
            EventKind::DeviceFault { .. } => actions.push(Action::MaskIrq { vector: 0 })?,
            EventKind::ActionComplete => {}
            EventKind::ShutdownRequest => {
                shard.terminal = true;
                actions.push(Action::PowerOff)?;
            }
        }
        Ok(actions)
    }

    #[must_use]
    pub fn shard(&self, cpu: CpuId) -> Option<CpuShard> {
        self.shards.get(&cpu).copied()
    }
}

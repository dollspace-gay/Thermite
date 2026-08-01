use alloc::collections::{BTreeMap, VecDeque};

use crate::smp::{CpuId, CpuSet};

pub type TaskId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Runnable,
    Running(CpuId),
    Blocked,
    Exited(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Task {
    state: TaskState,
    generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerError {
    UnknownCpu,
    DuplicateTask,
    UnknownTask,
    WrongState,
    ForeignCpu,
    DuplicateQueueEntry,
    GenerationOverflow,
}

#[derive(Debug, Default)]
pub struct Scheduler {
    queues: BTreeMap<CpuId, VecDeque<TaskId>>,
    tasks: BTreeMap<TaskId, Task>,
    current: BTreeMap<CpuId, Option<TaskId>>,
    completed_dispatches: u64,
    steals: u64,
}

impl Scheduler {
    pub fn with_online_cpus(cpus: &CpuSet) -> Self {
        let queues = cpus.iter().map(|cpu| (cpu, VecDeque::new())).collect();
        let current = cpus.iter().map(|cpu| (cpu, None)).collect();
        Self {
            queues,
            tasks: BTreeMap::new(),
            current,
            completed_dispatches: 0,
            steals: 0,
        }
    }

    pub fn create_task(&mut self, task: TaskId, cpu: CpuId) -> Result<(), SchedulerError> {
        let queue = self
            .queues
            .get_mut(&cpu)
            .ok_or(SchedulerError::UnknownCpu)?;
        if self.tasks.contains_key(&task) {
            return Err(SchedulerError::DuplicateTask);
        }
        self.tasks.insert(
            task,
            Task {
                state: TaskState::Runnable,
                generation: 0,
            },
        );
        queue.push_back(task);
        Ok(())
    }

    pub fn dispatch(&mut self, cpu: CpuId) -> Result<Option<TaskId>, SchedulerError> {
        if self
            .current
            .get(&cpu)
            .ok_or(SchedulerError::UnknownCpu)?
            .is_some()
        {
            return Err(SchedulerError::WrongState);
        }
        let local = self
            .queues
            .get_mut(&cpu)
            .ok_or(SchedulerError::UnknownCpu)?
            .pop_front();
        let task = match local {
            Some(task) => Some(task),
            None => self.steal(cpu)?,
        };
        if let Some(task) = task {
            let state = self
                .tasks
                .get_mut(&task)
                .ok_or(SchedulerError::UnknownTask)?;
            if state.state != TaskState::Runnable {
                return Err(SchedulerError::WrongState);
            }
            state.state = TaskState::Running(cpu);
            *self
                .current
                .get_mut(&cpu)
                .ok_or(SchedulerError::UnknownCpu)? = Some(task);
            self.completed_dispatches = self.completed_dispatches.saturating_add(1);
        }
        Ok(task)
    }

    pub fn yield_current(&mut self, cpu: CpuId) -> Result<TaskId, SchedulerError> {
        let current = self
            .current
            .get_mut(&cpu)
            .ok_or(SchedulerError::UnknownCpu)?;
        let task = current.take().ok_or(SchedulerError::WrongState)?;
        let record = self
            .tasks
            .get_mut(&task)
            .ok_or(SchedulerError::UnknownTask)?;
        if record.state != TaskState::Running(cpu) {
            return Err(SchedulerError::ForeignCpu);
        }
        record.generation = record
            .generation
            .checked_add(1)
            .ok_or(SchedulerError::GenerationOverflow)?;
        record.state = TaskState::Runnable;
        self.queues
            .get_mut(&cpu)
            .ok_or(SchedulerError::UnknownCpu)?
            .push_back(task);
        Ok(task)
    }

    pub fn block_current(&mut self, cpu: CpuId) -> Result<TaskId, SchedulerError> {
        let task = self
            .current
            .get_mut(&cpu)
            .ok_or(SchedulerError::UnknownCpu)?
            .take()
            .ok_or(SchedulerError::WrongState)?;
        let record = self
            .tasks
            .get_mut(&task)
            .ok_or(SchedulerError::UnknownTask)?;
        if record.state != TaskState::Running(cpu) {
            return Err(SchedulerError::ForeignCpu);
        }
        record.state = TaskState::Blocked;
        Ok(task)
    }

    pub fn wake(&mut self, task: TaskId, cpu: CpuId) -> Result<(), SchedulerError> {
        let queue = self
            .queues
            .get_mut(&cpu)
            .ok_or(SchedulerError::UnknownCpu)?;
        let record = self
            .tasks
            .get_mut(&task)
            .ok_or(SchedulerError::UnknownTask)?;
        if record.state != TaskState::Blocked {
            return Err(SchedulerError::WrongState);
        }
        if queue.contains(&task) {
            return Err(SchedulerError::DuplicateQueueEntry);
        }
        record.state = TaskState::Runnable;
        queue.push_back(task);
        Ok(())
    }

    pub fn exit_current(&mut self, cpu: CpuId, code: i32) -> Result<TaskId, SchedulerError> {
        let task = self
            .current
            .get_mut(&cpu)
            .ok_or(SchedulerError::UnknownCpu)?
            .take()
            .ok_or(SchedulerError::WrongState)?;
        let record = self
            .tasks
            .get_mut(&task)
            .ok_or(SchedulerError::UnknownTask)?;
        if record.state != TaskState::Running(cpu) {
            return Err(SchedulerError::ForeignCpu);
        }
        record.state = TaskState::Exited(code);
        Ok(task)
    }

    fn steal(&mut self, requester: CpuId) -> Result<Option<TaskId>, SchedulerError> {
        let victim = self
            .queues
            .iter()
            .filter(|(cpu, _)| **cpu != requester)
            .max_by_key(|(cpu, queue)| (queue.len(), core::cmp::Reverse(**cpu)))
            .and_then(|(cpu, queue)| (!queue.is_empty()).then_some(*cpu));
        let Some(victim) = victim else {
            return Ok(None);
        };
        let task = self
            .queues
            .get_mut(&victim)
            .ok_or(SchedulerError::UnknownCpu)?
            .pop_back();
        if task.is_some() {
            self.steals = self.steals.saturating_add(1);
        }
        Ok(task)
    }

    #[must_use]
    pub fn task_state(&self, task: TaskId) -> Option<TaskState> {
        self.tasks.get(&task).map(|record| record.state)
    }

    #[must_use]
    pub const fn dispatches(&self) -> u64 {
        self.completed_dispatches
    }

    #[must_use]
    pub const fn steals(&self) -> u64 {
        self.steals
    }

    #[must_use]
    pub fn invariant_holds(&self) -> bool {
        let mut seen = alloc::collections::BTreeSet::new();
        for (cpu, queue) in &self.queues {
            if queue.iter().any(|task| {
                !seen.insert(*task)
                    || !matches!(
                        self.tasks.get(task),
                        Some(Task {
                            state: TaskState::Runnable,
                            ..
                        })
                    )
            }) {
                return false;
            }
            if let Some(task) = self.current.get(cpu).copied().flatten() {
                if !seen.insert(task)
                    || !matches!(
                        self.tasks.get(&task),
                        Some(Task {
                            state: TaskState::Running(owner),
                            ..
                        }) if owner == cpu
                    )
                {
                    return false;
                }
            }
        }
        self.tasks.iter().all(|(task, record)| {
            matches!(record.state, TaskState::Blocked | TaskState::Exited(_)) || seen.contains(task)
        })
    }
}

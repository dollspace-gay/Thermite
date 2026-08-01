use alloc::collections::{BTreeMap, BTreeSet};

pub type CpuId = u16;
pub type IpiEpoch = u64;
pub type ShootdownEpoch = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuLifecycle {
    Discovered,
    Prepared,
    Starting,
    Online,
    Failed(CpuFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuFailure {
    NoStack,
    NoLocalStorage,
    StartupTimeout,
    DescriptorInstall,
    InterruptController,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuSet {
    members: BTreeSet<CpuId>,
}

impl CpuSet {
    #[must_use]
    pub fn from_ids(ids: impl IntoIterator<Item = CpuId>) -> Self {
        Self {
            members: ids.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn contains(&self, cpu: CpuId) -> bool {
        self.members.contains(&cpu)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.members.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = CpuId> + '_ {
        self.members.iter().copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmpError {
    DuplicateCpu,
    UnknownCpu,
    IllegalTransition,
    CpuNotOnline,
    EpochOverflow,
    UnknownEpoch,
    ForeignCpu,
    DuplicateAcknowledgement,
    StaleEpoch,
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingEpoch {
    targets: CpuSet,
    acknowledgements: BTreeSet<CpuId>,
}

#[derive(Debug, Default)]
pub struct SmpState {
    cpus: BTreeMap<CpuId, CpuLifecycle>,
    next_ipi_epoch: IpiEpoch,
    next_shootdown_epoch: ShootdownEpoch,
    ipis: BTreeMap<IpiEpoch, PendingEpoch>,
    shootdowns: BTreeMap<ShootdownEpoch, PendingEpoch>,
}

impl SmpState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn discover(&mut self, cpu: CpuId) -> Result<(), SmpError> {
        if self.cpus.insert(cpu, CpuLifecycle::Discovered).is_some() {
            return Err(SmpError::DuplicateCpu);
        }
        Ok(())
    }

    pub fn transition(&mut self, cpu: CpuId, next: CpuLifecycle) -> Result<(), SmpError> {
        let current = self.cpus.get_mut(&cpu).ok_or(SmpError::UnknownCpu)?;
        let legal = matches!(
            (*current, next),
            (CpuLifecycle::Discovered, CpuLifecycle::Prepared)
                | (CpuLifecycle::Prepared, CpuLifecycle::Starting)
                | (CpuLifecycle::Starting, CpuLifecycle::Online)
                | (CpuLifecycle::Prepared, CpuLifecycle::Failed(_))
                | (CpuLifecycle::Starting, CpuLifecycle::Failed(_))
        );
        if !legal {
            return Err(SmpError::IllegalTransition);
        }
        *current = next;
        Ok(())
    }

    #[must_use]
    pub fn lifecycle(&self, cpu: CpuId) -> Option<CpuLifecycle> {
        self.cpus.get(&cpu).copied()
    }

    #[must_use]
    pub fn online(&self) -> CpuSet {
        CpuSet::from_ids(
            self.cpus
                .iter()
                .filter_map(|(cpu, state)| matches!(state, CpuLifecycle::Online).then_some(*cpu)),
        )
    }

    pub fn begin_ipi(&mut self, targets: CpuSet) -> Result<IpiEpoch, SmpError> {
        self.validate_online_targets(&targets)?;
        self.next_ipi_epoch = self
            .next_ipi_epoch
            .checked_add(1)
            .ok_or(SmpError::EpochOverflow)?;
        self.ipis.insert(
            self.next_ipi_epoch,
            PendingEpoch {
                targets,
                acknowledgements: BTreeSet::new(),
            },
        );
        Ok(self.next_ipi_epoch)
    }

    pub fn acknowledge_ipi(&mut self, epoch: IpiEpoch, cpu: CpuId) -> Result<bool, SmpError> {
        acknowledge(&mut self.ipis, epoch, cpu)
    }

    pub fn begin_shootdown(&mut self) -> Result<ShootdownEpoch, SmpError> {
        let targets = self.online();
        self.next_shootdown_epoch = self
            .next_shootdown_epoch
            .checked_add(1)
            .ok_or(SmpError::EpochOverflow)?;
        self.shootdowns.insert(
            self.next_shootdown_epoch,
            PendingEpoch {
                targets,
                acknowledgements: BTreeSet::new(),
            },
        );
        Ok(self.next_shootdown_epoch)
    }

    pub fn acknowledge_shootdown(
        &mut self,
        epoch: ShootdownEpoch,
        cpu: CpuId,
    ) -> Result<bool, SmpError> {
        acknowledge(&mut self.shootdowns, epoch, cpu)
    }

    pub fn complete_shootdown(&mut self, epoch: ShootdownEpoch) -> Result<(), SmpError> {
        let pending = self.shootdowns.get(&epoch).ok_or(SmpError::UnknownEpoch)?;
        if pending.acknowledgements != pending.targets.members {
            return Err(SmpError::Incomplete);
        }
        self.shootdowns.remove(&epoch);
        Ok(())
    }

    fn validate_online_targets(&self, targets: &CpuSet) -> Result<(), SmpError> {
        if targets
            .iter()
            .any(|cpu| !matches!(self.cpus.get(&cpu), Some(CpuLifecycle::Online)))
        {
            return Err(SmpError::CpuNotOnline);
        }
        Ok(())
    }
}

fn acknowledge(
    pending: &mut BTreeMap<u64, PendingEpoch>,
    epoch: u64,
    cpu: CpuId,
) -> Result<bool, SmpError> {
    let Some(current) = pending.keys().next_back().copied() else {
        return Err(SmpError::UnknownEpoch);
    };
    if epoch < current {
        return Err(SmpError::StaleEpoch);
    }
    let entry = pending.get_mut(&epoch).ok_or(SmpError::UnknownEpoch)?;
    if !entry.targets.contains(cpu) {
        return Err(SmpError::ForeignCpu);
    }
    if !entry.acknowledgements.insert(cpu) {
        return Err(SmpError::DuplicateAcknowledgement);
    }
    Ok(entry.acknowledgements == entry.targets.members)
}

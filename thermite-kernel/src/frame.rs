use alloc::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRun {
    pub base: u64,
    pub pages: u64,
    pub generation: u64,
    pub zeroed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    Empty,
    Misaligned,
    RangeOverflow,
    Overlap,
    OutOfMemory,
    UnknownRun,
    StaleGeneration,
    NonAdjacent,
    GenerationOverflow,
    AlreadyLent,
    NotLent,
    ForeignBorrower,
}

#[derive(Debug, Default)]
pub struct FrameAllocator {
    free: BTreeMap<u64, u64>,
    allocated: BTreeMap<u64, FrameRun>,
    generations: BTreeMap<u64, u64>,
    loans: BTreeMap<u64, u32>,
}

impl FrameAllocator {
    pub const PAGE_SIZE: u64 = 4096;

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_region(&mut self, base: u64, pages: u64) -> Result<(), FrameError> {
        validate_run(base, pages)?;
        let bytes = pages
            .checked_mul(Self::PAGE_SIZE)
            .ok_or(FrameError::RangeOverflow)?;
        let end = base.checked_add(bytes).ok_or(FrameError::RangeOverflow)?;
        if self.free.iter().any(|(other_base, other_pages)| {
            let other_end = other_base.saturating_add(other_pages.saturating_mul(Self::PAGE_SIZE));
            base < other_end && *other_base < end
        }) {
            return Err(FrameError::Overlap);
        }
        self.free.insert(base, pages);
        self.coalesce();
        Ok(())
    }

    pub fn allocate(&mut self, pages: u64, alignment_pages: u64) -> Result<FrameRun, FrameError> {
        if pages == 0 || alignment_pages == 0 || !alignment_pages.is_power_of_two() {
            return Err(FrameError::Empty);
        }
        let alignment = alignment_pages
            .checked_mul(Self::PAGE_SIZE)
            .ok_or(FrameError::RangeOverflow)?;
        let choice = self.free.iter().find_map(|(base, available)| {
            let aligned = base.checked_add(alignment - 1)? & !(alignment - 1);
            let prefix = aligned.checked_sub(*base)? / Self::PAGE_SIZE;
            (prefix.checked_add(pages)? <= *available)
                .then_some((*base, *available, aligned, prefix))
        });
        let Some((base, available, aligned, prefix)) = choice else {
            return Err(FrameError::OutOfMemory);
        };
        self.free.remove(&base);
        if prefix != 0 {
            self.free.insert(base, prefix);
        }
        let suffix = available - prefix - pages;
        if suffix != 0 {
            self.free.insert(aligned + pages * Self::PAGE_SIZE, suffix);
        }
        let generation = self.generations.get(&aligned).copied().unwrap_or(0);
        let run = FrameRun {
            base: aligned,
            pages,
            generation,
            zeroed: false,
        };
        self.allocated.insert(aligned, run);
        Ok(run)
    }

    pub fn mark_zeroed(&mut self, run: FrameRun) -> Result<FrameRun, FrameError> {
        let live = self.validate(run)?;
        let zeroed = FrameRun {
            zeroed: true,
            ..live
        };
        self.allocated.insert(run.base, zeroed);
        Ok(zeroed)
    }

    /// Consume one owned generation and lend the exact run to `borrower`.
    pub fn lend(&mut self, run: FrameRun, borrower: u32) -> Result<FrameRun, FrameError> {
        let live = self.validate(run)?;
        if self.loans.contains_key(&run.base) {
            return Err(FrameError::AlreadyLent);
        }
        let generation = live
            .generation
            .checked_add(1)
            .ok_or(FrameError::GenerationOverflow)?;
        let lent = FrameRun { generation, ..live };
        self.allocated.insert(run.base, lent);
        self.generations.insert(run.base, generation);
        self.loans.insert(run.base, borrower);
        Ok(lent)
    }

    /// Reclaim one loan only from the borrower recorded by `lend`.
    pub fn reclaim(&mut self, run: FrameRun, borrower: u32) -> Result<FrameRun, FrameError> {
        let live = self.validate(run)?;
        match self.loans.get(&run.base) {
            None => return Err(FrameError::NotLent),
            Some(owner) if *owner != borrower => return Err(FrameError::ForeignBorrower),
            Some(_) => {}
        }
        let generation = live
            .generation
            .checked_add(1)
            .ok_or(FrameError::GenerationOverflow)?;
        let reclaimed = FrameRun { generation, ..live };
        self.loans.remove(&run.base);
        self.generations.insert(run.base, generation);
        self.allocated.insert(run.base, reclaimed);
        Ok(reclaimed)
    }

    /// Replace one live allocator record with two exact adjacent records.
    pub fn split_allocated(
        &mut self,
        run: FrameRun,
        left_pages: u64,
    ) -> Result<(FrameRun, FrameRun), FrameError> {
        let live = self.validate(run)?;
        if self.loans.contains_key(&run.base) {
            return Err(FrameError::AlreadyLent);
        }
        let (left, right) = Self::split(live, left_pages)?;
        self.allocated.remove(&run.base);
        self.allocated.insert(left.base, left);
        self.allocated.insert(right.base, right);
        self.generations
            .entry(right.base)
            .or_insert(right.generation);
        Ok((left, right))
    }

    /// Replace two live adjacent records with their exact joined record.
    pub fn join_allocated(
        &mut self,
        left: FrameRun,
        right: FrameRun,
    ) -> Result<FrameRun, FrameError> {
        let left_live = self.validate(left)?;
        let right_live = self.validate(right)?;
        if self.loans.contains_key(&left.base) || self.loans.contains_key(&right.base) {
            return Err(FrameError::AlreadyLent);
        }
        let joined = Self::join(left_live, right_live)?;
        self.allocated.remove(&left.base);
        self.allocated.remove(&right.base);
        self.allocated.insert(joined.base, joined);
        Ok(joined)
    }

    pub fn release(&mut self, run: FrameRun) -> Result<(), FrameError> {
        self.validate(run)?;
        if self.loans.contains_key(&run.base) {
            return Err(FrameError::AlreadyLent);
        }
        self.allocated.remove(&run.base);
        let next = run
            .generation
            .checked_add(1)
            .ok_or(FrameError::GenerationOverflow)?;
        self.generations.insert(run.base, next);
        self.free.insert(run.base, run.pages);
        self.coalesce();
        Ok(())
    }

    pub fn split(run: FrameRun, left_pages: u64) -> Result<(FrameRun, FrameRun), FrameError> {
        if left_pages == 0 || left_pages >= run.pages {
            return Err(FrameError::Empty);
        }
        Ok((
            FrameRun {
                pages: left_pages,
                ..run
            },
            FrameRun {
                base: run.base + left_pages * Self::PAGE_SIZE,
                pages: run.pages - left_pages,
                ..run
            },
        ))
    }

    pub fn join(left: FrameRun, right: FrameRun) -> Result<FrameRun, FrameError> {
        if left.generation != right.generation
            || left.zeroed != right.zeroed
            || left.base + left.pages * Self::PAGE_SIZE != right.base
        {
            return Err(FrameError::NonAdjacent);
        }
        Ok(FrameRun {
            pages: left
                .pages
                .checked_add(right.pages)
                .ok_or(FrameError::RangeOverflow)?,
            ..left
        })
    }

    fn validate(&self, run: FrameRun) -> Result<FrameRun, FrameError> {
        let live = self
            .allocated
            .get(&run.base)
            .copied()
            .ok_or(FrameError::UnknownRun)?;
        if live.generation != run.generation {
            return Err(FrameError::StaleGeneration);
        }
        if live.pages != run.pages {
            return Err(FrameError::UnknownRun);
        }
        Ok(live)
    }

    fn coalesce(&mut self) {
        let entries: alloc::vec::Vec<_> = self.free.iter().map(|(a, b)| (*a, *b)).collect();
        self.free.clear();
        for (base, pages) in entries {
            if let Some((&previous_base, &previous_pages)) = self.free.iter().next_back() {
                if previous_base + previous_pages * Self::PAGE_SIZE == base {
                    self.free.insert(previous_base, previous_pages + pages);
                    continue;
                }
            }
            self.free.insert(base, pages);
        }
    }

    #[must_use]
    pub fn free_pages(&self) -> u64 {
        self.free.values().copied().sum()
    }
}

fn validate_run(base: u64, pages: u64) -> Result<(), FrameError> {
    if pages == 0 {
        return Err(FrameError::Empty);
    }
    if base % FrameAllocator::PAGE_SIZE != 0 {
        return Err(FrameError::Misaligned);
    }
    base.checked_add(
        pages
            .checked_mul(FrameAllocator::PAGE_SIZE)
            .ok_or(FrameError::RangeOverflow)?,
    )
    .ok_or(FrameError::RangeOverflow)?;
    Ok(())
}

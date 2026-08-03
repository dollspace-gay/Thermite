// Exact executable refinement for the sequentially-consistent u64 atomics used
// by the kernel acceptance slice. The executable operations below are the same
// vstd operations discharged by this Verus crate and linked into the image.

pub struct ExactAtomicInvariant;

impl vstd::atomic_ghost::AtomicInvariantPredicate<(), u64, Ghost<u64>> for ExactAtomicInvariant {
    open spec fn atomic_inv(_constant: (), value: u64, tracked_value: Ghost<u64>) -> bool {
        value == tracked_value@
    }
}

pub struct ExactAtomicU64 {
    inner: vstd::atomic_ghost::AtomicU64<(), Ghost<u64>, ExactAtomicInvariant>,
}

impl ExactAtomicU64 {
    #[verifier::type_invariant]
    pub(crate) closed spec fn exact_atomic_type_invariant(self) -> bool {
        self.inner.well_formed()
    }

    pub closed spec fn well_formed(&self) -> bool {
        self.inner.well_formed()
    }

    pub const fn new(value: u64) -> (result: Self)
        ensures
            result.well_formed(),
    {
        ExactAtomicU64 {
            inner: vstd::atomic_ghost::AtomicU64::new(
                Ghost(()),
                value,
                Tracked(Ghost(value)),
            ),
        }
    }

    pub fn load_seqcst(&self) -> (result: u64)
        requires
            self.well_formed(),
    {
        self.inner.load()
    }

    pub fn store_seqcst(&self, value: u64)
        requires
            self.well_formed(),
    {
        vstd::atomic_ghost::atomic_with_ghost!(
            &self.inner => store(value);
            update previous -> next;
            ghost tracked_value => {
                tracked_value = Ghost(next);
            }
        );
    }

    pub fn fetch_add_wrapping_seqcst(&self, value: u64) -> (previous: u64)
        requires
            self.well_formed(),
    {
        vstd::atomic_ghost::atomic_with_ghost!(
            &self.inner => fetch_add_wrapping(value);
            update old_value -> new_value;
            returning result;
            ghost tracked_value => {
                tracked_value = Ghost(new_value);
            }
        )
    }

    pub fn swap_seqcst(&self, value: u64) -> (previous: u64)
        requires
            self.well_formed(),
    {
        vstd::atomic_ghost::atomic_with_ghost!(
            &self.inner => swap(value);
            update old_value -> new_value;
            returning result;
            ghost tracked_value => {
                tracked_value = Ghost(new_value);
            }
        )
    }

    pub fn compare_exchange_seqcst(
        &self,
        current: u64,
        value: u64,
    ) -> (result: Result<u64, u64>)
        requires
            self.well_formed(),
        ensures
            match result {
                Result::Ok(previous) => previous == current,
                Result::Err(previous) => previous != current,
            },
    {
        vstd::atomic_ghost::atomic_with_ghost!(
            &self.inner => compare_exchange(current, value);
            update old_value -> new_value;
            returning exchange_result;
            ghost tracked_value => {
                tracked_value = Ghost(new_value);
            }
        )
    }

    pub fn compare_exchange_weak_seqcst(
        &self,
        current: u64,
        value: u64,
    ) -> (result: Result<u64, u64>)
        requires
            self.well_formed(),
    {
        vstd::atomic_ghost::atomic_with_ghost!(
            &self.inner => compare_exchange_weak(current, value);
            update old_value -> new_value;
            returning exchange_result;
            ghost tracked_value => {
                tracked_value = Ghost(new_value);
            }
        )
    }

    pub fn fetch_sub_wrapping_seqcst(&self, value: u64) -> (previous: u64)
        requires
            self.well_formed(),
    {
        vstd::atomic_ghost::atomic_with_ghost!(
            &self.inner => fetch_sub_wrapping(value);
            update old_value -> new_value;
            returning result;
            ghost tracked_value => {
                tracked_value = Ghost(new_value);
            }
        )
    }

    pub fn fetch_and_seqcst(&self, value: u64) -> (previous: u64)
        requires
            self.well_formed(),
    {
        vstd::atomic_ghost::atomic_with_ghost!(
            &self.inner => fetch_and(value);
            update old_value -> new_value;
            returning result;
            ghost tracked_value => {
                tracked_value = Ghost(new_value);
            }
        )
    }

    pub fn fetch_or_seqcst(&self, value: u64) -> (previous: u64)
        requires
            self.well_formed(),
    {
        vstd::atomic_ghost::atomic_with_ghost!(
            &self.inner => fetch_or(value);
            update old_value -> new_value;
            returning result;
            ghost tracked_value => {
                tracked_value = Ghost(new_value);
            }
        )
    }

    pub fn fetch_xor_seqcst(&self, value: u64) -> (previous: u64)
        requires
            self.well_formed(),
    {
        vstd::atomic_ghost::atomic_with_ghost!(
            &self.inner => fetch_xor(value);
            update old_value -> new_value;
            returning result;
            ghost tracked_value => {
                tracked_value = Ghost(new_value);
            }
        )
    }
}

pub fn establish_well_formed(cell: &ExactAtomicU64)
    ensures
        cell.well_formed(),
{
    proof {
        use_type_invariant(cell);
    }
}

#[no_mangle]
pub extern "C" fn tpl_atomic_load(
    cell: &ExactAtomicU64,
    _order: super::Ordering,
) -> (result: u64) {
    establish_well_formed(cell);
    cell.load_seqcst()
}

#[no_mangle]
pub extern "C" fn tpl_atomic_store(
    cell: &ExactAtomicU64,
    value: u64,
    _order: super::Ordering,
) {
    establish_well_formed(cell);
    cell.store_seqcst(value);
}

#[no_mangle]
pub extern "C" fn tpl_atomic_compare_exchange(
    cell: &ExactAtomicU64,
    current: u64,
    value: u64,
    _success: super::Ordering,
    _failure: super::Ordering,
) -> (result: super::Cas)
    ensures
        result.exchanged == (result.previous == current),
{
    establish_well_formed(cell);
    match cell.compare_exchange_seqcst(current, value) {
        Result::Ok(previous) => super::Cas {
            previous,
            exchanged: true,
        },
        Result::Err(previous) => super::Cas {
            previous,
            exchanged: false,
        },
    }
}

#[no_mangle]
pub extern "C" fn tpl_atomic_fetch(
    cell: &ExactAtomicU64,
    operation: super::FetchOp,
    value: u64,
    _order: super::Ordering,
) -> (result: u64) {
    establish_well_formed(cell);
    match operation {
        super::FetchOp::Add => cell.fetch_add_wrapping_seqcst(value),
        super::FetchOp::Sub => cell.fetch_sub_wrapping_seqcst(value),
        super::FetchOp::And => cell.fetch_and_seqcst(value),
        super::FetchOp::Or => cell.fetch_or_seqcst(value),
        super::FetchOp::Xor => cell.fetch_xor_seqcst(value),
    }
}

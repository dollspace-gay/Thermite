use vstd::prelude::*;
verus! {

spec fn spec_sum(xs: Seq<u32>) -> nat
    decreases xs.len()
{
    if xs.len() == 0 { 0 } else { xs[0] as nat + spec_sum(xs.drop_first()) }
}

proof fn lemma_sum_push(xs: Seq<u32>, k: int)
    requires 0 <= k < xs.len(),
    ensures spec_sum(xs.subrange(0, k + 1)) == spec_sum(xs.subrange(0, k)) + xs[k] as nat,
    decreases k,
{
    if k == 0 {
        assert(xs.subrange(0, 1).drop_first() =~= xs.subrange(0, 0));
    } else {
        lemma_sum_push(xs.drop_first(), k - 1);
        assert(xs.subrange(0, k + 1).drop_first() =~= xs.drop_first().subrange(0, k));
        assert(xs.subrange(0, k).drop_first() =~= xs.drop_first().subrange(0, k - 1));
    }
}

fn sum(xs: &[u32]) -> (result: u64)
    requires xs.len() <= 1_000_000,
    ensures
        result as nat == spec_sum(xs@),
        result <= xs.len() as u64 * u32::MAX as u64,
{
    let mut acc: u64 = 0;
    let mut i: usize = 0;
    while i < xs.len()
        invariant
            i <= xs.len(),
            xs.len() <= 1_000_000,
            acc as nat == spec_sum(xs@.subrange(0, i as int)),
            acc <= i as u64 * u32::MAX as u64,
        decreases xs.len() - i,
    {
        proof { lemma_sum_push(xs@, i as int); }
        assert(acc + xs[i as int] as u64 <= (i as u64 + 1) * u32::MAX as u64) by(nonlinear_arith)
            requires acc <= i as u64 * u32::MAX as u64, i < xs.len(), xs.len() <= 1_000_000;
        acc = acc + xs[i] as u64;
        i = i + 1;
    }
    assert(xs@.subrange(0, xs.len() as int) =~= xs@);
    acc
}

}
fn main() {}

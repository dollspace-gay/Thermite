// L3 Verus lowering of `conformance/vec_demo.th` — the bounded-collection rung
// (.design/basis/04-collections.md REQ-5, Basis Stage 4 / issue #73). Reference
// oracle for `thermite-lower::lower`'s `Vec<T>` wrapping: hand-authored from the
// design's GROUNDED `BVec`-over-`vstd::vec::Vec<u64>` form (R-CHAR-3 — never
// regenerated from the lowerer), and CONFIRMED to pass the real `verus` binary
// (verus 0.2026.05.24): `verus --no-cheating <this> => 4 verified, 0 errors`.
//
// A Thermite `Vec<u64>` lowers to the Thermite-runtime newtype `TVecU64` over
// `vstd::vec::Vec<u64>` carrying the capacity invariant `well_formed`
// (len() <= CAP), the spec `len`/`spec_get`, the no-OOB exec `get` (req i < len),
// and the capacity-preserving exec `push` (req well_formed && len < CAP) with the
// `final(self)` &mut postcondition (the design's recorded grounding finding that
// verus 0.2026.05.24 needs `final(self)`, not bare `self`, in a &mut ensures).
//
// `checked_get` → L3, fx pure: req i < v.len() discharges get's bound; the spec
// `v.get(i)` lowers to `v.spec_get(i as int)`. `push_one` → L3, fx alloc: req
// v.len() < CAP gives push's headroom; ens result.len() == v.len()+1 holds by
// push's spec. NO assume/external_body (R-DEFER-9); the non-vacuity reject
// (`bad`, a get without `req i < len`) FAILS verus (the no-OOB guarantee is real).

use vstd::prelude::*;
verus! {

pub struct TVecU64 { pub data: Vec<u64> }
impl TVecU64 {
    pub open spec fn well_formed(&self) -> bool { self.data.len() <= 1000000 }
    pub open spec fn len(&self) -> nat { self.data.len() as nat }
    pub open spec fn spec_get(&self, i: int) -> u64 { self.data@[i] }
    pub fn get(&self, i: usize) -> (result: u64)
        requires i < self.data.len(),
        ensures result == self.data@[i as int],
    { self.data[i] }
    pub fn push(&mut self, x: u64)
        requires old(self).well_formed(), old(self).data.len() < 1000000,
        ensures
            final(self).well_formed(),
            final(self).data.len() == old(self).data.len() + 1,
            final(self).data@[old(self).data.len() as int] == x,
    { self.data.push(x) }
}

fn checked_get(v: TVecU64, i: usize) -> (result: u64)
    requires i < v.len(),
    ensures
        result == v.spec_get(i as int),
{
    v.get(i)
}


fn push_one(v: TVecU64, x: u64) -> (result: TVecU64)
    requires v.len() < 1000000,
    ensures
        result.len() == v.len() + 1,
{
    let mut v2 = v;
    v2.push(x);
    v2
}


}
fn main() {}

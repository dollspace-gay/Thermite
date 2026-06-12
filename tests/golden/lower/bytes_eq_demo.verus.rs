// L3 Verus lowering of `conformance/bytes_eq_demo.th` — the C8 byte-range-equality
// content-pin rung (.design/basis/07-strings.md REQ-17..REQ-20, Basis Stage 7 /
// issue #278; the SECOND #276 prerequisite after #277's slice/concat byte-content
// ens). Reference oracle for `thermite-lower::lower`'s `bytes_eq` emission:
// hand-authored from the design's GROUNDED forms (R-CHAR-3 — never regenerated from
// the lowerer), and CONFIRMED to pass the real `verus` binary (verus 0.2026.05.24):
// `verus --no-cheating <this> => 18 verified, 0 errors`.
//
// `bytes_eq(a, b, ai, bi, n)` is a REGISTERED built-in spec predicate (REQ-17): the
// lowerer owns the canonical `Seq<u8>` LOW-PEEL recursion `__thermite_bytes_eq`
// (peel the leading byte, recurse ai+1/bi+1/n-1) AND ships the FOUR prove-once
// bridge lemmas beside it (REQ-18) — the `lemma_count_push`/`lemma_parse_push`
// precedent (a generated spec fn is only as usable as the induction lemma emitted
// with it). The four laws: the CORE INDUCTION `__thermite_lemma_bytes_eq_from_
// pointwise` (the explicit `#[trigger] a[ai + k]` is LOAD-BEARING — auto-inference
// fails on the arithmetic index), the cheap converse `__thermite_lemma_bytes_eq_to_
// pointwise`, the subrange corollary `__thermite_lemma_bytes_eq_from_subrange`
// (the #276 STOP's named minimum), and the no-arg quantified-equivalence
// `__thermite_lemma_bytes_eq_bridge` (the ONE-CALL citation form: its `=~=` plants
// the extensionality term so a single `proof { __thermite_lemma_bytes_eq_bridge(); }`
// body-start citation, REQ-19, discharges `slice_id` + all three `insert_str`
// conjuncts with ZERO per-conjunct glue — no append-window corollaries needed, the
// recorded grounding simplification). All emitted under the #130 reserved namespace
// (`__thermite_`) so a future user `spec fn bytes_eq` is a DISTINCT name.
//
// `slice_id(a) = a.slice(0, a.len())` (AC-13, the EXACT #276 counterexample) certifies
// L3: `ens bytes_eq(&result, &a, 0, 0, a.len())` lowers the `&result`/`&a` ref operands
// to their byte `Seq<u8>` views (`result.data@`/`a.data@`) and the `0`/`0`/`a.len()`
// index args `as int`; the body-start citation discharges the content pin over the
// #277 subrange ens. `insert_str` (AC-14) is the editor's three-conjunct splice
// (`head.concat(ins).concat(tail)` over `slice(0, cursor)`/`slice(cursor, len)`) — the
// unchanged-prefix / inserted-run / shifted-suffix windows EACH pinned by one bytes_eq
// conjunct, all discharged by the single citation. Both carry `fx alloc` (constructing).
// NO assume/external_body/admit (R-DEFER-9); the four lemmas are REAL induction proofs.
// The non-vacuity reject (the length-preserving head/tail-SWAP of insert_str's body)
// FAILS verus (17 verified, 1 errors, postcondition not satisfied) — the content pins
// are real teeth a length pin cannot fake (AC-16).
//
// `buf_prefix_pin(b: Buf)` exercises the FIELD-ACCESS operand shape (#279): the editor
// wraps its `String` in a `Buf { text: String }` ADT, so its bytes_eq pins name
// `result.text`/`b.text` (field accesses, NOT bare paths). The byte-view rewrite
// (`lower_spec_arg`'s `byteview_string_operand`) lowers `&result.text` →
// `result.text.data@` and `&b.text` → `b.text.data@` — without the field arm a
// field-access operand emitted `&result.text` (a `&TString`) against bytes_eq's
// `Seq<u8>` params (E0308, the #279 STOP). The `Buf` invariant (`cursor <= text.len()
// && text.len() <= CAP`) is woven into the fn's requires/ensures, so the slice's
// well_formed precondition discharges; certifies L3.
use vstd::prelude::*;
verus! {

pub struct TString { pub data: Vec<u8> }
impl TString {
    pub open spec fn well_formed(&self) -> bool { self.data.len() <= 1000000 }
    pub open spec fn spec_len(&self) -> nat { self.data.len() as nat }
    pub fn len(&self) -> (result: u64)
        ensures result == self.data.len(),
    { self.data.len() as u64 }
    pub open spec fn spec_byte_at(&self, i: int) -> u64 { self.data@[i] as u64 }
    pub fn byte_at(&self, i: usize) -> (result: u64)
        requires i < self.data.len(),
        ensures result == self.data@[i as int],
    { self.data[i] as u64 }
    pub fn concat(&self, b: TString) -> (result: TString)
        requires self.well_formed(), b.well_formed(),
                 self.data.len() + b.data.len() <= 1000000,
        ensures result.well_formed(),
                result.data.len() == self.data.len() + b.data.len(),
                result.data@ == self.data@ + b.data@,
    {
        let mut out: Vec<u8> = Vec::new();
        let mut i: usize = 0;
        while i < self.data.len()
            invariant i <= self.data.len(), out.len() == i,
                      self.data.len() + b.data.len() <= 1000000,
                      out@ == self.data@.subrange(0, i as int),
            decreases self.data.len() - i,
        {
            let ghost old_out = out@;
            out.push(self.data[i]);
            assert(out@ =~= self.data@.subrange(0, (i + 1) as int)) by {
                assert(self.data@.subrange(0, (i + 1) as int) =~= self.data@.subrange(0, i as int).push(self.data@[i as int]));
            }
            i = i + 1;
        }
        assert(out@ =~= self.data@) by {
            assert(self.data@.subrange(0, i as int) =~= self.data@);
        }
        let mut j: usize = 0;
        while j < b.data.len()
            invariant j <= b.data.len(), out.len() == self.data.len() + j,
                      self.data.len() + b.data.len() <= 1000000,
                      out@ == self.data@ + b.data@.subrange(0, j as int),
            decreases b.data.len() - j,
        {
            let ghost old_out = out@;
            out.push(b.data[j]);
            assert(out@ =~= self.data@ + b.data@.subrange(0, (j + 1) as int)) by {
                assert(b.data@.subrange(0, (j + 1) as int) =~= b.data@.subrange(0, j as int).push(b.data@[j as int]));
            }
            j = j + 1;
        }
        assert(out@ =~= self.data@ + b.data@) by {
            assert(b.data@.subrange(0, j as int) =~= b.data@);
        }
        TString { data: out }
    }
    pub fn slice(&self, lo: usize, hi: usize) -> (result: TString)
        requires self.well_formed(), lo <= hi, hi <= self.data.len(),
        ensures result.well_formed(), result.data.len() == hi - lo,
                result.data@ == self.data@.subrange(lo as int, hi as int),
    {
        let mut out: Vec<u8> = Vec::new();
        let mut i: usize = lo;
        while i < hi
            invariant lo <= i, i <= hi, hi <= self.data.len(), self.data.len() <= 1000000, out.len() == i - lo,
                      out@ == self.data@.subrange(lo as int, i as int),
            decreases hi - i,
        {
            let ghost old_out = out@;
            out.push(self.data[i]);
            assert(out@ =~= self.data@.subrange(lo as int, (i + 1) as int)) by {
                assert(self.data@.subrange(lo as int, (i + 1) as int) =~= self.data@.subrange(lo as int, i as int).push(self.data@[i as int]));
            }
            i = i + 1;
        }
        assert(out@ == self.data@.subrange(lo as int, hi as int));
        TString { data: out }
    }
    pub fn from_byte(b: u64) -> (result: TString)
        ensures result.well_formed(), result.data.len() == 1,
                result.data@[0] == b as u8,
    {
        let mut data: Vec<u8> = Vec::new();
        data.push(b as u8);
        TString { data }
    }
    pub fn push_byte(&self, b: u64) -> (result: TString)
        requires self.well_formed(), self.data.len() < 1000000,
        ensures result.well_formed(),
                result.data.len() == self.data.len() + 1,
                result.data@[self.data.len() as int] == b as u8,
                forall|j: int| 0 <= j < self.data.len()
                    ==> result.data@[j] == self.data@[j],
    {
        let mut out: Vec<u8> = Vec::new();
        let mut i: usize = 0;
        while i < self.data.len()
            invariant i <= self.data.len(), out.len() == i,
                      self.data.len() < 1000000,
                      forall|j: int| 0 <= j < i ==> #[trigger] out@[j] == self.data@[j],
            decreases self.data.len() - i,
        { out.push(self.data[i]); i = i + 1; }
        out.push(b as u8);
        TString { data: out }
    }
}

pub open spec fn __thermite_bytes_eq(a: Seq<u8>, b: Seq<u8>, ai: int, bi: int, n: int) -> bool
    decreases n
{
    if n <= 0 { true } else { a[ai] == b[bi] && __thermite_bytes_eq(a, b, ai + 1, bi + 1, n - 1) }
}
pub proof fn __thermite_lemma_bytes_eq_from_pointwise(a: Seq<u8>, b: Seq<u8>, ai: int, bi: int, n: int)
    requires forall|k: int| 0 <= k < n ==> #[trigger] a[ai + k] == b[bi + k],
    ensures __thermite_bytes_eq(a, b, ai, bi, n),
    decreases n
{
    if n > 0 {
        assert(a[ai] == b[bi]) by { assert(a[ai + 0] == b[bi + 0]); }
        assert forall|k: int| 0 <= k < n - 1 implies #[trigger] a[(ai + 1) + k] == b[(bi + 1) + k] by {
            assert(a[ai + (k + 1)] == b[bi + (k + 1)]);
        }
        __thermite_lemma_bytes_eq_from_pointwise(a, b, ai + 1, bi + 1, n - 1);
    }
}
pub proof fn __thermite_lemma_bytes_eq_to_pointwise(a: Seq<u8>, b: Seq<u8>, ai: int, bi: int, n: int)
    requires __thermite_bytes_eq(a, b, ai, bi, n),
    ensures forall|k: int| 0 <= k < n ==> #[trigger] a[ai + k] == b[bi + k],
    decreases n
{
    if n > 0 {
        __thermite_lemma_bytes_eq_to_pointwise(a, b, ai + 1, bi + 1, n - 1);
        assert forall|k: int| 0 <= k < n implies #[trigger] a[ai + k] == b[bi + k] by {
            if k == 0 { assert(a[ai] == b[bi]); }
            else { assert(a[(ai + 1) + (k - 1)] == b[(bi + 1) + (k - 1)]); }
        }
    }
}
pub proof fn __thermite_lemma_bytes_eq_from_subrange(a: Seq<u8>, b: Seq<u8>, ai: int, bi: int, n: int)
    requires 0 <= ai, 0 <= bi, 0 <= n, ai + n <= a.len(), bi + n <= b.len(),
             a.subrange(ai, ai + n) == b.subrange(bi, bi + n),
    ensures __thermite_bytes_eq(a, b, ai, bi, n),
{
    assert forall|k: int| 0 <= k < n implies #[trigger] a[ai + k] == b[bi + k] by {
        assert(a.subrange(ai, ai + n)[k] == a[ai + k]);
        assert(b.subrange(bi, bi + n)[k] == b[bi + k]);
        assert(a.subrange(ai, ai + n)[k] == b.subrange(bi, bi + n)[k]);
    }
    __thermite_lemma_bytes_eq_from_pointwise(a, b, ai, bi, n);
}
pub proof fn __thermite_lemma_bytes_eq_bridge()
    ensures forall|a: Seq<u8>, b: Seq<u8>, ai: int, bi: int, n: int|
        0 <= ai && 0 <= bi && 0 <= n && ai + n <= a.len() && bi + n <= b.len()
        ==> (#[trigger] __thermite_bytes_eq(a, b, ai, bi, n)
             <==> a.subrange(ai, ai + n) =~= b.subrange(bi, bi + n)),
{
    assert forall|a: Seq<u8>, b: Seq<u8>, ai: int, bi: int, n: int|
        0 <= ai && 0 <= bi && 0 <= n && ai + n <= a.len() && bi + n <= b.len()
        implies (#[trigger] __thermite_bytes_eq(a, b, ai, bi, n)
             <==> a.subrange(ai, ai + n) =~= b.subrange(bi, bi + n)) by {
        if __thermite_bytes_eq(a, b, ai, bi, n) {
            __thermite_lemma_bytes_eq_to_pointwise(a, b, ai, bi, n);
            assert(a.subrange(ai, ai + n) =~= b.subrange(bi, bi + n)) by {
                assert forall|k: int| 0 <= k < n implies
                    #[trigger] a.subrange(ai, ai + n)[k] == b.subrange(bi, bi + n)[k] by {
                    assert(a.subrange(ai, ai + n)[k] == a[ai + k]);
                    assert(b.subrange(bi, bi + n)[k] == b[bi + k]);
                }
            }
        }
        if a.subrange(ai, ai + n) =~= b.subrange(bi, bi + n) {
            __thermite_lemma_bytes_eq_from_subrange(a, b, ai, bi, n);
        }
    }
}

fn slice_id(a: &TString) -> (result: TString)
    requires a.spec_len() <= 1000000,
    ensures
        result.spec_len() == a.spec_len(),
        __thermite_bytes_eq(result.data@, a.data@, 0 as int, 0 as int, a.spec_len() as int),
{
    proof { __thermite_lemma_bytes_eq_bridge(); }
    a.slice(0, a.len() as usize)
}


fn insert_str(text: &TString, ins: &TString, cursor: u64) -> (result: TString)
    requires cursor <= text.spec_len() && text.spec_len() + ins.spec_len() <= 1000000,
    ensures
        __thermite_bytes_eq(result.data@, text.data@, 0 as int, 0 as int, cursor as int),
        __thermite_bytes_eq(result.data@, ins.data@, cursor as int, 0 as int, ins.spec_len() as int),
        __thermite_bytes_eq(result.data@, text.data@, (cursor + ins.spec_len()) as int, cursor as int, (text.spec_len() - cursor) as int),
{
    proof { __thermite_lemma_bytes_eq_bridge(); }
    text.slice(0, cursor as usize).concat(ins.slice(0, ins.len() as usize)).concat(text.slice(cursor as usize, text.len() as usize))
}


pub struct Buf {
    pub text: TString,
    pub cursor: u64,
}

impl Buf {
    pub open spec fn well_formed(&self) -> bool {
        self.cursor <= self.text.spec_len() && self.text.spec_len() <= 1000000
    }
}


fn buf_prefix_pin(b: Buf) -> (result: Buf)
    requires
        b.well_formed(),
    ensures
        result.well_formed(),
        __thermite_bytes_eq(result.text.data@, b.text.data@, 0 as int, 0 as int, b.cursor as int),
{
    proof { __thermite_lemma_bytes_eq_bridge(); }
    Buf { text: b.text.slice(0, b.text.len() as usize), cursor: b.cursor }
}


}
fn main() {}

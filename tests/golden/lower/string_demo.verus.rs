// L3 Verus lowering of `conformance/string_demo.th` — the bounded-String rung
// (.design/basis/07-strings.md REQ-1/REQ-4, Basis Stage 7 / issue #79). Reference
// oracle for `thermite-lower::lower`'s `String` wrapping: hand-authored from the
// design's GROUNDED `TString`-over-`vstd::vec::Vec<u8>` form (R-CHAR-3 — never
// regenerated from the lowerer), and CONFIRMED to pass the real `verus` binary
// (verus 0.2026.05.24): `verus --no-cheating <this> => 11 verified, 0 errors`.
//
// A Thermite `String` lowers to the Thermite-runtime newtype `TString` over
// `vstd::vec::Vec<u8>` (the char model is bytes / u8 for v1) carrying the capacity
// invariant `well_formed` (len() <= CAP), the spec `spec_len` (the exec `len`
// returns u64 and cannot be named in a contract, so a contract names `spec_len`),
// the no-OOB exec `byte_at` (req i < len, returning u64 — the corpus oracle's
// `first_byte -> u64`), the bounded constructing `concat` (a two-loop append, req
// len_a + len_b <= CAP, ens result.len() == len_a + len_b), and the bounded
// `slice` (req self.well_formed() && lo <= hi && hi <= len, ens result.len() ==
// hi - lo). The wrapper REUSES the Stage-4 bounded-Vec machinery applied to u8.
//
// `greeting_len` → L3, fx pure: `s.len()` over a String; ens result == s.spec_len()
// (the exec `len` returns u64, spec compares u64 == nat). `first_byte` → L3, fx
// pure: req s.len() > 0 discharges byte_at(0)'s bound; ens result == s.byte_at(0)
// (the no-OOB accessor). `join` → L3, fx alloc: the constructing concat. `literal_len`
// → L3, fx alloc: the string literal "hello" materializes an owned TString by
// byte-push (104,101,108,108,111) and len() == 5 (the Expr::StrLit surface
// end-to-end). NO assume/external_body (R-DEFER-9); the non-vacuity reject
// (`oob_byte_at_no_req`, a byte_at without `req s.len() > 0`) FAILS verus (2
// verified, 1 errors, `failed precondition`) — the no-OOB guarantee is real.
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
    {
        let mut out: Vec<u8> = Vec::new();
        let mut i: usize = 0;
        while i < self.data.len()
            invariant i <= self.data.len(), out.len() == i,
                      self.data.len() + b.data.len() <= 1000000,
            decreases self.data.len() - i,
        { out.push(self.data[i]); i = i + 1; }
        let mut j: usize = 0;
        while j < b.data.len()
            invariant j <= b.data.len(), out.len() == self.data.len() + j,
                      self.data.len() + b.data.len() <= 1000000,
            decreases b.data.len() - j,
        { out.push(b.data[j]); j = j + 1; }
        TString { data: out }
    }
    pub fn slice(&self, lo: usize, hi: usize) -> (result: TString)
        requires self.well_formed(), lo <= hi, hi <= self.data.len(),
        ensures result.well_formed(), result.data.len() == hi - lo,
    {
        let mut out: Vec<u8> = Vec::new();
        let mut i: usize = lo;
        while i < hi
            invariant lo <= i, i <= hi, hi <= self.data.len(), self.data.len() <= 1000000, out.len() == i - lo,
            decreases hi - i,
        { out.push(self.data[i]); i = i + 1; }
        TString { data: out }
    }
}

fn greeting_len(s: TString) -> (result: u64)
    ensures result == s.spec_len(),
{ s.len() }

fn first_byte(s: TString) -> (result: u64)
    requires s.spec_len() > 0,
    ensures result == s.spec_byte_at(0),
{ s.byte_at(0) }

fn join(a: TString, b: TString) -> (result: TString)
    requires a.spec_len() + b.spec_len() <= 1000000,
    ensures result.spec_len() == a.spec_len() + b.spec_len(),
{ a.concat(b) }

fn literal_len() -> (result: u64)
    ensures result == 5,
{ ({ let mut data: Vec<u8> = Vec::new(); data.push(104u8); data.push(101u8); data.push(108u8); data.push(108u8); data.push(111u8); TString { data } }).len() }

}
fn main() {}

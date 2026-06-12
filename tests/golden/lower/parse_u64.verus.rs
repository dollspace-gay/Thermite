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

pub open spec fn __thermite_pow10(k: nat) -> nat
    decreases k
{ if k == 0 { 1 } else { 10 * __thermite_pow10((k - 1) as nat) } }
pub open spec fn __thermite_parse_le(s: Seq<u8>) -> nat
    decreases s.len()
{ if s.len() == 0 { 0 }
  else { ((s[0] - 48) as nat) + 10 * __thermite_parse_le(s.subrange(1, s.len() as int)) } }
pub open spec fn __thermite_parse_be(s: Seq<u8>) -> nat
    decreases s.len()
{ if s.len() == 0 { 0 }
  else { __thermite_parse_be(s.subrange(0, (s.len() - 1) as int)) * 10 + ((s[(s.len() - 1) as int] - 48) as nat) } }
pub open spec fn __thermite_seq_reverse(s: Seq<u8>) -> Seq<u8>
    decreases s.len()
{ if s.len() == 0 { Seq::<u8>::empty() }
  else { __thermite_seq_reverse(s.subrange(1, s.len() as int)).push(s[0]) } }

proof fn __thermite_lemma_parse_push(s: Seq<u8>, d: u8)
    ensures __thermite_parse_le(s.push(d)) == __thermite_parse_le(s) + ((d - 48) as nat) * __thermite_pow10(s.len()),
    decreases s.len(),
{
    let sd = s.push(d);
    if s.len() == 0 {
        assert(sd.len() == 1);
        assert(sd[0] == d);
        assert(sd.subrange(1, sd.len() as int) =~= Seq::<u8>::empty());
        assert(__thermite_parse_le(sd.subrange(1, sd.len() as int)) == 0);
        assert(__thermite_parse_le(sd) == ((d - 48) as nat));
        assert(__thermite_parse_le(s) == 0);
        assert(__thermite_pow10(0) == 1);
        assert(((d - 48) as nat) * __thermite_pow10(0) == ((d - 48) as nat)) by(nonlinear_arith);
        assert(__thermite_parse_le(sd) == __thermite_parse_le(s) + ((d - 48) as nat) * __thermite_pow10(s.len()));
    } else {
        let t = s.subrange(1, s.len() as int);
        __thermite_lemma_parse_push(t, d);
        assert(sd.len() == s.len() + 1);
        assert(sd[0] == s[0]);
        assert(sd.subrange(1, sd.len() as int) =~= t.push(d));
        assert(t.len() == s.len() - 1);
        assert(sd.subrange(1, sd.len() as int) == t.push(d));
        assert(__thermite_parse_le(sd) == ((sd[0] - 48) as nat) + 10 * __thermite_parse_le(sd.subrange(1, sd.len() as int)));
        assert(__thermite_parse_le(sd) == ((s[0] - 48) as nat) + 10 * __thermite_parse_le(t.push(d)));
        assert(__thermite_parse_le(s) == ((s[0] - 48) as nat) + 10 * __thermite_parse_le(t));
        assert(__thermite_pow10(s.len()) == 10 * __thermite_pow10(t.len()));
        assert(10 * (((d - 48) as nat) * __thermite_pow10(t.len())) == ((d - 48) as nat) * __thermite_pow10(s.len()))
            by(nonlinear_arith)
            requires __thermite_pow10(s.len()) == 10 * __thermite_pow10(t.len());
        assert(10 * (__thermite_parse_le(t) + ((d - 48) as nat) * __thermite_pow10(t.len()))
            == 10 * __thermite_parse_le(t) + 10 * (((d - 48) as nat) * __thermite_pow10(t.len()))) by(nonlinear_arith);
        assert(__thermite_parse_le(t.push(d)) == __thermite_parse_le(t) + ((d - 48) as nat) * __thermite_pow10(t.len()));
        assert(10 * __thermite_parse_le(t.push(d))
            == 10 * __thermite_parse_le(t) + ((d - 48) as nat) * __thermite_pow10(s.len()));
        assert(__thermite_parse_le(sd) == __thermite_parse_le(s) + ((d - 48) as nat) * __thermite_pow10(s.len()));
    }
}

proof fn __thermite_lemma_parse_be_push(s: Seq<u8>, d: u8)
    ensures __thermite_parse_be(s.push(d)) == __thermite_parse_be(s) * 10 + ((d - 48) as nat),
{
    let sd = s.push(d);
    assert(sd.len() == s.len() + 1);
    assert(sd[(sd.len() - 1) as int] == d);
    assert(sd.subrange(0, (sd.len() - 1) as int) =~= s);
}

proof fn __thermite_lemma_parse_be_reverse(s: Seq<u8>)
    ensures __thermite_parse_be(__thermite_seq_reverse(s)) == __thermite_parse_le(s),
    decreases s.len(),
{
    if s.len() == 0 {
        assert(__thermite_seq_reverse(s) =~= Seq::<u8>::empty());
    } else {
        let t = s.subrange(1, s.len() as int);
        __thermite_lemma_parse_be_reverse(t);
        __thermite_lemma_parse_be_push(__thermite_seq_reverse(t), s[0]);
    }
}

proof fn __thermite_lemma_pow10_le(a: nat, b: nat)
    requires a <= b,
    ensures __thermite_pow10(a) <= __thermite_pow10(b),
    decreases b,
{
    if a < b {
        __thermite_lemma_pow10_le(a, (b - 1) as nat);
        assert(__thermite_pow10(b) == 10 * __thermite_pow10((b - 1) as nat));
        assert(__thermite_pow10((b - 1) as nat) <= 10 * __thermite_pow10((b - 1) as nat)) by(nonlinear_arith);
    }
}

proof fn __thermite_lemma_pow10_20_gt_u64max()
    ensures __thermite_pow10(20) > u64::MAX as nat,
{
    reveal_with_fuel(__thermite_pow10, 21);
    assert(__thermite_pow10(20) == 100_000_000_000_000_000_000nat) by(compute);
}

pub fn __thermite_u64_to_string(n: u64) -> (result: TString)
    ensures
        __thermite_parse_be(result.data@) == n as nat,
        result.data.len() >= 1,
        result.data.len() <= 20,
{
    let mut data: Vec<u8> = Vec::new();
    let mut m: u64 = n;
    proof {
        assert(data@ =~= Seq::<u8>::empty());
        assert(__thermite_parse_le(data@) == 0);
        assert(__thermite_pow10(0) == 1);
        assert((n as nat) * __thermite_pow10(0) == n as nat) by(nonlinear_arith);
    }
    if m == 0 {
        data.push(48u8);
        proof {
            assert(data@.len() == 1);
            assert(data@[0] == 48u8);
            assert(data@.subrange(1, data@.len() as int) =~= Seq::<u8>::empty());
            assert(__thermite_parse_le(data@.subrange(1, data@.len() as int)) == 0);
            assert(__thermite_parse_le(data@) == 0);
            assert((m as nat) == 0);
            assert((m as nat) * __thermite_pow10(data.len() as nat) == 0) by(nonlinear_arith)
                requires (m as nat) == 0;
        }
    }
    while m > 0
        invariant
            __thermite_parse_le(data@) + (m as nat) * __thermite_pow10(data.len() as nat) == n as nat,
            data.len() >= 1 || m > 0,
            data.len() <= 20,
        decreases m,
    {
        let d: u8 = (m % 10) as u8 + 48u8;
        let ghost old_data = data@;
        let ghost old_m = m as nat;
        let ghost old_len = data.len() as nat;
        proof {
            if data.len() == 20 {
                __thermite_lemma_pow10_20_gt_u64max();
                assert(__thermite_pow10(20) <= (m as nat) * __thermite_pow10(20)) by(nonlinear_arith)
                    requires (m as nat) >= 1;
                assert((m as nat) * __thermite_pow10(data.len() as nat) <= n as nat);
                assert(false);
            }
        }
        data.push(d);
        proof {
            __thermite_lemma_parse_push(old_data, d);
            assert((m as nat) == 10 * ((m / 10) as nat) + ((m % 10) as nat)) by(nonlinear_arith);
            assert(__thermite_pow10((old_len + 1) as nat) == 10 * __thermite_pow10(old_len));
        }
        m = m / 10;
        proof {
            assert(old_m * __thermite_pow10(old_len)
                == ((d - 48) as nat) * __thermite_pow10(old_len) + (m as nat) * __thermite_pow10((old_len + 1) as nat))
                by(nonlinear_arith)
                requires
                    old_m == 10 * (m as nat) + ((d - 48) as nat),
                    __thermite_pow10((old_len + 1) as nat) == 10 * __thermite_pow10(old_len);
        }
    }
    assert(data.len() >= 1);
    assert(data.len() <= 20);
    let mut out: Vec<u8> = Vec::new();
    let mut i: usize = 0;
    while i < data.len()
        invariant
            i <= data.len(),
            data.len() <= 20,
            out.len() == i,
            out@ =~= __thermite_seq_reverse(data@.subrange((data.len() - i) as int, data.len() as int)),
        decreases data.len() - i,
    {
        let ghost prefix = data@.subrange((data.len() - i) as int, data.len() as int);
        out.push(data[data.len() - 1 - i]);
        i = i + 1;
        proof {
            let lo = (data.len() - i) as int;
            let whole = data@.subrange(lo, data@.len() as int);
            assert(whole.len() > 0);
            assert(whole[0] == data@[lo]);
            assert(whole.subrange(1, whole.len() as int) =~= prefix);
            assert(__thermite_seq_reverse(whole) =~= __thermite_seq_reverse(prefix).push(data@[lo]));
        }
    }
    proof {
        assert(data@.subrange(0, data@.len() as int) =~= data@);
        __thermite_lemma_parse_be_reverse(data@);
    }
    TString { data: out }
}

pub open spec fn __thermite_is_digit(b: u8) -> bool { 48 <= b && b <= 57 }
pub open spec fn __thermite_all_digits(s: Seq<u8>) -> bool
{ forall|i: int| 0 <= i < s.len() ==> __thermite_is_digit(#[trigger] s[i]) }

proof fn __thermite_lemma_parse_be_prefix_le(s: Seq<u8>, k: int)
    requires 0 <= k <= s.len(),
    ensures __thermite_parse_be(s.subrange(0, k)) <= __thermite_parse_be(s),
    decreases s.len() - k,
{
    if k == s.len() {
        assert(s.subrange(0, k) =~= s);
    } else {
        let m = (s.len() - 1) as int;
        assert(s.subrange(0, m).subrange(0, k) =~= s.subrange(0, k));
        __thermite_lemma_parse_be_prefix_le(s.subrange(0, m), k);
        assert(__thermite_parse_be(s) == __thermite_parse_be(s.subrange(0, m)) * 10 + ((s[m] - 48) as nat));
        assert(__thermite_parse_be(s.subrange(0, m)) * 10 >= __thermite_parse_be(s.subrange(0, m))) by(nonlinear_arith);
    }
}

pub fn __thermite_parse_u64(s: &TString) -> (result: Option<u64>)
    ensures
        (__thermite_all_digits(s.data@) && s.data.len() >= 1 && __thermite_parse_be(s.data@) <= u64::MAX) ==> result is Some,
        match result {
            Some(v) => __thermite_all_digits(s.data@) && s.data.len() >= 1 && __thermite_parse_be(s.data@) == v as nat,
            None => true,
        },
        result is None ==> (!__thermite_all_digits(s.data@) || s.data.len() == 0 || __thermite_parse_be(s.data@) > u64::MAX),
{
    if s.data.len() == 0 { return None; }
    let mut acc: u64 = 0;
    let mut i: usize = 0;
    while i < s.data.len()
        invariant
            i <= s.data.len(),
            __thermite_all_digits(s.data@.subrange(0, i as int)),
            __thermite_parse_be(s.data@.subrange(0, i as int)) == acc as nat,
        decreases s.data.len() - i,
    {
        let b: u8 = s.data[i];
        if b < 48 || b > 57 {
            assert(!__thermite_is_digit(s.data@[i as int]));
            assert(!__thermite_all_digits(s.data@));
            return None;
        }
        let digit: u64 = (b - 48) as u64;
        let ghost old_i = i as int;
        assert(s.data@.subrange(0, (i + 1) as int).subrange(0, old_i) =~= s.data@.subrange(0, old_i));
        assert(s.data@.subrange(0, (i + 1) as int)[old_i] == b);
        assert(__thermite_parse_be(s.data@.subrange(0, (i + 1) as int)) == __thermite_parse_be(s.data@.subrange(0, old_i)) * 10 + ((b - 48) as nat));
        if acc > (u64::MAX - digit) / 10 {
            proof {
                assert(digit <= 9);
                assert((acc as nat) * 10 + (digit as nat) > u64::MAX as nat) by(nonlinear_arith)
                    requires acc as nat > ((u64::MAX - digit) / 10) as nat, digit <= 9;
                assert(__thermite_parse_be(s.data@.subrange(0, (i + 1) as int)) > u64::MAX);
                if __thermite_all_digits(s.data@) {
                    __thermite_lemma_parse_be_prefix_le(s.data@, (i + 1) as int);
                }
            }
            return None;
        }
        acc = acc * 10 + digit;
        i = i + 1;
    }
    assert(s.data@.subrange(0, i as int) =~= s.data@);
    Some(acc)
}

fn parse_valid(s: TString) -> (result: Option<u64>)
    requires __thermite_all_digits(s.data@) && s.spec_len() >= 1 && __thermite_parse_be(s.data@) <= 18446744073709551615,
    ensures
        (result is Some),
        match result {
            Some(v) => v as nat == __thermite_parse_be(s.data@),
            None => true,
        },
{
    __thermite_parse_u64(&s)
}


fn parse_rejects_nondigit(s: TString) -> (result: Option<u64>)
    requires s.spec_len() >= 1 && !__thermite_all_digits(s.data@),
    ensures
        match result {
            Some(v) => false,
            None => true,
        },
{
    __thermite_parse_u64(&s)
}


}
fn main() {}

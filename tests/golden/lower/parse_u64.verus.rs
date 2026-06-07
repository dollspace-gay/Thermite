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

pub open spec fn pow10(k: nat) -> nat
    decreases k
{ if k == 0 { 1 } else { 10 * pow10((k - 1) as nat) } }
pub open spec fn parse_le(s: Seq<u8>) -> nat
    decreases s.len()
{ if s.len() == 0 { 0 }
  else { ((s[0] - 48) as nat) + 10 * parse_le(s.subrange(1, s.len() as int)) } }
pub open spec fn parse_be(s: Seq<u8>) -> nat
    decreases s.len()
{ if s.len() == 0 { 0 }
  else { parse_be(s.subrange(0, (s.len() - 1) as int)) * 10 + ((s[(s.len() - 1) as int] - 48) as nat) } }
pub open spec fn seq_reverse(s: Seq<u8>) -> Seq<u8>
    decreases s.len()
{ if s.len() == 0 { Seq::<u8>::empty() }
  else { seq_reverse(s.subrange(1, s.len() as int)).push(s[0]) } }

proof fn lemma_parse_push(s: Seq<u8>, d: u8)
    ensures parse_le(s.push(d)) == parse_le(s) + ((d - 48) as nat) * pow10(s.len()),
    decreases s.len(),
{
    let sd = s.push(d);
    if s.len() == 0 {
        assert(sd.len() == 1);
        assert(sd[0] == d);
        assert(sd.subrange(1, sd.len() as int) =~= Seq::<u8>::empty());
        assert(parse_le(sd.subrange(1, sd.len() as int)) == 0);
        assert(parse_le(sd) == ((d - 48) as nat));
        assert(parse_le(s) == 0);
        assert(pow10(0) == 1);
        assert(((d - 48) as nat) * pow10(0) == ((d - 48) as nat)) by(nonlinear_arith);
        assert(parse_le(sd) == parse_le(s) + ((d - 48) as nat) * pow10(s.len()));
    } else {
        let t = s.subrange(1, s.len() as int);
        lemma_parse_push(t, d);
        assert(sd.len() == s.len() + 1);
        assert(sd[0] == s[0]);
        assert(sd.subrange(1, sd.len() as int) =~= t.push(d));
        assert(t.len() == s.len() - 1);
        assert(sd.subrange(1, sd.len() as int) == t.push(d));
        assert(parse_le(sd) == ((sd[0] - 48) as nat) + 10 * parse_le(sd.subrange(1, sd.len() as int)));
        assert(parse_le(sd) == ((s[0] - 48) as nat) + 10 * parse_le(t.push(d)));
        assert(parse_le(s) == ((s[0] - 48) as nat) + 10 * parse_le(t));
        assert(pow10(s.len()) == 10 * pow10(t.len()));
        assert(10 * (((d - 48) as nat) * pow10(t.len())) == ((d - 48) as nat) * pow10(s.len()))
            by(nonlinear_arith)
            requires pow10(s.len()) == 10 * pow10(t.len());
        assert(10 * (parse_le(t) + ((d - 48) as nat) * pow10(t.len()))
            == 10 * parse_le(t) + 10 * (((d - 48) as nat) * pow10(t.len()))) by(nonlinear_arith);
        assert(parse_le(t.push(d)) == parse_le(t) + ((d - 48) as nat) * pow10(t.len()));
        assert(10 * parse_le(t.push(d))
            == 10 * parse_le(t) + ((d - 48) as nat) * pow10(s.len()));
        assert(parse_le(sd) == parse_le(s) + ((d - 48) as nat) * pow10(s.len()));
    }
}

proof fn lemma_parse_be_push(s: Seq<u8>, d: u8)
    ensures parse_be(s.push(d)) == parse_be(s) * 10 + ((d - 48) as nat),
{
    let sd = s.push(d);
    assert(sd.len() == s.len() + 1);
    assert(sd[(sd.len() - 1) as int] == d);
    assert(sd.subrange(0, (sd.len() - 1) as int) =~= s);
}

proof fn lemma_parse_be_reverse(s: Seq<u8>)
    ensures parse_be(seq_reverse(s)) == parse_le(s),
    decreases s.len(),
{
    if s.len() == 0 {
        assert(seq_reverse(s) =~= Seq::<u8>::empty());
    } else {
        let t = s.subrange(1, s.len() as int);
        lemma_parse_be_reverse(t);
        lemma_parse_be_push(seq_reverse(t), s[0]);
    }
}

pub fn u64_to_string(n: u64) -> (result: TString)
    ensures
        parse_be(result.data@) == n as nat,
        result.data.len() >= 1,
{
    let mut data: Vec<u8> = Vec::new();
    let mut m: u64 = n;
    proof {
        assert(data@ =~= Seq::<u8>::empty());
        assert(parse_le(data@) == 0);
        assert(pow10(0) == 1);
        assert((n as nat) * pow10(0) == n as nat) by(nonlinear_arith);
    }
    if m == 0 {
        data.push(48u8);
        proof {
            assert(data@.len() == 1);
            assert(data@[0] == 48u8);
            assert(data@.subrange(1, data@.len() as int) =~= Seq::<u8>::empty());
            assert(parse_le(data@.subrange(1, data@.len() as int)) == 0);
            assert(parse_le(data@) == 0);
            assert((m as nat) == 0);
            assert((m as nat) * pow10(data.len() as nat) == 0) by(nonlinear_arith)
                requires (m as nat) == 0;
        }
    }
    while m > 0
        invariant
            parse_le(data@) + (m as nat) * pow10(data.len() as nat) == n as nat,
            data.len() >= 1 || m > 0,
        decreases m,
    {
        let d: u8 = (m % 10) as u8 + 48u8;
        let ghost old_data = data@;
        let ghost old_m = m as nat;
        let ghost old_len = data.len() as nat;
        data.push(d);
        proof {
            lemma_parse_push(old_data, d);
            assert((m as nat) == 10 * ((m / 10) as nat) + ((m % 10) as nat)) by(nonlinear_arith);
            assert(pow10((old_len + 1) as nat) == 10 * pow10(old_len));
        }
        m = m / 10;
        proof {
            assert(old_m * pow10(old_len)
                == ((d - 48) as nat) * pow10(old_len) + (m as nat) * pow10((old_len + 1) as nat))
                by(nonlinear_arith)
                requires
                    old_m == 10 * (m as nat) + ((d - 48) as nat),
                    pow10((old_len + 1) as nat) == 10 * pow10(old_len);
        }
    }
    assert(data.len() >= 1);
    let mut out: Vec<u8> = Vec::new();
    let mut i: usize = 0;
    while i < data.len()
        invariant
            i <= data.len(),
            out.len() == i,
            out@ =~= seq_reverse(data@.subrange((data.len() - i) as int, data.len() as int)),
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
            assert(seq_reverse(whole) =~= seq_reverse(prefix).push(data@[lo]));
        }
    }
    proof {
        assert(data@.subrange(0, data@.len() as int) =~= data@);
        lemma_parse_be_reverse(data@);
    }
    TString { data: out }
}

pub open spec fn is_digit(b: u8) -> bool { 48 <= b && b <= 57 }
pub open spec fn all_digits(s: Seq<u8>) -> bool
{ forall|i: int| 0 <= i < s.len() ==> is_digit(#[trigger] s[i]) }

proof fn lemma_parse_be_prefix_le(s: Seq<u8>, k: int)
    requires 0 <= k <= s.len(),
    ensures parse_be(s.subrange(0, k)) <= parse_be(s),
    decreases s.len() - k,
{
    if k == s.len() {
        assert(s.subrange(0, k) =~= s);
    } else {
        let m = (s.len() - 1) as int;
        assert(s.subrange(0, m).subrange(0, k) =~= s.subrange(0, k));
        lemma_parse_be_prefix_le(s.subrange(0, m), k);
        assert(parse_be(s) == parse_be(s.subrange(0, m)) * 10 + ((s[m] - 48) as nat));
        assert(parse_be(s.subrange(0, m)) * 10 >= parse_be(s.subrange(0, m))) by(nonlinear_arith);
    }
}

pub fn parse_u64(s: &TString) -> (result: Option<u64>)
    ensures
        (all_digits(s.data@) && s.data.len() >= 1 && parse_be(s.data@) <= u64::MAX) ==> result is Some,
        match result {
            Some(v) => all_digits(s.data@) && s.data.len() >= 1 && parse_be(s.data@) == v as nat,
            None => true,
        },
        result is None ==> (!all_digits(s.data@) || s.data.len() == 0 || parse_be(s.data@) > u64::MAX),
{
    if s.data.len() == 0 { return None; }
    let mut acc: u64 = 0;
    let mut i: usize = 0;
    while i < s.data.len()
        invariant
            i <= s.data.len(),
            all_digits(s.data@.subrange(0, i as int)),
            parse_be(s.data@.subrange(0, i as int)) == acc as nat,
        decreases s.data.len() - i,
    {
        let b: u8 = s.data[i];
        if b < 48 || b > 57 {
            assert(!is_digit(s.data@[i as int]));
            assert(!all_digits(s.data@));
            return None;
        }
        let digit: u64 = (b - 48) as u64;
        let ghost old_i = i as int;
        assert(s.data@.subrange(0, (i + 1) as int).subrange(0, old_i) =~= s.data@.subrange(0, old_i));
        assert(s.data@.subrange(0, (i + 1) as int)[old_i] == b);
        assert(parse_be(s.data@.subrange(0, (i + 1) as int)) == parse_be(s.data@.subrange(0, old_i)) * 10 + ((b - 48) as nat));
        if acc > (u64::MAX - digit) / 10 {
            proof {
                assert(digit <= 9);
                assert((acc as nat) * 10 + (digit as nat) > u64::MAX as nat) by(nonlinear_arith)
                    requires acc as nat > ((u64::MAX - digit) / 10) as nat, digit <= 9;
                assert(parse_be(s.data@.subrange(0, (i + 1) as int)) > u64::MAX);
                if all_digits(s.data@) {
                    lemma_parse_be_prefix_le(s.data@, (i + 1) as int);
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
    requires all_digits(s.data@) && s.spec_len() >= 1 && parse_be(s.data@) <= 18446744073709551615,
    ensures
        (result is Some),
        match result {
            Some(v) => v as nat == parse_be(s.data@),
            None => true,
        },
{
    parse_u64(&s)
}


fn parse_rejects_nondigit(s: TString) -> (result: Option<u64>)
    requires s.spec_len() >= 1 && !all_digits(s.data@),
    ensures
        match result {
            Some(v) => false,
            None => true,
        },
{
    parse_u64(&s)
}


}
fn main() {}

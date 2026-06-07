use vstd::prelude::*;
verus! {

pub struct TMapU64U64 { pub data: Vec<(u64, u64)> }
impl TMapU64U64 {
    pub open spec fn spec_dom(&self) -> Set<int> {
        Set::new(|kk: int| exists|j: int|
            0 <= j < self.data.len() && #[trigger] self.data@[j].0 as int == kk)
    }
    pub open spec fn well_formed(&self) -> bool {
        &&& self.data.len() <= 1000000
        &&& (forall|a: int, b: int| #![trigger self.data@[a].0, self.data@[b].0]
                0 <= a < self.data.len() && 0 <= b < self.data.len() && a != b
                ==> self.data@[a].0 != self.data@[b].0)
    }
    pub open spec fn spec_contains_key(&self, k: u64) -> bool {
        exists|j: int| 0 <= j < self.data.len() && #[trigger] self.data@[j].0 == k
    }
    pub open spec fn len(&self) -> nat { self.data.len() as nat }
    pub fn contains_key(&self, k: u64) -> (result: bool)
        requires self.well_formed(),
        ensures result == self.spec_contains_key(k),
    {
        let mut i: usize = 0;
        while i < self.data.len()
            invariant
                i <= self.data.len(),
                forall|j: int| 0 <= j < i ==> self.data@[j].0 != k,
            decreases self.data.len() - i,
        {
            if self.data[i].0 == k {
                assert(self.data@[i as int].0 == k);
                return true;
            }
            i = i + 1;
        }
        false
    }
    pub fn get(&self, k: u64) -> (result: Option<u64>)
        requires self.well_formed(),
        ensures match result {
            Some(v) => self.spec_contains_key(k)
                && (exists|j: int| 0 <= j < self.data.len()
                       && self.data@[j].0 == k && self.data@[j].1 == v),
            None => !self.spec_contains_key(k),
        },
    {
        let mut i: usize = 0;
        while i < self.data.len()
            invariant
                i <= self.data.len(),
                forall|j: int| 0 <= j < i ==> self.data@[j].0 != k,
            decreases self.data.len() - i,
        {
            if self.data[i].0 == k {
                let v: u64 = self.data[i].1;
                assert(self.data@[i as int].0 == k && self.data@[i as int].1 == v);
                return Some(v);
            }
            i = i + 1;
        }
        None
    }
    pub fn insert(&mut self, k: u64, v: u64)
        requires old(self).well_formed(), old(self).data.len() < 1000000,
                 !old(self).spec_contains_key(k),
        ensures
            final(self).well_formed(),
            final(self).spec_contains_key(k),
            exists|j: int| 0 <= j < final(self).data.len()
                && final(self).data@[j].0 == k && final(self).data@[j].1 == v,
            final(self).data.len() == old(self).data.len() + 1,
    {
        let ghost old_len = self.data.len();
        self.data.push((k, v));
        assert(self.data@[old_len as int].0 == k && self.data@[old_len as int].1 == v);
        assert(self.spec_contains_key(k)) by {
            assert(0 <= old_len < self.data.len() && self.data@[old_len as int].0 == k);
        }
        assert(self.well_formed()) by {
            assert forall|a: int, b: int|
                0 <= a < self.data.len() && 0 <= b < self.data.len() && a != b
                implies self.data@[a].0 != self.data@[b].0 by {
                if a < old_len && b < old_len {
                } else if a == old_len {
                    assert(self.data@[b].0 != k);
                } else if b == old_len {
                    assert(self.data@[a].0 != k);
                }
            }
        }
    }
}

fn build_one(k: u64, v: u64) -> (result: TMapU64U64)
    ensures
        result.spec_contains_key(k),
{
    let mut m: TMapU64U64 = TMapU64U64 { data: Vec::new() };
    m.insert(k, v);
    m
}


fn has_key(m: TMapU64U64, k: u64) -> (result: bool)
    requires
        m.well_formed(),
    ensures
        result == m.spec_contains_key(k),
{
    m.contains_key(k)
}


fn demo() -> (result: u64)
    ensures
        true,
{
    let mut m: TMapU64U64 = TMapU64U64 { data: Vec::new() };
    m.insert(7, 42);
    match m.get(7) {
            Some(v) => v,
            None => 0,
        }
}


fn lookup_absent(m: TMapU64U64, k: u64) -> (result: Option<u64>)
    requires
        m.well_formed(),
        !m.spec_contains_key(k),
    ensures
        (result is None),
{
    m.get(k)
}


}
fn main() {}

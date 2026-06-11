use vstd::prelude::*;
verus! {

pub open spec fn sorted(s: Seq<u32>) -> bool {
    forall|i: int, j: int| 0 <= i <= j < s.len() ==> s[i] <= s[j]
}

pub open spec fn forall_in(s: Seq<u32>, p: spec_fn(u32) -> bool) -> bool {
    forall|i: int| 0 <= i < s.len() ==> #[trigger] p(s[i])
}

pub open spec fn forall_below(s: Seq<u32>, n: int, p: spec_fn(u32) -> bool) -> bool {
    forall|i: int| 0 <= i < n && i < s.len() ==> #[trigger] p(s[i])
}

pub open spec fn forall_from(s: Seq<u32>, n: int, p: spec_fn(u32) -> bool) -> bool {
    forall|i: int| n <= i < s.len() ==> #[trigger] p(s[i])
}

fn binary_search(haystack: &[u32], needle: u32) -> (result: Option<usize>)
    requires sorted(haystack@),
    ensures
        match result {
            Some(i) => i < haystack.len() && haystack@[i as int] == needle,
            None => forall_in(haystack@, |x: u32| x != needle),
        },
{
    let mut lo: usize = 0;
    let mut hi: usize = haystack.len();
    loop
        invariant
            lo <= hi <= haystack.len(),
            sorted(haystack@),
            forall_below(haystack@, lo as int, |x: u32| x < needle),
            forall_from(haystack@, hi as int, |x: u32| x > needle),
        decreases hi - lo,
    {
        if lo == hi {
            assert(forall_in(haystack@, |x: u32| x != needle)) by {
                assert forall|k: int| 0 <= k < haystack@.len()
                    implies (|x: u32| x != needle)(haystack@[k]) by {
                    if k < lo as int {
                        assert((|x: u32| x < needle)(haystack@[k]));
                    } else {
                        assert((|x: u32| x > needle)(haystack@[k]));
                    }
                }
            }
            return None;
        }
        let mid = lo + (hi - lo) / 2;
        if haystack[mid] == needle {
            return Some(mid);
        }
        if haystack[mid] < needle {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
}

}
fn main() {}

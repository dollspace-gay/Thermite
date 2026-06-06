use vstd::prelude::*;
verus! {

enum List {
    Nil,
    Cons(u64, Box<List>),
}

spec fn sum_list(l: List) -> nat
    decreases l,
{
    match l {
        List::Nil => 0,
        List::Cons(h, t) => h as nat + sum_list(*t),
    }
}

}
fn main() {}

use vstd::prelude::*;
verus! {

pub enum List {
    Nil,
    Cons(u64, Box<List>),
}

pub open spec fn sum_list(l: List) -> nat
    decreases l,
{
    match l {
        List::Nil => 0,
        List::Cons(h, t) => h as nat + sum_list(*t),
    }
}

}
fn main() {}

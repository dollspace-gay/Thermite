use vstd::prelude::*;
verus! {

pub enum Shape {
    Circle(u64),
    Rect { w: u64, h: u64 },
}

fn is_circle(s: Shape) -> (result: bool)
    ensures
        result == (s is Circle),
{
    match s {
        Shape::Circle(r) => true,
        Shape::Rect { w, h } => false,
    }
}

}
fn main() {}

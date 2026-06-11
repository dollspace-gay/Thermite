use vstd::prelude::*;
verus! {

pub enum ParseErr {
    NotDigit,
    Overflow,
    Empty,
}


fn make_some() -> (result: Option<u64>)
    ensures
        match result {
            Some(v) => v == 5,
            None => true,
        },
{
    Some(5)
}


fn small(x: u64) -> (result: Option<u64>)
    ensures
        match result {
            Some(v) => v == x,
            None => x >= 10,
        },
{
    if x < 10 { Some(x) } else { None }
}


fn ok_seven() -> (result: Result<u64, ParseErr>)
    ensures
        match result {
            Ok(v) => v == 7,
            Err(e) => true,
        },
{
    Ok(7)
}


fn checked(x: u64) -> (result: Result<u64, ParseErr>)
    ensures
        match result {
            Ok(v) => v == x,
            Err(e) => x == 0,
        },
{
    if x > 0 { Ok(x) } else { Err(ParseErr::Empty) }
}


}
fn main() {}

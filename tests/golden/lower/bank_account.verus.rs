use vstd::prelude::*;
verus! {

pub struct Account {
    pub balance: u64,
}

impl Account {
    pub open spec fn well_formed(&self) -> bool {
        self.balance <= 1000000
    }
}

fn deposit(a: Account, amount: u64) -> (result: Account)
    requires
        a.well_formed(),
        a.balance + amount <= 1000000,
    ensures
        result.well_formed(),
        result.balance == a.balance + amount,
{
    Account { balance: a.balance + amount }
}

}
fn main() {}

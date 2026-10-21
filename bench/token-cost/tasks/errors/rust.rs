#[derive(Debug, PartialEq)]
pub enum WithdrawError {
    NotPositive,
    Insufficient,
}

pub fn withdraw(balance: i64, amount: i64) -> Result<i64, WithdrawError> {
    if amount <= 0 {
        return Err(WithdrawError::NotPositive);
    }
    if amount > balance {
        return Err(WithdrawError::Insufficient);
    }
    Ok(balance - amount)
}

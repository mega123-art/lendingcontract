use anchor_lang::prelude::*;

#[error_code]
pub enum Lendingerror{
    #[msg("Insufficient Funds available!!!")]
    InsufficientFunds,
    #[msg("Over Borrowable Amount!!!")]
    OverBorrowableAmount,
    #[msg("Over Repay Amount!!!")]
    OverRepay,
}
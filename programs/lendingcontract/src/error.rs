use anchor_lang::prelude::*;

#[error_code]
pub enum Lendingerror {
    #[msg("Insufficient Funds available!!!")]
    InsufficientFunds,
    #[msg("Over Borrowable Amount!!!")]
    OverBorrowableAmount,
    #[msg("Over Repay Amount!!!")]
    OverRepay,
    #[msg("Health Factor is above 1, liquidation not required!")]
    HealthFactorAboveOne,
    #[msg("Math Overflow")]
    MathOverflow,
    #[msg("Insufficient Balance")]
    InsufficientBalance,
    #[msg("Oracle Price Error")] // Add this line
    OracleError,
}
use anchor_lang::prelude::*;
use crate::state::*;
use anchor_spl::{associated_token::AssociatedToken, token_interface::TransferChecked, token_interface::{self, Mint, TokenAccount, TokenInterface}};
#[derive(Accounts)]
pub struct Deposit<'info>{
    #[account(mut)]
    pub signer:Signer<'info>,
    pub mint:InterfaceAccount<'info,Mint>,
    #[account(mut,
    seeds=[mint.key().as_ref()],
bump,
)]
pub bank:Account<'info,Bank>,
#[account(
    mut,
    seeds=[b"treasury",
    mint.key().as_ref()],
    bump,
)]
pub bank_token_account:InterfaceAccount<'info,TokenAccount>,
#[account(
    mut,
    seeds=[signer.key().as_ref()],
    bump,
)]
pub user_account:Account<'info,User>,

#[account(
    mut,
    associated_token::mint=mint,
    associated_token::authority=signer,
    associated_token::token_program=token_program
)]
pub user_token_account:InterfaceAccount<'info,TokenAccount>,
pub token_program:Interface<'info,TokenInterface>,
pub associated_token_program:Program<'info,AssociatedToken>,
pub system_program:Program<'info,System>

}

 
pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    let bank = &mut ctx.accounts.bank;
    let user = &mut ctx.accounts.user_account;
    
    // Update bank interest rates before deposit
    bank.update_interest()?;
    
    // Transfer tokens from user to bank
    let transfer_cpi_acc = TransferChecked {
        from: ctx.accounts.user_token_account.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.bank_token_account.to_account_info(),
        authority: ctx.accounts.signer.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_ctx = CpiContext::new(cpi_program, transfer_cpi_acc);
    let decimals: u8 = ctx.accounts.mint.decimals;
    token_interface::transfer_checked(cpi_ctx, amount, decimals)?;
    
    // Calculate shares for the user
    let user_shares = if bank.total_deposits == 0 || bank.total_deposits_shares == 0 {
        // First deposit - shares equal to amount
        amount
    } else {
        // Calculate shares based on current exchange rate
        // user_shares = amount * total_deposits_shares / total_deposits
        amount
            .checked_mul(bank.total_deposits_shares)
            .ok_or(LendingError::MathOverflow)?
            .checked_div(bank.total_deposits)
            .ok_or(LendingError::MathOverflow)?
    };
    
    // Update user balances based on mint type
    match ctx.accounts.mint.to_account_info().key() {
        key if key == user.usdc_address => {
            user.deposited_usdc = user.deposited_usdc
                .checked_add(amount)
                .ok_or(LendingError::MathOverflow)?;
            user.deposited_usdc_shares = user.deposited_usdc_shares
                .checked_add(user_shares)
                .ok_or(LendingError::MathOverflow)?;
        },
        _ => {
            user.deposited_sol = user.deposited_sol
                .checked_add(amount)
                .ok_or(LendingError::MathOverflow)?;
            user.deposited_sol_shares = user.deposited_sol_shares
                .checked_add(user_shares)
                .ok_or(LendingError::MathOverflow)?;
        }
    }
    
    // Update bank totals
    bank.total_deposits = bank.total_deposits
        .checked_add(amount)
        .ok_or(LendingError::MathOverflow)?;
    bank.total_deposits_shares = bank.total_deposits_shares
        .checked_add(user_shares)
        .ok_or(LendingError::MathOverflow)?;
    
    // Update timestamp
    user.last_updated = Clock::get()?.unix_timestamp;
    
    // Update interest rates after deposit
    bank.update_interest()?;
    
    Ok(())
}

#[error_code]
pub enum LendingError {
    #[msg("Math overflow")]
    MathOverflow,
}
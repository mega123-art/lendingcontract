use anchor_lang::prelude::*;
use anchor_spl::{token::TransferChecked, token_interface::{Mint, TokenAccount, TokenInterface}};
use crate::state::*;

#[derive(Accounts)]
pub struct InitiateFlashLoan<'info> {
    #[account(mut)]
    pub borrower: Signer<'info>,
    
    pub mint: InterfaceAccount<'info, Mint>,
    
    #[account(
        mut,
        seeds = [mint.key().as_ref()],
        bump
    )]
    pub bank: Account<'info, Bank>,
    
    #[account(
        init,
        payer = borrower,
        space = 8 + FlashLoan::INIT_SPACE,
        seeds = [b"flash_loan", borrower.key().as_ref(), mint.key().as_ref()],
        bump
    )]
    pub flash_loan: Account<'info, FlashLoan>,
    
    #[account(
        mut,
        seeds = [b"treasury", mint.key().as_ref()],
        bump,
        token::mint = mint,
    )]
    pub bank_token_account: InterfaceAccount<'info, TokenAccount>,
    
    #[account(
        mut,
        token::mint = mint,
        token::authority = borrower,
    )]
    pub borrower_token_account: InterfaceAccount<'info, TokenAccount>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RepayFlashLoan<'info> {
    #[account(mut)]
    pub borrower: Signer<'info>,
    
    pub mint: InterfaceAccount<'info, Mint>,
    
    #[account(
        mut,
        seeds = [mint.key().as_ref()],
        bump
    )]
    pub bank: Account<'info, Bank>,
    
    #[account(
        mut,
        seeds = [b"flash_loan", borrower.key().as_ref(), mint.key().as_ref()],
        bump,
        constraint = flash_loan.is_active @ ErrorCode::FlashLoanNotActive,
        constraint = flash_loan.borrower == borrower.key() @ ErrorCode::UnauthorizedFlashLoan,
        close = borrower
    )]
    pub flash_loan: Account<'info, FlashLoan>,
    
    #[account(
        mut,
        seeds = [b"treasury", mint.key().as_ref()],
        bump,
        token::mint = mint,
    )]
    pub bank_token_account: InterfaceAccount<'info, TokenAccount>,
    
    #[account(
        mut,
        token::mint = mint,
        token::authority = borrower,
    )]
    pub borrower_token_account: InterfaceAccount<'info, TokenAccount>,
    
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
pub fn initiate_flash_loan(
    ctx: Context<InitiateFlashLoan>,
    amount: u64,
) -> Result<()> {
    let bank = &mut ctx.accounts.bank;
    let flash_loan = &mut ctx.accounts.flash_loan;
    let mint_key = ctx.accounts.mint.key();
    
    // Update bank interest before processing
    bank.update_interest()?;
    
    // Check if bank has sufficient liquidity
    let available_liquidity = ctx.accounts.bank_token_account.amount;
    require!(available_liquidity >= amount, ErrorCode::InsufficientLiquidity);
    
    // Calculate flash loan fee
    let fee = bank.calculate_flash_loan_fee(amount);
    
    // Initialize flash loan state
    flash_loan.borrower = ctx.accounts.borrower.key();
    flash_loan.bank = bank.key();
    flash_loan.mint = mint_key;
    flash_loan.amount = amount;
    flash_loan.fee = fee;
    flash_loan.is_active = true;
    flash_loan.created_at = Clock::get()?.unix_timestamp;
    
    // Transfer tokens from bank treasury to borrower account
    // Using the same pattern as your borrow function
    let transfer_cpi_acc = TransferChecked {
        from: ctx.accounts.bank_token_account.to_account_info(),
        to: ctx.accounts.borrower_token_account.to_account_info(),
        authority: ctx.accounts.bank_token_account.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
    };
    
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let signer_seeds: &[&[&[u8]]] = &[
        &[
            b"treasury",
            mint_key.as_ref(),
            &[ctx.bumps.bank_token_account],
        ]
    ];
    let cpi_ctx = CpiContext::new(cpi_program, transfer_cpi_acc).with_signer(signer_seeds);
    let decimals = ctx.accounts.mint.decimals;
    token_interface::transfer_checked(cpi_ctx, amount, decimals)?;
    
    msg!("Flash loan initiated: {} tokens to {}", amount, ctx.accounts.borrower.key());
    msg!("Fee: {} tokens", fee);
    
    // Emit event for flash loan initiated
    emit!(FlashLoanInitiated {
        borrower: ctx.accounts.borrower.key(),
        bank: bank.key(),
        mint: mint_key,
        amount,
        fee,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}

pub fn repay_flash_loan(ctx: Context<RepayFlashLoan>) -> Result<()> {
    let bank = &mut ctx.accounts.bank;
    let flash_loan = &ctx.accounts.flash_loan;
    
    // Check that flash loan is within same transaction (security measure)
    let current_time = Clock::get()?.unix_timestamp;
    require!(
        current_time == flash_loan.created_at,
        ErrorCode::FlashLoanMustBeRepaidInSameTransaction
    );
    
    let total_repayment = flash_loan.amount + flash_loan.fee;
    
    // Check borrower has sufficient balance
    require!(
        ctx.accounts.borrower_token_account.amount >= total_repayment,
        ErrorCode::InsufficientBalanceForRepayment
    );
    
    // Transfer repayment from borrower to bank treasury
    // Using TransferChecked like in your borrow function
    let transfer_cpi_acc = TransferChecked {
        from: ctx.accounts.borrower_token_account.to_account_info(),
        to: ctx.accounts.bank_token_account.to_account_info(),
        authority: ctx.accounts.borrower.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
    };
    
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_ctx = CpiContext::new(cpi_program, transfer_cpi_acc);
    let decimals = ctx.accounts.mint.decimals;
    token_interface::transfer_checked(cpi_ctx, total_repayment, decimals)?;
    
    // Update bank's total deposits with the fee earned
    bank.total_deposits = bank.total_deposits.checked_add(flash_loan.fee).unwrap();
    
    msg!("Flash loan repaid: {} + {} fee = {} total", 
         flash_loan.amount, flash_loan.fee, total_repayment);
    
    // Emit event for flash loan repaid
    emit!(FlashLoanRepaid {
        borrower: ctx.accounts.borrower.key(),
        bank: bank.key(),
        mint: flash_loan.mint,
        amount: flash_loan.amount,
        fee: flash_loan.fee,
        timestamp: current_time,
    });
    
    Ok(())
}

// Events
#[event]
pub struct FlashLoanInitiated {
    pub borrower: Pubkey,
    pub bank: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub fee: u64,
    pub timestamp: i64,
}

#[event]
pub struct FlashLoanRepaid {
    pub borrower: Pubkey,
    pub bank: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub fee: u64,
    pub timestamp: i64,
}

// Error codes
#[error_code]
pub enum ErrorCode {
    #[msg("Insufficient liquidity in the bank")]
    InsufficientLiquidity,
    #[msg("Flash loan is not active")]
    FlashLoanNotActive,
    #[msg("Unauthorized to repay this flash loan")]
    UnauthorizedFlashLoan,
    #[msg("Flash loan must be repaid within the same transaction")]
    FlashLoanMustBeRepaidInSameTransaction,
    #[msg("Insufficient balance for repayment")]
    InsufficientBalanceForRepayment,
}

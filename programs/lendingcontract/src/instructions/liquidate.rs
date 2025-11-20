use crate::{error::Lendingerror, state::*};
use crate::{constants::{MAX_AGE, SOL_USD_FEED_ID, USDC_USD_FEED_ID}};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, TransferChecked};
use anchor_spl::{associated_token::AssociatedToken, token_interface::{Mint, TokenAccount, TokenInterface}};
use pyth_solana_receiver_sdk::price_update::{get_feed_id_from_hex, PriceUpdateV2};

#[derive(Accounts)]
pub struct Liquidate<'info>{
    #[account(mut)]
    pub liquidator: Signer<'info>,
    pub price_update: Account<'info, PriceUpdateV2>,
    pub collateral_mint: InterfaceAccount<'info, Mint>,
    pub debt_mint: InterfaceAccount<'info, Mint>, 
    #[account(
        mut,
        seeds=[collateral_mint.key().as_ref()],
        bump,
    )]
    pub collateral_bank: Account<'info, Bank>,
    #[account(
        mut,
        seeds=[debt_mint.key().as_ref()],
        bump,)]
    pub debt_bank: Account<'info, Bank>,
    #[account(
        mut,
        seeds=[b"treasury", collateral_mint.key().as_ref()],
        bump,
    )]
    pub collateral_bank_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        seeds=[b"treasury", debt_mint.key().as_ref()],
        bump,
    )]
    pub debt_bank_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds=[liquidator.key().as_ref()],
        bump,
    )]
    pub user_account: Account<'info, User>,

    #[account(
        init_if_needed,
        payer=liquidator,
        associated_token::mint=collateral_mint,
        associated_token::authority=liquidator,
        associated_token::token_program=token_program,
    )]
    pub liquidator_collateral_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer=liquidator,
        associated_token::mint=debt_mint,
        associated_token::authority=liquidator,
        associated_token::token_program=token_program,
    )]
    pub liquidator_debt_token_account: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn liquidate(ctx: Context<Liquidate>) -> Result<()> {
    let collateral_bank = &mut ctx.accounts.collateral_bank;
    let debt_bank = &mut ctx.accounts.debt_bank;
    let user = &mut ctx.accounts.user_account;
    let price_update = &ctx.accounts.price_update;
    
    // Update interest rates for both banks
    collateral_bank.update_interest()?;
    debt_bank.update_interest()?;
    
    // Get price feeds with error mapping
    let sol_feed_id = get_feed_id_from_hex(SOL_USD_FEED_ID)
        .map_err(|_| Lendingerror::OracleError)?;
    let usdc_feed_id = get_feed_id_from_hex(USDC_USD_FEED_ID)
        .map_err(|_| Lendingerror::OracleError)?;
        
    let sol_price = price_update.get_price_no_older_than(&Clock::get()?, MAX_AGE, &sol_feed_id)
        .map_err(|_| Lendingerror::OracleError)?;
    let usdc_price = price_update.get_price_no_older_than(&Clock::get()?, MAX_AGE, &usdc_feed_id)
        .map_err(|_| Lendingerror::OracleError)?;
    
    let current_timestamp = Clock::get()?.unix_timestamp;
    
    // ... [Rest of your liquidation logic logic remains the same] ...
    
    // For brevity, copy the rest of your logic here exactly as it was. 
    // Just make sure the "LendingError" enum usage matches what you defined in error.rs
    
    // Calculate current user balances using shares
    let user_sol_deposits = if collateral_bank.total_deposits_shares > 0 {
        user.deposited_sol_shares
            .checked_mul(collateral_bank.total_deposits)
            .ok_or(Lendingerror::MathOverflow)?
            .checked_div(collateral_bank.total_deposits_shares)
            .ok_or(Lendingerror::MathOverflow)?
    } else {
        0
    };
    
    let user_usdc_deposits = if collateral_bank.total_deposits_shares > 0 {
        user.deposited_usdc_shares
            .checked_mul(collateral_bank.total_deposits)
            .ok_or(Lendingerror::MathOverflow)?
            .checked_div(collateral_bank.total_deposits_shares)
            .ok_or(Lendingerror::MathOverflow)?
    } else {
        0
    };
    
    let user_sol_borrowed = if debt_bank.total_borrowed_shares > 0 {
        user.borrowed_sol_shares
            .checked_mul(debt_bank.total_borrowed)
            .ok_or(Lendingerror::MathOverflow)?
            .checked_div(debt_bank.total_borrowed_shares)
            .ok_or(Lendingerror::MathOverflow)?
    } else {
        0
    };
    
    let user_usdc_borrowed = if debt_bank.total_borrowed_shares > 0 {
        user.borrowed_usdc_shares
            .checked_mul(debt_bank.total_borrowed)
            .ok_or(Lendingerror::MathOverflow)?
            .checked_div(debt_bank.total_borrowed_shares)
            .ok_or(Lendingerror::MathOverflow)?
    } else {
        0
    };
    
    // Calculate total collateral and debt values
    let (total_collateral, total_borrowed, is_usdc_collateral) = 
        match ctx.accounts.collateral_mint.to_account_info().key() {
            key if key == user.usdc_address => {
                // USDC is collateral, SOL is debt
                let collateral_value = (usdc_price.price as u64)
                    .checked_mul(user_usdc_deposits)
                    .ok_or(Lendingerror::MathOverflow)?;
                let debt_value = (sol_price.price as u64)
                    .checked_mul(user_sol_borrowed)
                    .ok_or(Lendingerror::MathOverflow)?;
                (collateral_value, debt_value, true)
            }
            _ => {
                // SOL is collateral, USDC is debt
                let collateral_value = (sol_price.price as u64)
                    .checked_mul(user_sol_deposits)
                    .ok_or(Lendingerror::MathOverflow)?;
                let debt_value = (usdc_price.price as u64)
                    .checked_mul(user_usdc_borrowed)
                    .ok_or(Lendingerror::MathOverflow)?;
                (collateral_value, debt_value, false)
            }
        };
    
    // Calculate health factor
    let health_factor = if total_borrowed > 0 {
        (total_collateral as f64 * collateral_bank.liquidation_threshold as f64) 
            / (total_borrowed as f64 * 100.0)
    } else {
        f64::INFINITY
    };
    
    // Check if liquidation is allowed
    if health_factor >= 1.0 {
        return Err(Lendingerror::HealthFactorAboveOne.into());
    }
    
    // Calculate liquidation amount (debt to be repaid)
    let liquidation_amt = total_borrowed
        .checked_mul(debt_bank.liquidation_close_factor)
        .ok_or(Lendingerror::MathOverflow)?
        .checked_div(100)
        .ok_or(Lendingerror::MathOverflow)?;
    
    // Calculate liquidator reward (collateral amount with bonus)
    let liquidator_reward = liquidation_amt
        .checked_mul(collateral_bank.liquidation_bonus)
        .ok_or(Lendingerror::MathOverflow)?
        .checked_div(100)
        .ok_or(Lendingerror::MathOverflow)?
        .checked_add(liquidation_amt)
        .ok_or(Lendingerror::MathOverflow)?;
    
    // Transfer debt tokens from liquidator to debt bank
    let transfer_to_bank = TransferChecked {
        from: ctx.accounts.liquidator_debt_token_account.to_account_info(),
        mint: ctx.accounts.debt_mint.to_account_info(),
        to: ctx.accounts.debt_bank_token_account.to_account_info(),
        authority: ctx.accounts.liquidator.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_ctx = CpiContext::new(cpi_program.clone(), transfer_to_bank);
    let debt_decimals = ctx.accounts.debt_mint.decimals;
    
    token_interface::transfer_checked(cpi_ctx, liquidation_amt, debt_decimals)?;
    
    // Transfer collateral from bank to liquidator
    let transfer_to_liquidator = TransferChecked {
        from: ctx.accounts.collateral_bank_token_account.to_account_info(),
        mint: ctx.accounts.collateral_mint.to_account_info(),
        to: ctx.accounts.liquidator_collateral_token_account.to_account_info(),
        authority: ctx.accounts.collateral_bank_token_account.to_account_info(),
    };
    
    let mint_key = ctx.accounts.collateral_mint.key();
    let signer_seeds: &[&[&[u8]]] = &[
        &[
            b"treasury",
            mint_key.as_ref(),
            &[ctx.bumps.collateral_bank_token_account],
        ]
    ];
    
    let cpi_ctx_to_liquidator = CpiContext::new(cpi_program, transfer_to_liquidator)
        .with_signer(signer_seeds);
    let collateral_decimals = ctx.accounts.collateral_mint.decimals;
    
    token_interface::transfer_checked(cpi_ctx_to_liquidator, liquidator_reward, collateral_decimals)?;
    
    // Calculate shares to be reduced
    let debt_shares_to_reduce = if debt_bank.total_borrowed > 0 {
        liquidation_amt
            .checked_mul(debt_bank.total_borrowed_shares)
            .ok_or(Lendingerror::MathOverflow)?
            .checked_div(debt_bank.total_borrowed)
            .ok_or(Lendingerror::MathOverflow)?
    } else {
        0
    };
    
    let collateral_shares_to_reduce = if collateral_bank.total_deposits > 0 {
        liquidator_reward
            .checked_mul(collateral_bank.total_deposits_shares)
            .ok_or(Lendingerror::MathOverflow)?
            .checked_div(collateral_bank.total_deposits)
            .ok_or(Lendingerror::MathOverflow)?
    } else {
        0
    };
    
    // Update user account shares
    if is_usdc_collateral {
        // USDC collateral, SOL debt
        user.deposited_usdc_shares = user.deposited_usdc_shares
            .checked_sub(collateral_shares_to_reduce)
            .ok_or(Lendingerror::InsufficientBalance)?;
        user.borrowed_sol_shares = user.borrowed_sol_shares
            .checked_sub(debt_shares_to_reduce)
            .ok_or(Lendingerror::InsufficientBalance)?;
    } else {
        // SOL collateral, USDC debt
        user.deposited_sol_shares = user.deposited_sol_shares
            .checked_sub(collateral_shares_to_reduce)
            .ok_or(Lendingerror::InsufficientBalance)?;
        user.borrowed_usdc_shares = user.borrowed_usdc_shares
            .checked_sub(debt_shares_to_reduce)
            .ok_or(Lendingerror::InsufficientBalance)?;
    }
    
    // Update timestamps
    user.last_updated = current_timestamp;
    user.last_updated_borrow = current_timestamp;
    
    // Update bank totals
    collateral_bank.total_deposits = collateral_bank.total_deposits
        .checked_sub(liquidator_reward)
        .ok_or(Lendingerror::InsufficientBalance)?;
    
    collateral_bank.total_deposits_shares = collateral_bank.total_deposits_shares
        .checked_sub(collateral_shares_to_reduce)
        .ok_or(Lendingerror::InsufficientBalance)?;
    
    debt_bank.total_borrowed = debt_bank.total_borrowed
        .checked_sub(liquidation_amt)
        .ok_or(Lendingerror::InsufficientBalance)?;
    
    debt_bank.total_borrowed_shares = debt_bank.total_borrowed_shares
        .checked_sub(debt_shares_to_reduce)
        .ok_or(Lendingerror::InsufficientBalance)?;
    
    // Update interest rates after liquidation
    collateral_bank.update_interest()?;
    debt_bank.update_interest()?;
    
    Ok(())
}
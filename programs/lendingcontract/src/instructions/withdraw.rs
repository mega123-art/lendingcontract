use std::f64::consts::E;

use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked}};
use crate::{error::Lendingerror, state::*};

#[derive(Accounts)]
pub struct Withdraw<'info>{
#[account(mut)]
pub signer:Signer<'info>,
pub mint:InterfaceAccount<'info,Mint>,
#[account(mut,
seeds=[mint.key().as_ref()],
bump,)]
pub bank:Account<'info,Bank>,
#[account(
    mut,
seeds=[b"treasury",
mint.key().as_ref(),
],
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
    init_if_needed,
    payer=signer,
    associated_token::mint=mint,
    associated_token::authority=signer,
    associated_token::token_program=token_program,
)]
pub user_token_account:InterfaceAccount<'info,TokenAccount>,
pub token_program:Interface<'info,TokenInterface>,
pub associated_token_program:Program<'info,AssociatedToken>,
pub system_program:Program<'info,System>
}


pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
    let bank = &mut ctx.accounts.bank;
    let user = &mut ctx.accounts.user_account;
    
    // Update interest rates before withdrawing
    bank.update_interest()?;
    
    let deposited_val: u64;
    if ctx.accounts.mint.to_account_info().key() == user.usdc_address {
        deposited_val = user.deposited_usdc;
    } else {
        deposited_val = user.deposited_sol;
    }
    
    if amount > deposited_val {
        return Err(Lendingerror::InsufficientFunds.into());
    }
    
    let accrued_deposit = calculate_accrued_interest(deposited_val, bank.current_supply_rate, user.last_updated)?;
    
    if amount as f64 > accrued_deposit as f64 {
        return Err(Lendingerror::InsufficientFunds.into());
    }
    
    let transfer_cpi_acc = TransferChecked {
        from: ctx.accounts.bank_token_account.to_account_info(),
        to: ctx.accounts.user_token_account.to_account_info(),
        authority: ctx.accounts.bank_token_account.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
    };
    
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let mint_key = ctx.accounts.mint.key();
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
    
    let shares_to_remove = (amount as f64 / bank.total_deposits as f64) * bank.total_deposits_shares as f64;
    
    if ctx.accounts.mint.to_account_info().key() == user.usdc_address {
        user.deposited_usdc -= amount;
        user.deposited_usdc_shares -= shares_to_remove as u64;
    } else {
        user.deposited_sol -= amount;
        user.deposited_sol_shares -= shares_to_remove as u64;
    }
    
    bank.total_deposits -= amount;
    bank.total_deposits_shares -= shares_to_remove as u64;
    
    Ok(())
}
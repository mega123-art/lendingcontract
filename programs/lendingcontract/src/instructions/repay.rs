use std::f64::consts::E;

use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token_interface::{Mint, TokenAccount, TokenInterface}};
use crate::state::*;
#[derive(Accounts)]
pub struct Repay<'info>{
    #[account(mut)]
    pub signer:Signer<'info>,
    pub mint:InterfaceAccount<'info,Mint>,
    #[account(
        mut,
        seeds=[mint.key().as_ref()],
        bump,
    )]
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
pub system_program:Program<'info,System>,
}

pub fn repay(ctx:Context<Repay>,amount:u64)->Result<()>{
    let user=&mut ctx.accounts.user_account;
    let borrowed_val:u64;
    match ctx.accounts.mint.to_account_info().key(){
        key if key==user.usdc_address=>{
            borrowed_val=user.borrowed_usdc;
        },
        _=>{
            borrowed_val=user.borrowed_sol;
        }
    }
    let time_diff=user.last_updated_borrow-Clock::get()?.unix_timestamp;
    let bank=&mut ctx.accounts.bank;
    bank.total_borrowed=(bank.total_borrowed as f64 * E.powf(bank.interest_rate as f64 * time_diff as f64)) as u64;

    Ok(())
}
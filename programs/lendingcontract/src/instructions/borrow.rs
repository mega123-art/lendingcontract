use std::f64::consts::E;

use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked}};
use pyth_solana_receiver_sdk::price_update::{get_feed_id_from_hex, PriceUpdateV2};

use crate::{constants::{MAX_AGE, SOL_USD_FEED_ID, USDC_USD_FEED_ID}, error::Lendingerror, state::{calculate_accrued_interest, Bank, User}};

#[derive(Accounts)]
pub struct Borrow<'info>{
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
pub price_update:Account<'info,PriceUpdateV2>

}


// pub fn borrow(ctx:Context<Borrow>,amount:u64)->Result<()>{
//         let bank = &mut ctx.accounts.bank;
//         let user=&mut ctx.accounts.user_account;
//         let price_update=&mut ctx.accounts.price_update;
//         let total_collateral:u64;
//         match ctx.accounts.mint.to_account_info().key() {
//             key if key==user.usdc_address=>{
//                 let sol_feed_id=get_feed_id_from_hex(SOL_USD_FEED_ID)?;
//                 let sol_price=price_update.get_price_no_older_than(&Clock::get()? ,MAX_AGE, &sol_feed_id)?;
//                 let new_value= calculate_accured_interest(user.deposited_usdc, bank.interest_rate, user.last_updated)?;
//                  total_collateral=sol_price.price as u64 * new_value;
//             }
//             _=>{
//                 let usdc_feed_id=get_feed_id_from_hex(USDC_USD_FEED_ID)?;
//                 let usdc_price=price_update.get_price_no_older_than(&Clock::get()? ,MAX_AGE, &usdc_feed_id)?;
//                 let new_value= calculate_accured_interest(user.deposited_sol, bank.interest_rate, user.last_updated)?;
//                  total_collateral=new_value * usdc_price.price as u64;
//             }
//         }

//         let borrowable_amt=total_collateral.checked_mul(bank.liquidation_threshold).unwrap();
// if borrowable_amt<amount{
//     return Err(Lendingerror::OverBorrowableAmount.into());
// }
// let transfer_cpi_acc=TransferChecked{
//             from:ctx.accounts.bank_token_account.to_account_info(),
//             to:ctx.accounts.user_token_account.to_account_info(),
//             authority:ctx.accounts.bank_token_account.to_account_info(),
//             mint:ctx.accounts.mint.to_account_info(),
//         };
// let cpi_program=ctx.accounts.token_program.to_account_info();
// let mint_key=ctx.accounts.mint.key();
// let signer_seeds:&[&[&[u8]]]=&[
//     &[
//         b"treasury",
//         mint_key.as_ref(),
//         &[
//             ctx.bumps.bank_token_account,
//         ]
//     ]
// ];
// let cpi_ctx=CpiContext::new(cpi_program, transfer_cpi_acc).with_signer(signer_seeds);

// let decimals=ctx.accounts.mint.decimals;
// token_interface::transfer_checked(cpi_ctx, amount, decimals);


// if bank.total_borrowed==0{
//     bank.total_borrowed=amount;
//     bank.total_borrowed_shares=amount;
// }
// let borrow_ratio:u64=amount.checked_div(bank.total_borrowed).unwrap();
// let user_borrow_shares:u64=bank.total_borrowed_shares.checked_mul(borrow_ratio).unwrap();
// match ctx.accounts.mint.to_account_info().key(){
//     key if key==user.usdc_address=>{
//         user.borrowed_usdc+=amount;
//         user.borrowed_usdc_shares+=user_borrow_shares;
        
//     }
//     _=>{
//         user.borrowed_sol+=amount;
//         user.borrowed_sol_shares+=user_borrow_shares;
//     }
// }
// user.last_updated_borrow=Clock::get()?.unix_timestamp;
// bank.total_borrowed+=amount;
// bank.total_borrowed_shares+=user_borrow_shares;
// Ok(())
// }

pub fn borrow(ctx: Context<Borrow>, amount: u64) -> Result<()> {
    let bank = &mut ctx.accounts.bank;
    let user = &mut ctx.accounts.user_account;
    let price_update = &mut ctx.accounts.price_update;
    
    // Update interest rates before borrowing
    bank.update_interest()?;
    
    let total_collateral: u64;
    match ctx.accounts.mint.to_account_info().key() {
        key if key == user.usdc_address => {
            let sol_feed_id = get_feed_id_from_hex(SOL_USD_FEED_ID)?;
            let sol_price = price_update.get_price_no_older_than(&Clock::get()?, MAX_AGE, &sol_feed_id)?;
            let new_value = calculate_accrued_interest(user.deposited_usdc, bank.current_supply_rate, user.last_updated)?;
            total_collateral = sol_price.price as u64 * new_value;
        }
        _ => {
            let usdc_feed_id = get_feed_id_from_hex(USDC_USD_FEED_ID)?;
            let usdc_price = price_update.get_price_no_older_than(&Clock::get()?, MAX_AGE, &usdc_feed_id)?;
            let new_value = calculate_accrued_interest(user.deposited_sol, bank.current_supply_rate, user.last_updated)?;
            total_collateral = new_value * usdc_price.price as u64;
        }
    }

    let borrowable_amt = total_collateral.checked_mul(bank.liquidation_threshold).unwrap();
    if borrowable_amt < amount {
        return Err(Lendingerror::OverBorrowableAmount.into());
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

    if bank.total_borrowed == 0 {
        bank.total_borrowed = amount;
        bank.total_borrowed_shares = amount;
    }
    
    let borrow_ratio: u64 = amount.checked_div(bank.total_borrowed).unwrap();
    let user_borrow_shares: u64 = bank.total_borrowed_shares.checked_mul(borrow_ratio).unwrap();
    
    match ctx.accounts.mint.to_account_info().key() {
        key if key == user.usdc_address => {
            user.borrowed_usdc += amount;
            user.borrowed_usdc_shares += user_borrow_shares;
        }
        _ => {
            user.borrowed_sol += amount;
            user.borrowed_sol_shares += user_borrow_shares;
        }
    }
    
    user.last_updated_borrow = Clock::get()?.unix_timestamp;
    bank.total_borrowed += amount;
    bank.total_borrowed_shares += user_borrow_shares;
    
    Ok(())
}


// pub fn calculate_accured_interest(deposited:u64,interest_rate:u64,last_updated:i64)->Result<u64>{
//     let current_time=Clock::get()?.unix_timestamp;
//     let time_diff=current_time-last_updated;
//     let new_value=(deposited as f64*E.powf(interest_rate as f64*time_diff as f64)) as u64;
//     Ok(new_value)
// }
use std::f64::consts::E;

use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token_interface::{self,Mint, TokenAccount, TokenInterface ,TransferChecked}};
use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;
declare_id!("A9ALyfnt8LrVCz2uvhHnqHQFA3k5dUq7dAJxXo1Dikdy");

#[program]
pub mod lending {
   

    use std::f64::consts::E;

    use pyth_solana_receiver_sdk::price_update::{self, get_feed_id_from_hex};

    use super::*;

    pub fn initializebank(ctx: Context<InitializeBank>,liquidation_threshold:u64,max_ltv:u64) -> Result<()> {
        let bank=&mut ctx.accounts.bank;
        bank.mint_address=ctx.accounts.mint.key();
        bank.authority=ctx.accounts.signer.key();
        bank.liquidation_threshold=liquidation_threshold;
        bank.max_ltv=max_ltv;
        bank.interest_rate=0.05 as u64;

        Ok(())
    }
    pub fn initializeuser(ctx:Context<InitializeUser>,usdc_address:Pubkey)->Result<()>{
        let user_account=&mut ctx.accounts.user_account;
        user_account.owner=ctx.accounts.signer.key();
        user_account.usdc_address=usdc_address;
        Ok(())

    }
    pub fn deposit(ctx:Context<Deposit>,amount:u64)->Result<()>{

        let transfer_cpi_acc=TransferChecked{
            from:ctx.accounts.user_token_account.to_account_info(),
            mint:ctx.accounts.mint.to_account_info(),
            to:ctx.accounts.bank_token_account.to_account_info(),
            authority:ctx.accounts.signer.to_account_info(),
        };
        let cpi_program=ctx.accounts.token_program.to_account_info();
        let cpi_ctx=CpiContext::new(cpi_program,transfer_cpi_acc);
        let decimals:u8=ctx.accounts.mint.decimals;
        token_interface::transfer_checked(cpi_ctx, amount, decimals);
        let bank=&mut ctx.accounts.bank;
if bank.total_deposits==0{
    bank.total_deposits=amount;
    bank.total_deposits_shares=amount;
}
        let deposit_ratio:u64=amount.checked_div(bank.total_deposits).unwrap();
        let user_shares:u64=bank.total_deposits_shares.checked_mul(deposit_ratio).unwrap();

        let user=&mut ctx.accounts.user_account;
        match ctx.accounts.mint.to_account_info().key(){
            key if key ==user.usdc_address=>{
                user.deposited_usdc+=amount;
                user.deposited_usdc_shares+=user_shares;
            },
            _=>{
                user.deposited_sol+=amount;
                user.deposited_sol_shares+=user_shares;
                 

            }
        }
        bank.total_deposits+=amount;
        bank.total_deposits_shares+=user_shares;
    user.last_updated=Clock::get()?.unix_timestamp;
        Ok(())
    }

    pub fn withdraw(ctx:Context<Withdraw>,amount:u64)->Result<()>{
        let user=&mut ctx.accounts.user_account;
        let deposited_val:u64;
        if ctx.accounts.mint.to_account_info().key()==user.usdc_address{
            deposited_val=user.deposited_usdc;

        }else {
            deposited_val=user.deposited_sol;
        }
        if amount>deposited_val{
            return Err(Lendingerror::InsufficientFunds.into());
        }
let timediff=user.last_updated-Clock::get()?.unix_timestamp;
let bank=&mut ctx.accounts.bank;
bank.total_deposits=(bank.total_deposits as f64 * E.powf(bank.interest_rate as f64 *timediff as f64)) as u64;
let val_per_share=bank.total_deposits as f64/bank.total_deposits_shares as f64;
let user_value:f64=deposited_val as f64/ val_per_share;
if user_value<amount as f64{
    return Err(Lendingerror::InsufficientFunds.into());
}

        let transfer_cpi_acc=TransferChecked{
            from:ctx.accounts.bank_token_account.to_account_info(),
            to:ctx.accounts.user_token_account.to_account_info(),
            authority:ctx.accounts.bank_token_account.to_account_info(),
            mint:ctx.accounts.mint.to_account_info(),
        };
let cpi_program=ctx.accounts.token_program.to_account_info();
let mint_key=ctx.accounts.mint.key();
let signer_seeds:&[&[&[u8]]]=&[
    &[
        b"treasury",
        mint_key.as_ref(),
        &[
            ctx.bumps.bank_token_account,
        ]
    ]
];
let cpi_ctx=CpiContext::new(cpi_program, transfer_cpi_acc).with_signer(signer_seeds);

let decimals=ctx.accounts.mint.decimals;
token_interface::transfer_checked(cpi_ctx, amount, decimals);
let bank=&mut ctx.accounts.bank;
let shares_to_remove=(amount as f64/bank.total_deposits as f64)*bank.total_deposits_shares as f64;
let user=&mut ctx.accounts.user_account;
if ctx.accounts.mint.to_account_info().key()==user.usdc_address{
    user.deposited_usdc-=amount;
    user.deposited_usdc_shares-=shares_to_remove as u64;

}else {
    user.deposited_sol-=amount;
    user.deposited_sol_shares-=shares_to_remove as u64;
}
bank.total_deposits-=amount;
bank.total_deposits_shares-=shares_to_remove as u64;

        Ok(())
    }

    pub fn borrow(ctx:Context<Borrow>,amount:u64)->Result<()>{
        let bank = &mut ctx.accounts.bank;
        let user=&mut ctx.accounts.user_account;
        let price_update=&mut ctx.accounts.price_update;
        let total_collateral:u64;
        match ctx.accounts.mint.to_account_info().key() {
            key if key==user.usdc_address=>{
                let sol_feed_id=get_feed_id_from_hex(SOL_USD_FEED_ID)?;
                let sol_price=price_update.get_price_no_older_than(&Clock::get()? ,MAX_AGE, &sol_feed_id)?;
                let new_value= calculate_accured_interest(user.deposited_usdc, bank.interest_rate, user.last_updated)?;
                 total_collateral=sol_price.price as u64 * new_value;
            }
            _=>{
                let usdc_feed_id=get_feed_id_from_hex(USDC_USD_FEED_ID)?;
                let usdc_price=price_update.get_price_no_older_than(&Clock::get()? ,MAX_AGE, &usdc_feed_id)?;
                let new_value= calculate_accured_interest(user.deposited_sol, bank.interest_rate, user.last_updated)?;
                 total_collateral=new_value * usdc_price.price as u64;
            }
        }

        let borrowable_amt=total_collateral.checked_mul(bank.liquidation_threshold).unwrap();
if borrowable_amt<amount{
    return Err(Lendingerror::OverBorrowableAmount.into());
}
let transfer_cpi_acc=TransferChecked{
            from:ctx.accounts.bank_token_account.to_account_info(),
            to:ctx.accounts.user_token_account.to_account_info(),
            authority:ctx.accounts.bank_token_account.to_account_info(),
            mint:ctx.accounts.mint.to_account_info(),
        };
let cpi_program=ctx.accounts.token_program.to_account_info();
let mint_key=ctx.accounts.mint.key();
let signer_seeds:&[&[&[u8]]]=&[
    &[
        b"treasury",
        mint_key.as_ref(),
        &[
            ctx.bumps.bank_token_account,
        ]
    ]
];
let cpi_ctx=CpiContext::new(cpi_program, transfer_cpi_acc).with_signer(signer_seeds);

let decimals=ctx.accounts.mint.decimals;
token_interface::transfer_checked(cpi_ctx, amount, decimals);


if bank.total_borrowed==0{
    bank.total_borrowed=amount;
    bank.total_borrowed_shares=amount;
}
let borrow_ratio:u64=amount.checked_div(bank.total_borrowed).unwrap();
let user_borrow_shares:u64=bank.total_borrowed_shares.checked_mul(borrow_ratio).unwrap();
match ctx.accounts.mint.to_account_info().key(){
    key if key==user.usdc_address=>{
        user.borrowed_usdc+=amount;
        user.borrowed_usdc_shares+=user_borrow_shares;
        
    }
    _=>{
        user.borrowed_sol+=amount;
        user.borrowed_sol_shares+=user_borrow_shares;
    }
}
Ok(())
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
    banl.total_borrowed=(bank.total_borrowed as f64 * E.powf(bank.interest_rate as f64 * time_diff as f64)) as u64;

    Ok(())
}

}
fn calculate_accured_interest(deposited:u64,interest_rate:u64,last_updated:i64)->Result<u64>{
    let current_time=Clock::get()?.unix_timestamp;
    let time_diff=current_time-last_updated;
    let new_value=(deposited as f64*E.powf(interest_rate as f64*time_diff as f64)) as u64;
    Ok(new_value)
}
#[derive(Accounts)]
pub struct InitializeBank<'info> {
#[account(mut)]
pub signer:Signer<'info>,
pub mint:InterfaceAccount<'info,Mint>,
#[account(
    init,
    payer=signer,
    space=8+Bank::INIT_SPACE,
    seeds=[mint.key().as_ref()],
    bump,
)]
pub bank:Account<'info,Bank>,
#[account(
    init,
    token::mint=mint,
    token::authority=bank_token_account,
    payer=signer,
    seeds=[b"treasury",mint.key().as_ref()],
    bump,
)]
pub bank_token_account:InterfaceAccount<'info,TokenAccount>,
pub token_program:Interface<'info,TokenInterface>,
pub system_program:Program<'info,System>
}

#[derive(Accounts)]
pub struct InitializeUser<'info>{
    #[account(mut)]
    pub signer:Signer<'info>,

    #[account(
        init,
        payer=signer,
        space=8+User::INIT_SPACE,
        seeds=[signer.key().as_ref()],
        bump,
    )]
    pub user_account:Account<'info,User>,
    pub system_program:Program<'info,System>

}

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

#[account]
#[derive(InitSpace)]
pub struct Bank{
pub authority:Pubkey,
pub mint_address:Pubkey,
pub total_deposits:u64,
pub total_deposits_shares:u64,
  pub liquidation_threshold: u64,
    pub liquidation_bonus: u64,
    pub liquidation_close_factor: u64,
pub total_borrowed:u64,
    pub total_borrowed_shares: u64,
    pub max_ltv: u64,
pub usdc_address:Pubkey,
pub last_updated:i64,
    pub interest_rate: u64,

}

#[account]
#[derive(InitSpace)]
pub struct User{
pub owner:Pubkey,
pub deposited_sol_shares:u64,
pub deposited_sol:u64,
pub borrowed_sol:u64,
pub borrowed_sol_shares:u64,
pub deposited_usdc:u64,
pub deposited_usdc_shares:u64,
pub borrowed_usdc:u64,
pub borrowed_usdc_shares:u64,
pub usdc_address:Pubkey,
pub last_updated:i64,
pub last_updated_borrow:i64,
}

#[error_code]
pub enum Lendingerror{
    #[msg("Insufficient Funds available!!!")]
    InsufficientFunds,
    #[msg("Over Borrowable Amount!!!")]
    OverBorrowableAmount,
}
#[constant]
pub const SOL_USD_FEED_ID:&str="0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d";
#[constant]
pub const USDC_USD_FEED_ID:&str="0xeaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a";
#[constant]
pub const MAX_AGE:u64=100;
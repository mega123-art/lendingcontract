use anchor_lang::prelude::*;
use instructions::*;

mod state;
mod instructions;
mod error;
mod constants;


declare_id!("A9ALyfnt8LrVCz2uvhHnqHQFA3k5dUq7dAJxXo1Dikdy");

#[program]
pub mod lending {
   


    use super::*;

    pub fn initializebank(ctx: Context<InitializeBank>,liquidation_threshold:u64,max_ltv:u64) -> Result<()> {
        instructions::initbank(ctx, liquidation_threshold, max_ltv)
    }
    pub fn initializeuser(ctx:Context<InitializeUser>,usdc_address:Pubkey)->Result<()>{
       instructions::inituser(ctx, usdc_address)
    }
    pub fn depositmain(ctx:Context<Deposit>,amount:u64)->Result<()>{
        instructions::deposit(ctx, amount)
        
    }

    pub fn withdraw(ctx:Context<Withdraw>,amount:u64)->Result<()>{
        instructions::withdraw(ctx, amount)
    }

    pub fn borrow(ctx:Context<Borrow>,amount:u64)->Result<()>{
        instructions::borrow(ctx, amount)
    }

    pub fn repay(ctx:Context<Repay>,amount:u64)->Result<()>{
        instructions::repay(ctx, amount)
    }
}















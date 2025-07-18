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

   pub fn initialize_bank(
        ctx: Context<InitializeBank>,
        liquidation_threshold: u64,
        max_ltv: u64,
        base_rate: Option<u64>,
        multiplier: Option<u64>,
        jump_multiplier: Option<u64>,
        kink_utilization: Option<u64>,
        reserve_factor: Option<u64>,
    ) -> Result<()> {
        instructions::initbank(
            ctx,
            liquidation_threshold,
            max_ltv,
            base_rate,
            multiplier,
            jump_multiplier,
            kink_utilization,
            reserve_factor,
        )
    }
    pub fn initialize_user(ctx: Context<InitializeUser>, usdc_address: Pubkey) -> Result<()> {
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
    pub fn liquidate(ctx:Context<Liquidate>)->Result<()>{
        instructions::liquidate(ctx)
    }
    pub fn update_bank_kink_params(
        ctx: Context<UpdateBankParams>,
        base_rate: Option<u64>,
        multiplier: Option<u64>,
        jump_multiplier: Option<u64>,
        kink_utilization: Option<u64>,
        reserve_factor: Option<u64>,
    ) -> Result<()> {
        instructions::update_bank_kink_params(ctx, base_rate, multiplier, jump_multiplier, kink_utilization, reserve_factor)
    }
    pub fn update_bank_config(
        ctx: Context<UpdateBankParams>,
        liquidation_threshold: Option<u64>,
        liquidation_bonus: Option<u64>,
        liquidation_close_factor: Option<u64>,
        max_ltv: Option<u64>,
    ) -> Result<()> {
        instructions::update_bank_config(ctx, liquidation_threshold, liquidation_bonus, liquidation_close_factor, max_ltv)
    }
    pub fn update_interest(ctx: Context<UpdateInterest>) -> Result<()> {
        instructions::update_interest(ctx)
    }
    pub fn update_price(ctx: Context<UpdatePrice>) -> Result<()> {
        instructions::update_price(ctx)
    }
}















use anchor_lang::prelude::*;
use anchor_spl::token_interface::{ Mint, TokenAccount, TokenInterface };
use crate::state::*;

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
pub struct UpdateBankParams<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        mut,
        has_one = authority,
        seeds = [mint.key().as_ref()],
        bump,
    )]
    pub bank: Account<'info, Bank>,
    pub mint: InterfaceAccount<'info, Mint>,
}


pub fn initbank(ctx: Context<InitializeBank>,liquidation_threshold:u64,max_ltv:u64,base_rate: Option<u64>, multiplier: Option<u64>,jump_multiplier: Option<u64>,kink_utilization: Option<u64>,reserve_factor: Option<u64>,) -> Result<()> {
        let bank=&mut ctx.accounts.bank;
        bank.mint_address=ctx.accounts.mint.key();
        bank.authority=ctx.accounts.signer.key();
        bank.liquidation_threshold=liquidation_threshold;
        bank.max_ltv=max_ltv;
           // Initialize totals
    bank.total_deposits = 0;
    bank.total_deposits_shares = 0;
    bank.total_borrowed = 0;
    bank.total_borrowed_shares = 0;
    bank.liquidation_bonus = 500; // 5% default bonus
    bank.liquidation_close_factor = 5000; // 50% default close factor
// Set kink model parameters with defaults
    bank.base_rate = base_rate.unwrap_or(200);              // 2% default
    bank.multiplier = multiplier.unwrap_or(500);            // 5% default
    bank.jump_multiplier = jump_multiplier.unwrap_or(5000); // 50% default
    bank.kink_utilization = kink_utilization.unwrap_or(8000); // 80% default
    bank.reserve_factor = reserve_factor.unwrap_or(1000);   // 10% default
 // Initialize current rates
    bank.current_borrow_rate = bank.base_rate;
    bank.current_supply_rate = 0;
    bank.last_updated = Clock::get()?.unix_timestamp;
    
    msg!("Bank initialized with kink model:");
    msg!("Base rate: {}bp, Multiplier: {}bp, Jump multiplier: {}bp", 
         bank.base_rate, bank.multiplier, bank.jump_multiplier);
    msg!("Kink utilization: {}bp, Reserve factor: {}bp", 
         bank.kink_utilization, bank.reserve_factor);
        Ok(())
    }
    pub fn inituser(ctx:Context<InitializeUser>,usdc_address:Pubkey)->Result<()>{
        let user_account=&mut ctx.accounts.user_account;
        user_account.owner=ctx.accounts.signer.key();
        user_account.usdc_address=usdc_address;
        Ok(())

    }


    pub fn update_bank_kink_params(
    ctx: Context<UpdateBankParams>,
    base_rate: Option<u64>,
    multiplier: Option<u64>,
    jump_multiplier: Option<u64>,
    kink_utilization: Option<u64>,
    reserve_factor: Option<u64>,
) -> Result<()> {
    let bank = &mut ctx.accounts.bank;
    
    // Update interest before changing parameters
    bank.update_interest()?;
    
    // Update parameters if provided
    if let Some(rate) = base_rate {
        bank.base_rate = rate;
    }
    if let Some(mult) = multiplier {
        bank.multiplier = mult;
    }
    if let Some(jump) = jump_multiplier {
        bank.jump_multiplier = jump;
    }
    if let Some(kink) = kink_utilization {
        bank.kink_utilization = kink;
    }
    if let Some(reserve) = reserve_factor {
        bank.reserve_factor = reserve;
    }
    
    msg!("Bank parameters updated by authority: {}", ctx.accounts.authority.key());
    
    Ok(())
}

/// Update other bank parameters (liquidation settings, etc.)
pub fn update_bank_config(
    ctx: Context<UpdateBankParams>,
    liquidation_threshold: Option<u64>,
    liquidation_bonus: Option<u64>,
    liquidation_close_factor: Option<u64>,
    max_ltv: Option<u64>,
) -> Result<()> {
    let bank = &mut ctx.accounts.bank;
    
    // Update parameters if provided
    if let Some(threshold) = liquidation_threshold {
        bank.liquidation_threshold = threshold;
    }
    if let Some(bonus) = liquidation_bonus {
        bank.liquidation_bonus = bonus;
    }
    if let Some(factor) = liquidation_close_factor {
        bank.liquidation_close_factor = factor;
    }
    if let Some(ltv) = max_ltv {
        bank.max_ltv = ltv;
    }
    
    msg!("Bank configuration updated by authority: {}", ctx.accounts.authority.key());
    
    Ok(())
}

/// Get current bank status and rates (view function)
pub fn get_bank_status(ctx: Context<UpdateBankParams>) -> Result<()> {
    let bank = &ctx.accounts.bank;
    
    let utilization = bank.get_utilization_rate();
    let current_borrow_rate = bank.calculate_borrow_rate();
    let current_supply_rate = bank.calculate_supply_rate();
    
    msg!("=== Bank Status ===");
    msg!("Total Deposits: {}", bank.total_deposits);
    msg!("Total Borrowed: {}", bank.total_borrowed);
    msg!("Utilization Rate: {}bp ({}%)", utilization, utilization / 100);
    msg!("Current Borrow Rate: {}bp ({}%)", current_borrow_rate, current_borrow_rate / 100);
    msg!("Current Supply Rate: {}bp ({}%)", current_supply_rate, current_supply_rate / 100);
    msg!("==================");
    
    Ok(())
}

/// Emergency pause/unpause functionality
#[derive(Accounts)]
pub struct EmergencyControl<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        mut,
        has_one = authority,
        seeds = [mint.key().as_ref()],
        bump,
    )]
    pub bank: Account<'info, Bank>,
    pub mint: InterfaceAccount<'info, Mint>,
}

/// Emergency pause (sets rates to 0)
pub fn emergency_pause(ctx: Context<EmergencyControl>) -> Result<()> {
    let bank = &mut ctx.accounts.bank;
    
    // Update interest before pausing
    bank.update_interest()?;
    
    // Set all rates to 0
    bank.current_borrow_rate = 0;
    bank.current_supply_rate = 0;
    
    msg!("Emergency pause activated by authority: {}", ctx.accounts.authority.key());
    
    Ok(())
}

/// Resume normal operations (recalculate rates)
pub fn resume_operations(ctx: Context<EmergencyControl>) -> Result<()> {
    let bank = &mut ctx.accounts.bank;
    
    // Recalculate rates based on current utilization
    bank.current_borrow_rate = bank.calculate_borrow_rate();
    bank.current_supply_rate = bank.calculate_supply_rate();
    bank.last_updated = Clock::get()?.unix_timestamp;
    
    msg!("Normal operations resumed by authority: {}", ctx.accounts.authority.key());
    
    Ok(())
}
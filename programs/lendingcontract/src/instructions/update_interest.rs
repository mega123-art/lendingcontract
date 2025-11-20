use anchor_lang::prelude::*;
use crate::state::*;

#[derive(Accounts)]
pub struct UpdateInterest<'info> {
    #[account(mut)]
    pub bank: Account<'info, Bank>,
}

pub fn update_interest(ctx: Context<UpdateInterest>) -> Result<()> {
    let bank = &mut ctx.accounts.bank;
    bank.update_interest()?;
    Ok(())
}
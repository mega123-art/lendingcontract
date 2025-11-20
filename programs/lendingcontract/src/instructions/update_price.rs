use anchor_lang::prelude::*;
use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;

#[derive(Accounts)]
pub struct UpdatePrice<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    // The Pyth PriceUpdateV2 account. 
    // We don't mark it #[account(mut)] because we aren't writing to it directly; 
    // the Pyth program writes to it.
    pub price_update: Account<'info, PriceUpdateV2>,
}

pub fn update_price(ctx: Context<UpdatePrice>) -> Result<()> {
    // In the Pyth Pull Oracle model, the client (frontend/ts-script) sends two instructions:
    // 1. PythProgram.postUpdate(...) -> Writes new price to price_update account
    // 2. LendingProgram.updatePrice(...) -> This instruction
    
    // Since the data is already updated by the time this runs, we mostly use this 
    // for logging or optional verification.
    
    msg!("Price update account verified: {}", ctx.accounts.price_update.key());
    
    // Note: Your borrow/liquidate instructions will automatically read 
    // the fresh price from this account using 'get_price_no_older_than'.
    
    Ok(())
}
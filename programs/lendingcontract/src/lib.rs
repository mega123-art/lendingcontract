use anchor_lang::prelude::*;

declare_id!("A9ALyfnt8LrVCz2uvhHnqHQFA3k5dUq7dAJxXo1Dikdy");

#[program]
pub mod lendingcontract {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}

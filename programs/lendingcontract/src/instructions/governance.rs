use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use crate::state::*;

#[derive(Accounts)]
#[instruction(proposal_id: u64)]
pub struct CreateProposal<'info> {
    #[account(mut)]
    pub proposer: Signer<'info>,
    
    #[account(
        mut,
        seeds = [mint.key().as_ref()],
        bump,
    )]
    pub bank: Account<'info, Bank>,
    pub mint: InterfaceAccount<'info, Mint>,
    
    #[account(
        init,
        payer = proposer,
        space = 8 + Proposal::INIT_SPACE,
        seeds = [b"proposal", bank.key().as_ref(), proposal_id.to_le_bytes().as_ref()],
        bump
    )]
    pub proposal: Account<'info, Proposal>,
    
    // Proposer must be a valid user with some stake/deposit to avoid spam
    #[account(
        seeds = [proposer.key().as_ref()],
        bump,
        // FIX: Use constraint because 'user_account.owner' does not match 'proposer' name
        constraint = user_account.owner == proposer.key(),
    )]
    pub user_account: Account<'info, User>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Vote<'info> {
    #[account(mut)]
    pub voter: Signer<'info>,
    
    #[account(mut)]
    pub proposal: Account<'info, Proposal>,
    
    #[account(
        init,
        payer = voter,
        space = 8 + VoteRecord::INIT_SPACE,
        seeds = [b"vote", proposal.key().as_ref(), voter.key().as_ref()],
        bump
    )]
    pub vote_record: Account<'info, VoteRecord>,
    
    #[account(
        seeds = [voter.key().as_ref()],
        bump,
    )]
    pub user_account: Account<'info, User>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ExecuteProposal<'info> {
    #[account(mut)]
    pub executor: Signer<'info>, // Anyone can execute if conditions met
    
    #[account(
        mut,
        seeds = [mint.key().as_ref()],
        bump,
    )]
    pub bank: Account<'info, Bank>,
    pub mint: InterfaceAccount<'info, Mint>,
    
    #[account(
        mut,
        constraint = proposal.bank == bank.key(),
        constraint = !proposal.executed,
    )]
    pub proposal: Account<'info, Proposal>,
}

pub fn create_proposal(
    ctx: Context<CreateProposal>, 
    proposal_id: u64,
    proposal_type: u8,
    param_1: u64,
    param_2: u64,
    param_3: u64,
    param_4: u64,
    param_5: u64,
    duration: i64,
) -> Result<()> {
    let proposal = &mut ctx.accounts.proposal;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Anti-spam: Require some deposit share to propose (e.g., > 0)
    let user = &ctx.accounts.user_account;
    require!(user.deposited_sol_shares > 0 || user.deposited_usdc_shares > 0, GovernanceError::InsufficientStake);

    proposal.id = proposal_id;
    proposal.proposer = ctx.accounts.proposer.key();
    proposal.bank = ctx.accounts.bank.key();
    proposal.proposal_type = proposal_type;
    proposal.param_1 = param_1;
    proposal.param_2 = param_2;
    proposal.param_3 = param_3;
    proposal.param_4 = param_4;
    proposal.param_5 = param_5;
    
    proposal.votes_for = 0;
    proposal.votes_against = 0;
    proposal.created_at = current_time;
    proposal.end_time = current_time + duration;
    proposal.executed = false;
    
    msg!("Proposal {} created. Ends at {}", proposal_id, proposal.end_time);
    Ok(())
}

pub fn cast_vote(ctx: Context<Vote>, vote_for: bool) -> Result<()> {
    let proposal = &mut ctx.accounts.proposal;
    let user = &ctx.accounts.user_account;
    let vote_record = &mut ctx.accounts.vote_record;
    
    require!(Clock::get()?.unix_timestamp < proposal.end_time, GovernanceError::VotingEnded);
    
    // Calculate voting power based on shares held
    // Simple Strategy: 1 share = 1 vote (combining SOL and USDC shares roughly)
    let voting_power = user.deposited_sol_shares + user.deposited_usdc_shares;
    require!(voting_power > 0, GovernanceError::InsufficientStake);

    if vote_for {
        proposal.votes_for = proposal.votes_for.checked_add(voting_power).unwrap();
    } else {
        proposal.votes_against = proposal.votes_against.checked_add(voting_power).unwrap();
    }
    
    vote_record.proposal = proposal.key();
    vote_record.voter = ctx.accounts.voter.key();
    vote_record.voted = true;
    
    msg!("Vote cast. Power: {}", voting_power);
    Ok(())
}

pub fn execute_proposal(ctx: Context<ExecuteProposal>) -> Result<()> {
    let proposal = &mut ctx.accounts.proposal;
    let bank = &mut ctx.accounts.bank;
    
    require!(Clock::get()?.unix_timestamp >= proposal.end_time, GovernanceError::VotingNotEnded);
    require!(proposal.votes_for > proposal.votes_against, GovernanceError::ProposalDefeated);
    
    // Update Interest prior to changes
    bank.update_interest()?;

    if proposal.proposal_type == 1 {
        // Update Bank Config
        // Map params: 1=thresh, 2=bonus, 3=close_factor, 4=max_ltv
        if proposal.param_1 > 0 { bank.liquidation_threshold = proposal.param_1; }
        if proposal.param_2 > 0 { bank.liquidation_bonus = proposal.param_2; }
        if proposal.param_3 > 0 { bank.liquidation_close_factor = proposal.param_3; }
        if proposal.param_4 > 0 { bank.max_ltv = proposal.param_4; }
        msg!("Bank Config Updated via Governance");
    } else if proposal.proposal_type == 2 {
        // Update Kink Params
        // Map params: 1=base, 2=mult, 3=jump, 4=kink, 5=reserve
        if proposal.param_1 > 0 { bank.base_rate = proposal.param_1; }
        if proposal.param_2 > 0 { bank.multiplier = proposal.param_2; }
        if proposal.param_3 > 0 { bank.jump_multiplier = proposal.param_3; }
        if proposal.param_4 > 0 { bank.kink_utilization = proposal.param_4; }
        if proposal.param_5 > 0 { bank.reserve_factor = proposal.param_5; }
        msg!("Interest Params Updated via Governance");
    }
    
    proposal.executed = true;
    Ok(())
}

#[error_code]
pub enum GovernanceError {
    #[msg("Insufficient stake to propose or vote")]
    InsufficientStake,
    #[msg("Voting period has ended")]
    VotingEnded,
    #[msg("Voting period has not ended yet")]
    VotingNotEnded,
    #[msg("Proposal was defeated by votes")]
    ProposalDefeated,
}

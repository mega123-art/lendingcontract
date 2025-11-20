use anchor_lang::prelude::*;
use std::f64::consts::E;

#[account]
#[derive(InitSpace)]
pub struct Bank {
    pub authority: Pubkey,
    pub mint_address: Pubkey,
    pub total_deposits: u64,
    pub total_deposits_shares: u64,
    pub liquidation_threshold: u64,
    pub liquidation_bonus: u64,
    pub liquidation_close_factor: u64,
    pub total_borrowed: u64,
    pub total_borrowed_shares: u64,
    pub max_ltv: u64,
    pub usdc_address: Pubkey,
    pub last_updated: i64,

    // Dynamic Interest Rate Model Parameters
    pub base_rate: u64,
    pub multiplier: u64,
    pub jump_multiplier: u64,
    pub kink_utilization: u64,
    pub current_borrow_rate: u64,
    pub current_supply_rate: u64,
    pub reserve_factor: u64,
}

#[account]
#[derive(InitSpace)]
pub struct User {
    pub owner: Pubkey,
    pub deposited_sol_shares: u64,
    pub deposited_sol: u64,
    pub borrowed_sol: u64,
    pub borrowed_sol_shares: u64,
    pub deposited_usdc: u64,
    pub deposited_usdc_shares: u64,
    pub borrowed_usdc: u64,
    pub borrowed_usdc_shares: u64,
    pub usdc_address: Pubkey,
    pub last_updated: i64,
    pub last_updated_borrow: i64,
}

#[account]
#[derive(InitSpace)]
pub struct FlashLoan {
    pub borrower: Pubkey,
    pub bank: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub fee: u64,
    pub is_active: bool,
    pub created_at: i64,
}

// --- Governance State ---
#[account]
#[derive(InitSpace)]
pub struct Proposal {
    pub proposer: Pubkey,
    pub bank: Pubkey,
    pub id: u64,
    // 1 = Update Bank Config, 2 = Update Interest Params
    pub proposal_type: u8, 
    // Generic slots to store the proposed values
    pub param_1: u64, 
    pub param_2: u64,
    pub param_3: u64,
    pub param_4: u64,
    pub param_5: u64,
    
    pub votes_for: u64,
    pub votes_against: u64,
    pub created_at: i64,
    pub end_time: i64,
    pub executed: bool,
}

#[account]
#[derive(InitSpace)]
pub struct VoteRecord {
    pub proposal: Pubkey,
    pub voter: Pubkey,
    pub voted: bool,
}

const BASIS_POINTS: u64 = 10000;
const SECONDS_PER_YEAR: u64 = 365 * 24 * 60 * 60;

impl Bank {
    pub fn get_utilization_rate(&self) -> u64 {
        if self.total_deposits == 0 {
            return 0;
        }
        self.total_borrowed
            .checked_mul(BASIS_POINTS)
            .unwrap_or(0)
            .checked_div(self.total_deposits)
            .unwrap_or(0)
    }
    
    pub fn calculate_borrow_rate(&self) -> u64 {
        let utilization = self.get_utilization_rate();
        if utilization <= self.kink_utilization {
            self.base_rate + (utilization * self.multiplier / BASIS_POINTS)
        } else {
            let normal_rate = self.base_rate + (self.kink_utilization * self.multiplier / BASIS_POINTS);
            let excess_utilization = utilization - self.kink_utilization;
            let jump_rate = excess_utilization * self.jump_multiplier / BASIS_POINTS;
            normal_rate + jump_rate
        }
    }
    
    pub fn calculate_supply_rate(&self) -> u64 {
        let borrow_rate = self.calculate_borrow_rate();
        let utilization = self.get_utilization_rate();
        let rate_after_reserves = borrow_rate * (BASIS_POINTS - self.reserve_factor) / BASIS_POINTS;
        rate_after_reserves * utilization / BASIS_POINTS
    }
    
    pub fn update_interest(&mut self) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        let time_diff = current_time - self.last_updated;
        if time_diff > 0 {
            self.current_borrow_rate = self.calculate_borrow_rate();
            self.current_supply_rate = self.calculate_supply_rate();
            
            let borrow_rate_per_second = self.current_borrow_rate as f64 / BASIS_POINTS as f64 / SECONDS_PER_YEAR as f64;
            let supply_rate_per_second = self.current_supply_rate as f64 / BASIS_POINTS as f64 / SECONDS_PER_YEAR as f64;
            
            let borrow_multiplier = E.powf(borrow_rate_per_second * time_diff as f64);
            let supply_multiplier = E.powf(supply_rate_per_second * time_diff as f64);
            
            self.total_borrowed = (self.total_borrowed as f64 * borrow_multiplier) as u64;
            self.total_deposits = (self.total_deposits as f64 * supply_multiplier) as u64;
            self.last_updated = current_time;
        }
        Ok(())
    }

    pub fn calculate_flash_loan_fee(&self, amount: u64) -> u64 {
        const FLASH_LOAN_FEE_BASIS_POINTS: u64 = 9;
        amount * FLASH_LOAN_FEE_BASIS_POINTS / BASIS_POINTS
    }
}

pub fn calculate_accrued_interest(principal: u64, annual_rate: u64, last_updated: i64) -> Result<u64> {
    let current_time = Clock::get()?.unix_timestamp;
    let time_diff = current_time - last_updated;
    if time_diff <= 0 {
        return Ok(principal);
    }
    let rate_per_second = annual_rate as f64 / BASIS_POINTS as f64 / SECONDS_PER_YEAR as f64;
    let new_value = (principal as f64 * E.powf(rate_per_second * time_diff as f64)) as u64;
    Ok(new_value)
}
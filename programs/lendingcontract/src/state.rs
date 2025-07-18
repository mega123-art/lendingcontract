use anchor_lang::prelude::*;
use std::f64::consts::E;

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

    // Dynamic Interest Rate Model Parameters
    pub base_rate: u64,           // Base interest rate (in basis points, e.g., 200 = 2%)
    pub multiplier: u64,          // Rate multiplier before kink (in basis points)
    pub jump_multiplier: u64,     // Rate multiplier after kink (in basis points)
    pub kink_utilization: u64,    // Utilization rate where kink occurs (in basis points, e.g., 8000 = 80%)
    pub current_borrow_rate: u64, // Current calculated borrow rate
    pub current_supply_rate: u64, // Current calculated supply rate
    pub reserve_factor: u64,      // Reserve factor (in basis points, e.g., 1000 = 10%)
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

const BASIS_POINTS: u64 = 10000;
const SECONDS_PER_YEAR: u64 = 365 * 24 * 60 * 60;

impl Bank {
    /// Calculate current utilization rate
    pub fn get_utilization_rate(&self) -> u64 {
        if self.total_deposits == 0 {
            return 0;
        }
        
        // Utilization = total_borrowed / total_deposits * BASIS_POINTS
        self.total_borrowed
            .checked_mul(BASIS_POINTS)
            .unwrap_or(0)
            .checked_div(self.total_deposits)
            .unwrap_or(0)
    }
    
    /// Calculate borrow rate using kink model
    pub fn calculate_borrow_rate(&self) -> u64 {
        let utilization = self.get_utilization_rate();
        
        if utilization <= self.kink_utilization {
            // Before kink: base_rate + (utilization * multiplier / BASIS_POINTS)
            self.base_rate + (utilization * self.multiplier / BASIS_POINTS)
        } else {
            // After kink: base_rate + (kink * multiplier / BASIS_POINTS) + 
            //            ((utilization - kink) * jump_multiplier / BASIS_POINTS)
            let normal_rate = self.base_rate + (self.kink_utilization * self.multiplier / BASIS_POINTS);
            let excess_utilization = utilization - self.kink_utilization;
            let jump_rate = excess_utilization * self.jump_multiplier / BASIS_POINTS;
            
            normal_rate + jump_rate
        }
    }
    
    /// Calculate supply rate
    pub fn calculate_supply_rate(&self) -> u64 {
        let borrow_rate = self.calculate_borrow_rate();
        let utilization = self.get_utilization_rate();
        
        // Supply rate = borrow_rate * utilization * (1 - reserve_factor) / BASIS_POINTS^2
        let rate_after_reserves = borrow_rate * (BASIS_POINTS - self.reserve_factor) / BASIS_POINTS;
        rate_after_reserves * utilization / BASIS_POINTS
    }
    
    /// Update interest rates and accrued interest
    pub fn update_interest(&mut self) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        let time_diff = current_time - self.last_updated;
        
        if time_diff > 0 {
            // Calculate new rates
            self.current_borrow_rate = self.calculate_borrow_rate();
            self.current_supply_rate = self.calculate_supply_rate();
            
            // Convert annual rates to per-second rates
            let borrow_rate_per_second = self.current_borrow_rate as f64 / BASIS_POINTS as f64 / SECONDS_PER_YEAR as f64;
            let supply_rate_per_second = self.current_supply_rate as f64 / BASIS_POINTS as f64 / SECONDS_PER_YEAR as f64;
            
            // Apply compound interest
            let borrow_multiplier = E.powf(borrow_rate_per_second * time_diff as f64);
            let supply_multiplier = E.powf(supply_rate_per_second * time_diff as f64);
            
            // Update total borrowed with accrued interest
            self.total_borrowed = (self.total_borrowed as f64 * borrow_multiplier) as u64;
            
            // Update total deposits with accrued interest
            self.total_deposits = (self.total_deposits as f64 * supply_multiplier) as u64;
            
            self.last_updated = current_time;
        }
        
        Ok(())
    }
}


pub fn calculate_accrued_interest(principal: u64, annual_rate: u64, last_updated: i64) -> Result<u64> {
    let current_time = Clock::get()?.unix_timestamp;
    let time_diff = current_time - last_updated;
    
    if time_diff <= 0 {
        return Ok(principal);
    }
    
    // Convert annual rate to per-second rate
    let rate_per_second = annual_rate as f64 / BASIS_POINTS as f64 / SECONDS_PER_YEAR as f64;
    
    // Apply compound interest: P * e^(r * t)
    let new_value = (principal as f64 * E.powf(rate_per_second * time_diff as f64)) as u64;
    Ok(new_value)
}
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;

declare_id!("5kZPsxCvE2RkwE12daDyK7EGzqQUD5StoQRkWB7tePuy");

#[program]
pub mod cheese_rush {
    use super::*;
    pub fn initialize_player(
        ctx: Context<InitializePlayer>,
        referrer: Option<Pubkey>,
    ) -> Result<()> {
        let player = &mut ctx.accounts.player;
        let clock = Clock::get()?;
        if let Some(ref_addr) = referrer {
            require!(ref_addr != player.key(), CheeseError::InvalidReferrer);
        }
        player.owner = ctx.accounts.owner.key();
        player.referrer = referrer;
        player.cheese_balance = 0;
        player.total_cheese_claimed = 0;
        player.mouse_level = 1;
        player.last_rush_start = 0;
        player.rush_duration = 15;
        player.bros = vec![Bro {
            level: 1,
            capacity: 50,
            yield_per_min: 1,
            last_claim: clock.unix_timestamp,
        }];
        player.inventory = Inventory {
            cake: 0,
            milk: 0,
            burger: 0,
        };
        player.skills = Skills {
            yield_boost: 0,
            rush_time_reduction: 0,
            bros_capacity_boost: 0,
        };
        player.milk_boost_expiry = 0;
        Ok(())
    }

    pub fn start_rush(ctx: Context<StartRush>) -> Result<()> {
        let player = &mut ctx.accounts.player;
        let clock = Clock::get()?;
        require!(player.last_rush_start == 0, CheeseError::RushAlreadyActive);
        let base_duration = 15 + (player.mouse_level * 5);
        let reduction = player.skills.rush_time_reduction * 5;
        player.rush_duration = base_duration.saturating_sub(reduction).max(15);
        player.last_rush_start = clock.unix_timestamp;
        Ok(())
    }

    pub fn claim_rush(ctx: Context<ClaimRush>) -> Result<()> {
        let player = &mut ctx.accounts.player;
        let referrer = ctx.accounts.referrer.as_mut();
        let clock = Clock::get()?;
        let rush_end = player.last_rush_start + player.rush_duration as i64;
        require!(
            clock.unix_timestamp >= rush_end,
            CheeseError::RushNotComplete
        );
        let base_reward = 10 * (player.rush_duration / 15);
        let level_multiplier = 1.0 + (player.mouse_level as f64 * 0.1);
        let skill_boost = 1.0 + (player.skills.yield_boost as f64 * 0.01);
        let milk_boost = if clock.unix_timestamp < player.milk_boost_expiry {
            2.0
        } else {
            1.0
        };
        let reward =
            (base_reward as f64 * level_multiplier * skill_boost * milk_boost).round() as u64;
        player.cheese_balance += reward;
        player.total_cheese_claimed += reward;
        player.last_rush_start = 0;
        if let Some(referrer_account) = referrer {
            let referrer_share = reward / 20;
            referrer_account.cheese_balance = referrer_account
                .cheese_balance
                .saturating_add(referrer_share);
        }
        Ok(())
    }

    pub fn level_up_mouse(ctx: Context<LevelUpMouse>) -> Result<()> {
        let player = &mut ctx.accounts.player;
        let cost = (100_f64 * 1.15_f64.powi(player.mouse_level as i32 - 1)).round() as u64;
        require!(
            player.cheese_balance >= cost,
            CheeseError::InsufficientCheese
        );
        player.cheese_balance -= cost;
        player.mouse_level += 1;
        let clock = Clock::get()?;
        let slot_mod = (clock.slot % 100) as u8;
        if slot_mod < 50 {
            player.inventory.cake += 1;
        } else if slot_mod < 60 {
            player.inventory.milk += 1;
        } else if slot_mod < 65 {
            player.inventory.burger += 1;
        }
        Ok(())
    }

    pub fn claim_bros_cheese(ctx: Context<ClaimBrosCheese>, bro_index: u8) -> Result<()> {
        let player = &mut ctx.accounts.player;
        let referrer = ctx.accounts.referrer.as_mut();
        let clock = Clock::get()?;
        require!(
            bro_index < player.bros.len() as u8,
            CheeseError::InvalidBroIndex
        );
        let pending_cheese = {
            let bro = &mut player.bros[bro_index as usize];
            let time_elapsed = (clock.unix_timestamp - bro.last_claim) as u64 / 60;
            let pending = (bro.yield_per_min * time_elapsed).min(bro.capacity);
            bro.last_claim = clock.unix_timestamp;
            pending
        };
        player.cheese_balance += pending_cheese;
        player.total_cheese_claimed += pending_cheese;
        if let Some(referrer_account) = referrer {
            let referrer_share = pending_cheese / 20;
            referrer_account.cheese_balance = referrer_account
                .cheese_balance
                .saturating_add(referrer_share);
        }
        Ok(())
    }

    pub fn level_up_bro(ctx: Context<LevelUpBro>, bro_index: u8) -> Result<()> {
        let player = &mut ctx.accounts.player;
        require!(
            bro_index < player.bros.len() as u8,
            CheeseError::InvalidBroIndex
        );
        let current_bro_level = player.bros[bro_index as usize].level;
        let cost = (200_f64 * 1.25_f64.powi(current_bro_level as i32 - 1)).round() as u64;
        require!(
            player.cheese_balance >= cost,
            CheeseError::InsufficientCheese
        );
        {
            let bro = &mut player.bros[bro_index as usize];
            bro.level += 1;
            bro.capacity = (50.0 * (bro.level as f64).powf(1.5)).round() as u64;
            bro.yield_per_min = (1.0 * (bro.level as f64).powf(1.2)).round() as u64;
        }
        player.cheese_balance -= cost;
        Ok(())
    }

    pub fn use_boost(ctx: Context<UseBoost>, boost_type: BoostType) -> Result<()> {
        let player = &mut ctx.accounts.player;
        let clock = Clock::get()?;
        match boost_type {
            BoostType::Cake => {
                require!(player.last_rush_start > 0, CheeseError::NoActiveRush);
                require!(player.inventory.cake > 0, CheeseError::InsufficientBoost);
                player.inventory.cake -= 1;
                player.rush_duration = player.rush_duration.saturating_sub(300);
            }
            BoostType::Milk => {
                require!(player.inventory.milk > 0, CheeseError::InsufficientBoost);
                player.inventory.milk -= 1;
                player.milk_boost_expiry = clock.unix_timestamp + 7200;
            }
            BoostType::Burger => {
                require!(player.last_rush_start > 0, CheeseError::NoActiveRush);
                require!(player.inventory.burger > 0, CheeseError::InsufficientBoost);
                player.inventory.burger -= 1;
                player.last_rush_start = clock.unix_timestamp - player.rush_duration as i64;
            }
        }
        Ok(())
    }

    pub fn level_up_skill(ctx: Context<LevelUpSkill>, skill_type: SkillType) -> Result<()> {
        let player = &mut ctx.accounts.player;
        let current_skill_level = match skill_type {
            SkillType::YieldBoost => player.skills.yield_boost,
            SkillType::RushTimeReduction => player.skills.rush_time_reduction,
            SkillType::BrosCapacityBoost => player.skills.bros_capacity_boost,
        };
        let cost = (500_f64 * 1.5_f64.powi(current_skill_level as i32)).round() as u64;
        require!(
            player.cheese_balance >= cost,
            CheeseError::InsufficientCheese
        );
        require!(current_skill_level < 50, CheeseError::MaxLevelReached);
        player.cheese_balance -= cost;
        match skill_type {
            SkillType::YieldBoost => player.skills.yield_boost += 1,
            SkillType::RushTimeReduction => player.skills.rush_time_reduction += 1,
            SkillType::BrosCapacityBoost => player.skills.bros_capacity_boost += 1,
        }
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializePlayer<'info> {
    #[account(init, payer = owner, space = Player::LEN, seeds = [b"player", owner.key().as_ref()], bump)]
    pub player: Account<'info, Player>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StartRush<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClaimRush<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    #[account(mut)]
    pub referrer: Option<Account<'info, Player>>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct LevelUpMouse<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClaimBrosCheese<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    #[account(mut)]
    pub referrer: Option<Account<'info, Player>>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct LevelUpBro<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct UseBoost<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct LevelUpSkill<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    pub owner: Signer<'info>,
}

#[account]
pub struct Player {
    pub owner: Pubkey,
    pub referrer: Option<Pubkey>,
    pub cheese_balance: u64,
    pub total_cheese_claimed: u64,
    pub mouse_level: u32,
    pub last_rush_start: i64,
    pub rush_duration: u32,
    pub bros: Vec<Bro>,
    pub inventory: Inventory,
    pub skills: Skills,
    pub milk_boost_expiry: i64,
}

impl Player {
    pub const LEN: usize = 8 + 32 + 1 + 32 + 8 + 8 + 4 + 8 + 4 + (4 + (10 * 28)) + 12 + 12 + 8 + 8;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Bro {
    pub level: u32,
    pub capacity: u64,
    pub yield_per_min: u64,
    pub last_claim: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Inventory {
    pub cake: u32,
    pub milk: u32,
    pub burger: u32,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Skills {
    pub yield_boost: u32,
    pub rush_time_reduction: u32,
    pub bros_capacity_boost: u32,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum BoostType {
    Cake,
    Milk,
    Burger,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum SkillType {
    YieldBoost,
    RushTimeReduction,
    BrosCapacityBoost,
}

#[error_code]
pub enum CheeseError {
    #[msg("Invalid referrer provided")]
    InvalidReferrer,
    #[msg("Rush is already active")]
    RushAlreadyActive,
    #[msg("Rush is not yet complete")]
    RushNotComplete,
    #[msg("Insufficient cheese balance")]
    InsufficientCheese,
    #[msg("Invalid bro index")]
    InvalidBroIndex,
    #[msg("No active rush to boost")]
    NoActiveRush,
    #[msg("Insufficient boost items")]
    InsufficientBoost,
    #[msg("Skill has reached maximum level")]
    MaxLevelReached,
}

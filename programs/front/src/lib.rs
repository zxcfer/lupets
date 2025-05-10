use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};

declare_id!("41ERbSHwC8su7e59kyiQc39XYkCHRMQmjHQuPBsaq4J8");

#[program]
pub mod virtual_pet {
    use super::*;

    // Initialize a new pet profile
    pub fn initialize_pet(ctx: Context<InitializePet>) -> Result<()> {
        let pet = &mut ctx.accounts.pet;
        pet.owner = *ctx.accounts.owner.key;
        pet.health = 100;
        pet.happiness = 100;
        pet.coins_earned = 0;
        pet.last_interaction = Clock::get()?.unix_timestamp;
        Ok(())
    }

    // Request ownership from another user
    pub fn request_ownership(ctx: Context<RequestOwnership>) -> Result<()> {
        let request = &mut ctx.accounts.ownership_request;
        request.from = *ctx.accounts.from.key;
        request.to = *ctx.accounts.to.key;
        request.status = OwnershipStatus::Pending;
        Ok(())
    }

    // Respond to ownership request
    pub fn respond_to_request(
        ctx: Context<RespondToRequest>,
        accept: bool,
    ) -> Result<()> {
        let request = &mut ctx.accounts.ownership_request;
        if accept {
            request.status = OwnershipStatus::Accepted;
            let pet = &mut ctx.accounts.pet;
            pet.owner = request.from;
        } else {
            request.status = OwnershipStatus::Rejected;
        }
        Ok(())
    }

    // Feed pet (increases health)
    pub fn feed_pet(ctx: Context<FeedPet>, _item_id: u64) -> Result<()> {
        let pet = &mut ctx.accounts.pet;
        let item = &ctx.accounts.item;
        
        // Verify item is owned by feeder
        if item.owner != *ctx.accounts.feeder.key {
            return Err(ErrorCode::NotItemOwner.into());
        }
        
        // Apply item effects
        pet.health = pet.health.saturating_add(item.health_effect);
        pet.happiness = pet.happiness.saturating_add(item.happiness_effect);
        
        // Burn the item
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Burn {
                mint: ctx.accounts.item_mint.to_account_info(),
                from: ctx.accounts.item_token_account.to_account_info(),
                authority: ctx.accounts.feeder.to_account_info(),
            },
        );
        token::burn(cpi_ctx, 1)?;
        
        Ok(())
    }

    // Play with pet (increases happiness)
    pub fn play_with_pet(ctx: Context<PlayWithPet>) -> Result<()> {
        let pet = &mut ctx.accounts.pet;
        let now = Clock::get()?.unix_timestamp;
        
        // Can only play once per hour
        if now - pet.last_interaction < 3600 {
            return Err(ErrorCode::TooFrequentInteraction.into());
        }
        
        pet.happiness = pet.happiness.saturating_add(10).min(100);
        pet.last_interaction = now;
        Ok(())
    }

    // Earn coins by caring for pet
    pub fn earn_coins(ctx: Context<EarnCoins>) -> Result<()> {
        let pet = &mut ctx.accounts.pet;
        let now = Clock::get()?.unix_timestamp;
        
        // Can earn once per day
        if now - pet.last_coin_earn < 86400 {
            return Err(ErrorCode::TooFrequentCoinEarn.into());
        }
        
        // Calculate coins based on pet status
        let coins = u64::from((pet.health + pet.happiness) / 20); // 0-10 coins per day
        pet.coins_earned += coins;
        pet.last_coin_earn = now;
        
        // Mint coins to owner
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::MintTo {
                mint: ctx.accounts.pet_coin_mint.to_account_info(),
                to: ctx.accounts.owner_token_account.to_account_info(),
                authority: ctx.accounts.pet_coin_mint_authority.to_account_info(),
            },
        );
        token::mint_to(cpi_ctx, coins.into())?;
        
        Ok(())
    }
}

// Accounts
#[derive(Accounts)]
pub struct InitializePet<'info> {
    #[account(init, payer = owner, space = 8 + 32 + 1 + 1 + 8 + 8)]
    pub pet: Account<'info, Pet>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RequestOwnership<'info> {
    #[account(init, payer = from, space = 8 + 32 + 32 + 1)]
    pub ownership_request: Account<'info, OwnershipRequest>,
    #[account(mut)]
    pub from: Signer<'info>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub to: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RespondToRequest<'info> {
    #[account(mut, has_one = to)]
    pub ownership_request: Account<'info, OwnershipRequest>,
    #[account(mut)]
    pub pet: Account<'info, Pet>,
    #[account(mut)]
    pub to: Signer<'info>,
}

#[derive(Accounts)]
pub struct FeedPet<'info> {
    #[account(mut, has_one = owner)]
    pub pet: Account<'info, Pet>,
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub feeder: Signer<'info>,
    #[account(mut)]
    pub item: Account<'info, Item>,
    #[account(mut)]
    pub item_mint: Box<Account<'info, token::Mint>>,
    #[account(mut)]
    pub item_token_account: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct PlayWithPet<'info> {
    #[account(mut, has_one = owner)]
    pub pet: Account<'info, Pet>,
    #[account(mut)]
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct EarnCoins<'info> {
    #[account(mut, has_one = owner)]
    pub pet: Account<'info, Pet>,
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub pet_coin_mint: Account<'info, token::Mint>,
    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,
    /// CHECK: This account is used just as a signer for minting
    pub pet_coin_mint_authority: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

// Data structures
#[account]
pub struct Pet {
    pub owner: Pubkey,
    pub health: u8, // 0-100
    pub happiness: u8, // 0-100
    pub coins_earned: u64,
    pub last_interaction: i64,
    pub last_coin_earn: i64,
}

#[account]
pub struct OwnershipRequest {
    pub from: Pubkey,
    pub to: Pubkey,
    pub status: OwnershipStatus,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum OwnershipStatus {
    Pending,
    Accepted,
    Rejected,
}

#[account]
pub struct Item {
    pub id: u64,
    pub owner: Pubkey,
    pub health_effect: u8,
    pub happiness_effect: u8,
    pub price: u64,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Too frequent interaction")]
    TooFrequentInteraction,
    #[msg("Too frequent coin earn")]
    TooFrequentCoinEarn,
    #[msg("Not the item owner")]
    NotItemOwner,
}
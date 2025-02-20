use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Token, TokenAccount, Mint};

declare_id!("DLNR4oQQajCa6UyATqUzpqJgguUCkg3hShZPTMsVKq3h");

// Reference to sevens-token program
pub const SEVENS_TOKEN_PROGRAM_ID: &str = "Ah4sw8i5k74TC7tCzSrqkEitNdQVRhgrPsKfUrhqzEbn";

// Instruction discriminators for sevens-token CPI calls
// Generated as first 8 bytes of sha256("global:mint_token")
const MINT_TOKEN_DISCRIMINATOR: [u8; 8] = [0xac, 0x89, 0xb7, 0x0e, 0xcf, 0x6e, 0xea, 0x38];
// Generated as first 8 bytes of sha256("global:burn_token")
const BURN_TOKEN_DISCRIMINATOR: [u8; 8] = [0xb9, 0xa5, 0xd8, 0xf6, 0x90, 0x1f, 0x46, 0x4a];

#[program]
pub mod sevens_token_management {
    use super::*;

    /// Initialize tariffs with initial values
    /// Can only be called once to create the tariffs account
    pub fn initialize(
        ctx: Context<Initialize>,
        target_wallet: Pubkey,
        mint_fee: u64,
        set_sale_fee: u64,
        buy_fee: u8,
        burn_fee: u64,
    ) -> Result<()> {
        // Validate buy_fee is a valid percentage (0-99)
        require!(buy_fee < 100, ManagementError::InvalidBuyPercentage);

        // Validate target_wallet is not default (all zeros)
        require!(target_wallet != Pubkey::default(), ManagementError::InvalidTargetWallet);

        let tariffs = &mut ctx.accounts.tariffs;

        // Set the authority to the signer who initialized the contract
        tariffs.authority = ctx.accounts.authority.key();
        tariffs.target_wallet = target_wallet;
        tariffs.mint = mint_fee;
        tariffs.set_sale = set_sale_fee;
        tariffs.buy = buy_fee;
        tariffs.burn = burn_fee;
        tariffs.paused = false;

        emit!(TariffsUpdated {
            authority: ctx.accounts.authority.key(),
            target_wallet,
            mint: mint_fee,
            set_sale: set_sale_fee,
            buy: buy_fee,
            burn: burn_fee,
        });

        Ok(())
    }

    /// Update tariffs
    /// Only the authority can call this function
    pub fn update_tariffs(
        ctx: Context<UpdateTariffs>,
        target_wallet: Pubkey,
        mint_fee: u64,
        set_sale_fee: u64,
        buy_fee: u8,
        burn_fee: u64,
    ) -> Result<()> {
        // Validate buy_fee is a valid percentage (0-99)
        require!(buy_fee < 100, ManagementError::InvalidBuyPercentage);

        // Validate target_wallet is not default (all zeros)
        require!(target_wallet != Pubkey::default(), ManagementError::InvalidTargetWallet);

        let tariffs = &mut ctx.accounts.tariffs;

        tariffs.target_wallet = target_wallet;
        tariffs.mint = mint_fee;
        tariffs.set_sale = set_sale_fee;
        tariffs.buy = buy_fee;
        tariffs.burn = burn_fee;

        emit!(TariffsUpdated {
            authority: ctx.accounts.authority.key(),
            target_wallet,
            mint: mint_fee,
            set_sale: set_sale_fee,
            buy: buy_fee,
            burn: burn_fee,
        });

        Ok(())
    }

    /// Pause/unpause operations (emergency stop)
    pub fn set_paused(ctx: Context<UpdateTariffs>, paused: bool) -> Result<()> {
        let tariffs = &mut ctx.accounts.tariffs;
        tariffs.paused = paused;

        emit!(PauseStatusChanged {
            authority: ctx.accounts.authority.key(),
            paused,
        });

        Ok(())
    }

    /// Close tariffs account and reclaim rent
    /// This function can close orphaned PDAs from old deployments
    /// without needing to deserialize their data structure
    pub fn close_tariffs(ctx: Context<CloseTariffs>) -> Result<()> {
        // Transfer all lamports from tariffs PDA to authority
        let tariffs_lamports = ctx.accounts.tariffs.lamports();
        **ctx.accounts.tariffs.lamports.borrow_mut() = 0;
        **ctx.accounts.authority.lamports.borrow_mut() += tariffs_lamports;

        emit!(TariffsClosed {
            authority: ctx.accounts.authority.key(),
        });

        Ok(())
    }

    /// Close token management data PDA and reclaim rent
    /// This function can close orphaned token data PDAs from old deployments
    pub fn close_token_data(ctx: Context<CloseTokenData>) -> Result<()> {
        // Transfer all lamports from token data PDA to authority
        let token_data_lamports = ctx.accounts.token_data.lamports();
        **ctx.accounts.token_data.lamports.borrow_mut() = 0;
        **ctx.accounts.authority.lamports.borrow_mut() += token_data_lamports;

        Ok(())
    }

    /// Managed mint operation
    /// Creates a token through sevens-token with tariff collection
    pub fn managed_mint(
        ctx: Context<ManagedMint>,
        author: String,
        hash: String,
        description: String,
        token_name: String,
        can_be_burned: bool,
    ) -> Result<()> {
        let tariffs = &ctx.accounts.tariffs;

        // Check if operations are paused
        require!(!tariffs.paused, ManagementError::OperationsPaused);

        // Step 1: Collect tariff fee before minting
        if tariffs.mint > 0 {
            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    system_program::Transfer {
                        from: ctx.accounts.payer.to_account_info(),
                        to: ctx.accounts.target_wallet.to_account_info(),
                    },
                ),
                tariffs.mint,
            ).map_err(|_| ManagementError::InsufficientFundsForTariff)?;
        }

        // Step 2: Call sevens-token mint_token via CPI
        let mut data = Vec::with_capacity(8 + 4 + author.len() + 4 + hash.len() + 4 + description.len() + 4 + token_name.len() + 1);

        // Add instruction discriminator
        data.extend_from_slice(&MINT_TOKEN_DISCRIMINATOR);

        // Serialize parameters (Borsh format)
        // author (String: length + bytes)
        data.extend_from_slice(&(author.len() as u32).to_le_bytes());
        data.extend_from_slice(author.as_bytes());

        // hash (String: length + bytes)
        data.extend_from_slice(&(hash.len() as u32).to_le_bytes());
        data.extend_from_slice(hash.as_bytes());

        // description (String: length + bytes)
        data.extend_from_slice(&(description.len() as u32).to_le_bytes());
        data.extend_from_slice(description.as_bytes());

        // token_name (String: length + bytes)
        data.extend_from_slice(&(token_name.len() as u32).to_le_bytes());
        data.extend_from_slice(token_name.as_bytes());

        // can_be_burned (bool: 1 byte)
        data.push(if can_be_burned { 1 } else { 0 });

        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::instruction::Instruction {
                program_id: ctx.accounts.sevens_token_program.key(),
                accounts: vec![
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.payer.key(),
                        true, // payer must sign
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.mint.key(),
                        true, // mint must sign (it's being created)
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.metadata.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.sale.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.token_account.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.hash_registry.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        ctx.accounts.token_program.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        ctx.accounts.system_program.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        ctx.accounts.rent.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        ctx.accounts.associated_token_program.key(),
                        false,
                    ),
                ],
                data,
            },
            &[
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.metadata.to_account_info(),
                ctx.accounts.sale.to_account_info(),
                ctx.accounts.token_account.to_account_info(),
                ctx.accounts.hash_registry.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.rent.to_account_info(),
                ctx.accounts.associated_token_program.to_account_info(),
            ],
        )?;

        // Step 3: Update TokenManagementData after successful mint
        let token_data = &mut ctx.accounts.token_management_data;
        token_data.mint = ctx.accounts.mint.key();
        token_data.owner = ctx.accounts.payer.key();
        token_data.on_sale = false;
        token_data.price = 0;
        token_data.sale_fee = 0;
        token_data.minted_through_management = true;
        token_data.last_operation = LastOperation::Mint;
        token_data.last_operation_timestamp = Clock::get()?.unix_timestamp;

        emit!(ManagedOperationExecuted {
            mint: ctx.accounts.mint.key(),
            operation: LastOperation::Mint,
            user: ctx.accounts.payer.key(),
            tariff_collected: tariffs.mint,
        });

        Ok(())
    }

    /// Managed set_sale operation
    /// Sets token sale status through sevens-token with tariff collection
    pub fn managed_set_sale(
        ctx: Context<ManagedSetSale>,
        on_sale: bool,
        price: u64,
    ) -> Result<()> {
        let tariffs = &ctx.accounts.tariffs;

        // Check if operations are paused
        require!(!tariffs.paused, ManagementError::OperationsPaused);

        // Verify owner
        require!(
            ctx.accounts.owner.key() == ctx.accounts.token_account.owner,
            ManagementError::NotTokenOwner
        );

        // Verify token amount
        require!(
            ctx.accounts.token_account.amount == 1,
            ManagementError::NoTokens
        );

        // Validate price if setting on sale
        if on_sale {
            require!(price > 0, ManagementError::InvalidPrice);
        }

        // Collect tariff fee
        if tariffs.set_sale > 0 {
            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    system_program::Transfer {
                        from: ctx.accounts.owner.to_account_info(),
                        to: ctx.accounts.target_wallet.to_account_info(),
                    },
                ),
                tariffs.set_sale,
            )?;
        }

        // Freeze the sale fee from tariffs at listing time
        let sale_fee = if on_sale { tariffs.buy } else { 0 };

        // Call sevens-token set_sale via CPI
        // Create instruction data for set_sale
        let mut data = Vec::with_capacity(17);
        // Instruction discriminator for set_sale (first 8 bytes of sha256("global:set_sale"))
        data.extend_from_slice(&[0x7b, 0x75, 0xc4, 0x86, 0x60, 0x65, 0xff, 0x4d]);
        // on_sale (bool - 1 byte)
        data.push(if on_sale { 1 } else { 0 });
        // price (u64 - 8 bytes, little endian)
        data.extend_from_slice(&price.to_le_bytes());

        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::instruction::Instruction {
                program_id: ctx.accounts.sevens_token_program.key(),
                accounts: vec![
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.owner.key(),
                        true,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.mint.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.token_account.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.sale.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        ctx.accounts.sale_authority.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        ctx.accounts.token_program.key(),
                        false,
                    ),
                ],
                data,
            },
            &[
                ctx.accounts.owner.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.token_account.to_account_info(),
                ctx.accounts.sale.to_account_info(),
                ctx.accounts.sale_authority.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
        )?;

        // Update TokenManagementData
        let token_data = &mut ctx.accounts.token_management_data;

        // Initialize mint if this is first time (when PDA is created)
        if token_data.mint == Pubkey::default() {
            token_data.mint = ctx.accounts.mint.key();
            token_data.minted_through_management = false;
        }

        token_data.on_sale = on_sale;
        token_data.price = price;
        token_data.sale_fee = sale_fee;
        token_data.last_operation = LastOperation::SetSale;
        token_data.last_operation_timestamp = Clock::get()?.unix_timestamp;

        // Update owner if changed
        token_data.owner = ctx.accounts.token_account.owner;

        emit!(ManagedOperationExecuted {
            mint: ctx.accounts.mint.key(),
            operation: LastOperation::SetSale,
            user: ctx.accounts.owner.key(),
            tariff_collected: tariffs.set_sale,
        });

        Ok(())
    }

    /// Managed buy operation
    /// Buys a token through sevens-token with tariff collection
    pub fn managed_buy(
        ctx: Context<ManagedBuy>,
        expected_price: u64,
    ) -> Result<()> {
        let tariffs = &ctx.accounts.tariffs;

        // Check if operations are paused
        require!(!tariffs.paused, ManagementError::OperationsPaused);

        // Verify token is on sale
        let token_data = &ctx.accounts.token_management_data;
        require!(token_data.on_sale, ManagementError::TokenNotForSale);

        // Verify price matches
        require!(
            token_data.price == expected_price,
            ManagementError::PriceMismatch
        );

        // Calculate buy fee using frozen sale_fee (percentage of price)
        let buy_fee_amount = (token_data.price as u128)
            .checked_mul(token_data.sale_fee as u128)
            .ok_or(ManagementError::MathOverflow)?
            .checked_div(100)
            .ok_or(ManagementError::MathOverflow)? as u64;

        // Step 1: Transfer buy fee from buyer to target wallet (before buy_token call)
        if buy_fee_amount > 0 {
            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    system_program::Transfer {
                        from: ctx.accounts.buyer.to_account_info(),
                        to: ctx.accounts.target_wallet.to_account_info(),
                    },
                ),
                buy_fee_amount,
            )?;
        }

        // Step 2: Call sevens-token buy_token to handle token transfer and sale update
        // buy_token will: transfer price from buyer to seller, transfer token, and set sale to false
        let mut data = Vec::with_capacity(16);
        // Instruction discriminator for buy_token (sha256("global:buy_token")[0..8])
        data.extend_from_slice(&[0x8a, 0x7f, 0x0e, 0x5b, 0x26, 0x57, 0x73, 0x69]);
        // lamports (u64) - base price without fee
        data.extend_from_slice(&token_data.price.to_le_bytes());

        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::instruction::Instruction {
                program_id: ctx.accounts.sevens_token_program.key(),
                accounts: vec![
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.buyer.key(),
                        true, // buyer must sign
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.seller.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.buyer_token_account.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.seller_token_account.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.mint.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.sale.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        ctx.accounts.sale_authority.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        ctx.accounts.token_program.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        ctx.accounts.system_program.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        ctx.accounts.associated_token_program.key(),
                        false,
                    ),
                ],
                data,
            },
            &[
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.seller.to_account_info(),
                ctx.accounts.buyer_token_account.to_account_info(),
                ctx.accounts.seller_token_account.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.sale.to_account_info(),
                ctx.accounts.sale_authority.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.associated_token_program.to_account_info(),
            ],
        )?;

        // Update TokenManagementData
        let token_data = &mut ctx.accounts.token_management_data;
        token_data.owner = ctx.accounts.buyer.key();
        token_data.on_sale = false;
        token_data.price = 0;
        token_data.sale_fee = 0;
        token_data.last_operation = LastOperation::Buy;
        token_data.last_operation_timestamp = Clock::get()?.unix_timestamp;

        emit!(ManagedOperationExecuted {
            mint: ctx.accounts.mint.key(),
            operation: LastOperation::Buy,
            user: ctx.accounts.buyer.key(),
            tariff_collected: buy_fee_amount,
        });

        emit!(TokenPurchased {
            mint: ctx.accounts.mint.key(),
            seller: ctx.accounts.seller.key(),
            buyer: ctx.accounts.buyer.key(),
            price: expected_price,
            fee: buy_fee_amount,
        });

        Ok(())
    }

    /// Managed burn operation
    /// Burns a token through sevens-token with tariff collection and PDA closure
    pub fn managed_burn(
        ctx: Context<ManagedBurn>,
    ) -> Result<()> {
        let tariffs = &ctx.accounts.tariffs;

        // Check if operations are paused
        require!(!tariffs.paused, ManagementError::OperationsPaused);

        // Verify owner
        require!(
            ctx.accounts.owner.key() == ctx.accounts.token_account.owner,
            ManagementError::NotTokenOwner
        );

        // Verify token amount
        require!(
            ctx.accounts.token_account.amount == 1,
            ManagementError::NoTokens
        );

        // Step 1: Collect tariff fee before burning
        if tariffs.burn > 0 {
            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    system_program::Transfer {
                        from: ctx.accounts.owner.to_account_info(),
                        to: ctx.accounts.target_wallet.to_account_info(),
                    },
                ),
                tariffs.burn,
            )?;
        }

        // Step 2: Call sevens-token burn_token via CPI
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&BURN_TOKEN_DISCRIMINATOR);

        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::instruction::Instruction {
                program_id: ctx.accounts.sevens_token_program.key(),
                accounts: vec![
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.owner.key(),
                        true, // owner must sign
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.mint.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.token_account.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.metadata.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.sale.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        ctx.accounts.hash_registry.key(),
                        false,
                    ),
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        ctx.accounts.token_program.key(),
                        false,
                    ),
                ],
                data,
            },
            &[
                ctx.accounts.owner.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.token_account.to_account_info(),
                ctx.accounts.metadata.to_account_info(),
                ctx.accounts.sale.to_account_info(),
                ctx.accounts.hash_registry.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
        )?;

        emit!(ManagedOperationExecuted {
            mint: ctx.accounts.mint.key(),
            operation: LastOperation::Burn,
            user: ctx.accounts.owner.key(),
            tariff_collected: tariffs.burn,
        });

        // Step 3: TokenManagementData PDA will be closed automatically by Anchor
        // Rent will be returned to owner (specified in close constraint)

        Ok(())
    }
}

// Initialization Context
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + TariffsData::MAX_SIZE,
        seeds = [b"tariffs"],
        bump,
    )]
    pub tariffs: Account<'info, TariffsData>,

    pub system_program: Program<'info, System>,
}

// Update Tariffs Context
#[derive(Accounts)]
pub struct UpdateTariffs<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"tariffs"],
        bump,
        constraint = authority.key() == tariffs.authority @ ManagementError::Unauthorized
    )]
    pub tariffs: Account<'info, TariffsData>,
}

// Close Tariffs Context
// This context is used to close orphaned tariffs PDAs from old deployments
// that may have incompatible data structures
#[derive(Accounts)]
pub struct CloseTariffs<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: This is the tariffs PDA that we want to close
    /// We use UncheckedAccount to avoid deserialization errors with old structures
    /// The seeds constraint ensures this is the correct PDA
    #[account(
        mut,
        seeds = [b"tariffs"],
        bump,
    )]
    pub tariffs: UncheckedAccount<'info>,
}

// Close Token Data Context
#[derive(Accounts)]
pub struct CloseTokenData<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    pub mint: Account<'info, Mint>,

    /// CHECK: This is the token data PDA that we want to close
    /// We use UncheckedAccount to avoid deserialization errors with old structures
    #[account(
        mut,
        seeds = [b"token_data", mint.key().as_ref()],
        bump,
    )]
    pub token_data: UncheckedAccount<'info>,
}

// Managed Mint Context
#[derive(Accounts)]
#[instruction(author: String, hash: String, description: String, token_name: String, can_be_burned: bool)]
pub struct ManagedMint<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [b"tariffs"],
        bump,
    )]
    pub tariffs: Account<'info, TariffsData>,

    /// CHECK: Target wallet for tariff collection
    #[account(
        mut,
        constraint = target_wallet.key() == tariffs.target_wallet @ ManagementError::InvalidTargetWallet
    )]
    pub target_wallet: AccountInfo<'info>,

    // Accounts for CPI to sevens-token program
    // Mint must be a Signer because it's being created
    #[account(mut)]
    pub mint: Signer<'info>,

    /// CHECK: Metadata PDA in sevens-token program
    #[account(mut)]
    pub metadata: UncheckedAccount<'info>,

    /// CHECK: Sale PDA in sevens-token program
    #[account(mut)]
    pub sale: UncheckedAccount<'info>,

    /// CHECK: Token account that will be created by sevens-token program
    #[account(mut)]
    pub token_account: UncheckedAccount<'info>,

    /// CHECK: Hash registry PDA in sevens-token program
    #[account(mut)]
    pub hash_registry: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + TokenManagementData::MAX_SIZE,
        seeds = [b"token_data", mint.key().as_ref()],
        bump,
    )]
    pub token_management_data: Account<'info, TokenManagementData>,

    /// CHECK: Sevens token program
    pub sevens_token_program: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

// Managed SetSale Context
#[derive(Accounts)]
pub struct ManagedSetSale<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [b"tariffs"],
        bump,
    )]
    pub tariffs: Account<'info, TariffsData>,

    /// CHECK: Target wallet for tariff collection
    #[account(
        mut,
        constraint = target_wallet.key() == tariffs.target_wallet @ ManagementError::InvalidTargetWallet
    )]
    pub target_wallet: AccountInfo<'info>,

    #[account(mut)]
    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = token_account.mint == mint.key() @ ManagementError::InvalidMint,
        constraint = token_account.owner == owner.key() @ ManagementError::NotTokenOwner,
    )]
    pub token_account: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = owner,
        space = 8 + TokenManagementData::MAX_SIZE,
        seeds = [b"token_data", mint.key().as_ref()],
        bump,
    )]
    pub token_management_data: Account<'info, TokenManagementData>,

    // Accounts for CPI to sevens-token program
    /// CHECK: TokenSaleData PDA in sevens-token program
    #[account(mut)]
    pub sale: UncheckedAccount<'info>,

    /// CHECK: Sale authority PDA in sevens-token program
    pub sale_authority: UncheckedAccount<'info>,

    /// CHECK: Sevens token program
    pub sevens_token_program: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// Managed Buy Context
#[derive(Accounts)]
pub struct ManagedBuy<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        seeds = [b"tariffs"],
        bump,
    )]
    pub tariffs: Account<'info, TariffsData>,

    /// CHECK: Target wallet for tariff collection
    #[account(
        mut,
        constraint = target_wallet.key() == tariffs.target_wallet @ ManagementError::InvalidTargetWallet
    )]
    pub target_wallet: AccountInfo<'info>,

    #[account(mut)]
    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [b"token_data", mint.key().as_ref()],
        bump,
    )]
    pub token_management_data: Account<'info, TokenManagementData>,

    /// CHECK: Seller account (current owner)
    #[account(
        mut,
        constraint = seller.key() == token_management_data.owner @ ManagementError::InvalidSeller
    )]
    pub seller: AccountInfo<'info>,

    /// Seller's token account
    #[account(
        mut,
        constraint = seller_token_account.mint == mint.key() @ ManagementError::InvalidMint,
        constraint = seller_token_account.owner == seller.key() @ ManagementError::InvalidSeller,
    )]
    pub seller_token_account: Account<'info, TokenAccount>,

    /// Buyer's token account (created if needed)
    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = mint,
        associated_token::authority = buyer,
    )]
    pub buyer_token_account: Account<'info, TokenAccount>,

    /// Sale PDA from sevens-token program
    /// CHECK: This is the sale PDA from sevens-token program
    #[account(mut)]
    pub sale: AccountInfo<'info>,

    /// CHECK: Sale authority PDA from sevens-token program
    pub sale_authority: AccountInfo<'info>,

    /// CHECK: Sevens-token program
    pub sevens_token_program: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

// Managed Burn Context
#[derive(Accounts)]
pub struct ManagedBurn<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [b"tariffs"],
        bump,
    )]
    pub tariffs: Account<'info, TariffsData>,

    /// CHECK: Target wallet for tariff collection
    #[account(
        mut,
        constraint = target_wallet.key() == tariffs.target_wallet @ ManagementError::InvalidTargetWallet
    )]
    pub target_wallet: AccountInfo<'info>,

    #[account(mut)]
    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = token_account.mint == mint.key() @ ManagementError::InvalidMint,
        constraint = token_account.owner == owner.key() @ ManagementError::NotTokenOwner,
    )]
    pub token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"token_data", mint.key().as_ref()],
        bump,
        close = owner, // Return rent to owner
    )]
    pub token_management_data: Account<'info, TokenManagementData>,

    // Accounts for CPI to sevens-token program
    /// CHECK: Metadata PDA in sevens-token program
    #[account(mut)]
    pub metadata: UncheckedAccount<'info>,

    /// CHECK: Sale PDA in sevens-token program
    #[account(mut)]
    pub sale: UncheckedAccount<'info>,

    /// CHECK: Hash registry PDA in sevens-token program
    #[account(mut)]
    pub hash_registry: UncheckedAccount<'info>,

    /// CHECK: Sevens token program
    pub sevens_token_program: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// Data Structures

#[account]
pub struct TariffsData {
    /// Authority that can update tariffs
    pub authority: Pubkey,       // 32
    /// Target wallet to receive fees
    pub target_wallet: Pubkey,   // 32
    /// Mint fee in lamports
    pub mint: u64,               // 8
    /// Set sale fee in lamports
    pub set_sale: u64,           // 8
    /// Buy fee as percentage (0-99%)
    pub buy: u8,                 // 1
    /// Burn fee in lamports
    pub burn: u64,               // 8
    /// Emergency pause flag
    pub paused: bool,            // 1
}

impl TariffsData {
    pub const MAX_SIZE: usize = 32 + 32 + 8 + 8 + 1 + 8 + 1;
}

#[account]
pub struct TokenManagementData {
    /// Token mint address
    pub mint: Pubkey,                    // 32
    /// Current owner
    pub owner: Pubkey,                   // 32
    /// Token sale status
    pub on_sale: bool,                   // 1
    /// Token base price in lamports (without fee)
    pub price: u64,                      // 8
    /// Sale fee percentage frozen at listing time (0-99)
    pub sale_fee: u8,                    // 1
    /// Was minted through management
    pub minted_through_management: bool, // 1
    /// Last operation type
    pub last_operation: LastOperation,   // 1
    /// Last operation timestamp
    pub last_operation_timestamp: i64,   // 8
}

impl TokenManagementData {
    pub const MAX_SIZE: usize = 32 + 32 + 1 + 8 + 1 + 1 + 1 + 8; // 84 bytes
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum LastOperation {
    Mint = 0,
    SetSale = 1,
    Buy = 2,
    Burn = 3,
}

// Events

#[event]
pub struct TariffsUpdated {
    pub authority: Pubkey,
    pub target_wallet: Pubkey,
    pub mint: u64,
    pub set_sale: u64,
    pub buy: u8,
    pub burn: u64,
}

#[event]
pub struct PauseStatusChanged {
    pub authority: Pubkey,
    pub paused: bool,
}

#[event]
pub struct TariffsClosed {
    pub authority: Pubkey,
}

#[event]
pub struct ManagedOperationExecuted {
    pub mint: Pubkey,
    pub operation: LastOperation,
    pub user: Pubkey,
    pub tariff_collected: u64,
}

#[event]
pub struct TokenPurchased {
    pub mint: Pubkey,
    pub seller: Pubkey,
    pub buyer: Pubkey,
    pub price: u64,
    pub fee: u64,
}

// Errors

#[error_code]
pub enum ManagementError {
    #[msg("Unauthorized: only the authority can update tariffs.")]
    Unauthorized,
    #[msg("Invalid buy percentage: must be between 0 and 99.")]
    InvalidBuyPercentage,
    #[msg("Invalid target wallet: cannot be the default address.")]
    InvalidTargetWallet,
    #[msg("Operations are currently paused.")]
    OperationsPaused,
    #[msg("Not the token owner.")]
    NotTokenOwner,
    #[msg("No tokens in account.")]
    NoTokens,
    #[msg("Invalid price: must be greater than 0 when setting on sale.")]
    InvalidPrice,
    #[msg("Token is not for sale.")]
    TokenNotForSale,
    #[msg("Price mismatch: expected price doesn't match current price.")]
    PriceMismatch,
    #[msg("Invalid mint address.")]
    InvalidMint,
    #[msg("Invalid seller address.")]
    InvalidSeller,
    #[msg("Math overflow occurred.")]
    MathOverflow,
    #[msg("Insufficient funds to pay tariff fee.")]
    InsufficientFundsForTariff,
    #[msg("Insufficient funds to mint token (not enough for rent and fees).")]
    InsufficientFundsForMint,
}

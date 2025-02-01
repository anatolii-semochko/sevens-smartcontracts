use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Approve, Burn, CloseAccount, Mint, MintTo, Revoke, Token, TokenAccount, Transfer};
use anchor_lang::system_program;
use anchor_lang::solana_program::hash::{hash as solana_hash};

declare_id!("Ah4sw8i5k74TC7tCzSrqkEitNdQVRhgrPsKfUrhqzEbn");

pub const MAX_AUTHOR_LENGTH: usize = 32;
pub const MAX_HASH_LENGTH: usize = 64;
pub const MAX_DESCRIPTION_LENGTH: usize = 128;
pub const MAX_TOKEN_NAME_LENGTH: usize = 32;

#[program]
pub mod sevens_token {
    use super::*;

    #[constant]
    pub const MAX_AUTHOR_LENGTH: usize = 32;
    #[constant]
    pub const MAX_HASH_LENGTH: usize = 64;
    #[constant]
    pub const MAX_DESCRIPTION_LENGTH: usize = 128;
    #[constant]
    pub const MAX_TOKEN_NAME_LENGTH: usize = 32;

    pub fn mint_token(
        ctx: Context<MintToken>,
        author: String,
        hash: String,
        description: String,
        token_name: String,
        can_be_burned: bool,
    ) -> Result<()> {
        require!(description.len() <= MAX_DESCRIPTION_LENGTH, TrustDataError::DescriptionTooLong);
        require!(author.len() <= MAX_AUTHOR_LENGTH, TrustDataError::AuthorTooLong);
        require!(hash.len() <= MAX_HASH_LENGTH, TrustDataError::HashTooLong);
        require!(token_name.len() <= MAX_TOKEN_NAME_LENGTH, TrustDataError::TokenNameTooLong);
        require!(!token_name.trim().is_empty(), TrustDataError::TokenNameEmpty);
        require!(!hash.trim().is_empty(), TrustDataError::HashEmpty);
        require!(hash.chars().all(|c| c.is_ascii_hexdigit()), TrustDataError::InvalidHashFormat);

        let hash_registry = &mut ctx.accounts.hash_registry;
        if !hash_registry.hash.is_empty() {
            msg!("Hash already exists. Existing mint key: {}", hash_registry.mint_key);
            return Err(TrustDataError::HashAlreadyExists.into());
        }

        hash_registry.hash = hash.clone();
        hash_registry.mint_key = ctx.accounts.mint.key();

        let metadata = &mut ctx.accounts.metadata;
        metadata.author = author;
        metadata.hash = hash;
        metadata.description = description;
        metadata.token_name = token_name;
        metadata.can_be_burned = can_be_burned;
        metadata.timestamp = Clock::get()?.unix_timestamp;

        let sale = &mut ctx.accounts.sale;
        sale.on_sale = false;
        sale.price = 0;

        token::mint_to(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.token_account.to_account_info(),
                    authority: ctx.accounts.payer_account.to_account_info(),
                },
            ),
            1,
        )?;

        emit!(TokenMinted {
            mint: ctx.accounts.mint.key(),
            timestamp: metadata.timestamp,
        });

        Ok(())
    }

    pub fn set_sale(ctx: Context<SetSale>, on_sale: bool, price: u64) -> Result<()> {
        let token_account = &ctx.accounts.token_account;
        require!(token_account.amount == 1, TrustDataError::NoTokens);

        let sale = &mut ctx.accounts.sale;

        if on_sale {
            require!(price > 0, TrustDataError::InvalidPrice);
            token::approve(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Approve {
                        to: token_account.to_account_info(),
                        delegate: ctx.accounts.sale_authority.to_account_info(),
                        authority: ctx.accounts.owner_account.to_account_info(),
                    },
                ),
                1,
            )?;
            sale.price = price;
            sale.on_sale = true;
            emit!(TokenListed {
                mint: ctx.accounts.mint.key(),
                price,
            });
        } else {
            token::revoke(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Revoke {
                        source: token_account.to_account_info(),
                        authority: ctx.accounts.owner_account.to_account_info(),
                    },
                ),
            )?;
            sale.price = 0;
            sale.on_sale = false;
        }

        Ok(())
    }

    pub fn buy_token(ctx: Context<BuyToken>, lamports: u64) -> Result<()> {
        let sale = &mut ctx.accounts.sale;
        require!(sale.on_sale, TrustDataError::NotOnSale);
        require!(lamports >= sale.price, TrustDataError::InsufficientPayment);
        
        // Reentrancy protection: change state before external calls
        sale.on_sale = false;
        sale.price = 0;

        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.buyer_account.to_account_info(),
                    to: ctx.accounts.owner_account.to_account_info(),
                },
            ),
            lamports,
        )?;

        let bump = *ctx.bumps.get("sale_authority").unwrap();
        let mint_key = ctx.accounts.mint.key();
        let signer_seeds: &[&[u8]] = &[b"sale", mint_key.as_ref(), &[bump]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.owner_token_account.to_account_info(),
                    to: ctx.accounts.buyer_token_account.to_account_info(),
                    authority: ctx.accounts.sale_authority.to_account_info(),
                },
                &[signer_seeds],
            ),
            1,
        )?;

        emit!(TokenSold {
            mint: ctx.accounts.mint.key(),
            buyer: ctx.accounts.buyer_account.key(),
            price: lamports,
        });

        Ok(())
    }

    pub fn burn_token(ctx: Context<BurnToken>) -> Result<()> {
        let metadata = &ctx.accounts.metadata;
        require!(metadata.can_be_burned, TrustDataError::BurnNotAllowed);
        require!(ctx.accounts.token_account.amount == 1, TrustDataError::NoTokens);

        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.mint.to_account_info(),
                    from: ctx.accounts.token_account.to_account_info(),
                    authority: ctx.accounts.payer_account.to_account_info(),
                },
            ),
            1,
        )?;

        token::close_account(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                CloseAccount {
                    account: ctx.accounts.token_account.to_account_info(),
                    destination: ctx.accounts.payer_account.to_account_info(),
                    authority: ctx.accounts.payer_account.to_account_info(),
                },
            ),
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct BurnToken<'info> {
    #[account(mut)]
    pub payer_account: Signer<'info>,

    #[account(mut)]
    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = payer_account,
    )]
    pub token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"metadata", mint.key().as_ref()],
        bump,
        close = payer_account
    )]
    pub metadata: Account<'info, TrustDataMetadata>,

    #[account(
        mut,
        seeds = [b"sale", mint.key().as_ref()],
        bump,
        close = payer_account
    )]
    pub sale: Account<'info, TokenSaleData>,

    #[account(
        mut,
        seeds = [b"hash", &solana_hash(metadata.hash.as_bytes()).to_bytes()[..28]],
        bump,
        close = payer_account
    )]
    pub hash_registry: Account<'info, HashRegistry>,


    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(author: String, hash: String, description: String, token_name: String, can_be_burned: bool)]
pub struct MintToken<'info> {
    #[account(mut)]
    pub payer_account: Signer<'info>,

    #[account(
        init,
        payer = payer_account,
        mint::decimals = 0,
        mint::authority = payer_account,
        mint::freeze_authority = payer_account,
    )]
    pub mint: Account<'info, Mint>,

    #[account(
        init,
        seeds = [b"metadata", mint.key().as_ref()],
        bump,
        payer = payer_account,
        space = 8 + TrustDataMetadata::MAX_SIZE,
    )]
    pub metadata: Account<'info, TrustDataMetadata>,

    #[account(
        init,
        seeds = [b"sale", mint.key().as_ref()],
        bump,
        payer = payer_account,
        space = 8 + TokenSaleData::MAX_SIZE,
    )]
    pub sale: Account<'info, TokenSaleData>,

    #[account(
        init_if_needed,
        payer = payer_account,
        associated_token::mint = mint,
        associated_token::authority = payer_account,
    )]
    pub token_account: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = payer_account,
        space = 8 + HashRegistry::MAX_SIZE,
        seeds = [b"hash", &solana_hash(hash.as_bytes()).to_bytes()[..28]],
        bump
    )]
    pub hash_registry: Account<'info, HashRegistry>,


    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[account]
pub struct TrustDataMetadata {
    pub token_name: String,
    pub hash: String,
    pub author: String,
    pub description: String,
    pub can_be_burned: bool,
    pub timestamp: i64,
}

impl TrustDataMetadata {
    pub const MAX_SIZE: usize =
        4 + 32 +    // token_name
        4 + 64 +    // hash
        4 + 32 +    // author
        4 + 128 +   // description
        1 +         // can_be_burned
        8;          // timestamp
}

#[account]
pub struct TokenSaleData {
    pub on_sale: bool,
    pub price: u64,
}

impl TokenSaleData {
    pub const MAX_SIZE: usize = 1 + 8;
}

#[account]
pub struct HashRegistry {
    pub hash: String,
    pub mint_key: Pubkey,
}

impl HashRegistry {
    pub const MAX_SIZE: usize = 4 + 64 + 32; // String hash + Pubkey
}


#[derive(Accounts)]
pub struct SetSale<'info> {
    #[account(mut)]
    pub owner_account: Signer<'info>,

    #[account(mut)]
    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = owner_account,
    )]
    pub token_account: Account<'info, TokenAccount>,

    #[account(mut, seeds = [b"sale", mint.key().as_ref()], bump)]
    pub sale: Account<'info, TokenSaleData>,

    /// CHECK: PDA used as authority for token transfers, derived from mint
    #[account(seeds = [b"sale", mint.key().as_ref()], bump)]
    pub sale_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct BuyToken<'info> {
    #[account(mut)]
    pub buyer_account: Signer<'info>,

    /// CHECK: This account is validated to match owner_token_account.owner via constraint
    #[account(mut, constraint = owner_account.key() == owner_token_account.owner @ TrustDataError::InvalidTokenOwner)]
    pub owner_account: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = buyer_account,
        associated_token::mint = mint,
        associated_token::authority = buyer_account,
    )]
    pub buyer_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = owner_account,
    )]
    pub owner_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub mint: Account<'info, Mint>,

    #[account(mut, seeds = [b"sale", mint.key().as_ref()], bump)]
    pub sale: Account<'info, TokenSaleData>,

    /// CHECK: PDA used as authority for token transfers, derived from mint
    #[account(seeds = [b"sale", mint.key().as_ref()], bump)]
    pub sale_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[event]
pub struct TokenMinted {
    pub mint: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct TokenListed {
    pub mint: Pubkey,
    pub price: u64,
}

#[event]
pub struct TokenSold {
    pub mint: Pubkey,
    pub buyer: Pubkey,
    pub price: u64,
}

#[error_code]
pub enum TrustDataError {
    #[msg("Description too long (max 128 characters).")]
    DescriptionTooLong,
    #[msg("Author name too long (max 32 characters).")]
    AuthorTooLong,
    #[msg("Hash too long (max 64 characters).")]
    HashTooLong,
    #[msg("Token name too long (max 32 characters).")]
    TokenNameTooLong,
    #[msg("Burn not allowed for this token.")]
    BurnNotAllowed,
    #[msg("Unauthorized: only token owner can perform this action.")]
    Unauthorized,
    #[msg("Token account does not match provided mint.")]
    InvalidMint,
    #[msg("No tokens available.")]
    NoTokens,
    #[msg("Token is not on sale.")]
    NotOnSale,
    #[msg("Insufficient lamports sent for purchase.")]
    InsufficientPayment,
    #[msg("Hash already exists")]
    HashAlreadyExists,
    #[msg("Invalid token owner: requested owner_token does not belong to owner")]
    InvalidTokenOwner,
    #[msg("Token name cannot be empty")]
    TokenNameEmpty,
    #[msg("Hash cannot be empty")]
    HashEmpty,
    #[msg("Hash must contain only hexadecimal characters")]
    InvalidHashFormat,
    #[msg("Price must be greater than 0")]
    InvalidPrice,
}

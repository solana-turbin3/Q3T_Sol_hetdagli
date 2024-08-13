use anchor_lang::prelude::*;
use anchor_spl::metadata::mpl_token_metadata::instructions::{FreezeDelegatedAccountCpi, FreezeDelegatedAccountCpiAccounts, ThawDelegatedAccountCpi, ThawDelegatedAccountCpiAccounts};
use anchor_spl::token::{approve, Approve, Mint, Token, TokenAccount};
use anchor_spl::metadata::{Metadata, MetadataAccount, MasterEditionAccount};
use crate::state::*;
use crate::error::ErrorCode;

#[derive(Accounts)]
#[instruction(stake_account_bump: u8)]
pub struct Stake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub mint: Account<'info, Mint>,
    pub collection: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = user
    )]
    pub mint_ata: Account<'info, TokenAccount>,
    #[account(
        seeds = [
            b"metadata".as_ref(), 
            metadata_program.key().as_ref(),
            mint.key().as_ref(),
        ],
        seeds::program = metadata_program.key(),
        constraint = metadata.collection.as_ref().unwrap().key.as_ref() == collection.key().as_ref(),
        constraint = metadata.collection.as_ref().unwrap().verified == true,
        bump
    )]  
    pub metadata: Account<'info, MetadataAccount>,
    #[account(
        seeds = [
            b"metadata".as_ref(), 
            metadata_program.key().as_ref(),
            mint.key().as_ref(),
            b"edition".as_ref(),
        ],
        seeds::program = metadata_program.key(),
        bump
    )]  
    pub edition: Account<'info, MasterEditionAccount>,
    pub config: Account<'info, StakeConfig>,
    #[account(
        mut,
        seeds = [b"user".as_ref(), user.key().as_ref()],
        bump = user_account.bump
    )]
    pub user_account: Account<'info, UserAccount>,
    #[account(
        init,
        payer = user,
        space = StakeAccount::INIT_SPACE,
        seeds = [b"stake".as_ref(), mint.key().as_ref(), config.key().as_ref()],
        bump
    )]
    pub stake_account: Account<'info, StakeAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub metadata_program: Program<'info, Metadata>,
}

impl<'info> Stake<'info> {
    pub fn stake(&mut self, bumps: &StakeBumps) -> Result<()> {

        require!(self.user_account.amount_staked < self.config.max_stake, ErrorCode::MaxStakeReached);

        let cpi_program = self.token_program.to_account_info();
        let cpi_accounts = Approve {
            to: self.mint_ata.to_account_info(),
            delegate: self.stake_account.to_account_info(),
            authority: self.user.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        approve(cpi_ctx, 1)?;

        let delegate = &self.stake_account.to_account_info();
        let token_account = &self.mint_ata.to_account_info();
        let edition = &self.edition.to_account_info();
        let mint = &self.mint.to_account_info();
        let token_program = &self.token_program.to_account_info();
        let metadata_program = &self.metadata_program.to_account_info();

        FreezeDelegatedAccountCpi::new(
            metadata_program, 
            FreezeDelegatedAccountCpiAccounts {
                delegate,
                token_account,
                edition,
                mint,
                token_program,
            }
        ).invoke()?;

        self.stake_account.set_inner(StakeAccount {
            owner: self.user.key(),
            mint: self.mint.key(),
            last_updated: Clock::get()?.unix_timestamp,
            bump: bumps.stake_account,
        });

        self.user_account.amount_staked += 1;

        Ok(())
    }

    pub fn unstake(&mut self, bumps: &StakeBumps) -> Result<()> {
        //to unstake we need to thaw the account that was frozen above
        //need to transfer the nft to the user
        //need to update the user account subtract 1 from amount_staked
        //need to close the stake account
        // Thaw the frozen account
        require!(self.user_account.amount_staked > 0, ErrorCode::NoStake);

        let delegate = &self.stake_account.to_account_info();
        let token_account = &self.mint_ata.to_account_info();
        let edition = &self.edition.to_account_info();
        let mint = &self.mint.to_account_info();
        let token_program = &self.token_program.to_account_info();
        let metadata_program = &self.metadata_program.to_account_info();

        let binding = self.mint.key();
        let seeds = &[
            b"stake".as_ref(),
            self.user.key.as_ref(),
            binding.as_ref(),
            &[bumps.stake_account],
        ];

        let signer_seeds = &[&seeds[..]];

        ThawDelegatedAccountCpi::new(
            metadata_program,
            ThawDelegatedAccountCpiAccounts {
                delegate,
                token_account,
                edition,
                mint,
                token_program,
            }
        ).invoke_signed(signer_seeds)?;

        // Tansfer the authority to the user
        let cpi_program = self.token_program.to_account_info();
        let cpi_accounts = Approve {
            to: self.mint_ata.to_account_info(),
            delegate: self.user.to_account_info(),
            authority: self.stake_account.to_account_info(),
        };


        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
        approve(cpi_ctx, 1)?;

        // Update user account
        self.user_account.amount_staked -= 1;

        Ok(())
    }

    pub fn claim(&mut self, bumps: &StakeBumps) -> Result<()> {
        //this is for user to claim rewards
        //Rewards are calculated based on the amount of time that has passed since the last_updated time_stamp, the longer the better
        //We would be minting new tokens for the reward use MintTo
        Ok(())
    }
}
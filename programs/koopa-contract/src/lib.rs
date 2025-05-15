//lib.rs
use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

pub mod errors;
pub mod events;
pub mod state;
pub mod utils;

use errors::*;
use events::*;
use state::*;
use utils::*;

// This is your program's public key and it will update
// automatically when you build the project.

declare_id!("5upMRrwYFpvhkfmyUfb9Eun2EPWWu4XyBpkBLfUK2Tgm");

#[program]
mod koopa {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, fee_percentage: u8) -> Result<()> {
        require!(fee_percentage <= 100, KooPaaError::InvalidFeePercentage);

        let global_state = &mut ctx.accounts.global_state;

        global_state.total_groups = 0;
        global_state.total_revenue = 0;
        global_state.active_groups = 0;
        global_state.completed_groups = 0;
        global_state.admin = ctx.accounts.admin.key();
        global_state.fee_percentage = fee_percentage;

        // Set fixed security deposit amounts in USDC with 6 decimals
        global_state.creator_security_deposit = 5_000_000; // 5 USDC
        global_state.joiner_security_deposit = 2_000_000; // 2 USDC

        global_state.bumps = ctx.bumps.global_state;

        Ok(())
    }

    pub fn create_ajo_group(
        ctx: Context<CreateAjoGroup>,
        name: String,
        contribution_amount: u64,
        contribution_interval: u16,
        payout_interval: u16,
        num_participants: u8,
    ) -> Result<()> {
        require!(
            contribution_amount > 0,
            KooPaaError::InvalidContributionAmount
        );
        require!(
            contribution_interval > 0 && contribution_interval <= 90,
            KooPaaError::InvalidInterval
        );
        require!(
            payout_interval >= 7 && payout_interval <= 90,
            KooPaaError::InvalidInterval
        );
        require!(
            num_participants >= 3 && num_participants <= 20,
            KooPaaError::InvalidParticipantCount
        );
        require!(name.len() <= 50, KooPaaError::NameTooLong);

        // Use the fixed security deposit from global state
        let security_deposit = ctx.accounts.global_state.creator_security_deposit;

        // Transfer security deposit from creator to the vault
        let transfer_accounts = Transfer {
            from: ctx.accounts.creator_token_account.to_account_info(),
            to: ctx.accounts.group_token_vault.to_account_info(),
            authority: ctx.accounts.creator.to_account_info(),
        };

        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                transfer_accounts,
            ),
            security_deposit,
        )?;

        let group = &mut ctx.accounts.ajo_group;
        let creator = &ctx.accounts.creator;
        let global_state = &mut ctx.accounts.global_state;
        let clock = Clock::get()?;

        group.name = name.clone();
        group.contribution_amount = contribution_amount;
        group.contribution_interval = contribution_interval;
        group.security_deposit = security_deposit;
        group.payout_interval = payout_interval;
        group.num_participants = num_participants - 1;

        group.participants = vec![AjoParticipant {
            pubkey: creator.key(),
            claim_round: 0,
            contribution_round: 0,
            bump: ctx.bumps.group_token_vault,
        }];
        group.payout_round = 0;
        group.start_timestamp = None;
        group.close_votes = vec![];
        group.is_closed = false;
        group.bumps = ctx.bumps.ajo_group;

        global_state.total_groups += 1;

        emit!(AjoGroupCreatedEvent {
            group_name: name.clone(),
            security_deposit,
            contribution_amount,
            num_participants,
            contribution_interval,
            payout_interval,
        });

        emit!(ParticipantJoinedEvent {
            group_name: name,
            participant: creator.key(),
            join_timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    pub fn join_ajo_group(ctx: Context<JoinAjoGroup>) -> Result<()> {
        let group = &mut ctx.accounts.ajo_group;
        let global_state = &mut ctx.accounts.global_state;
        let participant = &ctx.accounts.participant;
        let clock = Clock::get()?;

        // Use the joiner security deposit from global state
        let security_deposit = global_state.joiner_security_deposit;

        // Transfer security deposit from participant to the vault
        let transfer_accounts = Transfer {
            from: ctx.accounts.participant_token_account.to_account_info(),
            to: ctx.accounts.group_token_vault.to_account_info(),
            authority: participant.to_account_info(),
        };

        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                transfer_accounts,
            ),
            security_deposit,
        )?;

        require!(
            group.start_timestamp.is_none(),
            KooPaaError::GroupAlreadyStarted
        );

        let already_joined = group
            .participants
            .iter()
            .any(|p| p.pubkey == participant.key());

        require!(!already_joined, KooPaaError::AlreadyJoined);

        group.participants.push(AjoParticipant {
            pubkey: participant.key(),
            claim_round: 0,
            contribution_round: 0,
            bump: ctx.bumps.group_token_vault,
        });

        if group.participants.len() == group.num_participants as usize {
            group.start_timestamp = Some(clock.unix_timestamp);
            global_state.active_groups += 1;
        }

        emit!(ParticipantJoinedEvent {
            group_name: group.name.clone(),
            participant: participant.key(),
            join_timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    pub fn contribute(ctx: Context<Contribute>) -> Result<()> {
        let group = &mut ctx.accounts.ajo_group;
        let contributor = &ctx.accounts.contributor;
        let clock = Clock::get()?;

        require!(
            group.start_timestamp.is_some(),
            KooPaaError::GroupNotStarted
        );

        // Get all values we need before the mutable borrow
        let start_timestamp = group.start_timestamp.unwrap();
        let contribution_interval = group.contribution_interval;
        let contribution_amount = group.contribution_amount;

        // Now find the participant and create a mutable reference
        let participant = group
            .participants
            .iter_mut()
            .find(|p| p.pubkey == contributor.key())
            .ok_or(KooPaaError::NotParticipant)?;

        let time_since_start = clock.unix_timestamp - start_timestamp;
        let current_round = (time_since_start / contribution_interval as i64) as u8;

        let last_paid_round = participant.contribution_round;
        require!(
            last_paid_round < current_round,
            KooPaaError::AlreadyContributed
        );

        let rounds_missed = current_round - last_paid_round;
        let transfer_amount = contribution_amount * rounds_missed as u64;

        // Transfer tokens from contributor to the group vault
        let transfer_accounts = Transfer {
            from: ctx.accounts.contributor_token_account.to_account_info(),
            to: ctx.accounts.group_token_vault.to_account_info(),
            authority: contributor.to_account_info(),
        };

        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                transfer_accounts,
            ),
            transfer_amount,
        )?;

        participant.contribution_round = current_round;

        emit!(ContributionMadeEvent {
            group_name: group.name.clone(),
            contributor: contributor.key(),
            contribution_amount: transfer_amount,
            current_round,
        });

        Ok(())
    }

    pub fn claim_round(ctx: Context<ClaimRound>) -> Result<()> {
        let group = &mut ctx.accounts.ajo_group;
        let recipient = &ctx.accounts.recipient;
        let clock = Clock::get()?;

        // Check if the group has started
        require!(
            group.start_timestamp.is_some(),
            KooPaaError::GroupNotStarted
        );

        // Check if the group is closed
        require!(!group.is_closed, KooPaaError::GroupAlreadyClosed);

        // Find the recipient in the group
        let recipient_index = group
            .participants
            .iter()
            .position(|p| p.pubkey == recipient.key())
            .ok_or(KooPaaError::NotParticipant)?;

        // Calculate current round based on time elapsed
        let time_since_start = clock.unix_timestamp - group.start_timestamp.unwrap();
        let payout_interval_secs = group.payout_interval as i64 * 86400; // Convert days to seconds
        let current_round = (time_since_start / payout_interval_secs) as u8;

        // Check if this is the recipient's turn
        let expected_recipient_index = (group.payout_round as usize) % group.participants.len();
        require!(
            recipient_index == expected_recipient_index,
            KooPaaError::NotCurrentRecipient
        );

        // Check if all participants have contributed for this round
        let all_contributed = group
            .participants
            .iter()
            .all(|p| p.contribution_round >= current_round);

        require!(all_contributed, KooPaaError::NotAllContributed);

        // Calculate the total amount to be claimed
        let claim_amount = group.contribution_amount * (group.participants.len() as u64);

        // Transfer the funds (handled in a separate payout instruction)
        // This function just checks eligibility

        Ok(())
    }

    pub fn payout(ctx: Context<Payout>) -> Result<()> {
        let group = &mut ctx.accounts.ajo_group;
        let clock = Clock::get()?;

        let start_timestamp = group.start_timestamp.ok_or(KooPaaError::GroupNotStarted)?;
        let time_since_start = clock.unix_timestamp - start_timestamp;

        let payout_interval_secs = group.payout_interval as i64 * 86400; // Convert days to seconds
        let expected_round = (time_since_start / payout_interval_secs) as u8;

        require!(
            group.payout_round < expected_round,
            KooPaaError::PayoutNotYetDue
        );

        let num_participants = group.participants.len() as u8;
        let recipient_index = (group.payout_round as usize) % (num_participants as usize);
        let recipient_pubkey = group.participants[recipient_index].pubkey;

        // Verify the recipient is the correct one
        require!(
            recipient_pubkey == ctx.accounts.recipient.key(),
            KooPaaError::NotCurrentRecipient
        );

        let transfer_accounts = Transfer {
            from: ctx.accounts.group_token_vault.to_account_info(),
            to: ctx.accounts.recipient_token_account.to_account_info(),
            authority: ctx.accounts.group_signer.to_account_info(),
        };

        // Each participant contributes the contribution_amount
        let payout_amount = group.contribution_amount * (num_participants as u64);

        // Get the correct seeds for the vault PDA
        let group_name = group.name.clone();
        let signer_seeds = &[
            b"group-vault",
            group_name.as_bytes(),
            &[ctx.bumps.group_signer],
        ];

        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                transfer_accounts,
                &[signer_seeds],
            ),
            payout_amount,
        )?;

        group.payout_round += 1;

        emit!(PayoutMadeEvent {
            group_name: group.name.clone(),
            recipient: recipient_pubkey,
            payout_amount,
            payout_round: group.payout_round,
        });

        Ok(())
    }

    pub fn close_ajo_group(ctx: Context<CloseAjoGroup>) -> Result<()> {
        let group = &mut ctx.accounts.ajo_group;
        let participant = &ctx.accounts.participant;
        let global_state = &mut ctx.accounts.global_state;

        if group.is_closed {
            return err!(KooPaaError::GroupAlreadyClosed);
        }

        // Check if the caller is a participant
        let is_participant = group
            .participants
            .iter()
            .any(|p| p.pubkey == participant.key());

        require!(is_participant, KooPaaError::NotParticipant);

        // Check if they've already voted
        let already_voted = group.close_votes.contains(&participant.key());
        if already_voted {
            return err!(KooPaaError::AlreadyVotedToClose);
        }

        // Add their vote
        group.close_votes.push(participant.key());

        let total_participants = group.participants.len();
        let total_votes = group.close_votes.len();

        // If majority votes to close
        if total_votes * 2 > total_participants {
            // Refund security deposits if group has started
            if group.start_timestamp.is_some() {
                global_state.active_groups -= 1;
            }

            // Mark group as permanently inactive
            group.is_closed = true;

            emit!(AjoGroupClosedEvent {
                group_name: group.name.clone(),
                total_votes: total_votes as u8,
                group_size: total_participants as u8,
            });
        }

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = GlobalState::SIZE,
        seeds = [b"global-state"],
        bump
    )]
    pub global_state: Account<'info, GlobalState>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(
    name: String,
    contribution_amount: u64,
    contribution_interval: u16,
    payout_interval: u16,
    num_participants: u8
)]
pub struct CreateAjoGroup<'info> {
    #[account(
        init,
        payer = creator,
        space = AjoGroup::calculate_size(&name),
        seeds = [b"ajo-group", name.as_bytes()],
        bump
    )]
    pub ajo_group: Account<'info, AjoGroup>,

    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        mut,
        seeds = [b"global-state"],
        bump = global_state.bumps
    )]
    pub global_state: Account<'info, GlobalState>,

    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = creator_token_account.owner == creator.key(),
        constraint = creator_token_account.mint == token_mint.key()
    )]
    pub creator_token_account: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = creator,
        seeds = [b"group-vault", name.as_bytes()],
        bump,
        token::mint = token_mint,
        token::authority = creator
    )]
    pub group_token_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct JoinAjoGroup<'info> {
    #[account(mut)]
    pub ajo_group: Account<'info, AjoGroup>,

    #[account(mut)]
    pub participant: Signer<'info>,

    #[account(
        mut,
        seeds = [b"global-state"],
        bump = global_state.bumps
    )]
    pub global_state: Account<'info, GlobalState>,

    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = participant_token_account.owner == participant.key(),
        constraint = participant_token_account.mint == token_mint.key()
    )]
    pub participant_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"group-vault", ajo_group.key().as_ref()],
        bump
    )]
    pub group_token_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Contribute<'info> {
    #[account(mut)]
    pub ajo_group: Account<'info, AjoGroup>,

    pub contributor: Signer<'info>,

    #[account(
        mut,
        constraint = contributor_token_account.owner == contributor.key(),
        constraint = contributor_token_account.mint == token_mint.key(),
    )]
    pub contributor_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"group-vault", ajo_group.key().as_ref()],
        bump
    )]
    pub group_token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"global-state"],
        bump = global_state.bumps
    )]
    pub global_state: Account<'info, GlobalState>,

    pub token_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimRound<'info> {
    #[account(mut)]
    pub ajo_group: Account<'info, AjoGroup>,

    pub recipient: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Payout<'info> {
    #[account(mut)]
    pub ajo_group: Account<'info, AjoGroup>,

    #[account(
        seeds = [b"group-vault", ajo_group.name.as_bytes()],
        bump,
    )]
    /// CHECK: This is the PDA that signs for the vault
    pub group_signer: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"group-vault", ajo_group.name.as_bytes()],
        bump,
    )]
    pub group_token_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub recipient: Signer<'info>,

    #[account(
        mut,
        constraint = recipient_token_account.owner == recipient.key(),
        constraint = recipient_token_account.mint == token_mint.key()
    )]
    pub recipient_token_account: Account<'info, TokenAccount>,

    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CloseAjoGroup<'info> {
    #[account(mut)]
    pub ajo_group: Account<'info, AjoGroup>,

    pub participant: Signer<'info>,

    #[account(
        mut,
        seeds = [b"global-state"],
        bump = global_state.bumps
    )]
    pub global_state: Account<'info, GlobalState>,

    pub system_program: Program<'info, System>,
}

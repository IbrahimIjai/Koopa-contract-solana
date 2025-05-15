//lib.rs
use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

pub mod errors;
pub mod events;
pub mod state;
pub mod utils;
pub mod events;

use errors::*;
use events::*;
use state::*;
use utils::*;
use events::*;

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
        security_deposit: u64,
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

        let group = &mut ctx.accounts.ajo_group;
        let creator = &ctx.accounts.creator;
        let global_state = &mut ctx.accounts.global_state;
        let clock = Clock::get()?;

        // Set group data
        group.name = name;
        group.contribution_amount = contribution_amount;
        group.interval_in_days = interval_in_days;
        group.num_participants = num_participants;
        group.creator = creator.key();
        group.participants = vec![];
        group.current_round = 0;
        group.started = false;
        group.completed = false;
        group.current_receiver_index = 0;
        group.total_distributed = 0;
        group.last_round_timestamp = 0;
        group.bumps = ctx.bumps.ajo_group;

        global_state.total_groups += 1;
        global_state.active_groups += 1;

        Ok(())
    }

    pub fn join_ajo_group(ctx: Context<JoinAjoGroup>) -> Result<()> {
        let group = &mut ctx.accounts.ajo_group;
        let participant = &ctx.accounts.participant;

        // Check if the group has already started
        require!(!group.started, KooPaaError::GroupAlreadyStarted);

        // Check if the group is already full
        require!(
            group.participants.len() < group.num_participants as usize,
            KooPaaError::GroupAlreadyFull
        );

        // Check if the participant is already in the group
        let already_joined = group
            .participants
            .iter()
            .any(|p| p.pubkey == participant.key());

        require!(!already_joined, KooPaaError::AlreadyJoined);

        group.participants.push(AjoParticipant {
            pubkey: participant.key(),
            turn_number: current_position,
            claim_round: current_position, // Claim order based on join position
            claimed: false,
            claim_time: 0,
            claim_amount: 0,
            rounds_contributed: vec![],
            bump: 0,
        });

        Ok(())
    }

    pub fn contribute(ctx: Context<Contribute>) -> Result<()> {
        let group = &mut ctx.accounts.ajo_group;
        let contributor = &ctx.accounts.contributor;
        let clock = Clock::get()?;

        // Check if the group has started
        require!(group.started, KooPaaError::GroupNotStarted);

        // Check if the group has completed
        require!(!group.completed, KooPaaError::GroupCompleted);

        // Find the participant in the group
        let participant_index = group
            .participants
            .iter()
            .position(|p| p.pubkey == contributor.key())
            .ok_or(KooPaaError::NotParticipant)?;

        // Find the current recipient (the one whose claim_round matches current_round)
        let recipient_index = group
            .participants
            .iter()
            .position(|p| p.claim_round == group.current_round)
            .ok_or(KooPaaError::NotCurrentRecipient)?;

        let recipient_pubkey = group.participants[recipient_index].pubkey;

        // If contributor is the current recipient, they don't need to contribute
        if contributor.key() == recipient_pubkey {
            return Ok(());
        }

        // Check if already contributed to this round
        let already_contributed = group.participants[participant_index]
            .rounds_contributed
            .contains(&group.current_round);
        require!(!already_contributed, KooPaaError::AlreadyContributed);

        // Calculate fee (if any)
        let fee_amount = calculate_fee(group.contribution_amount, global_state.fee_percentage);
        let transfer_amount = group.contribution_amount - fee_amount;

        // Transfer tokens from contributor to the current recipient
        let transfer_accounts = Transfer {
            from: ctx.accounts.contributor_token_account.to_account_info(),
            to: ctx.accounts.recipient_token_account.to_account_info(),
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

        // Check if it's the creator calling
        require!(
            group.creator == ctx.accounts.creator.key(),
            KooPaaError::OnlyCreatorCanStart
        );

        // Check if the group has started
        require!(group.started, KooPaaError::GroupNotStarted);

        // Check if the group has completed
        require!(!group.completed, KooPaaError::GroupCompleted);

        // Check if interval has passed
        let current_time = clock.unix_timestamp;
        let interval_seconds = days_to_seconds(group.interval_in_days);
        require!(
            current_time >= group.last_round_timestamp + interval_seconds,
            KooPaaError::IntervalNotPassed
        );

        // Update round information
        group.current_round += 1;
        group.last_round_timestamp = current_time;

        // Check if all rounds are completed
        if group.current_round >= group.num_participants {
            group.completed = true;
            global_state.active_groups -= 1;
            global_state.completed_groups += 1;
        }

        Ok(())
    }

    // Update global state settings (admin only)
    pub fn update_global_settings(
        ctx: Context<UpdateGlobalSettings>,
        fee_percentage: u8,
    ) -> Result<()> {
        require!(fee_percentage <= 100, KooPaaError::InvalidFeePercentage);

        let global_state = &mut ctx.accounts.global_state;
        global_state.fee_percentage = fee_percentage;

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

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StartAjoGroup<'info> {
    #[account(mut)]
    pub ajo_group: Account<'info, AjoGroup>,

    #[account(constraint = ajo_group.creator == creator.key())]
    pub creator: Signer<'info>,

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
pub struct NextRound<'info> {
    #[account(mut)]
    pub ajo_group: Account<'info, AjoGroup>,

    #[account(constraint = ajo_group.creator == creator.key())]
    pub creator: Signer<'info>,

    #[account(
        mut,
        seeds = [b"global-state"],
        bump = global_state.bumps
    )]
    pub global_state: Account<'info, GlobalState>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateGlobalSettings<'info> {
    #[account(
        mut,
        seeds = [b"global-state"],
        bump = global_state.bumps,
        constraint = global_state.admin == admin.key() @ KooPaaError::OnlyAdminCanUpdate
    )]
    pub global_state: Account<'info, GlobalState>,

    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

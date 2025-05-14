//lib.rs
use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

pub mod errors;
pub mod state;
pub mod utils;
pub mod events;

use errors::*;
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
        require!(security_deposit > 0, KooPaaError::InvalidSecurityDeposit);
        // Need to make security deposit when calling this function

        let group = &mut ctx.accounts.ajo_group;
        let creator = &ctx.accounts.creator;
        let global_state = &mut ctx.accounts.global_state;

        group.name = name;
        group.contribution_amount = contribution_amount;
        group.contribution_interval = contribution_interval;
        group.security_deposit = security_deposit;
        group.payout_interval = payout_interval;
        group.num_participants = num_participants - 1;

        group.participants = vec![AjoParticipant {
            pubkey: creator.key(),
            claim_round: 0,
            contribution_round: 0,
        }];
        group.payout_round = 0;
        group.start_timestamp = None;
        group.close_votes = vec![];
        group.is_closed = false;
        group.bumps = ctx.bumps.ajo_group;

        global_state.total_groups += 1;

        emit!(AjoGroupCreatedEvent {
            group_name: name,
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
        // let global_state = &mut ctx.accounts.global_state; -> Need this
        let participant = &ctx.accounts.participant;
        let clock = Clock::get()?;
        // Make security deposit
        
        require!(group.start_timestamp.is_none(), KooPaaError::GroupAlreadyStarted);

        let already_joined = group
            .participants
            .iter()
            .any(|p| p.pubkey == participant.key());

        require!(!already_joined, KooPaaError::AlreadyJoined);

        group.participants.push(AjoParticipant {
            pubkey: participant.key(),
            claim_round: 0,
            contribution_round: 0,
            bump: 0, // AI used this ->  bump: ctx.bumps.get("participant").copied().unwrap_or_default(),
        });

        if group.participants.len() == group.num_participants as usize {
            group.start_timestamp = Some(clock.unix_timestamp);
            // global_state.active_groups += 1; -> So I can do this
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
        let global_state = &mut ctx.accounts.global_state;
        let clock = Clock::get()?;
        
        require!(group.start_timestamp.is_some(), KooPaaError::GroupNotStarted);
        
        let participant = group
        .participants
        .iter_mut()
        .find(|p| p.pubkey == contributor.key())
        .ok_or(KooPaaError::NotParticipant)?;
        
        let time_since_start = clock.unix_timestamp - group.start_timestamp.unwrap();
        let current_round = (time_since_start / group.contribution_interval as i64) as u8;

        let last_paid_round = participant.contribution_round;
        require!(last_paid_round < current_round, KooPaaError::AlreadyContributed);

        let rounds_missed = current_round - last_paid_round;
        let transfer_amount = group.contribution_amount * rounds_missed as u64;

        // Transfer tokens from contributor to the smart contract
        let transfer_accounts = Transfer {
            from: ctx.accounts.contributor_token_account.to_account_info(),
            to: ctx.accounts.group_vault_token_account.to_account_info(), // this is wrong. help fix
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
        require!(group.started, KooPaaError::GroupNotStarted);

        // Check if the group has completed
        require!(!group.completed, KooPaaError::GroupCompleted);

        // Find the recipient in the group
        let recipient_index = group
            .participants
            .iter()
            .position(|p| p.pubkey == recipient.key())
            .ok_or(KooPaaError::NotParticipant)?;

        // Check if this is the recipient's turn
        require!(
            group.participants[recipient_index].claim_round == group.current_round,
            KooPaaError::NotCurrentRecipient
        );

        // Check if they've already claimed
        require!(
            !group.participants[recipient_index].claimed,
            KooPaaError::AlreadyClaimed
        );

        // Check if all participants have contributed
        let all_contributed = all_contributed(group);
        require!(all_contributed, KooPaaError::NotAllContributed);

        // Calculate the total amount to be claimed
        let claim_amount = calculate_round_total(group);

        // Update the recipient's claim data
        group.participants[recipient_index].claimed = true;
        group.participants[recipient_index].claim_time = clock.unix_timestamp;
        group.participants[recipient_index].claim_amount = claim_amount;

        // Update group stats
        group.total_distributed += claim_amount;

        Ok(())
    }

    pub fn payout(ctx: Context<Payout>) -> Result<()> {
        let group = &mut ctx.accounts.ajo_group;
        let clock = Clock::get()?;
    
        let start_timestamp = group.start_timestamp.ok_or(KooPaaError::GroupNotStarted)?;
        let time_since_start = clock.unix_timestamp - start_timestamp;
    
        let payout_interval_secs = group.payout_interval as i64 * 86400;
        let expected_round = (time_since_start / payout_interval_secs) as u8;
        require!(
            group.payout_round < expected_round,
            KooPaaError::PayoutNotYetDue
        );
    
        let num_participants = group.num_participants;
        let recipient_index = (group.payout_round as usize) % num_participants;
        let recipient = &group.participants[recipient_index];
    
        let transfer_accounts = Transfer {
            from: ctx.accounts.group_token_vault.to_account_info(),
            to: ctx.accounts.recipient_token_account.to_account_info(),
            authority: ctx.accounts.group_signer.to_account_info(),
        };
        let payout_amount = group.contribution_amount * (num_participants + 1);
    
        // Group signer seeds (PDA for vault authority) -> there is something fundamentally wrong here. Please review
        let signer_seeds = &[
            b"group",
            group.creator.as_ref(),
            &[group.bump],
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
            recipient: recipient.key(),
            payout_amount,
            payout_round: group.payout_round,
        });
    
        Ok(())
    }
 
    pub fn close_ajo_group(ctx: Context<CloseAjoGroup>) -> Result<()> {
        let group = &mut ctx.accounts.ajo_group;
        let caller = ctx.accounts.participant.key();
    
        if group.is_closed {
            return err!(KooPaaError::GroupAlreadyClosed);
        }
    
        require!(
            group.participants.contains(&caller),
            KooPaaError::NotParticipant
        );
    
        if group.close_votes.contains(&caller) {
            return err!(KooPaaError::AlreadyVotedToClose);
        }
    
        group.close_votes.push(caller);
    
        let total = group.participants.len();
        let votes = group.close_votes.len();
        if votes * 2 > total {
            // Need to think throung the process of refunding users if there are some, who are yet to pay contribution, or a payout
    
            // Final cleanup: refund security deposits.

            // Mark group as permanently inactive here
            group.is_closed = true;
    
            emit!(AjoGroupClosedEvent {
                group_name: group.name.clone(),
                total_votes: votes as u8,
                group_size: total as u8,
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
#[instruction(name: String)]
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

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct JoinAjoGroup<'info> {
    #[account(mut)]
    pub ajo_group: Account<'info, AjoGroup>,

    pub participant: Signer<'info>,

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
        constraint = recipient_token_account.mint == token_mint.key(),
        constraint = recipient_token_account.owner == ajo_group.participants[ajo_group.current_receiver_index as usize].pubkey,

    )]
    pub recipient_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = treasury_token_account.mint == token_mint.key(),
        constraint = treasury_token_account.owner == global_state.admin,
    )]
    pub treasury_token_account: Account<'info, TokenAccount>,

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

    /// CHECK: PDA signer for vault authority
    pub group_signer: UncheckedAccount<'info>,

    #[account(mut)]
    pub group_token_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub recipient_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

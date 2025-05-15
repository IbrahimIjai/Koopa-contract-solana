use anchor_lang::prelude::*;

#[event]
pub struct AjoGroupCreatedEvent {
    pub group_name: String,
    pub security_deposit: u64,
    pub contribution_amount: u64,
    pub num_participants: u8,
    pub contribution_interval: u16,
    pub payout_interval: u16,
}

#[event]
pub struct ParticipantJoinedEvent {
    pub group_name: String,
    pub participant: Pubkey,
    pub join_timestamp: i64,
}

#[event]
pub struct ContributionMadeEvent {
    pub group_name: String,
    pub contributor: Pubkey,
    pub contribution_amount: u64,
    pub current_round: u8,
}

#[event]
pub struct PayoutMadeEvent {
    pub group_name: String,
    pub recipient: Pubkey,
    pub payout_amount: u64,
    pub payout_round: u8,
}

#[event]
pub struct AjoGroupClosedEvent {
    pub group_name: String,
    pub total_votes: u8,
    pub group_size: u8,
}

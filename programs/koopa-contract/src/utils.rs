use crate::state::*;
use anchor_lang::prelude::*;

// Helper function to find the PDA for an Ajo group
pub fn find_group_pda(name: &str, program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"ajo-group", name.as_bytes()], program_id)
}

// Convert days to seconds
pub fn days_to_seconds(days: u16) -> i64 {
    (days as i64) * 24 * 60 * 60
}

// Calculate fee amount based on contribution
pub fn calculate_fee(amount: u64, fee_percentage: u8) -> u64 {
    // Fee is calculated as (amount * fee_percentage) / 1000
    // This allows for fractional percentages (e.g., 1 = 0.1%)
    (amount * fee_percentage as u64) / 1000
}

// Check if a participant should contribute in the current round
pub fn should_contribute(group: &AjoGroup, participant_pubkey: &Pubkey) -> bool {
    // Get the current recipient's pubkey
    let current_recipient = group
        .participants
        .iter()
        .find(|p| p.claim_round == group.payout_round)
        .map(|p| p.pubkey);

    // If participant is the current recipient, they don't need to contribute
    if let Some(recipient) = current_recipient {
        if recipient == *participant_pubkey {
            return false;
        }
    }

    // Otherwise, they should contribute
    true
}

// Calculate the total group contribution per round
pub fn calculate_round_total(group: &AjoGroup) -> u64 {
    // Total contribution is the contribution amount times the number of contributors
    // (which is all participants except the current recipient)
    group.contribution_amount * (group.participants.len() as u64 - 1)
}

// Check if all participants have contributed for the current round
pub fn all_contributed(group: &AjoGroup) -> bool {
    let current_round = group.payout_round;

    // Get the current recipient
    let recipient_index = (current_round as usize) % group.participants.len();
    let current_recipient = group.participants[recipient_index].pubkey;

    // Check if all other participants have contributed to this round
    group
        .participants
        .iter()
        .filter(|p| p.pubkey != current_recipient)
        .all(|p| p.contribution_round >= current_round)
}

use anchor_lang::prelude::*;


#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AjoParticipant {
    pub pubkey: Pubkey,
    pub claim_round: u8,
    pub contribution_round: u8,
    pub bump: u8,
}

#[account]
pub struct AjoGroup {
    // Basic group information
    pub name: String,                  // Unique name for the group
    pub security_deposit: u64           // Amount in USDC to join this group
    pub contribution_amount: u64,      // Amount in USDC to contribute each round
    pub contribution_interval: u16     // Time between rounds when a user should pay (in days)
    pub payout_interval: u16,         // Time between payouts (in days)
    pub num_participants: u8,          // Total number of participants needed
    
    // Participants and round management
    pub participants: Vec<AjoParticipant>,     // List of all participants (ordered by join time)
    pub start_timestamp: Option<i64>, 
    pub payout_round: u8, // state for payouts made, useful in calc current round, index of recipient
    
    pub close_votes: Vec<Pubkey>, // Track who has voted to close
    pub is_closed: bool,
    
    pub bumps: u8,                     // PDA bump
}

impl AjoGroup {
    // Calculate space required for account
    pub fn calculate_size(name: &str) -> usize {
        // Space for fixed fields
        let fixed_size = 8 +  // account discriminator
                         (4 + name.len()) +  // name (string)
                         8 +  // contribution_amount (u64)
                         2 +  // interval_in_days (u16)
                         1 +  // num_participants (u8)
                         32 + // creator (Pubkey)
                         4 +  // participants vector length
                         1 +  // current_round (u8)
                         1 +  // current_receiver_index (u8)
                         1 +  // started (bool)
                         1 +  // completed (bool)
                         8 +  // total_distributed (u64)
                         8 +  // last_round_timestamp (i64)
                         1;   // bumps (u8)
        
        // Space for participants (with all their data)
        // Each participant has: pubkey (32) + turn_number (1) + claim_time (8) + 
        // claimed (1) + claim_round (1) + rounds_contributed vec (4 + num_participants) +
        // claim_amount (8)
        let participant_size = 32 + 1 + 8 + 1 + 1 + (4 + 20) + 8; // ~75 bytes per participant
        let participants_size = 20 * participant_size;  // Max 20 participants
        
        fixed_size + participants_size
    }
}

#[account]
pub struct GlobalState {
    pub total_groups: u64,         // Total number of groups created
    pub total_revenue: u64,        // Total fees collected
    pub active_groups: u64,        // Number of currently active groups
    pub completed_groups: u64,     // Number of completed groups
    pub admin: Pubkey,             // Protocol admin
    pub fee_percentage: u8,        // Fee percentage (e.g., 1 = 0.1%)
    pub bumps: u8,                 // PDA bump
}

impl GlobalState {
    pub const SIZE: usize = 8 +    // discriminator
                            8 +    // total_groups
                            8 +    // total_revenue
                            8 +    // active_groups
                            8 +    // completed_groups
                            32 +   // admin
                            1 +    // fee_percentage
                            1;     // bumps
}
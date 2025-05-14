use anchor_lang::prelude::*;

#[error_code]
pub enum KooPaaError {

    #[msg("You have already claimed your payout")]
    AlreadyClaimed,

    #[msg("Not all participants have contributed yet")]
    NotAllContributed,

    #[msg("Contribution amount must be greater than zero")]
    InvalidContributionAmount,
    
    #[msg("Interval must be between 1 and 90 days")]
    InvalidInterval,
    
    #[msg("Number of participants must be between 3 and 20")]
    InvalidParticipantCount,
    
    #[msg("Group name is too long (maximum 50 characters)")]
    NameTooLong,
    
    #[msg("Group has already started")]
    GroupAlreadyStarted,
    
    #[msg("Group is already closed")]
    GroupAlreadyClosed,
    
    #[msg("You have already joined this group")]
    AlreadyJoined,
    
    #[msg("Security Deposit must be greater than 0")]
    InvalidSecurityDeposit,

    #[msg("Only admin can update global state")]
    OnlyAdminCanUpdate,
    
    #[msg("You have already voted to close this group")]
    AlreadyVotedToClose,

    #[msg("You are not a participant in this group")]
    NotParticipant,
    
    #[msg("Group has not started yet")]
    GroupNotStarted,
    
    #[msg("Group has completed all rounds")]
    GroupCompleted,
    
    #[msg("You are not a participant in this group")]
    NotAParticipant,

     #[msg("You cannot contribute to this round")]
    CannotContributeToThisRound,
    
    #[msg("Interval has not passed yet")]
    IntervalNotPassed,
    
    #[msg("Insufficient funds in token account")]
    InsufficientFunds,

    #[msg("Fee percentage must be between 0 and 100")]
    InvalidFeePercentage,

    #[msg("You have already contributed to this round")]
    AlreadyContributed,

    #[msg("You are not the recipient for this round")]
    NotCurrentRecipient,
}
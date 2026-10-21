use soroban_sdk::{contracttype, Address, String};

/// Circle configuration shared across circle-factory and circle contracts.
///
/// Consolidated from the factory-local `CircleConfig` to eliminate duplication (#303).
#[contracttype]
#[derive(Clone, Debug)]
pub struct CircleConfig {
    pub organizer: Address,
    /// The token used for contributions and payouts within this circle.
    pub token: Address,
    pub name: String,
    pub contribution_amount: i128,
    pub max_members: u32,
    pub payout_type: u32,
    pub total_rounds: u32,
    pub contribution_deadline_seconds: u64,
    pub min_moi_score: u32,
    pub collateral_amount: i128,
    pub penalty_bps: u32,
    pub grace_period_seconds: u64,
    pub max_strikes: u32,
    pub slug: String,
    pub fee_bps: u32,
}

/// Canonical ErrorEnvelope for consistent error responses across all handlers.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorEnvelope {
    pub code: u32,
    pub message: String,
    pub details: String,
    pub request_id: u64,
}

impl ErrorEnvelope {
    pub fn new(
        env: &soroban_sdk::Env,
        code: u32,
        message: &str,
        details: &str,
        request_id: u64,
    ) -> Self {
        Self {
            code,
            message: String::from_str(env, message),
            details: String::from_str(env, details),
            request_id,
        }
    }
}

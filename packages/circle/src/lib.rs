#![cfg_attr(not(test), no_std)]
// Deny unused imports at the crate level so stale imports (e.g. a previously
// re-exported `load_round_details` from payout.rs) are caught at compile time
// rather than silently emitting warnings that can be overlooked.
#![deny(unused_imports)]
#[cfg(test)]
#[path = "../../../circle.rs"]
pub mod circle_rs;
mod contract;
mod oracle;
mod payout;
#[cfg(test)]
mod test;
#[cfg(test)]
mod tests;
mod types;
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env};

pub use types::CircleError;

/// # Circle Contract
///
/// A rotating savings and credit association (ROSCA) implementation on Soroban.
/// Members contribute a fixed amount each round, and one member receives the
/// pool per round via random selection, fixed order, auction, or vote.
///
/// ## Example: Initialize a circle
///
/// ```no_run
/// use soroban_sdk::{Address, Env, String};
///
/// let env = Env::default();
/// let admin = Address::from_string(&String::from_str(&env, "GAADMIN..."));
/// let factory = Address::from_string(&String::from_str(&env, "GAFACTORY..."));
/// let token = Address::from_string(&String::from_str(&env, "GATOKEN..."));
/// let organizer = Address::from_string(&String::from_str(&env, "GAORG..."));
///
/// // CircleConfig would be constructed here in a real deployment
/// // The factory deploys the contract and calls __constructor
/// ```
///
/// ## Example: Join a circle
///
/// ```no_run
/// use soroban_sdk::{Address, Env, String};
/// use circle::CircleClient;
///
/// let env = Env::default();
/// let circle_id = Address::from_string(&String::from_str(&env, "GACIRCLE..."));
/// let member = Address::from_string(&String::from_str(&env, "GAMEMBER..."));
///
/// let client = CircleClient::new(&env, &circle_id);
/// client.join(&member);
/// ```
///
/// ## Example: Contribute to a round
///
/// ```no_run
/// use soroban_sdk::{Address, Env, String};
/// use circle::CircleClient;
///
/// let env = Env::default();
/// let circle_id = Address::from_string(&String::from_str(&env, "GACIRCLE..."));
/// let member = Address::from_string(&String::from_str(&env, "GAMEMBER..."));
///
/// let client = CircleClient::new(&env, &circle_id);
/// client.contribute(&member, &100_0000000, &1); // amount, round
/// ```
///
/// ## Example: Trigger payout for a round
///
/// ```no_run
/// use soroban_sdk::{Address, Env, String};
/// use circle::CircleClient;
///
/// let env = Env::default();
/// let circle_id = Address::from_string(&String::from_str(&env, "GACIRCLE..."));
/// let organizer = Address::from_string(&String::from_str(&env, "GAORG..."));
///
/// let client = CircleClient::new(&env, &circle_id);
/// client.trigger_payout(&organizer, &1); // round
/// ```
///
/// ## Example: Claim referral bonus
///
/// ```no_run
/// use soroban_sdk::{Address, Env, String};
/// use circle::CircleClient;
///
/// let env = Env::default();
/// let circle_id = Address::from_string(&String::from_str(&env, "GACIRCLE..."));
/// let referrer = Address::from_string(&String::from_str(&env, "GAREFERRER..."));
///
/// let client = CircleClient::new(&env, &circle_id);
/// client.claim_referral_bonus(&referrer);
/// ```
///
/// ## Example: Claim streak bonus
///
/// ```no_run
/// use soroban_sdk::{Address, Env, String};
/// use circle::CircleClient;
///
/// let env = Env::default();
/// let circle_id = Address::from_string(&String::from_str(&env, "GACIRCLE..."));
/// let member = Address::from_string(&String::from_str(&env, "GAMEMBER..."));
///
/// let client = CircleClient::new(&env, &circle_id);
/// client.claim_streak_bonus(&member);
/// ```

#[contract]
pub struct Circle;
#[contractimpl]
impl Circle {
    pub fn __constructor(
        env: Env,
        admin: Address,
        factory: Address,
        config: types::CircleConfig,
    ) -> Result<(), types::CircleError> {
        contract::init(&env, &admin, &factory, &config)
    }
    pub fn join(env: Env, member: Address) -> Result<(), types::CircleError> {
        contract::join(&env, &member)
    }
    pub fn contribute(
        env: Env,
        member: Address,
        amount: i128,
        round: u32,
    ) -> Result<(), types::CircleError> {
        contract::contribute(&env, &member, amount, round)
    }
    pub fn check_contribution_deadline(env: Env) -> Result<(), types::CircleError> {
        contract::check_contribution_deadline(&env)
    }
    pub fn trigger_payout(env: Env, caller: Address, round: u32) -> Result<(), types::CircleError> {
        contract::trigger_payout(&env, &caller, round)
    }
    pub fn auction_bid(
        env: Env,
        bidder: Address,
        discount_bips: u32,
        round: u32,
    ) -> Result<(), types::CircleError> {
        contract::auction_bid(&env, &bidder, discount_bips, round)
    }
    /// Refunds a capped batch of losing auction deposits. Returns how many
    /// losers remain; call again until the result is 0 (#436).
    pub fn refund_losing_bids(
        env: Env,
        caller: Address,
        round: u32,
        limit: u32,
    ) -> Result<u32, types::CircleError> {
        contract::refund_losing_bids(&env, &caller, round, limit)
    }
    pub fn init_dutch_auction(
        env: Env,
        caller: Address,
        round: u32,
        start_bips: u32,
        floor_bips: u32,
        decay_bips_per_ledger: u32,
        expiry_ledgers: u32,
    ) -> Result<(), types::CircleError> {
        contract::init_dutch_auction(
            &env,
            &caller,
            round,
            start_bips,
            floor_bips,
            decay_bips_per_ledger,
            expiry_ledgers,
        )
    }
    pub fn dutch_auction_price(env: Env, round: u32) -> Result<(u32, bool), types::CircleError> {
        contract::dutch_auction_price(&env, round)
    }
    pub fn dutch_auction_bid(
        env: Env,
        bidder: Address,
        round: u32,
    ) -> Result<u32, types::CircleError> {
        contract::dutch_auction_bid(&env, &bidder, round)
    }
    pub fn vote_payout(
        env: Env,
        voter: Address,
        vote_for: Address,
        round: u32,
    ) -> Result<(), types::CircleError> {
        contract::vote_payout(&env, &voter, &vote_for, round)
    }
    pub fn exit_circle(env: Env, member: Address) -> Result<(), types::CircleError> {
        contract::exit(&env, &member)
    }
    pub fn cancel_circle(env: Env, caller: Address) -> Result<(), types::CircleError> {
        contract::cancel_circle(&env, &caller)
    }
    pub fn cancel(env: Env, caller: Address) -> Result<(), types::CircleError> {
        contract::cancel(&env, &caller)
    }
    pub fn report_late(
        env: Env,
        reporter: Address,
        late_member: Address,
        round: u32,
    ) -> Result<(), types::CircleError> {
        contract::report_late(&env, &reporter, &late_member, round)
    }
    pub fn dispute(
        env: Env,
        member: Address,
        evidence_hash: BytesN<32>,
    ) -> Result<(), types::CircleError> {
        contract::dispute(&env, &member, &evidence_hash)
    }
    pub fn raise_dispute(
        env: Env,
        member: Address,
        evidence_hash: BytesN<32>,
    ) -> Result<(), types::CircleError> {
        contract::raise_dispute(&env, &member, &evidence_hash)
    }
    pub fn resolve_dispute(
        env: Env,
        admin: Address,
        resolution: u32,
    ) -> Result<(), types::CircleError> {
        contract::resolve_dispute(&env, &admin, resolution)
    }
    pub fn get_status(env: Env) -> types::Circle {
        contract::get_status(&env)
    }
    pub fn get_dispute_resolution(env: Env) -> Option<types::DisputeResolutionRecord> {
        contract::get_dispute_resolution(&env)
    }
    /// #478 — Organizer/admin treasury withdrawal, capped per transaction and per UTC day.
    pub fn withdraw_treasury(
        env: Env,
        caller: Address,
        amount: i128,
    ) -> Result<(), types::CircleError> {
        contract::withdraw_treasury(&env, &caller, amount)
    }
    /// #469 — Top contributors by total received, newest-round weighted.
    pub fn query_top_contributors(
        env: Env,
        n: u32,
    ) -> Result<soroban_sdk::Vec<(Address, i128)>, types::CircleError> {
        contract::query_top_contributors(&env, n)
    }
    /// #470 — Cancel an unresolved auction and refund the highest bid atomically.
    pub fn cancel_auction(env: Env, caller: Address) -> Result<(), types::CircleError> {
        contract::cancel_auction(&env, &caller)
    }
    pub fn get_members(env: Env) -> soroban_sdk::Vec<types::Member> {
        contract::get_members(&env)
    }
    pub fn get_contributions(
        env: Env,
        member: Address,
        page: u32,
        page_size: u32,
    ) -> soroban_sdk::Vec<types::Contribution> {
        contract::get_contributions(&env, &member, page, page_size)
    }
    pub fn query_round_config(env: Env, round: u32) -> Result<BytesN<32>, types::CircleError> {
        contract::query_round_config(&env, round)
    }
    pub fn get_round_fee_ledger(env: Env, round: u32) -> types::RoundFeeLedger {
        contract::get_round_fee_ledger(&env, round)
    }
    pub fn schedule_payout(
        env: Env,
        caller: Address,
        round: u32,
    ) -> Result<(), types::CircleError> {
        contract::schedule_payout(&env, &caller, round)
    }
    pub fn is_payout_scheduled(env: Env, round: u32) -> bool {
        contract::is_payout_scheduled(&env, round)
    }

    pub fn pause_circle(env: Env, admin: Address) -> Result<(), types::CircleError> {
        contract::pause_circle(&env, &admin)
    }
    pub fn unpause_circle(env: Env, admin: Address) -> Result<(), types::CircleError> {
        contract::unpause_circle(&env, &admin)
    }
    pub fn batch_invite(
        env: Env,
        caller: Address,
        members: soroban_sdk::Vec<Address>,
    ) -> Result<(), types::CircleError> {
        contract::batch_invite(&env, &caller, &members)
    }
    pub fn batch_payout(
        env: Env,
        caller: Address,
        recipients: soroban_sdk::Vec<Address>,
        amounts: soroban_sdk::Vec<i128>,
        round: u32,
    ) -> Result<(), types::CircleError> {
        contract::batch_payout(&env, &caller, &recipients, &amounts, round)
    }
    pub fn register_referral(
        env: Env,
        referrer: Address,
        referred: Address,
        bonus_pct: u32,
    ) -> Result<(), types::CircleError> {
        contract::register_referral(&env, &referrer, &referred, bonus_pct)
    }
    pub fn claim_referral_bonus(env: Env, referrer: Address) -> Result<(), types::CircleError> {
        contract::claim_referral_bonus(&env, &referrer)
    }
    pub fn update_streak(env: Env, member: Address, round: u32) -> Result<(), types::CircleError> {
        contract::update_streak(&env, &member, round)
    }
    pub fn claim_streak_bonus(env: Env, member: Address) -> Result<(), types::CircleError> {
        contract::claim_streak_bonus(&env, &member)
    }
    pub fn get_referrals(env: Env) -> soroban_sdk::Vec<types::Referral> {
        contract::get_referrals(&env)
    }
    pub fn get_streaks(env: Env) -> soroban_sdk::Vec<types::Streak> {
        contract::get_streaks(&env)
    }
    pub fn get_member_streak(env: Env, member: Address) -> types::Streak {
        contract::get_member_streak(&env, &member)
    }
    pub fn set_reputation_registry(
        env: Env,
        admin: Address,
        registry: Address,
    ) -> Result<(), types::CircleError> {
        contract::set_reputation_registry(&env, &admin, &registry)
    }
    pub fn get_reputation_registry(env: Env) -> Option<Address> {
        contract::get_reputation_registry(&env)
    }
    pub fn set_treasury(
        env: Env,
        admin: Address,
        treasury: Address,
    ) -> Result<(), types::CircleError> {
        contract::set_treasury(&env, &admin, &treasury)
    }
    pub fn set_token(env: Env, admin: Address, token: Address) -> Result<(), types::CircleError> {
        contract::set_token(&env, &admin, &token)
    }
    pub fn set_fee_bps(env: Env, admin: Address, fee_bps: u32) -> Result<(), types::CircleError> {
        contract::set_fee_bps(&env, &admin, fee_bps)
    }
    pub fn set_allowlist(
        env: Env,
        admin: Address,
        allowlist: soroban_sdk::Vec<Address>,
    ) -> Result<(), types::CircleError> {
        contract::set_allowlist(&env, &admin, allowlist)
    }
    pub fn get_allowlist(env: Env) -> soroban_sdk::Vec<Address> {
        contract::get_allowlist(&env)
    }
    pub fn set_oracle(env: Env, admin: Address, oracle: Address) -> Result<(), types::CircleError> {
        contract::set_oracle(&env, &admin, &oracle)
    }
    pub fn set_fallback_oracle(
        env: Env,
        admin: Address,
        oracle: Address,
    ) -> Result<(), types::CircleError> {
        contract::set_fallback_oracle(&env, &admin, &oracle)
    }
    pub fn get_oracle(env: Env) -> Option<Address> {
        contract::get_oracle(&env)
    }
    pub fn get_fallback_oracle(env: Env) -> Option<Address> {
        contract::get_fallback_oracle(&env)
    }
    pub fn configure_from_factory(
        env: Env,
        factory: Address,
        treasury: Address,
        reputation_registry: Address,
        fee_bps: u32,
    ) -> Result<(), types::CircleError> {
        contract::configure_from_factory(&env, &factory, &treasury, &reputation_registry, fee_bps)
    }
    pub fn get_treasury(env: Env) -> Option<Address> {
        contract::get_treasury(&env)
    }
    pub fn get_fee_bps(env: Env) -> u32 {
        contract::get_fee_bps(&env)
    }
}

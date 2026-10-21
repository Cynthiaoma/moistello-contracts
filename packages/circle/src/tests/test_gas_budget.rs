#![cfg(test)]

use crate::types::*;
use crate::{Circle, CircleArgs, CircleClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env, String, Vec};

fn create_test_config(env: &Env, organizer: &Address, token: &Address) -> CircleConfig {
    CircleConfig {
        organizer: organizer.clone(),
        token: token.clone(),
        name: String::from_str(env, "Gas Test Circle"),
        contribution_amount: 100_0000000i128,
        max_members: 3u32,
        payout_type: 0u32,
        total_rounds: 3u32,
        contribution_deadline_seconds: 604800u64,
        min_moi_score: 0u32,
        collateral_amount: 0i128,
        penalty_bps: 500u32,
        grace_period_seconds: 86400u64,
        max_strikes: 3u32,
        slug: String::from_str(env, "gas-test"),
        max_withdrawal_per_tx: 0,
        daily_withdrawal_limit: 0,
        min_duration_seconds: 0,
    }
}

/// #406: Gas budget test for contribute operation (< 80,000 CPU instructions / gas budget).
#[test]
fn test_contribute_within_gas_budget() {
    let env = Env::default();
    env.mock_all_auths();

    let organizer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token.address());

    let config = create_test_config(&env, &organizer, &token.address());
    let admin = organizer.clone();
    let factory = Address::generate(&env);
    let contract_id = env.register(Circle, CircleArgs::__constructor(&admin, &factory, &config));
    let client = CircleClient::new(&env, &contract_id);

    // Join 3 members to activate the circle
    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    let m3 = Address::generate(&env);

    client.join(&m1);
    client.join(&m2);
    client.join(&m3);

    token_client.mint(&m1, &1000_0000000i128);
    token_client.mint(&m2, &1000_0000000i128);
    token_client.mint(&m3, &1000_0000000i128);

    // Reset budget tracker immediately before contribute to measure only the contribute execution
    env.cost_estimate().budget().reset_tracker();

    // Act: member contributes for round 0
    client.contribute(&m1, &config.contribution_amount, &0u32);

    let cpu_used = env.cost_estimate().budget().cpu_instruction_cost();

    // Verify gas consumption is strictly within the 80,000 budget (< 80,000 for contribute)
    assert!(
        cpu_used < 80_000,
        "Contribute gas usage {} exceeded maximum budget of 80,000 CPU instructions",
        cpu_used
    );
}

/// #406: Verify write-time pre-computation of derived values (RoundTotalContributed, RoundContributors).
#[test]
fn test_precomputed_derived_values_at_write_time() {
    let env = Env::default();
    env.mock_all_auths();

    let organizer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token.address());

    let config = create_test_config(&env, &organizer, &token.address());
    let admin = organizer.clone();
    let factory = Address::generate(&env);
    let contract_id = env.register(Circle, CircleArgs::__constructor(&admin, &factory, &config));
    let client = CircleClient::new(&env, &contract_id);

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    let m3 = Address::generate(&env);

    client.join(&m1);
    client.join(&m2);
    client.join(&m3);

    token_client.mint(&m1, &1000_0000000i128);
    token_client.mint(&m2, &1000_0000000i128);
    token_client.mint(&m3, &1000_0000000i128);

    // First contribution
    client.contribute(&m1, &config.contribution_amount, &0u32);

    // Verify precomputed round total and round contributors via contract storage
    env.as_contract(&contract_id, || {
        let total: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::RoundTotalContributed(0))
            .expect("RoundTotalContributed not precomputed");
        assert_eq!(total, config.contribution_amount);

        let contributors: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::RoundContributors(0))
            .expect("RoundContributors not precomputed");
        assert_eq!(contributors.len(), 1);
        assert_eq!(contributors.get(0).unwrap(), m1);

        let contributed: bool = env
            .storage()
            .persistent()
            .get(&DataKey::MemberContributed(m1.clone(), 0))
            .expect("MemberContributed flag not set");
        assert!(contributed);
    });

    // Second contribution in same round
    client.contribute(&m2, &config.contribution_amount, &0u32);

    env.as_contract(&contract_id, || {
        let total: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::RoundTotalContributed(0))
            .unwrap();
        assert_eq!(total, config.contribution_amount * 2);

        let contributors: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::RoundContributors(0))
            .unwrap();
        assert_eq!(contributors.len(), 2);
        assert_eq!(contributors.get(1).unwrap(), m2);
    });
}

/// #406: Verify storage tier separation (Instance storage for small data, Persistent for collections).
#[test]
fn test_storage_tier_separation() {
    let env = Env::default();
    env.mock_all_auths();

    let organizer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin);

    let config = create_test_config(&env, &organizer, &token.address());
    let admin = organizer.clone();
    let factory = Address::generate(&env);
    let contract_id = env.register(Circle, CircleArgs::__constructor(&admin, &factory, &config));
    let client = CircleClient::new(&env, &contract_id);

    let m1 = Address::generate(&env);
    client.join(&m1);

    env.as_contract(&contract_id, || {
        // Small, frequently accessed data in instance storage
        assert!(env.storage().instance().has(&DataKey::Circle));
        assert!(env.storage().instance().has(&DataKey::Admin));
        assert!(env.storage().instance().has(&DataKey::Factory));

        // Collections and growing records in persistent storage
        assert!(env.storage().persistent().has(&DataKey::Members));
        assert!(env.storage().persistent().has(&DataKey::Contributions));
        assert!(env.storage().persistent().has(&DataKey::MemberStatus(m1.clone())));
    });
}

/// #406: Verify fast O(1) duplicate contribution rejection.
#[test]
fn test_duplicate_contribution_rejected_fast() {
    let env = Env::default();
    env.mock_all_auths();

    let organizer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token.address());

    let config = create_test_config(&env, &organizer, &token.address());
    let admin = organizer.clone();
    let factory = Address::generate(&env);
    let contract_id = env.register(Circle, CircleArgs::__constructor(&admin, &factory, &config));
    let client = CircleClient::new(&env, &contract_id);

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    let m3 = Address::generate(&env);

    client.join(&m1);
    client.join(&m2);
    client.join(&m3);

    token_client.mint(&m1, &1000_0000000i128);

    // First contribution succeeds
    assert!(client.try_contribute(&m1, &config.contribution_amount, &0u32).is_ok());

    // Duplicate contribution in same round is rejected with AlreadyContributed
    let res = client.try_contribute(&m1, &config.contribution_amount, &0u32);
    assert_eq!(res, Err(Ok(CircleError::AlreadyContributed)));
}

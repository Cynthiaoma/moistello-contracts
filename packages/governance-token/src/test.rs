#![cfg_attr(not(test), no_std)]
#![allow(clippy::inconsistent_digit_grouping)]

#[cfg(test)]
mod tests {
    use crate as governance_token;
    use crate::types::TokenError;
    use governance_token::{GovernanceToken, GovernanceTokenClient};
    use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _, MuxedAddress as _};
    use soroban_sdk::{
        symbol_short, Address, Env, IntoVal, Map, MuxedAddress, String, Symbol, Val,
    };

    fn setup(env: &Env) -> (Address, GovernanceTokenClient<'_>) {
        let admin = Address::generate(env);
        let contract_id = env.register(GovernanceToken, ());
        let client = GovernanceTokenClient::new(env, &contract_id);
        client.initialize(
            &admin,
            &String::from_str(env, "Governance Token"),
            &String::from_str(env, "GOV"),
            &7u32,
        );
        (admin, client)
    }

    /// Asserts that the last invocation published exactly one event with the
    /// given topics and data.
    fn assert_event(env: &Env, contract_id: &Address, topics: soroban_sdk::Vec<Val>, data: Val) {
        assert_eq!(
            env.events().all(),
            soroban_sdk::vec![env, (contract_id.clone(), topics.into_val(env), data)],
        );
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        assert_eq!(client.name(), String::from_str(&env, "Governance Token"));
        assert_eq!(client.symbol(), String::from_str(&env, "GOV"));
        assert_eq!(client.decimals(), 7u32);
        assert_eq!(client.total_supply(), 0i128);
        assert_eq!(client.get_admin(), admin);
    }

    #[test]
    fn test_initialize_double_init_fails() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let contract_id = env.register(GovernanceToken, ());
        let client = GovernanceTokenClient::new(&env, &contract_id);
        client.initialize(
            &admin,
            &String::from_str(&env, "Token"),
            &String::from_str(&env, "TKN"),
            &7u32,
        );
        let result = client.try_initialize(
            &admin,
            &String::from_str(&env, "Token2"),
            &String::from_str(&env, "TK2"),
            &7u32,
        );
        assert!(result.is_err());
    }

    /// The metadata is stored through `soroban-token-sdk`'s `TokenUtils`, i.e.
    /// in the canonical `METADATA` instance entry shared by every SEP-41
    /// token.
    #[test]
    fn test_metadata_stored_in_canonical_sdk_entry() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let contract_id = client.address.clone();
        env.as_contract(&contract_id, || {
            assert!(env.storage().instance().has(&symbol_short!("METADATA")));
        });
        assert_eq!(client.get_admin(), admin);
    }

    #[test]
    fn test_mint() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &recipient, &1_000_0000000i128);
        assert_eq!(client.balance(&recipient), 1_000_0000000i128);
        assert_eq!(client.total_supply(), 1_000_0000000i128);
    }

    #[test]
    fn test_mint_unauthorized() {
        let env = Env::default();
        let (_, client) = setup(&env);
        let not_admin = Address::generate(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();
        let result = client.try_mint(&not_admin, &recipient, &1_000_0000000i128);
        assert!(result.is_err());
    }

    #[test]
    fn test_mint_zero_fails() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();
        let result = client.try_mint(&admin, &recipient, &0i128);
        assert!(result.is_err());
    }

    #[test]
    fn test_mint_negative_fails() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();
        let result = client.try_mint(&admin, &recipient, &-1i128);
        assert!(result.is_err());
    }

    /// The `mint` event must use the canonical SEP-41 shape:
    /// topics `("mint", to)` and the amount as single-value data.
    #[test]
    fn test_mint_publishes_canonical_event() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let contract_id = client.address.clone();
        let recipient = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &recipient, &1_000i128);
        assert_event(
            &env,
            &contract_id,
            soroban_sdk::vec![
                &env,
                symbol_short!("mint").into_val(&env),
                recipient.into_val(&env),
            ],
            1_000i128.into_val(&env),
        );
    }

    #[test]
    fn test_burn() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let holder = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &holder, &1_000_0000000i128);
        client.burn(&holder, &500_0000000i128);
        assert_eq!(client.balance(&holder), 500_0000000i128);
        assert_eq!(client.total_supply(), 500_0000000i128);
    }

    #[test]
    fn test_burn_insufficient_balance() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let holder = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &holder, &100i128);
        let result = client.try_burn(&holder, &200i128);
        assert!(result.is_err());
    }

    #[test]
    fn test_burn_unauthorized() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let holder = Address::generate(&env);
        let other = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &holder, &1000i128);
        let result = client.try_burn(&other, &500i128);
        assert!(result.is_err());
    }

    /// The `burn` event must use the canonical SEP-41 shape:
    /// topics `("burn", from)` and the amount as single-value data.
    #[test]
    fn test_burn_publishes_canonical_event() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let contract_id = client.address.clone();
        let holder = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &holder, &1_000i128);
        client.burn(&holder, &400i128);
        assert_event(
            &env,
            &contract_id,
            soroban_sdk::vec![
                &env,
                symbol_short!("burn").into_val(&env),
                holder.into_val(&env),
            ],
            400i128.into_val(&env),
        );
    }

    #[test]
    fn test_transfer() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &alice, &1_000_0000000i128);
        client.transfer(&alice, MuxedAddress::from(&bob), &300_0000000i128);
        assert_eq!(client.balance(&alice), 700_0000000i128);
        assert_eq!(client.balance(&bob), 300_0000000i128);
    }

    /// A plain (non-muxed) transfer publishes the single-value data format,
    /// exactly like the Stellar Asset Contract does.
    #[test]
    fn test_transfer_publishes_canonical_event() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let contract_id = client.address.clone();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &alice, &1_000i128);
        client.transfer(&alice, MuxedAddress::from(&bob), &300i128);
        assert_event(
            &env,
            &contract_id,
            soroban_sdk::vec![
                &env,
                symbol_short!("transfer").into_val(&env),
                alice.into_val(&env),
                bob.into_val(&env),
            ],
            300i128.into_val(&env),
        );
    }

    /// A muxed transfer publishes the map data format carrying the muxed id,
    /// and credits the underlying account.
    #[test]
    fn test_transfer_to_muxed_address() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let contract_id = client.address.clone();
        let alice = Address::generate(&env);
        let muxed = MuxedAddress::generate(&env);
        let underlying = muxed.address();
        let muxed_id = muxed.id().unwrap();
        env.mock_all_auths();
        client.mint(&admin, &alice, &1_000i128);
        client.transfer(&alice, &muxed, &300i128);
        let data = Map::<Symbol, Val>::from_array(
            &env,
            [
                (Symbol::new(&env, "to_muxed_id"), muxed_id.into_val(&env)),
                (Symbol::new(&env, "amount"), 300i128.into_val(&env)),
            ],
        );
        assert_event(
            &env,
            &contract_id,
            soroban_sdk::vec![
                &env,
                symbol_short!("transfer").into_val(&env),
                alice.into_val(&env),
                underlying.into_val(&env),
            ],
            data.into_val(&env),
        );
        // The balance is credited to the underlying account.
        assert_eq!(client.balance(&underlying), 300i128);
    }

    #[test]
    fn test_transfer_insufficient_balance() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &alice, &100i128);
        let result = client.try_transfer(&alice, MuxedAddress::from(&bob), &200i128);
        assert!(result.is_err());
    }

    #[test]
    fn test_transfer_zero_fails() {
        let env = Env::default();
        let (_admin, client) = setup(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();
        let result = client.try_transfer(&alice, MuxedAddress::from(&bob), &0i128);
        assert!(result.is_err());
    }

    #[test]
    fn test_approve_and_transfer_from() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &owner, &1_000_0000000i128);
        client.approve(&owner, &spender, &500_0000000i128, &0u32);
        assert_eq!(client.allowance(&owner, &spender), 500_0000000i128);
        client.transfer_from(&spender, &owner, &recipient, &200_0000000i128);
        assert_eq!(client.balance(&owner), 800_0000000i128);
        assert_eq!(client.balance(&recipient), 200_0000000i128);
        assert_eq!(client.allowance(&owner, &spender), 300_0000000i128);
    }

    /// The `approve` event must use the canonical SEP-41 shape:
    /// topics `("approve", from, spender)` and `[amount, expiration_ledger]`.
    #[test]
    fn test_approve_publishes_canonical_event() {
        let env = Env::default();
        let (_admin, client) = setup(&env);
        let contract_id = client.address.clone();
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();
        client.approve(&owner, &spender, &500i128, &1_234u32);
        assert_event(
            &env,
            &contract_id,
            soroban_sdk::vec![
                &env,
                symbol_short!("approve").into_val(&env),
                owner.into_val(&env),
                spender.into_val(&env),
            ],
            (500i128, 1_234u32).into_val(&env),
        );
    }

    /// SEP-41: `approve` overwrites the allowance, it never adds to it.
    #[test]
    fn test_approve_overwrites_previous_allowance() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &owner, &1_000i128);
        client.approve(&owner, &spender, &500i128, &0u32);
        client.approve(&owner, &spender, &100i128, &0u32);
        assert_eq!(client.allowance(&owner, &spender), 100i128);
    }

    /// SEP-41: a live allowance may not expire in the past.
    #[test]
    fn test_approve_rejects_expiration_in_the_past() {
        let env = Env::default();
        let (_admin, client) = setup(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();
        env.ledger().set_sequence_number(100);
        let result = client.try_approve(&owner, &spender, &100i128, &99u32);
        assert_eq!(result, Err(Ok(TokenError::InvalidExpirationLedger)));
    }

    /// SEP-41: a zero allowance is a revocation and is always accepted.
    #[test]
    fn test_approve_zero_amount_revokes_allowance() {
        let env = Env::default();
        let (_admin, client) = setup(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();
        client.approve(&owner, &spender, &500i128, &0u32);
        assert!(client
            .try_approve(&owner, &spender, &0i128, &1_000u32)
            .is_ok());
        assert_eq!(client.allowance(&owner, &spender), 0i128);
    }

    /// SEP-41: an expired allowance reads back as zero.
    #[test]
    fn test_expired_allowance_reads_as_zero() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();
        env.ledger().set_sequence_number(100);
        client.mint(&admin, &owner, &1_000i128);
        client.approve(&owner, &spender, &500i128, &100u32);
        assert_eq!(client.allowance(&owner, &spender), 500i128);
        // Move past the expiration ledger.
        env.ledger().set_sequence_number(101);
        assert_eq!(client.allowance(&owner, &spender), 0i128);
        // The raw record is still available for UIs.
        let data = client.allowance_data(&owner, &spender);
        assert_eq!(data.amount, 500i128);
        assert_eq!(data.expiration_ledger, 100u32);
        // And it can no longer be spent.
        let result = client.try_transfer_from(&spender, &owner, &recipient, &100i128);
        assert_eq!(result, Err(Ok(TokenError::AllowanceExpired)));
    }

    #[test]
    fn test_transfer_from_exceeds_allowance() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &owner, &1_000_0000000i128);
        client.approve(&owner, &spender, &100i128, &0u32);
        let result = client.try_transfer_from(&spender, &owner, &recipient, &200i128);
        assert!(result.is_err());
    }

    #[test]
    fn test_transfer_from_no_allowance() {
        let env = Env::default();
        let (_, client) = setup(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();
        let result = client.try_transfer_from(&spender, &owner, &recipient, &100i128);
        assert!(result.is_err());
    }

    /// SEP-41 `burn_from`: burns from `from` while consuming `spender`'s
    /// allowance.
    #[test]
    fn test_burn_from_consumes_allowance() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let contract_id = client.address.clone();
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &owner, &1_000i128);
        client.approve(&owner, &spender, &500i128, &0u32);
        client.burn_from(&spender, &owner, &200i128);
        assert_event(
            &env,
            &contract_id,
            soroban_sdk::vec![
                &env,
                symbol_short!("burn").into_val(&env),
                owner.into_val(&env),
            ],
            200i128.into_val(&env),
        );
        assert_eq!(client.balance(&owner), 800i128);
        assert_eq!(client.total_supply(), 800i128);
        assert_eq!(client.allowance(&owner, &spender), 300i128);
    }

    #[test]
    fn test_burn_from_without_allowance_fails() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &owner, &1_000i128);
        let result = client.try_burn_from(&spender, &owner, &100i128);
        assert!(result.is_err());
        assert_eq!(client.balance(&owner), 1_000i128);
    }

    #[test]
    fn test_burn_from_exceeds_allowance_fails() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &owner, &1_000i128);
        client.approve(&owner, &spender, &100i128, &0u32);
        let result = client.try_burn_from(&spender, &owner, &200i128);
        assert!(result.is_err());
        assert_eq!(client.balance(&owner), 1_000i128);
        assert_eq!(client.allowance(&owner, &spender), 100i128);
    }

    /// #518 — admin-authorized burn, kept as a governance extension under the
    /// `admin_burn_from` name so that `burn_from` can follow SEP-41.
    #[test]
    fn test_admin_burn_from() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let holder = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &holder, &1_000i128);
        client.admin_burn_from(&admin, &holder, &400i128);
        assert_eq!(client.balance(&holder), 600i128);
        assert_eq!(client.total_supply(), 600i128);
    }

    #[test]
    fn test_admin_burn_from_unauthorized() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let holder = Address::generate(&env);
        let not_admin = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &holder, &1_000i128);
        let result = client.try_admin_burn_from(&not_admin, &holder, &400i128);
        assert!(result.is_err());
        assert_eq!(client.balance(&holder), 1_000i128);
    }

    #[test]
    fn test_clawback() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let holder = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &holder, &1_000_0000000i128);
        client.clawback(&admin, &holder, &400_0000000i128);
        assert_eq!(client.balance(&holder), 600_0000000i128);
        assert_eq!(client.total_supply(), 600_0000000i128);
    }

    /// The `clawback` event must use the canonical SEP-41 shape:
    /// topics `("clawback", from)` and the amount as single-value data.
    #[test]
    fn test_clawback_publishes_canonical_event() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let contract_id = client.address.clone();
        let holder = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &holder, &1_000i128);
        client.clawback(&admin, &holder, &400i128);
        assert_event(
            &env,
            &contract_id,
            soroban_sdk::vec![
                &env,
                symbol_short!("clawback").into_val(&env),
                holder.into_val(&env),
            ],
            400i128.into_val(&env),
        );
    }

    #[test]
    fn test_clawback_from_frozen_account_permitted() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let holder = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &holder, &1_000_0000000i128);
        client.freeze(&admin, &holder);
        assert!(client.is_frozen(&holder));
        // Admin clawback succeeds even on frozen accounts (intentional regulatory override)
        client.clawback(&admin, &holder, &400_0000000i128);
        assert_eq!(client.balance(&holder), 600_0000000i128);
        assert_eq!(client.total_supply(), 600_0000000i128);
    }

    #[test]
    fn test_clawback_unauthorized() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let holder = Address::generate(&env);
        let not_admin = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &holder, &1000i128);
        let result = client.try_clawback(&not_admin, &holder, &500i128);
        assert!(result.is_err());
    }

    #[test]
    fn test_freeze_blocks_transfer() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &alice, &1_000_0000000i128);
        client.freeze(&admin, &alice);
        assert!(client.is_frozen(&alice));
        let result = client.try_transfer(&alice, MuxedAddress::from(&bob), &100i128);
        assert!(result.is_err());
    }

    #[test]
    fn test_freeze_blocks_receive() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &alice, &1_000_0000000i128);
        client.freeze(&admin, &bob);
        let result = client.try_transfer(&alice, MuxedAddress::from(&bob), &100i128);
        assert!(result.is_err());
    }

    #[test]
    fn test_unfreeze() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &alice, &1_000_0000000i128);
        client.freeze(&admin, &alice);
        assert!(client.is_frozen(&alice));
        client.unfreeze(&admin, &alice);
        assert!(!client.is_frozen(&alice));
        assert!(client
            .try_transfer(&alice, MuxedAddress::from(&bob), &100i128)
            .is_ok());
    }

    #[test]
    fn test_freeze_blocks_mint() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let holder = Address::generate(&env);
        env.mock_all_auths();
        client.freeze(&admin, &holder);
        let result = client.try_mint(&admin, &holder, &100i128);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_admin() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();
        client.set_admin(&admin, &new_admin);
        assert_eq!(client.get_admin(), new_admin);
    }

    #[test]
    fn test_set_admin_unauthorized() {
        let env = Env::default();
        let (_, client) = setup(&env);
        let not_admin = Address::generate(&env);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();
        let result = client.try_set_admin(&not_admin, &new_admin);
        assert!(result.is_err());
    }

    #[test]
    fn test_balance_nonexistent_account() {
        let env = Env::default();
        let (_, client) = setup(&env);
        let ghost = Address::generate(&env);
        assert_eq!(client.balance(&ghost), 0i128);
    }

    #[test]
    fn test_allowance_nonexistent_pair() {
        let env = Env::default();
        let (_, client) = setup(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        assert_eq!(client.allowance(&owner, &spender), 0i128);
        let data = client.allowance_data(&owner, &spender);
        assert_eq!(data.amount, 0i128);
        assert_eq!(data.expiration_ledger, 0u32);
    }

    #[test]
    fn test_approve_zero_clears_allowance() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &owner, &1000i128);
        client.approve(&owner, &spender, &500i128, &0u32);
        assert_eq!(client.allowance(&owner, &spender), 500i128);
        client.approve(&owner, &spender, &0i128, &0u32);
        assert_eq!(client.allowance(&owner, &spender), 0i128);
    }

    #[test]
    fn test_full_lifecycle() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let charlie = Address::generate(&env);
        env.mock_all_auths();

        // Mint to alice
        client.mint(&admin, &alice, &10_000_0000000i128);
        assert_eq!(client.total_supply(), 10_000_0000000i128);

        // Transfer alice -> bob
        client.transfer(&alice, MuxedAddress::from(&bob), &3_000_0000000i128);
        assert_eq!(client.balance(&alice), 7_000_0000000i128);
        assert_eq!(client.balance(&bob), 3_000_0000000i128);

        // Approve bob to spend alice's tokens, transfer to charlie
        client.approve(&alice, &bob, &2_000_0000000i128, &0u32);
        client.transfer_from(&bob, &alice, &charlie, &1_000_0000000i128);
        assert_eq!(client.balance(&alice), 6_000_0000000i128);
        assert_eq!(client.balance(&charlie), 1_000_0000000i128);
        assert_eq!(client.allowance(&alice, &bob), 1_000_0000000i128);

        // Burn some of bob's tokens
        client.burn(&bob, &500_0000000i128);
        assert_eq!(client.balance(&bob), 2_500_0000000i128);
        assert_eq!(client.total_supply(), 9_500_0000000i128);

        // Clawback 100 from charlie
        client.clawback(&admin, &charlie, &100_0000000i128);
        assert_eq!(client.balance(&charlie), 900_0000000i128);
        assert_eq!(client.total_supply(), 9_400_0000000i128);
    }

    #[test]
    fn test_transfer_to_contract_address_rejected() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let alice = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &alice, &1_000_0000000i128);

        let result =
            client.try_transfer(&alice, MuxedAddress::from(&client.address), &500_0000000i128);
        assert_eq!(result, Err(Ok(crate::types::TokenError::CannotTransferToSelf)));
    }

    #[test]
    fn test_transfer_from_to_contract_address_rejected() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &alice, &1_000_0000000i128);
        client.approve(&alice, &bob, &1_000_0000000i128, &0u32);

        let result = client.try_transfer_from(&bob, &alice, &client.address, &500_0000000i128);
        assert_eq!(result, Err(Ok(crate::types::TokenError::CannotTransferToSelf)));
    }

    #[test]
    fn test_mint_to_contract_address_rejected() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        env.mock_all_auths();

        let result = client.try_mint(&admin, &client.address, &1_000_0000000i128);
        assert_eq!(result, Err(Ok(crate::types::TokenError::CannotTransferToSelf)));
    }

    #[test]
    fn test_allowlist_mode_default_off() {
        let env = Env::default();
        let (_, client) = setup(&env);
        assert!(!client.is_allowlist_mode_enabled());
    }

    #[test]
    fn test_set_allowlist_mode_toggles_and_takes_effect() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &alice, &1_000_0000000i128);

        client.set_allowlist_mode(&admin, &true);
        assert!(client.is_allowlist_mode_enabled());

        // bob not allowlisted -> transfer fails
        let result = client.try_transfer(&alice, MuxedAddress::from(&bob), &100i128);
        assert!(result.is_err());

        // allowlist bob -> transfer succeeds
        client.add_to_allowlist(&admin, &bob);
        assert!(client.is_allowlisted(&bob));
        client.transfer(&alice, MuxedAddress::from(&bob), &100i128);
        assert_eq!(client.balance(&bob), 100i128);

        // remove bob -> transfer fails again
        client.remove_from_allowlist(&admin, &bob);
        assert!(!client.is_allowlisted(&bob));
        let result = client.try_transfer(&alice, MuxedAddress::from(&bob), &100i128);
        assert!(result.is_err());

        // disable mode -> transfer succeeds normally
        client.set_allowlist_mode(&admin, &false);
        assert!(!client.is_allowlist_mode_enabled());
        client.transfer(&alice, MuxedAddress::from(&bob), &100i128);
        assert_eq!(client.balance(&bob), 200i128);
    }

    #[test]
    fn test_allowlist_mode_transfer_from_gated() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &owner, &1_000_0000000i128);
        client.approve(&owner, &spender, &500_0000000i128, &0u32);
        client.set_allowlist_mode(&admin, &true);

        let result = client.try_transfer_from(&spender, &owner, &recipient, &100i128);
        assert!(result.is_err());

        client.add_to_allowlist(&admin, &recipient);
        client.transfer_from(&spender, &owner, &recipient, &100i128);
        assert_eq!(client.balance(&recipient), 100i128);
    }

    #[test]
    fn test_allowlist_management_unauthorized() {
        let env = Env::default();
        let (_, client) = setup(&env);
        let not_admin = Address::generate(&env);
        let account = Address::generate(&env);
        env.mock_all_auths();

        assert!(client.try_set_allowlist_mode(&not_admin, &true).is_err());
        assert!(client.try_add_to_allowlist(&not_admin, &account).is_err());
        assert!(client
            .try_remove_from_allowlist(&not_admin, &account)
            .is_err());
    }

    #[test]
    fn test_pause_blocks_transfer_and_mint() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &alice, &1_000_0000000i128);

        client.pause(&admin);

        // Mutations are rejected while paused.
        assert_eq!(
            client.try_transfer(&alice, MuxedAddress::from(&bob), &100i128),
            Err(Ok(TokenError::ContractPaused))
        );
        assert_eq!(client.try_mint(&admin, &bob, &100i128), Err(Ok(TokenError::ContractPaused)));
        assert_eq!(client.try_burn(&alice, &100i128), Err(Ok(TokenError::ContractPaused)));
        assert_eq!(
            client.try_approve(&alice, &bob, &100i128, &0u32),
            Err(Ok(TokenError::ContractPaused))
        );
        // Reads still work while paused.
        assert_eq!(client.balance(&alice), 1_000_0000000i128);

        client.unpause(&admin);

        // Mutations resume after unpause.
        client.transfer(&alice, MuxedAddress::from(&bob), &100i128);
        assert_eq!(client.balance(&bob), 100i128);
    }

    #[test]
    fn test_pause_blocks_admin_mutations() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let holder = Address::generate(&env);
        let account = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &holder, &1_000_0000000i128);

        client.pause(&admin);

        assert_eq!(
            client.try_admin_burn_from(&admin, &holder, &100i128),
            Err(Ok(TokenError::ContractPaused))
        );
        assert_eq!(
            client.try_clawback(&admin, &holder, &100i128),
            Err(Ok(TokenError::ContractPaused))
        );
        assert_eq!(client.try_freeze(&admin, &account), Err(Ok(TokenError::ContractPaused)));
        assert_eq!(client.try_unfreeze(&admin, &account), Err(Ok(TokenError::ContractPaused)));
        assert_eq!(client.try_set_admin(&admin, &account), Err(Ok(TokenError::ContractPaused)));
        assert_eq!(
            client.try_set_allowlist_mode(&admin, &true),
            Err(Ok(TokenError::ContractPaused))
        );
        assert_eq!(
            client.try_add_to_allowlist(&admin, &account),
            Err(Ok(TokenError::ContractPaused))
        );
        assert_eq!(
            client.try_remove_from_allowlist(&admin, &account),
            Err(Ok(TokenError::ContractPaused))
        );

        client.unpause(&admin);
        client.freeze(&admin, &account);
        assert!(client.is_frozen(&account));
    }

    #[test]
    fn test_pause_unpause_unauthorized() {
        let env = Env::default();
        let (_, client) = setup(&env);
        let not_admin = Address::generate(&env);
        env.mock_all_auths();

        assert!(client.try_pause(&not_admin).is_err());
        assert!(client.try_unpause(&not_admin).is_err());
    }

    #[test]
    fn test_pause_unpause_full_cycle() {
        let env = Env::default();
        let (admin, client) = setup(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();

        for _ in 0..2 {
            client.pause(&admin);
            assert!(client.try_mint(&admin, &recipient, &100i128).is_err());
            client.unpause(&admin);
            client.mint(&admin, &recipient, &100i128);
        }
        assert_eq!(client.balance(&recipient), 200i128);
    }
}

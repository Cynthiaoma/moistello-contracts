#![cfg_attr(not(test), no_std)]

#[cfg(test)]
mod tests {
    use crate as escrow_swap;
    use crate::types::EscrowError;
    use escrow_swap::{EscrowSwap, EscrowSwapArgs};
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Address, BytesN, Env};

    fn create_hash_lock(env: &Env) -> BytesN<32> {
        let secret = soroban_sdk::Bytes::from_array(env, &[1u8; 32]);
        env.crypto().sha256(&secret).into()
    }

    fn setup_token(env: &Env, admin: &Address) -> Address {
        env.register_stellar_asset_contract_v2(admin.clone())
            .address()
    }

    #[test]
    fn test_create_swap() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(EscrowSwap, EscrowSwapArgs::__constructor(&admin));
        let client = escrow_swap::EscrowSwapClient::new(&env, &contract_id);

        let token_a = setup_token(&env, &admin);
        let token_b = setup_token(&env, &admin);
        let token_a_client = soroban_sdk::token::Client::new(&env, &token_a);

        let initiator = Address::generate(&env);
        let responder = Address::generate(&env);

        let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_a);
        token_a_admin.mint(&initiator, &100_0000000i128);

        let hash_lock = create_hash_lock(&env);
        let time_lock = env.ledger().timestamp() + 3600;
        assert!(client
            .try_create_swap(
                &initiator,
                &responder,
                &token_a,
                &token_b,
                &100_0000000i128,
                &200_0000000i128,
                &hash_lock,
                &time_lock
            )
            .is_ok());
    }

    #[test]
    fn test_create_swap_invalid_amount() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(EscrowSwap, EscrowSwapArgs::__constructor(&admin));
        let client = escrow_swap::EscrowSwapClient::new(&env, &contract_id);

        let token_a = setup_token(&env, &admin);
        let token_b = setup_token(&env, &admin);

        let initiator = Address::generate(&env);
        let responder = Address::generate(&env);
        let hash_lock = create_hash_lock(&env);
        let time_lock = env.ledger().timestamp() + 3600;
        assert!(client
            .try_create_swap(
                &initiator,
                &responder,
                &token_a,
                &token_b,
                &0i128,
                &200_0000000i128,
                &hash_lock,
                &time_lock
            )
            .is_err());
    }

    #[test]
    fn test_create_swap_self_referral() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(EscrowSwap, EscrowSwapArgs::__constructor(&admin));
        let client = escrow_swap::EscrowSwapClient::new(&env, &contract_id);

        let token_a = setup_token(&env, &admin);
        let token_b = setup_token(&env, &admin);

        let initiator = Address::generate(&env);
        let hash_lock = create_hash_lock(&env);
        let time_lock = env.ledger().timestamp() + 3600;
        assert!(client
            .try_create_swap(
                &initiator,
                &initiator,
                &token_a,
                &token_b,
                &100_0000000i128,
                &200_0000000i128,
                &hash_lock,
                &time_lock
            )
            .is_err());
    }

    #[test]
    fn test_get_swaps() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(EscrowSwap, EscrowSwapArgs::__constructor(&admin));
        let client = escrow_swap::EscrowSwapClient::new(&env, &contract_id);

        let token_a = setup_token(&env, &admin);
        let token_b = setup_token(&env, &admin);

        let initiator = Address::generate(&env);
        let responder = Address::generate(&env);

        let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_a);
        token_a_admin.mint(&initiator, &100_0000000i128);

        let hash_lock = create_hash_lock(&env);
        let time_lock = env.ledger().timestamp() + 3600;
        let _ = client
            .try_create_swap(
                &initiator,
                &responder,
                &token_a,
                &token_b,
                &100_0000000i128,
                &200_0000000i128,
                &hash_lock,
                &time_lock,
            )
            .unwrap();
        let swaps = client.get_swaps();
        assert_eq!(swaps.len(), 1);
    }

    fn setup_paused(
        env: &Env,
    ) -> (escrow_swap::EscrowSwapClient<'_>, Address, Address, Address, Address, Address) {
        env.mock_all_auths();
        let admin = Address::generate(env);
        let contract_id = env.register(EscrowSwap, EscrowSwapArgs::__constructor(&admin));
        let client = escrow_swap::EscrowSwapClient::new(env, &contract_id);
        let token_a = setup_token(env, &admin);
        let token_b = setup_token(env, &admin);
        let initiator = Address::generate(env);
        let responder = Address::generate(env);
        (client, admin, token_a, token_b, initiator, responder)
    }

    #[test]
    fn test_pause_blocks_create_swap() {
        let env = Env::default();
        let (client, admin, token_a, token_b, initiator, responder) = setup_paused(&env);
        let hash_lock = create_hash_lock(&env);
        let time_lock = env.ledger().timestamp() + 3600;

        client.pause(&admin);

        // Mutations are rejected while paused; reads still work.
        assert_eq!(
            client.try_create_swap(
                &initiator,
                &responder,
                &token_a,
                &token_b,
                &100_0000000i128,
                &200_0000000i128,
                &hash_lock,
                &time_lock,
            ),
            Err(Ok(EscrowError::ContractPaused))
        );
        assert_eq!(client.get_swaps().len(), 0);

        client.unpause(&admin);

        let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_a);
        token_a_admin.mint(&initiator, &100_0000000i128);
        assert!(client
            .try_create_swap(
                &initiator,
                &responder,
                &token_a,
                &token_b,
                &100_0000000i128,
                &200_0000000i128,
                &hash_lock,
                &time_lock,
            )
            .is_ok());
    }

    #[test]
    fn test_pause_unpause_unauthorized() {
        let env = Env::default();
        let (client, _admin, ..) = setup_paused(&env);
        let not_admin = Address::generate(&env);

        assert!(client.try_pause(&not_admin).is_err());
        assert!(client.try_unpause(&not_admin).is_err());
    }

    #[test]
    fn test_pause_unpause_full_cycle() {
        let env = Env::default();
        let (client, admin, token_a, token_b, initiator, responder) = setup_paused(&env);
        let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_a);
        token_a_admin.mint(&initiator, &300_0000000i128);

        for _ in 0..2 {
            client.pause(&admin);
            let hash_lock = create_hash_lock(&env);
            let time_lock = env.ledger().timestamp() + 3600;
            assert_eq!(
                client.try_create_swap(
                    &initiator,
                    &responder,
                    &token_a,
                    &token_b,
                    &100_0000000i128,
                    &200_0000000i128,
                    &hash_lock,
                    &time_lock,
                ),
                Err(Ok(EscrowError::ContractPaused))
            );
            client.unpause(&admin);
        }

        let hash_lock = create_hash_lock(&env);
        let time_lock = env.ledger().timestamp() + 3600;
        assert!(client
            .try_create_swap(
                &initiator,
                &responder,
                &token_a,
                &token_b,
                &100_0000000i128,
                &200_0000000i128,
                &hash_lock,
                &time_lock,
            )
            .is_ok());
    }

    #[test]
    fn test_accept_swap_already_accepted() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(EscrowSwap, EscrowSwapArgs::__constructor(&admin));
        let client = escrow_swap::EscrowSwapClient::new(&env, &contract_id);

        let token_a = setup_token(&env, &admin);
        let token_b = setup_token(&env, &admin);

        let initiator = Address::generate(&env);
        let responder = Address::generate(&env);

        let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_a);
        token_a_admin.mint(&initiator, &100_0000000i128);
        let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_b);
        token_b_admin.mint(&responder, &200_0000000i128);

        let hash_lock = create_hash_lock(&env);
        let time_lock = env.ledger().timestamp() + 3600;
        let swap_id = client
            .try_create_swap(
                &initiator,
                &responder,
                &token_a,
                &token_b,
                &100_0000000i128,
                &200_0000000i128,
                &hash_lock,
                &time_lock,
            )
            .unwrap()
            .unwrap();

        let secret = BytesN::from_array(&env, &[1u8; 32]);
        assert!(client.try_accept_swap(&swap_id, &responder, &secret).is_ok());

        // Double-accept must be rejected with AlreadyAccepted
        assert_eq!(
            client.try_accept_swap(&swap_id, &responder, &secret),
            Err(Ok(EscrowError::AlreadyAccepted))
        );
    }

    #[test]
    fn test_accept_swap_insufficient_balance() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(EscrowSwap, EscrowSwapArgs::__constructor(&admin));
        let client = escrow_swap::EscrowSwapClient::new(&env, &contract_id);

        let token_a = setup_token(&env, &admin);
        let token_b = setup_token(&env, &admin);

        let initiator = Address::generate(&env);
        let responder = Address::generate(&env);

        let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_a);
        token_a_admin.mint(&initiator, &100_0000000i128);
        // responder holds no token_b balance

        let hash_lock = create_hash_lock(&env);
        let time_lock = env.ledger().timestamp() + 3600;
        let swap_id = client
            .try_create_swap(
                &initiator,
                &responder,
                &token_a,
                &token_b,
                &100_0000000i128,
                &200_0000000i128,
                &hash_lock,
                &time_lock,
            )
            .unwrap()
            .unwrap();

        let secret = BytesN::from_array(&env, &[1u8; 32]);
        assert_eq!(
            client.try_accept_swap(&swap_id, &responder, &secret),
            Err(Ok(EscrowError::InsufficientBalance))
        );
    }
}

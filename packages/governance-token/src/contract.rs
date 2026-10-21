use crate::types::*;
use common::pause;
use soroban_sdk::{symbol_short, Address, Env, Map, MuxedAddress, String};
use soroban_token_sdk::events::{
    Approve, Burn, Clawback, MintWithAmountOnly, Transfer, TransferWithAmountOnly,
};
use soroban_token_sdk::metadata::TokenMetadata;
use soroban_token_sdk::TokenUtils;

const ADMIN_KEY: soroban_sdk::Symbol = symbol_short!("admin");
const BALANCES_KEY: soroban_sdk::Symbol = symbol_short!("bal");
const ALLOWANCES_KEY: soroban_sdk::Symbol = symbol_short!("alw");
const TOTAL_KEY: soroban_sdk::Symbol = symbol_short!("total");
const FROZEN_KEY: soroban_sdk::Symbol = symbol_short!("frz");
const ALLOWLIST_MODE_KEY: soroban_sdk::Symbol = symbol_short!("alw_mode");
const ALLOWLIST_KEY: soroban_sdk::Symbol = symbol_short!("alw_list");

/// Initializes the token: sets the admin, the canonical token metadata and
/// the (empty) balance / allowance / freeze registries.
///
/// The metadata is stored through `soroban-token-sdk`'s [`TokenUtils`], i.e.
/// in the standard `METADATA` instance entry, so the token is laid out exactly
/// like every other SEP-41 token.
pub fn initialize(
    env: &Env,
    admin: &Address,
    name: &String,
    symbol: &String,
    decimals: u32,
) -> Result<(), TokenError> {
    if env.storage().instance().has(&ADMIN_KEY) {
        return Err(TokenError::NotInitialized);
    }
    env.storage().instance().set(&ADMIN_KEY, admin);
    TokenUtils::new(env)
        .metadata()
        .set_metadata(&TokenMetadata {
            decimal: decimals,
            name: name.clone(),
            symbol: symbol.clone(),
        });
    env.storage().instance().set(&TOTAL_KEY, &0i128);
    env.storage()
        .persistent()
        .set(&BALANCES_KEY, &Map::<Address, i128>::new(env));
    env.storage()
        .persistent()
        .set(&ALLOWANCES_KEY, &Map::<(Address, Address), AllowanceData>::new(env));
    env.storage()
        .persistent()
        .set(&FROZEN_KEY, &Map::<Address, bool>::new(env));
    Ok(())
}

/// Transfers `amount` from `from` to `to`.
///
/// `to` is a [`MuxedAddress`], as mandated by SEP-41. The balance is credited
/// to the underlying account (`to.address()`); when the muxed address carries
/// an id, the id is published in the `transfer` event so that the recipient
/// can be reconstructed by indexers.
pub fn transfer(
    env: &Env,
    from: &Address,
    to: &MuxedAddress,
    amount: i128,
) -> Result<(), TokenError> {
    pause::when_not_paused(env).map_err(|_| TokenError::ContractPaused)?;
    from.require_auth();
    let recipient = to.address();
    if recipient == env.current_contract_address() {
        return Err(TokenError::CannotTransferToSelf);
    }
    validate_non_frozen(env, from)?;
    validate_non_frozen(env, &recipient)?;
    validate_allowlisted(env, &recipient)?;
    if amount <= 0 {
        return Err(TokenError::InvalidAmount);
    }
    let mut balances: Map<Address, i128> = env
        .storage()
        .persistent()
        .get(&BALANCES_KEY)
        .ok_or(TokenError::NotInitialized)?;
    let from_balance: i128 = balances.get(from.clone()).unwrap_or(0);
    if from_balance < amount {
        return Err(TokenError::InsufficientBalance);
    }
    balances.set(
        from.clone(),
        from_balance
            .checked_sub(amount)
            .ok_or(TokenError::Underflow)?,
    );
    let to_balance: i128 = balances.get(recipient.clone()).unwrap_or(0);
    balances.set(recipient.clone(), to_balance.checked_add(amount).ok_or(TokenError::Overflow)?);
    env.storage().persistent().set(&BALANCES_KEY, &balances);
    publish_transfer(env, from, to, amount);
    Ok(())
}

/// Transfers `amount` from `from` to `to`, consuming `spender`'s allowance.
///
/// The allowance is reduced by `amount` and its expiration ledger is left
/// untouched, as required by SEP-41.
pub fn transfer_from(
    env: &Env,
    spender: &Address,
    from: &Address,
    to: &Address,
    amount: i128,
) -> Result<(), TokenError> {
    pause::when_not_paused(env).map_err(|_| TokenError::ContractPaused)?;
    spender.require_auth();
    if to == &env.current_contract_address() {
        return Err(TokenError::CannotTransferToSelf);
    }
    validate_non_frozen(env, from)?;
    validate_non_frozen(env, to)?;
    validate_allowlisted(env, to)?;
    if amount <= 0 {
        return Err(TokenError::InvalidAmount);
    }
    consume_allowance(env, from, spender, amount)?;
    let mut balances: Map<Address, i128> = env
        .storage()
        .persistent()
        .get(&BALANCES_KEY)
        .ok_or(TokenError::NotInitialized)?;
    let from_balance: i128 = balances.get(from.clone()).unwrap_or(0);
    if from_balance < amount {
        return Err(TokenError::InsufficientBalance);
    }
    balances.set(
        from.clone(),
        from_balance
            .checked_sub(amount)
            .ok_or(TokenError::Underflow)?,
    );
    let to_balance: i128 = balances.get(to.clone()).unwrap_or(0);
    balances.set(to.clone(), to_balance.checked_add(amount).ok_or(TokenError::Overflow)?);
    env.storage().persistent().set(&BALANCES_KEY, &balances);
    TransferWithAmountOnly {
        from: from.clone(),
        to: to.clone(),
        amount,
    }
    .publish(env);
    Ok(())
}

/// Sets (overwrites) the allowance `spender` may spend from `from`'s balance.
///
/// Per SEP-41 the new allowance *replaces* the previous one — it is never
/// added to it — and an allowance is a spending limit, not a reservation of
/// balance. A zero amount revokes the allowance.
pub fn approve(
    env: &Env,
    owner: &Address,
    spender: &Address,
    amount: i128,
    expiration_ledger: u32,
) -> Result<(), TokenError> {
    pause::when_not_paused(env).map_err(|_| TokenError::ContractPaused)?;
    owner.require_auth();
    // Reject negative amounts (#257)
    if amount < 0 {
        return Err(TokenError::InvalidAmount);
    }
    // SEP-41: a live allowance may not expire in the past. A zero allowance
    // is a revocation and is always accepted.
    if amount > 0 && !is_expiration_valid(env, expiration_ledger) {
        return Err(TokenError::InvalidExpirationLedger);
    }
    let mut allowances: Map<(Address, Address), AllowanceData> = env
        .storage()
        .persistent()
        .get(&ALLOWANCES_KEY)
        .ok_or(TokenError::NotInitialized)?;
    let key = (owner.clone(), spender.clone());
    allowances.set(
        key,
        AllowanceData {
            amount,
            expiration_ledger,
        },
    );
    env.storage().persistent().set(&ALLOWANCES_KEY, &allowances);
    Approve {
        from: owner.clone(),
        spender: spender.clone(),
        amount,
        expiration_ledger,
    }
    .publish(env);
    Ok(())
}

/// Returns the amount `spender` may still transfer from `from`.
///
/// SEP-41 requires an expired allowance to be reported as a zero allowance.
pub fn allowance(env: &Env, owner: &Address, spender: &Address) -> i128 {
    let data = allowance_data(env, owner, spender);
    if is_expired(env, &data) {
        0
    } else {
        data.amount
    }
}

/// Returns the raw allowance record, including the expiration ledger.
///
/// `allowance` is the SEP-41 view (expired allowances read as zero); this
/// extension exposes the stored record for UIs that need to render when the
/// allowance lapses.
pub fn allowance_data(env: &Env, owner: &Address, spender: &Address) -> AllowanceData {
    let allowances: Map<(Address, Address), AllowanceData> = env
        .storage()
        .persistent()
        .get(&ALLOWANCES_KEY)
        .unwrap_or_else(|| Map::new(env));
    let key = (owner.clone(), spender.clone());
    allowances.get(key).unwrap_or(AllowanceData {
        amount: 0,
        expiration_ledger: 0,
    })
}

pub fn balance(env: &Env, account: &Address) -> i128 {
    let balances: Map<Address, i128> = env
        .storage()
        .persistent()
        .get(&BALANCES_KEY)
        .unwrap_or_else(|| Map::new(env));
    balances.get(account.clone()).unwrap_or(0)
}

pub fn total_supply(env: &Env) -> i128 {
    env.storage().instance().get(&TOTAL_KEY).unwrap_or(0)
}

pub fn name(env: &Env) -> String {
    if !is_initialized(env) {
        return String::from_str(env, "");
    }
    TokenUtils::new(env).metadata().get_metadata().name
}

pub fn symbol(env: &Env) -> String {
    if !is_initialized(env) {
        return String::from_str(env, "");
    }
    TokenUtils::new(env).metadata().get_metadata().symbol
}

pub fn decimals(env: &Env) -> u32 {
    if !is_initialized(env) {
        return 0;
    }
    TokenUtils::new(env).metadata().get_metadata().decimal
}

pub fn mint(env: &Env, admin: &Address, to: &Address, amount: i128) -> Result<(), TokenError> {
    pause::when_not_paused(env).map_err(|_| TokenError::ContractPaused)?;
    admin.require_auth();
    require_admin(env, admin)?;
    if to == &env.current_contract_address() {
        return Err(TokenError::CannotTransferToSelf);
    }
    validate_non_frozen(env, to)?;
    if amount <= 0 {
        return Err(TokenError::InvalidAmount);
    }
    let mut balances: Map<Address, i128> = env
        .storage()
        .persistent()
        .get(&BALANCES_KEY)
        .ok_or(TokenError::NotInitialized)?;
    let current: i128 = balances.get(to.clone()).unwrap_or(0);
    balances.set(to.clone(), current.checked_add(amount).ok_or(TokenError::Overflow)?);
    env.storage().persistent().set(&BALANCES_KEY, &balances);
    let total: i128 = env.storage().instance().get(&TOTAL_KEY).unwrap_or(0);
    env.storage()
        .instance()
        .set(&TOTAL_KEY, &total.checked_add(amount).ok_or(TokenError::Overflow)?);
    MintWithAmountOnly {
        to: to.clone(),
        amount,
    }
    .publish(env);
    Ok(())
}

pub fn burn(env: &Env, from: &Address, amount: i128) -> Result<(), TokenError> {
    pause::when_not_paused(env).map_err(|_| TokenError::ContractPaused)?;
    from.require_auth();
    validate_non_frozen(env, from)?;
    if amount <= 0 {
        return Err(TokenError::InvalidAmount);
    }
    let mut balances: Map<Address, i128> = env
        .storage()
        .persistent()
        .get(&BALANCES_KEY)
        .ok_or(TokenError::NotInitialized)?;
    let current: i128 = balances.get(from.clone()).unwrap_or(0);
    if current < amount {
        return Err(TokenError::InsufficientBalance);
    }
    balances.set(from.clone(), current.checked_sub(amount).ok_or(TokenError::Underflow)?);
    env.storage().persistent().set(&BALANCES_KEY, &balances);
    let total: i128 = env.storage().instance().get(&TOTAL_KEY).unwrap_or(0);
    env.storage()
        .instance()
        .set(&TOTAL_KEY, &total.checked_sub(amount).ok_or(TokenError::Underflow)?);
    Burn {
        from: from.clone(),
        amount,
    }
    .publish(env);
    Ok(())
}

/// Burns `amount` from `from`, consuming `spender`'s allowance.
///
/// SEP-41 `burn_from`: the allowance is reduced by `amount` and its expiration
/// ledger is left untouched. Use [`admin_burn_from`] for the administrative
/// variant that does not require an allowance.
pub fn burn_from(
    env: &Env,
    spender: &Address,
    from: &Address,
    amount: i128,
) -> Result<(), TokenError> {
    pause::when_not_paused(env).map_err(|_| TokenError::ContractPaused)?;
    spender.require_auth();
    if amount <= 0 {
        return Err(TokenError::InvalidAmount);
    }
    validate_non_frozen(env, from)?;
    consume_allowance(env, from, spender, amount)?;
    burn_balance(env, from, amount)?;
    Burn {
        from: from.clone(),
        amount,
    }
    .publish(env);
    Ok(())
}

/// Admin-authorized burn (distinct from holder burn and from the SEP-41
/// allowance-consuming `burn_from`).
///
/// Allows admin to burn tokens from any account. Emits `Burn` event.
/// Requires admin authorization and validates total supply invariant.
pub fn admin_burn_from(
    env: &Env,
    admin: &Address,
    from: &Address,
    amount: i128,
) -> Result<(), TokenError> {
    pause::when_not_paused(env).map_err(|_| TokenError::ContractPaused)?;
    admin.require_auth();
    require_admin(env, admin)?;
    validate_non_frozen(env, from)?;
    if amount <= 0 {
        return Err(TokenError::InvalidAmount);
    }
    burn_balance(env, from, amount)?;
    Burn {
        from: from.clone(),
        amount,
    }
    .publish(env);
    Ok(())
}

/// Claws back tokens from an account balance and reduces total supply.
///
/// # Intentional Design: Frozen Account Bypass
/// Clawback is an administrative action intended for regulatory compliance, fraud
/// mitigation, and legal enforcement (aligning with Stellar SEP-0041 asset standards).
/// An administrator is intentionally permitted to claw back tokens from an account even
/// if that account is frozen. Requiring unfreezing prior to clawback would create a race
/// condition allowing unauthorized outbound transfers before the clawback completes.
/// Regular user operations (`transfer`, `burn`) strictly enforce non-frozen checks.
pub fn clawback(
    env: &Env,
    admin: &Address,
    from: &Address,
    amount: i128,
) -> Result<(), TokenError> {
    pause::when_not_paused(env).map_err(|_| TokenError::ContractPaused)?;
    admin.require_auth();
    require_admin(env, admin)?;
    if amount <= 0 {
        return Err(TokenError::InvalidAmount);
    }
    burn_balance(env, from, amount)?;
    Clawback {
        from: from.clone(),
        amount,
    }
    .publish(env);
    Ok(())
}

pub fn freeze(env: &Env, admin: &Address, account: &Address) -> Result<(), TokenError> {
    pause::when_not_paused(env).map_err(|_| TokenError::ContractPaused)?;
    admin.require_auth();
    require_admin(env, admin)?;
    let mut frozen: Map<Address, bool> = env
        .storage()
        .persistent()
        .get(&FROZEN_KEY)
        .ok_or(TokenError::NotInitialized)?;
    frozen.set(account.clone(), true);
    env.storage().persistent().set(&FROZEN_KEY, &frozen);
    AccountFrozen {
        account: account.clone(),
    }
    .publish(env);
    Ok(())
}

pub fn unfreeze(env: &Env, admin: &Address, account: &Address) -> Result<(), TokenError> {
    pause::when_not_paused(env).map_err(|_| TokenError::ContractPaused)?;
    admin.require_auth();
    require_admin(env, admin)?;
    let mut frozen: Map<Address, bool> = env
        .storage()
        .persistent()
        .get(&FROZEN_KEY)
        .ok_or(TokenError::NotInitialized)?;
    frozen.set(account.clone(), false);
    env.storage().persistent().set(&FROZEN_KEY, &frozen);
    AccountUnfrozen {
        account: account.clone(),
    }
    .publish(env);
    Ok(())
}

pub fn is_frozen(env: &Env, account: &Address) -> bool {
    let frozen: Map<Address, bool> = env
        .storage()
        .persistent()
        .get(&FROZEN_KEY)
        .unwrap_or_else(|| Map::new(env));
    frozen.get(account.clone()).unwrap_or(false)
}

pub fn set_admin(env: &Env, admin: &Address, new_admin: &Address) -> Result<(), TokenError> {
    pause::when_not_paused(env).map_err(|_| TokenError::ContractPaused)?;
    admin.require_auth();
    require_admin(env, admin)?;
    env.storage().instance().set(&ADMIN_KEY, new_admin);
    Ok(())
}

pub fn get_admin(env: &Env) -> Result<Address, TokenError> {
    env.storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(TokenError::NotInitialized)
}

/// Pauses the token, rejecting all mutations until unpaused.
///
/// Only the stored admin can pause. Emergency response for security incidents.
/// Auth is performed once inside `common::pause` — calling `require_auth`
/// here as well would double-authorize the same address in one frame.
pub fn pause(env: &Env, admin: &Address) -> Result<(), TokenError> {
    require_admin(env, admin)?;
    pause::pause(env, admin).map_err(|_| TokenError::ContractPaused)
}

/// Unpauses the token, allowing mutations to resume.
///
/// Only the stored admin can unpause.
pub fn unpause(env: &Env, admin: &Address) -> Result<(), TokenError> {
    require_admin(env, admin)?;
    pause::unpause(env, admin).map_err(|_| TokenError::ContractPaused)
}

fn require_admin(env: &Env, caller: &Address) -> Result<(), TokenError> {
    let stored: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(TokenError::NotInitialized)?;
    if caller != &stored {
        return Err(TokenError::Unauthorized);
    }
    Ok(())
}

fn validate_non_frozen(env: &Env, account: &Address) -> Result<(), TokenError> {
    if is_frozen(env, account) {
        return Err(TokenError::Frozen);
    }
    Ok(())
}

/// Enable or disable allowlist mode for transfer recipients (admin-only).
///
/// When enabled, `transfer` and `transfer_from` reject any transfer whose
/// recipient is not on the allowlist (see `add_to_allowlist`). Default is
/// off. Emits `AllowlistModeChanged`.
///
/// # Authorization
/// Requires `admin.require_auth()` and `admin` must be the stored admin.
pub fn set_allowlist_mode(env: &Env, admin: &Address, enabled: bool) -> Result<(), TokenError> {
    pause::when_not_paused(env).map_err(|_| TokenError::ContractPaused)?;
    admin.require_auth();
    require_admin(env, admin)?;
    env.storage().instance().set(&ALLOWLIST_MODE_KEY, &enabled);
    AllowlistModeChanged { enabled }.publish(env);
    Ok(())
}

/// Returns whether allowlist mode is currently enabled. Defaults to `false`
/// when never toggled.
pub fn is_allowlist_mode_enabled(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&ALLOWLIST_MODE_KEY)
        .unwrap_or(false)
}

/// Add `account` to the recipient allowlist (admin-only). Emits `AllowlistAdded`.
///
/// # Authorization
/// Requires `admin.require_auth()` and `admin` must be the stored admin.
pub fn add_to_allowlist(env: &Env, admin: &Address, account: &Address) -> Result<(), TokenError> {
    pause::when_not_paused(env).map_err(|_| TokenError::ContractPaused)?;
    admin.require_auth();
    require_admin(env, admin)?;
    let mut list: Map<Address, bool> = env
        .storage()
        .persistent()
        .get(&ALLOWLIST_KEY)
        .unwrap_or_else(|| Map::new(env));
    list.set(account.clone(), true);
    env.storage().persistent().set(&ALLOWLIST_KEY, &list);
    AllowlistAdded {
        account: account.clone(),
    }
    .publish(env);
    Ok(())
}

/// Remove `account` from the recipient allowlist (admin-only). Emits `AllowlistRemoved`.
///
/// # Authorization
/// Requires `admin.require_auth()` and `admin` must be the stored admin.
pub fn remove_from_allowlist(
    env: &Env,
    admin: &Address,
    account: &Address,
) -> Result<(), TokenError> {
    pause::when_not_paused(env).map_err(|_| TokenError::ContractPaused)?;
    admin.require_auth();
    require_admin(env, admin)?;
    let mut list: Map<Address, bool> = env
        .storage()
        .persistent()
        .get(&ALLOWLIST_KEY)
        .unwrap_or_else(|| Map::new(env));
    list.set(account.clone(), false);
    env.storage().persistent().set(&ALLOWLIST_KEY, &list);
    AllowlistRemoved {
        account: account.clone(),
    }
    .publish(env);
    Ok(())
}

/// Returns whether `account` is currently on the recipient allowlist.
pub fn is_allowlisted(env: &Env, account: &Address) -> bool {
    let list: Map<Address, bool> = env
        .storage()
        .persistent()
        .get(&ALLOWLIST_KEY)
        .unwrap_or_else(|| Map::new(env));
    list.get(account.clone()).unwrap_or(false)
}

/// When allowlist mode is enabled, reject transfers to a non-allowlisted `to`.
fn validate_allowlisted(env: &Env, to: &Address) -> Result<(), TokenError> {
    if is_allowlist_mode_enabled(env) && !is_allowlisted(env, to) {
        return Err(TokenError::RecipientNotAllowlisted);
    }
    Ok(())
}

/// True when the contract has been initialized.
fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&ADMIN_KEY)
}

/// True when `expiration_ledger` may be used for a live allowance.
///
/// `0` means "never expires" — the convention the Go backend has always used.
/// Any other value must be at or after the current ledger.
fn is_expiration_valid(env: &Env, expiration_ledger: u32) -> bool {
    expiration_ledger == 0 || expiration_ledger >= env.ledger().sequence()
}

/// True when the allowance record has lapsed.
fn is_expired(env: &Env, allowance: &AllowanceData) -> bool {
    allowance.expiration_ledger != 0 && env.ledger().sequence() > allowance.expiration_ledger
}

/// Reduces `spender`'s allowance on `from` by `amount`.
///
/// Follows SEP-41: the allowance is decreased, its expiration ledger is kept,
/// and the operation fails when the allowance is missing, expired or too
/// small.
fn consume_allowance(
    env: &Env,
    from: &Address,
    spender: &Address,
    amount: i128,
) -> Result<(), TokenError> {
    let mut allowances: Map<(Address, Address), AllowanceData> = env
        .storage()
        .persistent()
        .get(&ALLOWANCES_KEY)
        .ok_or(TokenError::NotInitialized)?;
    let key = (from.clone(), spender.clone());
    let allowance: AllowanceData = allowances
        .get(key.clone())
        .ok_or(TokenError::Unauthorized)?;
    if is_expired(env, &allowance) {
        return Err(TokenError::AllowanceExpired);
    }
    if allowance.amount < amount {
        return Err(TokenError::AllowanceExceeded);
    }
    let new_allowance = allowance
        .amount
        .checked_sub(amount)
        .ok_or(TokenError::Underflow)?;
    allowances.set(
        key,
        AllowanceData {
            amount: new_allowance,
            expiration_ledger: allowance.expiration_ledger,
        },
    );
    env.storage().persistent().set(&ALLOWANCES_KEY, &allowances);
    Ok(())
}

/// Removes `amount` from `from`'s balance and from the total supply.
fn burn_balance(env: &Env, from: &Address, amount: i128) -> Result<(), TokenError> {
    let mut balances: Map<Address, i128> = env
        .storage()
        .persistent()
        .get(&BALANCES_KEY)
        .ok_or(TokenError::NotInitialized)?;
    let current: i128 = balances.get(from.clone()).unwrap_or(0);
    if current < amount {
        return Err(TokenError::InsufficientBalance);
    }
    balances.set(from.clone(), current.checked_sub(amount).ok_or(TokenError::Underflow)?);
    env.storage().persistent().set(&BALANCES_KEY, &balances);

    // Invariant: total supply decreases by exactly the burned amount
    let total: i128 = env.storage().instance().get(&TOTAL_KEY).unwrap_or(0);
    let new_total = total.checked_sub(amount).ok_or(TokenError::Underflow)?;
    env.storage().instance().set(&TOTAL_KEY, &new_total);
    Ok(())
}

/// Publishes the canonical SEP-41 `transfer` event.
///
/// Muxed transfers carry the muxed id in the event data (map format); plain
/// transfers use the single-value format, exactly like the Stellar Asset
/// Contract does.
fn publish_transfer(env: &Env, from: &Address, to: &MuxedAddress, amount: i128) {
    match to.id() {
        Some(to_muxed_id) => Transfer {
            from: from.clone(),
            to: to.address(),
            to_muxed_id: Some(to_muxed_id),
            amount,
        }
        .publish(env),
        None => TransferWithAmountOnly {
            from: from.clone(),
            to: to.address(),
            amount,
        }
        .publish(env),
    }
}

use soroban_sdk::{contracterror, contractevent, contracttype, Address};

/// Canonical Stellar token events, re-exported from `soroban-token-sdk`.
///
/// The governance token emits the very same event shapes as the Stellar Asset
/// Contract (SAC) so that standard indexers — and the Go backend — can decode
/// governance-token activity with the exact same logic they use for SAC
/// assets:
///
/// | Event                  | Topics                        | Data                                |
/// |------------------------|-------------------------------|-------------------------------------|
/// | `transfer`             | `("transfer", from, to)`      | `{ amount, to_muxed_id }` (map)     |
/// | `TransferWithAmountOnly` | `("transfer", from, to)`    | `amount` (single value)             |
/// | `approve`              | `("approve", from, spender)`  | `[amount, expiration_ledger]` (vec) |
/// | `mint`                 | `("mint", to)`                | `{ amount, to_muxed_id }` (map)     |
/// | `MintWithAmountOnly`   | `("mint", to)`                | `amount` (single value)             |
/// | `burn`                 | `("burn", from)`              | `amount` (single value)             |
/// | `clawback`             | `("clawback", from)`          | `amount` (single value)             |
pub use soroban_token_sdk::events::{
    Approve, Burn, Clawback, Mint, MintWithAmountOnly, Transfer, TransferWithAmountOnly,
};

/// Raw allowance record for an `(owner, spender)` pair.
///
/// `expiration_ledger` is the ledger at which the allowance stops being
/// spendable. `0` means "never expires" (a documented deviation from the
/// strict SEP-41 wording, kept for backwards compatibility with the Go
/// backend, which has always used `0` for non-expiring allowances).
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllowanceData {
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[contracterror]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenError {
    NotInitialized = 1,
    Unauthorized = 2,
    InsufficientBalance = 3,
    InvalidAmount = 4,
    Overflow = 5,
    AllowanceExpired = 6,
    AllowanceExceeded = 7,
    NegativeAllowance = 8,
    NotAdmin = 9,
    ContractPaused = 10,
    Underflow = 11,
    Frozen = 12,
    RecipientNotAllowlisted = 13,
    /// `expiration_ledger` is in the past while a non-zero allowance was set.
    ///
    /// SEP-41 requires the expiration ledger of a live allowance to be at or
    /// after the current ledger; only a zero (revoking) allowance may point
    /// into the past.
    InvalidExpirationLedger = 14,
    CannotTransferToSelf = 16,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct AccountFrozen {
    pub account: Address,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct AccountUnfrozen {
    pub account: Address,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct AllowlistModeChanged {
    pub enabled: bool,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct AllowlistAdded {
    pub account: Address,
}


#[contractevent]
#[derive(Clone, Debug)]
pub struct AllowlistRemoved {
    pub account: Address,
}

/// Event emitted when the contract admin is changed via set_admin
#[contractevent(topics = ["admin_changed"])]
#[derive(Clone, Debug)]
pub struct AdminChanged {
    #[topic]
    pub old_admin: Address,
    pub new_admin: Address,
}

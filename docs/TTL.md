# Storage TTL Policy

Soroban archives persistent and instance entries once their time-to-live (TTL)
expires. An archived entry cannot be read until it is restored, so every
contract that stores data must extend that TTL while it is still in use.

The shared helpers live in `packages/common/src/ttl.rs`:

- `ttl::extend_persistent(env, &key)` extends one persistent entry. It is a
  no-op when the key is absent, because `extend_ttl` traps the host on a
  missing key.
- `ttl::extend_instance(env)` extends the contract instance and its code.

## Thresholds

Values are in ledgers. One day is `LEDGERS_PER_DAY = 17_280` at the ~5 second
network close time.

| Storage | Extend when remaining TTL is below | Extend to |
|---|---|---|
| Persistent | 30 days | 90 days |
| Instance (and code) | 7 days | 30 days |

Both extend-to values stay below the network `max_entry_ttl`.

## Where it is applied

- `reputation-registry`: every persistent read and write in `storage.rs`,
  `scoring.rs` and `contract.rs`, and instance extension on the public entry
  points (`init`, `record`, `get_score`, `get_history`, `pause`, `unpause`).

Other contracts (`circle`, `circle-factory`, `treasury`, `staking`,
`escrow-swap`, `governance`, `governance-token`) do not yet call these helpers.

## Operational note

Extension is paid for by the caller on each access. Entries that are never
touched again still expire after the last extension, so an organiser or
off-chain job must touch long-lived data at least once every 90 days.

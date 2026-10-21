# circle — Core ROSCA Engine

Purpose: Per-circle Soroban contract implementing the Rotating Savings and Credit Association (ROSCA) lifecycle. Members join, contribute a fixed amount each round, and one member receives the pooled pot per round via the configured payout type. The circle is the primary fund-holding contract and integrates with treasury (fees), reputation-registry (MoiScore), and factory (deployment/configuration).

## Entry points

Lifecycle / membership:
- `__constructor(admin, factory, config)` — initialize circle with organizer/admin, factory, and CircleConfig
- `join(member)` — member joins the circle
- `exit_circle(member)` — member exits the circle
- `batch_invite(caller, members)` — organizer invites multiple members at once

Contributions:
- `contribute(member, amount, round)` — submit contribution for a round (must match configured amount)
- `check_contribution_deadline()` — enforce contribution deadlines for stale rounds

Payouts (4 types, configured via `config.payout_type`):
- `trigger_payout(caller, round)` — execute payout for the round
- `schedule_payout(caller, round)` — schedule a payout
- `is_payout_scheduled(round)` — query whether a payout is scheduled
- `auction_bid(bidder, discount_bips, round)` — place an English auction bid
- `refund_losing_bids(caller, round, limit)` — refund losing auction deposits in capped batches
- `init_dutch_auction(caller, round, start_bips, floor_bips, decay_bips_per_ledger, expiry_ledgers)` — configure a Dutch auction
- `dutch_auction_price(round)` — current Dutch auction price + resolved flag
- `dutch_auction_bid(bidder, round)` — place a Dutch auction bid
- `vote_payout(voter, vote_for, round)` — vote payout recipient (vote payout type)
- `cancel_auction(caller)` — cancel unresolved auction + refund highest bid atomically

Disputes:
- `report_late(reporter, late_member, round)` — report a late member
- `dispute(member, evidence_hash)` / `raise_dispute(member, evidence_hash)` — raise a dispute
- `resolve_dispute(admin, resolution)` — admin resolves a dispute

Treasury / configuration:
- `withdraw_treasury(caller, amount)` — organizer/admin treasury withdrawal (capped per tx and per UTC day)
- `set_reputation_registry(admin, registry)`, `get_reputation_registry()`
- `set_treasury(admin, treasury)`, `get_treasury()`
- `set_token(admin, token)`, `set_fee_bps(admin, fee_bps)`
- `set_allowlist(admin, allowlist)`, `get_allowlist()`
- `set_oracle(admin, oracle)`, `set_fallback_oracle(admin, oracle)`, `get_oracle()`, `get_fallback_oracle()`
- `configure_from_factory(factory, treasury, reputation_registry, fee_bps)` — factory pushes protocol config
- `get_fee_bps()`

Referrals / streaks:
- `register_referral(caller, referrer, referred, bonus_pct)`
- `claim_referral_bonus(referrer)`
- `update_streak(member, round)`
- `claim_streak_bonus(member)`
- `get_referrals()`, `get_streaks()`, `get_member_streak(member)`

Pause:
- `pause_circle(admin)`, `unpause_circle(admin)`

Reads:
- `get_status()` — Circle state
- `get_members()` — Vec<Member>
- `get_contributions(member, page, page_size)` — paginated contributions
- `get_dispute_resolution()` — Option<DisputeResolutionRecord>
- `query_top_contributors(n)` — top contributors by total received
- `query_round_config(round)` — BytesN<32> round config snapshot
- `get_round_fee_ledger(round)` — RoundFeeLedger

## Storage keys (DataKey)
Persistent unless noted:

| Key | Meaning |
|-----|---------|
| `Circle` | Circle state (instance storage) |
| `Admin` / `Factory` / `FeeBps` / `Treasury` / `Token` / `Allowlist` / `ReputationRegistry` / `OracleContract` / `FallbackOracle` | configuration |
| `Members` | Vec<Member> |
| `Contributions` | Vec<Contribution> per round |
| `Payouts` | payout history |
| `Bids` | auction bids |
| `Votes` | vote entries |
| `Dispute` / `DisputeResolution` | dispute state |
| `Referrals` / `Streaks` | referral + streak state |
| `RoundConfigSnapshot(u32)` | per-round config snapshot |
| `PayoutScheduled(u32)` | scheduled payout flag |
| `DutchAuction(u32)` | Dutch auction config per round |
| `RoundFeeLedger(u32)` | per-round fee ledger |
| `AuctionWinner(u32)` | resolved English auction winner |
| `WithdrawalDay` / `WithdrawalAmount` | #478 treasury withdrawal window tracking |
| `RoundEnforced` | #325 last round swept by deadline check |
| `RoundStrikes(u32)` | #503 strikes recorded per round |
| `RoundTotalContributed(u32)` | #406 pre-computed round contribution total |
| `RoundContributors(u32)` | #406 pre-computed round contributors list |
| `MemberContributed(Address, u32)` | #406 O(1) single-entry contribution flag |
| `MemberStatus(Address)` | #406 O(1) member status lookup |

## Events

| Event | Topics | Meaning |
|-------|--------|---------|
| `MemberJoined { member, position }` | — | member joined |
| `ContributionRecorded { member, round, amount, on_time }` | — | contribution recorded |
| `PayoutExecuted { recipient, round, amount, fee, payout_type }` | — | payout executed |
| `CircleHealthUpdated { health_score, on_time_rate_bps, completion_rate_bps, member_retention_bps, round }` | — | #369 health score recomputed after payout |
| `MemberExited { member, penalty }` | — | member exited |
| `MemberDefaulted { member, strikes }` | — | member defaulted |
| `CircleCompleted { total_payouts }` | — | circle completed all rounds |
| `CircleCancelled { circle_id, cancelled_by, cancelled_at }` | — | circle cancelled |
| `DisputeRaised { member, evidence_hash }` | — | dispute raised |
| `DisputeResolved { member, resolution, outcome_code, resolved_by }` | — | dispute resolved |
| `AuctionBidPlaced { bidder, discount_bips, round }` | — | auction bid placed |
| `AuctionLoserRefunded { bidder, round, amount }` | — | losing bid refunded |
| `VoteCast { voter, vote_for, round }` | — | vote cast |
| `ReferralRegistered { referrer, referred, bonus_pct }` | — | referral registered |
| `OracleFallbackUsed { round, primary_oracle, fallback_oracle }` | — | fallback oracle used |
| `FactoryConfigured { factory, treasury, reputation_registry, fee_bps }` | — | factory config pushed |
| `LatePenaltyApplied { member, round, penalty }` | — | late penalty applied |
| `TreasuryWithdrawn { caller, amount, daily_total }` | — | #478 treasury withdrawal |
| `AuctionCancelled { round, cancelled_by, refunded_bidder, refunded_amount }` | — | auction cancelled |

## Invariants
- Contribution amount must exactly match `CircleConfig.contribution_amount` for the round.
- A member may contribute at most once per round (`AlreadyContributed`).
- Payout bitmap (`u128`) caps circle size at 100 members (`MAX_CIRCLE_MEMBERS`).
- Circle size/contribution may not exceed the member's MoiScore tier (`CircleSizeExceedsTier`, `ContributionExceedsTier`).
- Max strikes enforced; exceeded strikes default/remove the member.
- Pausing rejects all mutating functions (`ContractPaused`).
- Treasury withdrawals capped per-transaction and per rolling UTC day (#478).
- Reentrant calls rejected (`ReentrantCall`) — defense in depth on top of Soroban host policy.

## Test command
```bash
# From repo root
cargo test -p circle
cargo test -p circle --doc
# Full workspace
cargo test --workspace
```

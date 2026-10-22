use crate::types::*;
use common::{pause, ttl};
use soroban_sdk::{symbol_short, Address, Env, Vec};

const ACTIVITY_PAGE_SIZE: u32 = 50;

pub fn init(env: &Env, admin: &Address) -> Result<(), ReputationError> {
    admin.require_auth();
    env.storage().instance().set(&DataKey::Admin, admin);
    ttl::extend_instance(env);
    Ok(())
}

pub fn record(env: &Env, user: &Address, t: u32, impact: u32) -> Result<(), ReputationError> {
    pause::when_not_paused(env).map_err(|_| ReputationError::ContractPaused)?;
    // Trust Model (Issue #221): This function relies entirely on cross-contract calls from
    // trusted circle contracts for data integrity. user.require_auth() prevents arbitrary
    // invocation, but does not prevent inflation if called by an unverified circle contract.
    user.require_auth();
    if t > ACTIVITY_PAYOUT_RECEIVED {
        return Err(ReputationError::InvalidActivityType);
    }
    if impact > 1000 {
        return Err(ReputationError::InvalidScoreImpact);
    }
    ttl::extend_instance(env);
    // Checked before any write so an overflowing counter leaves state untouched.
    let total_acts: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::UserActivityCount(user.clone()))
        .unwrap_or(0);
    let next_total_acts = total_acts.checked_add(1).ok_or(ReputationError::Overflow)?;
    let now = env.ledger().timestamp();
    let mut score = env
        .storage()
        .persistent()
        .get(&DataKey::Score(user.clone()))
        .unwrap_or_else(|| MoiScore {
            user: user.clone(),
            score: 0,
            tier: TIER_BRONZE,
            total_circles: 0,
            completed_circles: 0,
            defaulted_circles: 0,
            streak_count: 0,
            last_activity_at: now,
            updated_at: now,
        });
    let old = score.score;
    let ns = score.score.saturating_add(impact);
    score.score = if ns > 1000 { 1000 } else { ns };
    match t {
        ACTIVITY_JOIN => {
            score.total_circles = score
                .total_circles
                .checked_add(1)
                .ok_or(ReputationError::Overflow)?;
        }
        ACTIVITY_CONTRIBUTE => {
            score.streak_count = score
                .streak_count
                .checked_add(1)
                .ok_or(ReputationError::Overflow)?;
        }
        ACTIVITY_COMPLETE => {
            score.completed_circles = score
                .completed_circles
                .checked_add(1)
                .ok_or(ReputationError::Overflow)?;
        }
        ACTIVITY_DEFAULT => {
            score.defaulted_circles = score
                .defaulted_circles
                .checked_add(1)
                .ok_or(ReputationError::Overflow)?;
            score.streak_count = 0;
        }
        _ => {}
    }
    score.tier = if score.score >= 800 {
        TIER_DIAMOND
    } else if score.score >= 600 {
        TIER_PLATINUM
    } else if score.score >= 400 {
        TIER_GOLD
    } else if score.score >= 200 {
        TIER_SILVER
    } else {
        TIER_BRONZE
    };
    score.last_activity_at = now;
    score.updated_at = now;
    let score_key = DataKey::Score(user.clone());
    env.storage().persistent().set(&score_key, &score);
    ttl::extend_persistent(env, &score_key);

    // Paginated activity storage per user (fixes #245)
    let page_index = total_acts / ACTIVITY_PAGE_SIZE;
    let page_key = DataKey::UserActivityPage(user.clone(), page_index);
    let mut acts: Vec<Activity> = env
        .storage()
        .persistent()
        .get(&page_key)
        .unwrap_or_else(|| Vec::new(env));
    acts.push_back(Activity {
        user: user.clone(),
        activity_type: t,
        score_impact: impact,
        timestamp: now,
    });
    env.storage().persistent().set(&page_key, &acts);
    ttl::extend_persistent(env, &page_key);
    let count_key = DataKey::UserActivityCount(user.clone());
    env.storage().persistent().set(&count_key, &next_total_acts);
    ttl::extend_persistent(env, &count_key);

    env.events().publish(
        (env.current_contract_address(), symbol_short!("activity")),
        ActivityRecorded {
            user: user.clone(),
            activity_type: t,
            score_impact: impact,
            new_score: score.score,
        },
    );
    if old != score.score {
        env.events().publish(
            (env.current_contract_address(), symbol_short!("score_up")),
            ScoreUpdated {
                user: user.clone(),
                old_score: old,
                new_score: score.score,
                tier: score.tier,
            },
        );
    }
    Ok(())
}

// Issue #220: filed against a version of this contract that stored scores
// under a single shared `DataKey::Scores` key, so every user's `record()`
// overwrote the same slot. That's already been fixed here — the key is
// `DataKey::Score(Address)`, one persistent entry per user — confirmed by
// reading both this function and `record()` above, which write/read the
// same per-user key. No behavior change in this PR.
pub fn get_score(env: &Env, u: &Address) -> Result<MoiScore, ReputationError> {
    pause::when_not_paused(env).map_err(|_| ReputationError::ContractPaused)?;
    ttl::extend_instance(env);
    let score_key = DataKey::Score(u.clone());
    let stored: Option<MoiScore> = env.storage().persistent().get(&score_key);
    ttl::extend_persistent(env, &score_key);
    Ok(stored.unwrap_or_else(|| MoiScore {
        user: u.clone(),
        score: 0,
        tier: TIER_BRONZE,
        total_circles: 0,
        completed_circles: 0,
        defaulted_circles: 0,
        streak_count: 0,
        last_activity_at: 0,
        updated_at: 0,
    }))
}

pub fn get_history(env: &Env, user: &Address, page: u32, page_size: u32) -> Vec<Activity> {
    let mut out = Vec::new(env);
    if page_size == 0 {
        return out;
    }
    ttl::extend_instance(env);
    let count_key = DataKey::UserActivityCount(user.clone());
    let total: u32 = env.storage().persistent().get(&count_key).unwrap_or(0);
    ttl::extend_persistent(env, &count_key);
    let start = page.saturating_mul(page_size);
    if start >= total {
        return out;
    }
    let end = start.saturating_add(page_size).min(total);
    let mut current_page_idx = u32::MAX;
    let mut current_page_vec: Option<Vec<Activity>> = None;
    for idx in start..end {
        let p_idx = idx / ACTIVITY_PAGE_SIZE;
        let offset = idx % ACTIVITY_PAGE_SIZE;
        if p_idx != current_page_idx {
            current_page_idx = p_idx;
            let page_key = DataKey::UserActivityPage(user.clone(), p_idx);
            current_page_vec = env.storage().persistent().get(&page_key);
            ttl::extend_persistent(env, &page_key);
        }
        if let Some(ref p_vec) = current_page_vec {
            if let Some(act) = p_vec.get(offset) {
                out.push_back(act);
            }
        }
    }
    out
}

pub fn pause(env: &Env, a: &Address) -> Result<(), ReputationError> {
    let s: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(ReputationError::NotInitialized)?;
    if a != &s {
        return Err(ReputationError::Unauthorized);
    }
    ttl::extend_instance(env);
    pause::pause(env, a).map_err(|_| ReputationError::ContractPaused)
}

pub fn unpause(env: &Env, a: &Address) -> Result<(), ReputationError> {
    let s: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(ReputationError::NotInitialized)?;
    if a != &s {
        return Err(ReputationError::Unauthorized);
    }
    ttl::extend_instance(env);
    pause::unpause(env, a).map_err(|_| ReputationError::ContractPaused)
}

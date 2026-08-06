//! Manager entity, appointment, performance tracking, and firing/rehire (Design round 5,
//! Slice 7-8) — a new, lightweight, non-SoA entity type. There are only ~1,200 managers
//! (one per club) plus a small reserve pool; this is `Club`-scale data (a small `Vec` of a
//! small struct), not `Population`-scale, so no column-oriented storage is needed.
//!
//! "Generated but consistent": manager generation (7.2), identity-shift blending (7.3), and
//! the fire/rehire draw (8.4) are each pure functions of `world_seed` (+ manager/club/season
//! indices), on forked seed streams, never sharing state with match/transfer/injury RNG.

use crate::history;
use crate::world::{Club, ClubId, WorldGenesis, NUM_CLUBS};
use goat_core::roles::NUM_ROLES;
use goat_core::tactical_identity::TacticalIdentity;
use goat_fixed::Fixed;
use goat_rng::{GoatRng, RngSource};

pub type ManagerId = u32;

/// Rolling points-per-match form window length (Design's own pick, flagged in the task doc's
/// "Decisions" section for Tùng's sign-off — not derived from any real data).
pub const MANAGER_FORM_WINDOW: usize = 10;

/// Reserve pool of unemployed managers, on top of one per club (Design's own pick, flagged
/// alongside `MANAGER_FORM_WINDOW`).
pub const MANAGER_POOL_SIZE: usize = NUM_CLUBS / 4; // 300

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manager {
    pub id: ManagerId,
    pub name: String, // reuses history::name_from_seed
    /// The manager's own natural tactical bias — same type Doc B already built, reused
    /// verbatim, not extended.
    pub identity_bias: TacticalIdentity,
    /// Rolling points-per-match ring buffer (3/1/0), most recent overwrites oldest. Used by
    /// the firing trigger (8.3).
    pub recent_points: [u8; MANAGER_FORM_WINDOW],
    pub recent_idx: u8,
    pub tenure_start_season: u32,
    /// Count of matches recorded into `recent_points` since this manager's current tenure
    /// began (reset to 0 on every hire, including rehire). Gates the firing trigger (8.3)
    /// against evaluating a manager off a not-yet-full ring buffer.
    pub matches_played: u16,
}

fn manager_seed(world_seed: u64, m: usize) -> u64 {
    world_seed
        ^ 0xD3u64.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (m as u64)
            .rotate_left(23)
            .wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
}

impl Manager {
    fn generate(world_seed: u64, id: ManagerId) -> Self {
        let seed = manager_seed(world_seed, id as usize);
        Manager {
            id,
            name: history::name_from_seed(seed),
            // Distinct sub-seed from the name draw, same "^1" idiom this codebase's other
            // sibling-but-distinct rolls already use.
            identity_bias: TacticalIdentity::generate(seed ^ 1),
            recent_points: [0; MANAGER_FORM_WINDOW],
            recent_idx: 0,
            tenure_start_season: 0,
            matches_played: 0,
        }
    }
}

/// The whole manager population: one per club, plus a reserve pool. A closed system — total
/// manager count is fixed at genesis (`NUM_CLUBS + MANAGER_POOL_SIZE`); firing returns a
/// manager to `free_agents`, hiring draws one out. Never grows, never runs out.
#[derive(Debug, Clone)]
pub struct ManagerPool {
    pub managers: Vec<Manager>,       // index = ManagerId as usize
    pub club_manager: Vec<ManagerId>, // per-club current manager, index = ClubId
    pub free_agents: Vec<ManagerId>,  // currently unemployed, available to hire
}

impl ManagerPool {
    pub fn genesis(world_seed: u64, world: &WorldGenesis) -> Self {
        let total = world.clubs.len() + MANAGER_POOL_SIZE;
        let managers: Vec<Manager> = (0..total)
            .map(|m| Manager::generate(world_seed, m as ManagerId))
            .collect();
        let club_manager: Vec<ManagerId> = (0..world.clubs.len() as ManagerId).collect();
        let free_agents: Vec<ManagerId> =
            (world.clubs.len() as ManagerId..total as ManagerId).collect();
        Self {
            managers,
            club_manager,
            free_agents,
        }
    }

    /// Push this season's per-match points (both sides of every fixture) into each match's
    /// home/away manager's rolling form ring buffer. Not reset at season boundaries — form
    /// genuinely spans a rolling `MANAGER_FORM_WINDOW`-match window regardless of where in
    /// the season it falls.
    pub fn record_match_points(&mut self, match_points: &[(ClubId, u8)]) {
        for &(club_id, pts) in match_points {
            let mgr_id = self.club_manager[club_id];
            let mgr = &mut self.managers[mgr_id as usize];
            let idx = mgr.recent_idx as usize;
            mgr.recent_points[idx] = pts;
            mgr.recent_idx = ((idx + 1) % MANAGER_FORM_WINDOW) as u8;
            mgr.matches_played = mgr.matches_played.saturating_add(1);
        }
    }
}

/// New manager's weight in the appointment identity-shift blend, out of 1000 (Fixed scale) —
/// Design's own split (70% new manager / 30% club legacy), flagged for sign-off.
const MANAGER_IDENTITY_BLEND_PCT: i64 = 700;

/// Called whenever a club gets a new manager (initial genesis assignment excluded — genesis
/// managers and genesis clubs start with independently-generated identities, no blend needed
/// at t=0). Shifts `club.tactical_identity` toward the incoming manager's own bias rather
/// than replacing it outright — "shift," not "reset."
pub fn apply_manager_identity_shift(club: &mut Club, manager: &Manager) {
    let mut raw = [0i32; NUM_ROLES];
    let mut sum: i64 = 0;
    for (r, slot) in raw.iter_mut().enumerate() {
        let old = club.tactical_identity.role_weight[r].to_raw() as i64;
        let new = manager.identity_bias.role_weight[r].to_raw() as i64;
        let blended =
            (old * (1000 - MANAGER_IDENTITY_BLEND_PCT) + new * MANAGER_IDENTITY_BLEND_PCT) / 1000;
        *slot = blended as i32;
        sum += blended;
    }
    // Re-normalize so the weights still sum to NUM_ROLES * 1.0, the invariant
    // TacticalIdentity::generate itself maintains — a blend of two already-normalized
    // vectors can drift slightly from integer rounding; re-apply the same sum-and-rescale
    // step `generate` uses rather than a new normalization routine.
    let target = NUM_ROLES as i64 * 1000;
    for (r, &w) in raw.iter().enumerate() {
        club.tactical_identity.role_weight[r] = Fixed::raw(((w as i64 * target) / sum) as i32);
    }
}

/// Expected points-per-game for a club of this strength, on the standard 3/1/0 scale.
/// Linear: strength 50 (dead average) -> 1.5 ppg (a genuine mid-table rate); strength 99 ->
/// ~2.24 ppg; strength 1 -> ~0.76 ppg. Coefficients are Design's own fit, not derived from
/// any real points-per-strength data (none exists in this codebase) — flagged for sign-off.
fn expected_ppg(strength: u8) -> Fixed {
    Fixed::raw(1_500 + (strength as i32 - 50) * 15)
}

fn actual_ppg(recent_points: &[u8; MANAGER_FORM_WINDOW]) -> Fixed {
    let sum: i32 = recent_points.iter().map(|&p| p as i32).sum();
    Fixed::raw(sum * 1_000 / MANAGER_FORM_WINDOW as i32)
}

/// Actual ppg below this percent of expected ppg -> fired (Design's own pick, flagged for
/// sign-off alongside `expected_ppg`'s coefficients).
const FIRING_UNDERPERFORMANCE_PCT: i64 = 70;

/// Whether `manager` should be fired given their club's strength, gated on a grace period
/// (`matches_played >= MANAGER_FORM_WINDOW`) so a brand-new manager with a partially-zeroed
/// ring buffer never reads as catastrophic underperformance from partial data.
pub fn should_fire(manager: &Manager, strength: u8, _season: u32) -> bool {
    if (manager.matches_played as usize) < MANAGER_FORM_WINDOW {
        return false;
    }
    let actual = actual_ppg(&manager.recent_points);
    let threshold = expected_ppg(strength) * Fixed::raw((FIRING_UNDERPERFORMANCE_PCT * 10) as i32);
    actual < threshold
}

/// Fire `club`'s current manager and draw a deterministic, seed-derived replacement from the
/// free-agent pool. The fired manager goes back into `free_agents` (rather than being
/// discarded) — what keeps `ManagerPool` closed: a sacked manager is available to be hired
/// elsewhere later, matching real-world managerial merry-go-rounds without modeling
/// reputation/preference on the hiring side (every free agent is equally likely to be drawn
/// — Design's own simplification, flagged for sign-off).
pub fn hire_replacement(
    pool: &mut ManagerPool,
    club: &mut Club,
    club_id: ClubId,
    world_seed: u64,
    season: u32,
) {
    let fired = pool.club_manager[club_id];
    pool.free_agents.push(fired);
    pool.managers[fired as usize].recent_points = [0; MANAGER_FORM_WINDOW];
    pool.managers[fired as usize].recent_idx = 0;
    pool.managers[fired as usize].matches_played = 0;

    let seed = manager_seed(world_seed, 0xE4) ^ (club_id as u64) ^ (season as u64);
    let mut rng = GoatRng::new(seed);
    let draw = rng.next_range_u32(0, pool.free_agents.len() as u32 - 1) as usize;
    let hired = pool.free_agents.remove(draw);

    pool.club_manager[club_id] = hired;
    pool.managers[hired as usize].tenure_start_season = season;
    // Resetting `matches_played` on the *hired* manager (not just the fired one) matters
    // just as much: a free agent pulled out of the pool starts their new tenure's grace
    // period from zero, exactly like a freshly-generated manager would.
    pool.managers[hired as usize].matches_played = 0;
    apply_manager_identity_shift(club, &pool.managers[hired as usize]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldGenesis;

    // ── Slice 7 TDD anchors ──────────────────────────────────────────────────────

    #[test]
    fn manager_genesis_pool_is_closed() {
        let world = WorldGenesis::generate(11);
        let pool = ManagerPool::genesis(11, &world);
        assert_eq!(
            pool.managers.len(),
            pool.club_manager.len() + pool.free_agents.len()
        );
    }

    #[test]
    fn manager_generation_is_deterministic() {
        let a = Manager::generate(42, 7);
        let b = Manager::generate(42, 7);
        assert_eq!(a, b);
    }

    #[test]
    fn identity_shift_moves_toward_manager_not_past_it() {
        let world = WorldGenesis::generate(13);
        let mut club = world.clubs[0].clone();
        let manager = Manager::generate(99, 0);

        let before = club.tactical_identity.role_weight;
        apply_manager_identity_shift(&mut club, &manager);
        let after = club.tactical_identity.role_weight;

        for r in 0..NUM_ROLES {
            let old = before[r].to_raw();
            let new = manager.identity_bias.role_weight[r].to_raw();
            let shifted = after[r].to_raw();
            if old == new {
                assert_eq!(
                    shifted, old,
                    "role {r}: no movement expected when old == new"
                );
                continue;
            }
            let (lo, hi) = if old < new { (old, new) } else { (new, old) };
            assert!(
                shifted > lo && shifted < hi,
                "role {r}: shifted value {shifted} must lie strictly between old {old} and new {new}"
            );
        }
    }

    #[test]
    fn identity_shift_preserves_the_sum_invariant() {
        let world = WorldGenesis::generate(17);
        let mut club = world.clubs[0].clone();
        let manager = Manager::generate(101, 3);

        apply_manager_identity_shift(&mut club, &manager);

        let sum: i32 = club
            .tactical_identity
            .role_weight
            .iter()
            .map(|w| w.to_raw())
            .sum();
        assert!(
            (sum - NUM_ROLES as i32 * 1000).abs() <= NUM_ROLES as i32,
            "role weights should sum to ~{}, got {sum}",
            NUM_ROLES * 1000
        );
    }

    // ── Slice 8 TDD anchors ──────────────────────────────────────────────────────

    #[test]
    fn should_fire_false_before_grace_period_fills() {
        let mut manager = Manager::generate(1, 0);
        manager.matches_played = MANAGER_FORM_WINDOW as u16 - 1;
        // All-zero recent_points too — even so, grace period must gate this off.
        assert!(!should_fire(&manager, 80, 1));
    }

    #[test]
    fn should_fire_true_for_sustained_underperformance() {
        let mut manager = Manager::generate(1, 0);
        manager.matches_played = MANAGER_FORM_WINDOW as u16;
        manager.recent_points = [0; MANAGER_FORM_WINDOW];
        assert!(should_fire(&manager, 80, 1));
    }

    #[test]
    fn should_fire_false_for_matching_or_exceeding_expectation() {
        for &strength in &[1u8, 50, 99] {
            let mut manager = Manager::generate(1, 0);
            manager.matches_played = MANAGER_FORM_WINDOW as u16;
            // Every match a win (3pts) comfortably meets/exceeds expectation at any strength.
            manager.recent_points = [3; MANAGER_FORM_WINDOW];
            assert!(
                !should_fire(&manager, strength, 1),
                "strength {strength}: a manager on maximum form must never be fired"
            );
        }
    }

    #[test]
    fn hire_replacement_keeps_pool_closed() {
        let world = WorldGenesis::generate(23);
        let mut pool = ManagerPool::genesis(23, &world);
        let mut club = world.clubs[0].clone();
        hire_replacement(&mut pool, &mut club, 0, 23, 1);
        assert_eq!(
            pool.managers.len(),
            pool.club_manager.len() + pool.free_agents.len()
        );
        hire_replacement(&mut pool, &mut club, 0, 23, 2);
        assert_eq!(
            pool.managers.len(),
            pool.club_manager.len() + pool.free_agents.len()
        );
    }

    #[test]
    fn hire_replacement_resets_form_and_grace_period_but_keeps_identity() {
        let world = WorldGenesis::generate(29);
        let mut pool = ManagerPool::genesis(29, &world);
        let mut club = world.clubs[0].clone();

        let fired_id = pool.club_manager[0];
        pool.managers[fired_id as usize].recent_points = [3; MANAGER_FORM_WINDOW];
        pool.managers[fired_id as usize].matches_played = 25;
        let fired_identity_before = pool.managers[fired_id as usize].identity_bias.clone();

        hire_replacement(&mut pool, &mut club, 0, 29, 1);

        assert_eq!(
            pool.managers[fired_id as usize].recent_points,
            [0; MANAGER_FORM_WINDOW]
        );
        assert_eq!(pool.managers[fired_id as usize].matches_played, 0);
        assert_eq!(
            pool.managers[fired_id as usize].identity_bias,
            fired_identity_before
        );

        // Now put the same manager back through another club's firing cycle to prove a
        // rehire elsewhere also resets grace period but keeps identity. Move `fired_id` out
        // of `free_agents` into `club_manager[1]` "by hand" (simulating a prior, unrelated
        // hire) so the pool stays internally consistent (never both a free agent and
        // employed at once) before firing it again for real.
        let mut club2 = world.clubs[1].clone();
        let pos = pool
            .free_agents
            .iter()
            .position(|&id| id == fired_id)
            .expect("fired_id must be a free agent after the first hire_replacement");
        pool.free_agents.remove(pos);
        // Club 1's own genesis manager becomes a free agent in `fired_id`'s place, so no
        // manager is dropped from the pool's bookkeeping by this manual swap.
        pool.free_agents.push(pool.club_manager[1]);
        pool.club_manager[1] = fired_id;
        pool.managers[fired_id as usize].matches_played = 40;
        hire_replacement(&mut pool, &mut club2, 1, 29, 2);
        assert_eq!(pool.managers[fired_id as usize].matches_played, 0);
        assert_eq!(
            pool.managers[fired_id as usize].identity_bias,
            fired_identity_before
        );
        assert_eq!(
            pool.managers.len(),
            pool.club_manager.len() + pool.free_agents.len(),
            "manual free-agent bookkeeping must keep the pool closed"
        );
    }

    #[test]
    fn identity_shift_applies_on_every_hire_not_just_genesis() {
        let world = WorldGenesis::generate(31);
        let mut pool = ManagerPool::genesis(31, &world);
        let mut club = world.clubs[0].clone();

        let before = club.tactical_identity.role_weight;
        hire_replacement(&mut pool, &mut club, 0, 31, 1);
        let hired_id = pool.club_manager[0];
        let hired_bias = pool.managers[hired_id as usize].identity_bias.role_weight;

        assert_ne!(
            hired_bias, before,
            "test setup must draw a manager whose bias differs from the pre-hire identity"
        );
        assert_ne!(
            club.tactical_identity.role_weight, before,
            "hire_replacement must shift the club's tactical_identity, not just at genesis"
        );
    }
}

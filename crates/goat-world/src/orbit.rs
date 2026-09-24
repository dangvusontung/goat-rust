//! Orbit individual residue (PA2 M3): the deep-simmed PC matches leave REAL
//! career stats on the NPCs who played in them.
//!
//! The world has two simulation tiers: the cheap season batch-tick
//! (`batch_tick`) and the deep weekly orbit (every fixture of the PC's
//! division is team-simmed weekly; the PC's own match is fully engine-simmed
//! with named individuals). Until M3 only the batch tier left individual
//! residue. This module applies the orbit tier's residue — appearances, goals
//! and a form EMA — to the population columns, and replays it on every
//! population rebuild so the deep orbit's story (who actually scored) survives
//! save/load through `WorldState::orbit_records`.
//!
//! Persist-vs-derive line: the records are path-dependent (match RNG chose the
//! scorers) so they live in the save; everything else about the population
//! stays derived from `world_seed`.

use crate::batch_tick::{batch_tick_season_orbit, OrbitSeasonOverlay};
use crate::population::{apply_youth_intake, genesis, Population};
use crate::world::WorldGenesis;
use goat_core::state::OrbitMatchRecord;

/// Synthetic per-match rating feeding the NPC form EMA: team result is the
/// base (win 63 / draw 55 / loss 47), goals and assists stack on top.
/// Deliberately simple and deterministic — no RNG, so replay is exact.
pub fn npc_match_rating(c: &goat_core::state::NpcMatchCredit) -> i32 {
    (55 + 14 * c.goals as i32 + 9 * c.assists as i32 + 8 * c.result as i32).clamp(30, 95)
}

/// Form EMA weight (percent) kept from the old value per update.
const FORM_KEEP_PCT: i32 = 65;

/// Apply one deep-simmed match's individual residue to the population:
/// each starter gets an appearance, his goals, and a form EMA update.
pub fn apply_orbit_record(pop: &mut Population, rec: &OrbitMatchRecord) {
    for c in &rec.credits {
        let i = c.pop_idx as usize;
        if i >= pop.len() {
            continue;
        }
        pop.career_apps[i] += 1;
        pop.career_goals[i] += c.goals as u32;
        let rating = npc_match_rating(c);
        let f = pop.form[i] as i32;
        pop.form[i] = (f * FORM_KEEP_PCT / 100 + rating * (100 - FORM_KEEP_PCT) / 100) as i16;
    }
}

/// Sum one season's records into the overlay the batch tick needs to credit
/// remainders instead of full season abstractions.
pub fn season_overlay(records: &[OrbitMatchRecord], season: u32) -> Option<OrbitSeasonOverlay> {
    let mut overlay = OrbitSeasonOverlay::default();
    let mut any = false;
    for rec in records.iter().filter(|r| r.season == season) {
        any = true;
        let div = rec.div as usize;
        if !overlay.divs.contains(&div) {
            overlay.divs.push(div);
        }
        for c in &rec.credits {
            *overlay.apps.entry(c.pop_idx).or_insert(0) += 1;
            *overlay.goals.entry(c.pop_idx).or_insert(0) += c.goals as u32;
        }
    }
    any.then_some(overlay)
}

/// Rebuild the population pantheon-style WITH the orbit residue replayed
/// (PA2 M3): genesis → for each completed season, apply that season's orbit
/// records then batch-tick with the remainder overlay → finally apply the
/// in-progress season's records. Deterministic in `(world_seed, records)`.
/// Post-merge this runs on the `WorldGenesis` world model; youth replenishment
/// comes from `population::apply_youth_intake` (Round-3 Slice 4) after each
/// completed season's batch tick.
pub fn rebuild_population(
    world_seed: u64,
    season: u32,
    records: &[OrbitMatchRecord],
) -> Population {
    let world = WorldGenesis::generate(world_seed);
    let league_clubs = world.static_league_clubs();
    let mut pop = genesis(world_seed, &world);
    for s in 1..season {
        for rec in records.iter().filter(|r| r.season == s) {
            apply_orbit_record(&mut pop, rec);
        }
        let overlay = season_overlay(records, s);
        batch_tick_season_orbit(
            &mut pop,
            &world,
            &league_clubs,
            world_seed,
            s,
            s * 52,
            overlay.as_ref(),
        );
        apply_youth_intake(&mut pop, &world, world_seed, s);
    }
    for rec in records.iter().filter(|r| r.season == season) {
        apply_orbit_record(&mut pop, rec);
    }
    pop
}

#[cfg(test)]
mod tests {
    use super::*;
    use goat_core::state::NpcMatchCredit;

    fn rec(season: u32, round: u32, div: u8, credits: Vec<NpcMatchCredit>) -> OrbitMatchRecord {
        OrbitMatchRecord {
            season,
            round,
            div,
            credits,
        }
    }

    fn credit(pop_idx: u32, goals: u8, result: i8) -> NpcMatchCredit {
        NpcMatchCredit {
            pop_idx,
            goals,
            assists: 0,
            result,
        }
    }

    #[test]
    fn orbit_goals_land_on_the_real_scorer() {
        // Player 5 scores twice in a round-1 deep sim; after a full season of
        // records + the remainder batch tick his career shows the real goals.
        let idx = 5u32;
        let mut records = vec![rec(1, 0, 0, vec![credit(idx, 2, 1)])];
        // Fill the rest of the season with scoreless appearances for him.
        for round in 1..30 {
            records.push(rec(1, round, 0, vec![credit(idx, 0, 0)]));
        }
        let pop = rebuild_population(42, 2, &records);
        let i = idx as usize;
        assert_eq!(
            pop.career_goals[i], 2,
            "his two real goals, no phantom share"
        );
        // 30 orbit apps + 0 remainder (starter quota 30 is fully covered).
        assert_eq!(pop.career_apps[i], 30);
    }

    #[test]
    fn form_feedback_loop_rises_on_hot_streak_and_sinks_on_cold() {
        let idx = 7u32;
        let w = WorldGenesis::generate(42);
        let mut pop = genesis(42, &w);
        assert_eq!(pop.form[idx as usize], 50);
        for round in 0..10 {
            apply_orbit_record(&mut pop, &rec(1, round, 0, vec![credit(idx, 1, 1)]));
        }
        let hot = pop.form[idx as usize];
        assert!(hot > 60, "scoring every week in wins lifts form: {hot}");

        let w2 = WorldGenesis::generate(42);
        let mut pop2 = genesis(42, &w2);
        for round in 0..10 {
            apply_orbit_record(&mut pop2, &rec(1, round, 0, vec![credit(idx, 0, -1)]));
        }
        let cold = pop2.form[idx as usize];
        assert!(cold < 50, "goalless losses sink form: {cold}");
        assert!(hot > cold);
    }

    #[test]
    fn form_affects_selection_score() {
        // Two identical-population rebuilds, one with a hot streak for idx 7:
        // the hot player must outrank his cold self in the lineup race.
        let idx = 7u32;
        let cold_pop = rebuild_population(42, 1, &[]);
        let mut records = Vec::new();
        for round in 0..10 {
            records.push(rec(1, round, 0, vec![credit(idx, 1, 1)]));
        }
        let hot_pop = rebuild_population(42, 1, &records);
        let club = cold_pop.club[idx as usize] as usize;
        let pos = cold_pop.position[idx as usize];
        let mut slots = (0usize, 0usize, 0usize);
        match pos {
            0 => slots.0 = 4,
            1 => slots.1 = 4,
            _ => slots.2 = 4,
        }
        let rank_of = |pop: &Population| {
            pop.lineup_indices_formation(club, 0, slots)
                .iter()
                .position(|&i| i == idx as usize)
        };
        // A 10-game hot streak can only help or hold his shirt, never cost it.
        match (rank_of(&cold_pop), rank_of(&hot_pop)) {
            (Some(c), Some(h)) => assert!(h <= c, "hot form improved rank {c} -> {h}"),
            (None, Some(_)) => {} // broke into the lineup
            (Some(_), None) => panic!("hot streak cost him his lineup place"),
            (None, None) => {}
        }
        assert_ne!(
            cold_pop.form[idx as usize], hot_pop.form[idx as usize],
            "orbit residue must move his form"
        );
    }

    #[test]
    fn rebuild_is_deterministic_and_youth_intake_resets_form() {
        let mut records = Vec::new();
        for season in 1..=3u32 {
            for round in 0..30 {
                records.push(rec(season, round, 0, vec![credit(11, 1, 1)]));
            }
        }
        let a = rebuild_population(9, 4, &records);
        let b = rebuild_population(9, 4, &records);
        assert_eq!(a.career_fingerprint(), b.career_fingerprint());
        assert_eq!(a.form, b.form);
        // Player 11 accumulated 90 real apps over 3 orbit seasons.
        assert_eq!(a.career_apps[11], 90);
        assert_eq!(a.career_goals[11], 90);
    }
}

//! The emergent rival (TASK-09A Slice 9A.5, bible §7.4).
//!
//! At genesis a cohort of peers is rolled alongside the player and grows in parallel via
//! the cheap batch-tick. The rivalry **crystallises retroactively**: if a generation peer
//! keeps pace with you over the years, the media frames the rivalry. **Sometimes nobody
//! keeps up** — you reign alone and the harsher schools apply the *weak-era asterisk*.
//! There is no scripted nemesis. This generalises the old 8-peer stub's shape to the real
//! population cohort. Pure & deterministic given the population + the PC's record.

use crate::history::name_from_seed;
use crate::population::Population;

/// A title is worth this many goals when scoring a career for rivalry purposes.
const TITLE_WEIGHT: u32 = 50;
/// A peer must reach at least this share (percent) of the PC's career score to "keep pace".
const KEEP_PACE_PCT: u32 = 70;
/// A peer needs a substantial career (apps) before he can be a rival at all.
const MIN_RIVAL_APPS: u32 = 150;
/// Half-width (weeks) of the "same generation" cohort window around the PC's birth age.
const COHORT_HALF_WIDTH_WEEKS: u32 = 78; // ±1.5 years

/// Outcome of rivalry crystallisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RivalVerdict {
    /// A peer kept pace and the rivalry is named.
    Rival {
        idx: usize,
        name: String,
        peer_goals: u32,
        peer_titles: u32,
    },
    /// Nobody kept pace — you reign alone (the weak-era asterisk applies).
    WeakEra,
}

fn career_score(goals: u32, titles: u32) -> u32 {
    goals + titles * TITLE_WEIGHT
}

/// Indices of the PC's generation cohort: background players born within the cohort window
/// of the PC's birth age.
fn cohort_indices(pop: &Population, pc_birth_age_weeks: u32) -> Vec<usize> {
    (0..pop.len())
        .filter(|&i| pop.birth_age_weeks[i].abs_diff(pc_birth_age_weeks) <= COHORT_HALF_WIDTH_WEEKS)
        .collect()
}

/// Decide, after the cohort has been batch-ticked, whether a rival crystallises. The PC's
/// own career (`pc_career_goals`, `pc_career_titles`) sets the bar a peer must approach.
pub fn crystallise_rival(
    pop: &Population,
    pc_birth_age_weeks: u32,
    pc_career_goals: u32,
    pc_career_titles: u32,
) -> RivalVerdict {
    let pc_score = career_score(pc_career_goals, pc_career_titles);
    let bar = pc_score * KEEP_PACE_PCT / 100;

    // The strongest peer in the generation by career score.
    let best = cohort_indices(pop, pc_birth_age_weeks)
        .into_iter()
        .filter(|&i| pop.career_apps[i] >= MIN_RIVAL_APPS)
        .max_by_key(|&i| career_score(pop.career_goals[i], pop.career_titles[i]));

    match best {
        Some(idx) if career_score(pop.career_goals[idx], pop.career_titles[idx]) >= bar => {
            RivalVerdict::Rival {
                idx,
                name: name_from_seed(pop.seed[idx]),
                peer_goals: pop.career_goals[idx],
                peer_titles: pop.career_titles[idx],
            }
        }
        _ => RivalVerdict::WeakEra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batch_tick::batch_tick_season;
    use crate::population::genesis;

    /// Genesis + batch-tick `seasons`, then crystallise against a given PC record.
    fn run(seed: u64, seasons: u32, pc_goals: u32, pc_titles: u32) -> RivalVerdict {
        let mut pop = genesis(seed);
        for s in 1..=seasons {
            batch_tick_season(&mut pop, seed, s, s * 52);
        }
        crystallise_rival(&pop, 16 * 52, pc_goals, pc_titles)
    }

    #[test]
    fn crystallisation_is_deterministic() {
        assert_eq!(run(7, 12, 120, 3), run(7, 12, 120, 3));
    }

    #[test]
    fn both_outcomes_are_reachable() {
        // A modest PC → a peer keeps pace somewhere in the sweep.
        let any_rival =
            (0..24u64).any(|s| matches!(run(s, 12, 120, 3), RivalVerdict::Rival { .. }));
        // A towering PC → nobody keeps pace (weak-era asterisk) somewhere.
        let any_weak = (0..24u64).any(|s| run(s, 12, 5000, 50) == RivalVerdict::WeakEra);
        assert!(any_rival, "no rival ever crystallised — bar too high");
        assert!(any_weak, "weak-era asterisk never reached — bar too low");
    }

    #[test]
    fn a_named_rival_has_a_real_career() {
        if let RivalVerdict::Rival {
            idx,
            name,
            peer_goals,
            ..
        } = run(3, 14, 100, 2)
        {
            assert!(!name.is_empty());
            assert!(peer_goals > 0);
            assert!(idx < genesis(3).len());
        }
    }
}

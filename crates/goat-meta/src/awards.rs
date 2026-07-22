//! Season awards: league Best XI and Player of the Year.
//!
//! Awards are seeded and deterministic. AI competitors are generated from
//! (world_seed, season, candidate_idx) — never from real player stats.
//! The PC can win or lose; losing to a rival must be possible.

use goat_rng::{GoatRng, RngSource};

/// Result of one award computation.
#[derive(Debug, Clone)]
pub struct AwardResult {
    pub award_name: &'static str,
    pub winner_name: String,
    pub runner_up_name: Option<String>,
    pub pc_won: bool,
    /// PC's score used for comparison (output avg 0–100 or goal count).
    pub pc_score: i32,
    /// Winner's score (same unit as pc_score).
    pub winner_score: i32,
}

/// AI candidate names pool (deterministically indexed).
const AI_NAMES: &[&str] = &[
    "A. Ferreira",
    "L. Petersen",
    "M. Ibarra",
    "K. Tanaka",
    "R. Boateng",
    "S. Novak",
    "T. Reyes",
    "V. Kline",
    "D. Moreau",
    "C. Osei",
    "J. Haas",
    "P. Svensson",
    "N. Kalinda",
    "B. Varga",
    "O. Diouf",
    "F. Mota",
];

/// Generate an AI competitor score for this season.
fn ai_competitor_score(
    world_seed: u64,
    season: u32,
    candidate_idx: u8,
    base_min: i32,
    base_max: i32,
) -> i32 {
    // Hash-mix seed for a stochastic AI competitor score. This is a scramble, not an
    // arithmetic quantity — wrapping is the correct semantics (matches release-mode
    // behavior), not a bug being papered over.
    let mut rng = GoatRng::new(
        world_seed
            ^ (season as u64).wrapping_mul(0x9e3779b97f4a7c15)
            ^ (candidate_idx as u64).wrapping_mul(0xdeadbeef),
    );
    let range = (base_max - base_min).max(1) as u64;
    base_min + rng.next_range_u64(0, range - 1) as i32
}

/// Compute the Player of the Year award for a season.
///
/// The PC competes against a seeded pool of AI players. The highest score wins.
/// `pc_output` is the PC's average output this season (0–100).
/// `pc_goals` is used as a tiebreaker.
pub fn compute_player_of_year(
    pc_name: &str,
    pc_output: i32,
    _pc_goals: u32, // future tiebreaker
    season: u32,
    world_seed: u64,
) -> AwardResult {
    let pc_score = pc_output;

    // 8 AI competitors per season.
    let mut best_ai_score = 0i32;
    let mut best_ai_name = "";
    let mut second_ai_score = 0i32;
    let mut second_ai_name = "";

    for i in 0..8u8 {
        let ai_score = ai_competitor_score(world_seed, season, i, 55, 90);
        let ai_name = AI_NAMES[i as usize % AI_NAMES.len()];
        if ai_score > best_ai_score {
            second_ai_score = best_ai_score;
            second_ai_name = best_ai_name;
            best_ai_score = ai_score;
            best_ai_name = ai_name;
        } else if ai_score > second_ai_score {
            second_ai_score = ai_score;
            second_ai_name = ai_name;
        }
    }

    let pc_won = pc_score >= best_ai_score;

    if pc_won {
        AwardResult {
            award_name: "Player of the Year",
            winner_name: pc_name.to_string(),
            runner_up_name: Some(best_ai_name.to_string()),
            pc_won: true,
            pc_score,
            winner_score: pc_score,
        }
    } else {
        AwardResult {
            award_name: "Player of the Year",
            winner_name: best_ai_name.to_string(),
            runner_up_name: if second_ai_name.is_empty() {
                None
            } else {
                Some(second_ai_name.to_string())
            },
            pc_won: false,
            pc_score,
            winner_score: best_ai_score,
        }
    }
}

/// Compute the League Golden Boot (top scorer) for the PC's division.
pub fn compute_golden_boot(
    pc_name: &str,
    pc_goals: u32,
    season: u32,
    world_seed: u64,
) -> AwardResult {
    let pc_score = pc_goals as i32;

    let mut best_ai = 0i32;
    let mut best_ai_name = "";

    for i in 8..16u8 {
        let ai_goals = ai_competitor_score(world_seed, season, i, 5, 28);
        let ai_name = AI_NAMES[i as usize % AI_NAMES.len()];
        if ai_goals > best_ai {
            best_ai = ai_goals;
            best_ai_name = ai_name;
        }
    }

    let pc_won = pc_score >= best_ai;
    AwardResult {
        award_name: "Golden Boot",
        winner_name: if pc_won {
            pc_name.to_string()
        } else {
            best_ai_name.to_string()
        },
        runner_up_name: if pc_won {
            Some(best_ai_name.to_string())
        } else {
            None
        },
        pc_won,
        pc_score,
        winner_score: if pc_won { pc_score } else { best_ai },
    }
}

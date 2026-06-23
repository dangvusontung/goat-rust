//! Four-facet reputation (bible §8.2).
//!
//! Sporting / Marketability / Character / Club-fan each evolve independently.
//! Marketability is a stub until Phase 10.

use goat_fixed::Fixed;

/// Four-facet reputation, all 0–100.
#[derive(Debug, Clone, Copy)]
pub struct Reputation {
    /// Output + trophies + longevity → Sporting.
    pub sporting: Fixed,
    /// Flair + decisive moments → Marketability (stub at 50 until Phase 10).
    pub marketability: Fixed,
    /// Inversely tracks dirty/clean rep: high rep = good character.
    pub character: Fixed,
    /// Loyalty + performances for the club fan base.
    pub club_fan: Fixed,
}

impl Reputation {
    pub fn label(&self) -> &'static str {
        let avg = (self.sporting.to_int() + self.character.to_int() + self.club_fan.to_int()) / 3;
        match avg {
            0..=29 => "Unknown",
            30..=44 => "Journeyman",
            45..=59 => "Respected Pro",
            60..=74 => "Cult Hero",
            75..=89 => "Club Legend",
            _ => "Iconic",
        }
    }
}

/// Derive Reputation from WorldState scalars.
///
/// `discipline_rep` is the Phase 6 dirty/clean scalar (50 = neutral, 100 = very dirty).
/// Character is the inverse: 50 neutral → character 50; 0 dirty → character 100.
pub fn compute_reputation(
    pc_sporting_rep: i32,
    pc_discipline_rep: i32,
    pc_club_fan_rep: i32,
) -> Reputation {
    Reputation {
        sporting: Fixed::from_int(pc_sporting_rep.clamp(0, 100)),
        marketability: Fixed::from_int(50), // stub until Phase 10
        character: Fixed::from_int((100 - pc_discipline_rep).clamp(0, 100)),
        club_fan: Fixed::from_int(pc_club_fan_rep.clamp(0, 100)),
    }
}

/// Update sporting rep based on a season finish and output.
pub fn update_sporting_rep(current: i32, season_avg_output: i32, finish_pos: u32) -> i32 {
    let output_delta = (season_avg_output - 60) / 10; // +ve if above-average
    let position_delta = if finish_pos <= 3 {
        3
    } else if finish_pos <= 6 {
        1
    } else {
        -1
    };
    (current + output_delta + position_delta).clamp(0, 100)
}

/// Update club-fan rep based on loyalty and performance.
pub fn update_club_fan_rep(
    current: i32,
    longest_tenure: u32,
    season_avg_output: i32,
    clubs_served: u32,
) -> i32 {
    let loyalty_bonus = if longest_tenure >= 5 {
        3
    } else if longest_tenure >= 3 {
        1
    } else {
        0
    };
    let jumper_pen = if clubs_served > 4 { -3 } else { 0 };
    let perf_delta = (season_avg_output - 60) / 15;
    (current + loyalty_bonus + jumper_pen + perf_delta).clamp(0, 100)
}

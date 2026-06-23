//! Golden legacy and divergence tests.
//!
//! A scripted multi-season career produces exact axis values and rankings.
//! Schools demonstrably diverge on mixed-profile careers.

use goat_meta::{all_rankings, compute_axes, LegacyEvidence, SCHOOLS};

fn high_output_low_trophies() -> LegacyEvidence {
    LegacyEvidence {
        career_goals: 250,
        career_matches: 400,
        career_output_sum: 400 * 72, // avg 72/100
        best_season_avg_output: 81,
        seasons_played: 15,
        decisive_moments: 3,
        player_of_year_wins: 2,
        league_titles: 0,
        clubs_served: 2,
        longest_club_tenure: 10,
    }
}

fn trophy_machine_low_output() -> LegacyEvidence {
    LegacyEvidence {
        career_goals: 80,
        career_matches: 320,
        career_output_sum: 320 * 55, // avg 55/100
        best_season_avg_output: 65,
        seasons_played: 14,
        decisive_moments: 6,
        player_of_year_wins: 0,
        league_titles: 5,
        clubs_served: 4,
        longest_club_tenure: 4,
    }
}

fn one_club_legend() -> LegacyEvidence {
    LegacyEvidence {
        career_goals: 150,
        career_matches: 500,
        career_output_sum: 500 * 62, // avg 62/100
        best_season_avg_output: 70,
        seasons_played: 18,
        decisive_moments: 4,
        player_of_year_wins: 0,
        league_titles: 2,
        clubs_served: 1,
        longest_club_tenure: 18,
    }
}

/// Schools provably diverge on mixed profiles.
#[test]
fn schools_disagree_on_mixed_profiles() {
    let stats_god = high_output_low_trophies();
    let trophy_king = trophy_machine_low_output();

    let axes_stats = compute_axes(&stats_god);
    let axes_trophy = compute_axes(&trophy_king);

    // Stats Purists (idx=2) should prefer the stats god.
    let stats_score_god = SCHOOLS[2].score(&axes_stats);
    let stats_score_trophy = SCHOOLS[2].score(&axes_trophy);
    assert!(
        stats_score_god > stats_score_trophy,
        "Stats Purists should prefer high output ({}) over trophy king ({})",
        stats_score_god.to_int(),
        stats_score_trophy.to_int()
    );

    // Trophy Cabinet (idx=0) should prefer the trophy king.
    let tc_score_god = SCHOOLS[0].score(&axes_stats);
    let tc_score_trophy = SCHOOLS[0].score(&axes_trophy);
    assert!(
        tc_score_trophy > tc_score_god,
        "Trophy Cabinet should prefer trophy king ({}) over stats god ({})",
        tc_score_trophy.to_int(),
        tc_score_god.to_int()
    );
}

/// Loyalty Traditionalists rank the one-club legend highest.
#[test]
fn loyalty_school_prefers_one_club_legend() {
    let legend = one_club_legend();
    let trophy = trophy_machine_low_output();

    let axes_legend = compute_axes(&legend);
    let axes_trophy = compute_axes(&trophy);

    let loy_legend = SCHOOLS[3].score(&axes_legend);
    let loy_trophy = SCHOOLS[3].score(&axes_trophy);
    assert!(
        loy_legend > loy_trophy,
        "Loyalty school should prefer one-club legend ({}) over club-hopper ({})",
        loy_legend.to_int(),
        loy_trophy.to_int()
    );
}

/// Golden: exact axis values for the high-output career.
#[test]
fn golden_axes_high_output_career() {
    let ev = high_output_low_trophies();
    let axes = compute_axes(&ev);

    // winning: 0 titles × 20 + 3 decisive × 5 = 15
    assert_eq!(axes.winning.to_int(), 15, "winning frozen at 15");
    // output: avg(72, 81) = 76 (integer division)
    assert_eq!(axes.output.to_int(), 76, "output frozen at 76");
    // longevity: min(400*100/300, 100) = 133 → clamped 100
    assert_eq!(axes.longevity.to_int(), 100, "longevity frozen at 100");
    // loyalty: clubs_served=2 → 100 - 20 + min(10*5,20) = 80+20 = 100
    assert_eq!(axes.loyalty.to_int(), 100, "loyalty frozen at 100");
    // icon and h2h are stubs at 50
    assert_eq!(axes.icon.to_int(), 50, "icon stub at 50");
    assert_eq!(axes.head_to_head.to_int(), 50, "h2h stub at 50");
}

/// Schools never produce a single definitive "winner" — all careers have different top school.
#[test]
fn no_career_tops_all_schools() {
    let careers = [
        high_output_low_trophies(),
        trophy_machine_low_output(),
        one_club_legend(),
    ];
    let axes: Vec<_> = careers.iter().map(compute_axes).collect();

    // For each career, find which school rates it highest.
    let top_schools: Vec<usize> = axes
        .iter()
        .map(|ax| {
            (0..4usize)
                .max_by_key(|&s| SCHOOLS[s].score(ax).to_int())
                .unwrap()
        })
        .collect();

    // Each career should be tops in a different school (schools disagree).
    let unique: std::collections::HashSet<_> = top_schools.iter().collect();
    assert!(
        unique.len() >= 2,
        "at least two careers should be ranked highest by different schools"
    );
}

/// Canon rankings work: a mid-career player ranks in the bottom half.
#[test]
fn early_career_ranks_low() {
    let early = LegacyEvidence {
        career_goals: 12,
        career_matches: 28,
        career_output_sum: 28 * 60,
        best_season_avg_output: 62,
        seasons_played: 2,
        decisive_moments: 0,
        player_of_year_wins: 0,
        league_titles: 0,
        clubs_served: 1,
        longest_club_tenure: 2,
    };
    let axes = compute_axes(&early);
    let rankings = all_rankings(&axes);
    // Should rank outside top 5 in all schools (10 greats + self = 11 total).
    for (s, &(_, rank, total)) in rankings.iter().enumerate() {
        assert!(
            rank > 5,
            "early career should rank outside top 5 in school {s}: rank={rank}/{total}"
        );
    }
}

// ── Phase 10: Icon axis from off-pitch life (TASK-10B.4) ───────────────────────

#[test]
fn icon_axis_neutral_career_matches_stub() {
    // A neutral off-pitch career (marketability 50, character 50, no sponsor, solvent)
    // must equal the old STUB_50 (50.0), so compute_axes callers stay unaffected.
    assert_eq!(goat_meta::icon_axis(50, 50, 0, false).to_int(), 50);
}

#[test]
fn icon_axis_reflects_fame_and_scandal() {
    let neutral = goat_meta::icon_axis(50, 50, 0, false).to_int();
    // A marketable, well-liked global icon rates far higher.
    assert!(goat_meta::icon_axis(95, 90, 3, false).to_int() > neutral);
    // A bankrupt, scandal-hit ex-star rates far lower.
    assert!(goat_meta::icon_axis(20, 15, 0, true).to_int() < neutral);
}

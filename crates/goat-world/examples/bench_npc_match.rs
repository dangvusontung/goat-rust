//! Native calibration of the versioned autonomous NPC match kernel.
use goat_core::tactical::TacticalProfile;
use goat_rng::GoatRng;
use goat_world::npc_match::{simulate, Candidate, MatchRules};
use std::time::Instant;
fn squad(offset: u32, rating: u8, energy: u8, keeping: u8) -> Vec<Candidate> {
    (0..22)
        .map(|i| Candidate {
            player: offset + i,
            position: (i % 3) as u8,
            keeper: i >= 20,
            rating,
            keeping,
            energy,
            stamina: 70,
            aggression: 50,
            returning: false,
            banned: false,
        })
        .collect()
}
fn main() {
    let begin = Instant::now();
    let n = 20_000u32;
    let profiles = [
        TacticalProfile::derive(70, 0, 42),
        TacticalProfile::derive(70, 1, 42),
    ];
    for (label, rating, energy, keeping, cards) in [
        ("equal", 70, 90, 70, true),
        ("weak_away", 40, 90, 40, false),
        ("fatigued_away", 70, 35, 70, false),
        ("poor_keeper_away", 70, 90, 1, false),
        ("control_no_cards", 70, 90, 70, false),
    ] {
        let home = squad(0, 70, 90, 70);
        let away = squad(100, rating, energy, keeping);
        let rules = if cards {
            MatchRules::default()
        } else {
            MatchRules {
                yellow_per_1000: 0,
                red_per_1000: 0,
            }
        };
        let mut goals = [0u64; 2];
        let mut wins = [0u32; 3];
        let mut yellows = 0u32;
        let mut dismissals = 0u32;
        let mut direct_reds = 0u32;
        let mut subs = 0usize;
        let mut goalless = 0u32;
        for seed in 0..n {
            let result = simulate(
                [&home, &away],
                profiles,
                rules,
                &mut GoatRng::new(seed as u64),
            );
            for (side, total) in goals.iter_mut().enumerate() {
                *total += result.goals[side] as u64;
            }
            wins[if result.goals[0] > result.goals[1] {
                0
            } else if result.goals[0] == result.goals[1] {
                1
            } else {
                2
            }] += 1;
            goalless += u32::from(result.goals == [0, 0]);
            yellows += result.cards.iter().filter(|c| c.kind == 0).count() as u32;
            dismissals += result.cards.iter().filter(|c| c.kind > 0).count() as u32;
            direct_reds += result.cards.iter().filter(|c| c.kind == 2).count() as u32;
            subs += result.substitutions.iter().map(Vec::len).sum::<usize>();
        }
        println!("{label} matches={n} goals_home={} goals_away={} home_draw_away={wins:?} yellows={yellows} dismissals={dismissals} direct_reds={direct_reds} substitutions={subs} goalless={goalless}",goals[0],goals[1]);
    }
    println!("total_us={}", begin.elapsed().as_micros());
}

//! Native release cache comparison; not a mobile or full-match benchmark.
use goat_core::{
    history::NpcMatchLoad,
    state::{NpcMatchCredit, OrbitMatchRecord},
};
use goat_world::{orbit::rebuild_population_scheduled, session::SimulationSession};
use std::{hint::black_box, time::Instant};
fn main() {
    let seed = 42;
    let season = 5;
    let dated = std::env::args().any(|a| a == "--dated-calendar");
    let c = goat_core::chronology::Chronology::new(2023);
    let week = if dated {
        c.frame(season).start_day / 7 + 1
    } else {
        (season - 1) * 52 + 8
    };
    let mut session = SimulationSession::new();
    let mut records = Vec::new();
    let mut loads = Vec::new();
    for round in 0..3 {
        if round > 0 {
            records.push(OrbitMatchRecord {
                season,
                round: round - 1,
                div: 0,
                credits: vec![NpcMatchCredit {
                    pop_idx: 0,
                    goals: 1,
                    assists: 0,
                    result: 1,
                }],
            });
            loads.push(NpcMatchLoad {
                competition_id: 1,
                pop_idx: 0,
                fixture_id: goat_world::workload::league_fixture_id(
                    seed,
                    season,
                    (round - 1) as usize,
                    0,
                ),
                epoch_day: if dated {
                    goat_world::calendar::dated_fixture_day(c, season, (round - 1) as usize)
                } else {
                    (season - 1) * 364 + 54 + (round - 1) * 3
                },
                minutes: 30,
            });
        }
        let start = Instant::now();
        let cached = if dated {
            session.population_dated(seed, 2023, season, &records, &loads)
        } else {
            session.population(seed, season, &records, &loads)
        };
        let indices = cached.lineup_indices(0, week + round, 16);
        let sum: u32 = indices
            .iter()
            .map(|&i| cached.current_ovr(i, week + round) as u32)
            .sum();
        let cached_ms = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        let fresh = if dated {
            goat_world::orbit::rebuild_population_dated(seed, 2023, season, &records, &loads)
        } else {
            rebuild_population_scheduled(seed, season, &records, &loads)
        };
        let fresh_indices = fresh.lineup_indices(0, week + round, 16);
        let fresh_sum: u32 = fresh_indices
            .iter()
            .map(|&i| fresh.current_ovr(i, week + round) as u32)
            .sum();
        assert_eq!(indices, fresh_indices);
        assert_eq!(sum, fresh_sum);
        assert_eq!(cached.career_fingerprint(), fresh.career_fingerprint());
        black_box(sum);
        println!("season={season} round={round} players={} retained_ms={cached_ms:.3} fresh_ms={:.3} sum={sum}", cached.len(), start.elapsed().as_secs_f64() * 1000.0);
    }
    println!("rebuilds={}", session.rebuild_count());
}

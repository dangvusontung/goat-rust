//! Compare both versioned models using the same workload, in release mode.
use goat_rng::GoatRng;
use goat_world::{population, sim_team_match, sim_team_match_shared, world::WorldGenesis};
use std::{hint::black_box, time::Instant};

fn main() {
    for (label, resolver) in [
        (
            "legacy",
            sim_team_match as fn(u8, u8, &mut GoatRng) -> (u32, u32),
        ),
        (
            "shared",
            sim_team_match_shared as fn(u8, u8, &mut GoatRng) -> (u32, u32),
        ),
    ] {
        let start = Instant::now();
        let mut totals = (0, 0);
        let mut rng = GoatRng::new(42);
        for _ in 0..100_000 {
            let (a, b) = resolver(75, 75, &mut rng);
            totals.0 += a;
            totals.1 += b;
        }
        println!(
            "{label}: 100000 matches {:?}, goals={totals:?}, seed42={:?}",
            start.elapsed(),
            resolver(75, 75, &mut GoatRng::new(42))
        );
    }
    let world = WorldGenesis::generate(42);
    for shared in [false, true] {
        let pop = if shared {
            population::genesis_shared(42, &world)
        } else {
            population::genesis(42, &world)
        };
        for pass in 0..2 {
            let start = Instant::now();
            let sum: u64 = (0..pop.len()).map(|i| pop.current_ovr(i, 52) as u64).sum();
            black_box(sum);
            println!(
                "shared={shared} pass={pass}, {} NPC ratings {:?}, sum={sum}",
                pop.len(),
                start.elapsed()
            );
        }
    }
}

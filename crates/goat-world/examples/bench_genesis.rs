//! Genesis + replay timing benchmark (Design round 2, Doc A §A2.5/§A3.1).
//!
//! Run with `cargo run -p goat-world --release --example bench_genesis`. Not part of the
//! default test suite — a manual, occasionally-run benchmark, per the doc's requirement
//! for a *real* measurement (not just an estimate) before committing to A3's replay
//! architecture.

use goat_world::promotion::ReplayCache;
use goat_world::world::WorldGenesis;
use goat_world::{history, population};
use std::time::Instant;

fn main() {
    let seed = 42u64;

    let t0 = Instant::now();
    let world = WorldGenesis::generate(seed);
    let genesis_world = t0.elapsed();

    let t1 = Instant::now();
    let pop = population::genesis(seed, &world);
    let genesis_pop = t1.elapsed();

    let t2 = Instant::now();
    let hist = history::backfill_history(seed, 30, &world);
    let genesis_history = t2.elapsed();

    println!(
        "world genesis:      {genesis_world:?}  ({} clubs, {} nations, {} leagues)",
        world.clubs.len(),
        world.nations.len(),
        world.leagues.len()
    );
    println!(
        "population genesis: {genesis_pop:?}  ({} players)",
        pop.len()
    );
    println!(
        "history backfill:   {genesis_history:?}  ({} seasons, {} greats)",
        hist.seasons.len(),
        hist.greats.len()
    );
    println!(
        "TOTAL genesis:       {:?}",
        genesis_world + genesis_pop + genesis_history
    );

    // A3.1's flagged concern: per-season replay cost as a career gets long. Time
    // advancing the promotion/relegation replay cache one season at a time.
    let mut cache = ReplayCache::new(&world, seed);
    let n_seasons = 20u32;
    let t3 = Instant::now();
    for _ in 0..n_seasons {
        cache.advance_one_season(&world);
    }
    let replay_total = t3.elapsed();
    println!(
        "\n{n_seasons} seasons of promotion/relegation replay: {replay_total:?}  (avg {:?}/season)",
        replay_total / n_seasons
    );
}

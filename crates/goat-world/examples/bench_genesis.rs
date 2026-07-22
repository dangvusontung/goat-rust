//! Genesis + replay timing benchmark (Design round 2, Doc A §A2.5/§A3.1).
//!
//! Run with `cargo run -p goat-world --release --example bench_genesis`. Not part of the
//! default test suite — a manual, occasionally-run benchmark, per the doc's requirement
//! for a *real* measurement (not just an estimate) before committing to A3's replay
//! architecture.

use goat_world::batch_tick::batch_tick_season;
use goat_world::national_tournament::simulate_national_tournament;
use goat_world::promotion::ReplayCache;
use goat_world::world::WorldGenesis;
use goat_world::{continental, history, population, ContinentalTier};
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

    // TASK-DESIGN-round4-competitions-slice2-3 §3.2/DoD#6: a *real* measurement of the
    // continental qualification computation — full previous-season Tier-1 table data
    // across all 20 nations, computed the same way a real season boundary would (via
    // `batch_tick_season`, the existing "season boundary" machinery), not estimated.
    let mut qual_pop = population::genesis(seed, &world);
    let membership = world.static_league_clubs();
    let t4 = Instant::now();
    let (_results, tier1_tables) =
        batch_tick_season(&mut qual_pop, &world, &membership, seed, 1, 52);
    let season_boundary_tables = t4.elapsed();

    let t5 = Instant::now();
    for tier in ContinentalTier::ALL {
        let qualified = continental::qualify_clubs(&world, &tier1_tables, tier);
        assert_eq!(qualified.len(), tier.group_stage_size());
    }
    let qualification_total = t5.elapsed();
    println!(
        "\ncontinental qualification: {season_boundary_tables:?} to build all 20 nations' \
         Tier-1 tables (one season boundary) + {qualification_total:?} to qualify all 3 tiers \
         from them"
    );

    // TASK-DESIGN-round4-competitions-slice4-national-teams §DoD#6: a *real* measurement
    // of the full national-team tournament-cycle cost (qualifying: 4 groups of 5, 40
    // matches total, each resolving a per-nation eligible-population scan; then the
    // tournament proper: group stage + knockout), not estimated. Uses the same
    // background population `qual_pop` built above rather than genesis-ing a second one.
    const N_CYCLES: u32 = 10;
    let t6 = Instant::now();
    let mut last_champion = 0;
    for cycle in 0..N_CYCLES {
        let tournament_season = 1 + cycle * 4; // one World Cup cycle per iteration
        let result = simulate_national_tournament(&world, &qual_pop, seed, tournament_season);
        last_champion = result.champion;
    }
    let national_tournament_total = t6.elapsed();
    println!(
        "\nnational tournament cycle (qualifying + tournament proper): {:?} total for \
         {N_CYCLES} cycles (avg {:?}/cycle, last champion nation id {last_champion})",
        national_tournament_total,
        national_tournament_total / N_CYCLES
    );
}

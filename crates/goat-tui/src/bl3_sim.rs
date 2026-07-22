//! BL3 (Design round 5, club economy) end-to-end playability check — `cargo run -p goat-tui
//! --release --bin bl3-sim [-- --seed N --seasons N]`. Not part of the default test suite;
//! a manual verification harness (`career-sim`'s own idiom, a sibling `[[bin]]` in this same
//! crate) proving the real production `ReplayCache::advance_one_season` season-tick actually
//! moves club budgets, executes AI transfers, and fires/hires managers, at the real
//! ~1,200-club scale — Design round 5, Slice 9's own Definition of Done #7 requirement.
//!
//! Not wired into `goat-tui`'s own interactive main loop: no existing code path threads the
//! background-club economy into live gameplay yet (`main.rs`'s world screen re-derives a
//! static, budget-free view of the background population on every render — see its own doc
//! comment) — lockstep integration with the orbit calendar's day-level flashpoints is this
//! slice's own "Out of scope" section, deferred to a future round. This binary exercises the
//! same real `goat_world::promotion::ReplayCache` the interactive game will eventually call.

use goat_world::promotion::ReplayCache;
use goat_world::world::WorldGenesis;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args
        .iter()
        .position(|a| a == "--seed")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(2026);
    let seasons: u32 = args
        .iter()
        .position(|a| a == "--seasons")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);

    let mut world = WorldGenesis::generate(seed);
    let mut cache = ReplayCache::new(&world, seed);

    println!(
        "BL3 season-tick playability check — seed {seed}, {seasons} season(s), {} clubs\n",
        world.clubs.len()
    );

    let mut total_manager_changes = 0usize;
    let mut total_transfers = 0usize;
    let mut total_contested = 0usize;

    for season in 1..=seasons {
        let budgets_before = cache.club_budgets().to_vec();
        let managers_before = cache.managers().club_manager.clone();

        cache.advance_one_season(&mut world);

        let budgets_after = cache.club_budgets();
        let system_total_before: i64 = budgets_before.iter().sum();
        let system_total_after: i64 = budgets_after.iter().sum();
        let clubs_with_changed_budget = budgets_before
            .iter()
            .zip(budgets_after.iter())
            .filter(|(a, b)| a != b)
            .count();

        let transfers = cache.last_transfers();
        let contested = transfers
            .iter()
            .filter(|&&(_, _, _, fee, valuation, _)| fee > valuation)
            .count();
        let manager_changes = managers_before
            .iter()
            .zip(cache.managers().club_manager.iter())
            .filter(|(a, b)| a != b)
            .count();

        total_transfers += transfers.len();
        total_contested += contested;
        total_manager_changes += manager_changes;

        println!(
            "season {season}: system budget {system_total_before} -> {system_total_after} \
             (delta {}), {clubs_with_changed_budget}/{} clubs' budgets changed, \
             {} transfers ({contested} contested/above valuation), \
             {manager_changes} manager(s) fired/replaced",
            system_total_after - system_total_before,
            budgets_after.len(),
            transfers.len(),
        );
        println!(
            "  club 0: budget {} -> {}, academy_boost {}, manager tenure_start_season {}",
            budgets_before[0],
            budgets_after[0],
            cache.academy_boosts()[0],
            cache.managers().managers[cache.managers().club_manager[0] as usize]
                .tenure_start_season,
        );
    }

    println!(
        "\nTOTAL across {seasons} season(s): {total_transfers} transfers \
         ({total_contested} contested/above valuation), {total_manager_changes} manager \
         fire/replace event(s)."
    );
    assert!(
        total_transfers > 0,
        "BL3 playability check failed: no AI transfers executed across {seasons} season(s)"
    );
    println!("\nBL3 playability check: PASS (transfers executed; see per-season detail above).");
}

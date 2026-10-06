//! Cloud CPU baseline; release only. Does not certify phone performance.
use goat_fixed::Fixed;
use goat_world::{exposure::NpcExposure, population, promotion::ReplayCache, world::WorldGenesis};
use std::{hint::black_box, time::Instant};

fn main() {
    let world = WorldGenesis::generate(42);
    for dated in [false, true] {
        let mut pop = if dated {
            population::genesis_scheduled(42, &world)
        } else {
            population::genesis_lived(42, &world)
        };
        for idx in (0..pop.len()).step_by(10) {
            pop.record_exposure(idx, NpcExposure::balanced(26, Fixed::raw(1600)));
        }
        for week in [52, 53, 20 * 52] {
            for pass in 0..2 {
                let start = Instant::now();
                let sum: u64 = (0..pop.len())
                    .map(|i| pop.current_ovr(i, week) as u64)
                    .sum();
                black_box(sum);
                println!("scheduled={dated} week={week} pass={pass} players={} segments={} ms={:.3} sum={sum}",
                    pop.len(), pop.exposure_segment_count(), start.elapsed().as_secs_f64() * 1000.0);
            }
        }
    }
    let mut world = WorldGenesis::generate(42);
    let mut cache = ReplayCache::new_scheduled(&world, 42);
    let start = Instant::now();
    for season in 1..=20 {
        let step = Instant::now();
        cache.advance_one_season(&mut world);
        println!(
            "replay season={season} players={} segments={} ms={:.3}",
            cache.pop().len(),
            cache.pop().exposure_segment_count(),
            step.elapsed().as_secs_f64() * 1000.0
        );
    }
    println!(
        "replay total20 ms={:.3}",
        start.elapsed().as_secs_f64() * 1000.0
    );
}

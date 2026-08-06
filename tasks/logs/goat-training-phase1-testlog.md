# goat-training Phase 1 — test/golden log (captured 2026-08-06T21:14:28+07:00)

## cargo test -p goat-training (full output)
```
     Running unittests src/lib.rs (target/debug/deps/goat_training-443bc54063b43d12)
running 10 tests
test growth::tests::archetype_curves_have_distinct_shapes ... ok
test growth::tests::archetype_lookup_matches_goat_core_table ... ok
test growth::tests::energy_stays_in_bounds_through_any_sequence ... ok
test growth::tests::goat_core_clamp_idiom_holds ... ok
test growth::tests::growth_is_zero_at_the_ceiling ... ok
test growth::tests::growth_monotonic_in_intensity ... ok
test growth::tests::growth_never_exceeds_headroom ... ok
test growth::tests::low_energy_reduces_growth ... ok
test growth::tests::mental_appreciates_with_age ... ok
test growth::tests::technical_outgains_physical_when_young ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/golden_season.rs (target/debug/deps/golden_season-030598630c1444cd)
running 3 tests
test golden_training_season ... ok
test determinism_byte_identical ... ok
test engine_tick_matches_direct_drive ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests goat_training
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Golden values frozen in golden_training_season (script: 266 days, Mon–Fri Moderate ShortPassing, Sat match, Sun rest, Hard stretch days 140–153)
```
final ShortPassing raw: 69184 (60.000 -> 69.184)
final energy raw: 100000 (full recovery by season end)
final age_days: 6471
soft flashpoint days: [129, 148, 149, 150, 151, 153, 154, 155, 156, 157, 158, 164, 165, 172]
breakthrough days: [129]  payload: "Breakthrough! Short Pass reaches 65."
first overtrained payload (day 148): "Overtrained — gains suffer at 16% energy."
```

## cargo fmt --check
```
(clean — no output)
```

## cargo clippy --workspace --all-targets -- -D warnings (tail)
```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
```

## Determinism gate (grep — must be empty)
```
$ grep -rn "now()\|SystemTime\|Instant\|chrono\|f32\|f64" crates/goat-training/src
(no matches)
```

## cargo test --workspace
```
suites ok: 39
only failure: goat-tui --test smoke_stdin — 10 pre-existing failures on clean HEAD (baseline BASELINE_UNCHANGED)
```

# TASK 01 — goat-core: domain model + derive_attrs + role_rating + first reduce() golden test

Paste this into Claude Code from the repo root. Prereq: `cargo test` is green
(goat-rng 9 passed, goat-fixed 6 passed) and CLAUDE.md is in the repo root.

---

Read CLAUDE.md, then docs/BecomeTheGOAT-RustCore-TechDoc.md (especially the module
map and build order), then the public APIs of crates/goat-rng and crates/goat-fixed.
Do not write any code until you've read all three.

Then build the `goat-core` crate, milestone 1, in these steps — pause for my review
after each step:

## Step 1 — Crate skeleton + domain model
- New workspace member `crates/goat-core` (`#![forbid(unsafe_code)]`, no deps beyond
  goat-rng and goat-fixed).
- The ~30 sub-attributes from the design bible §5.1, grouped under the 6 families
  (Pace, Shooting, Passing, Dribbling, Defending, Physical). Goalkeeping family is
  parked — leave a documented placeholder, no implementation.
- Each attribute carries: current value, potential, and an age-curve archetype
  (Physical / Technical / Mental per bible §5.1). Scale 1–99, fixed-point.
- Storage is struct-of-arrays: attribute columns indexed by player id. A single-player
  view type may exist for ergonomics, but the storage is columnar.
- The ~14 outfield roles as an enum + a role-weights table (Key/Important/baseline)
  and the 4 familiarity tiers — all placeholder numbers as named constants in one
  `tuning` module.

## Step 2 — derive_attrs + role_rating
- `derive_attrs`: compute the 6 family display values from the 30 sub-attributes
  (derived layer, never stored as truth).
- `role_rating(player, role) = Σ(weight × attr) × familiarity` per bible §5.2,
  entirely in goat-fixed math.
- OVR = best role rating. No context-free overall anywhere.
- Property tests: monotonic in key attributes; familiarity tier ordering preserved;
  all outputs in valid range.

## Step 3 — reduce() + golden-seed test (the project's test #1)
- The core state + `reduce(state, intent, rng) -> state` entry point as specified in
  the tech doc. Implement only what milestone 1 needs (e.g. a no-op/advance intent and
  one attribute-affecting intent), but lock the function signature and state shape.
- Write the first golden-seed test: fixed seed, fixed intent sequence, assert exact
  resulting state values. This test's expected values become FROZEN once I approve.

## Rules reminders (from CLAUDE.md — these override convenience)
- No floats in sim. No std HashMap iteration feeding results. RNG only via injection.
- Do not touch goat-rng or goat-fixed. If their API seems insufficient, stop and ask.
- All pre-existing golden tests must stay green with their original expected values.
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test` clean before each pause.

At each pause: show me the file tree of what you added, the key type definitions,
and the test output.

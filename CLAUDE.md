# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

Single-career football life-sim. Headless deterministic simulation core in Rust with a
**playable text-based renderer** (`goat-tui`) — the first proof of the swappable-renderer
architecture. Mobile (Flutter) renderer comes later; the core never knows the difference.
100% offline at runtime.

## Commands

```bash
# Full quality gate (fmt → clippy → tests → career-sim sanity)
./scripts/test.sh

# Individual steps
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Run specific crate tests
cargo test -p goat-core
cargo test -p goat-match -- golden        # run only tests matching "golden"

# Run the TUI
cargo run -p goat-tui

# Headless simulation harnesses (in goat-tui's career-sim binary)
cargo run -p goat-tui --bin career-sim -- --seed 42           # single career, seed 42
cargo run -p goat-tui --bin career-sim -- --scan              # seed scanner (1-50)
cargo run -p goat-tui --bin career-sim -- --match-sim 100 7   # 100 matches, player seed 7
cargo run -p goat-tui --bin career-sim -- --world-sim 20 20   # 20 seeds × 20 seasons

# Convenience wrappers around the harnesses
./scripts/match-sim.sh [N_matches] [player_seed]
./scripts/world-sim.sh [batches] [seasons] [pc_goals] [pc_titles]
```

## Source of truth (read in this order before any non-trivial change)

1. `docs/DESIGN_BIBLE.md` + `docs/DESIGN_BIBLE_APP_A.md` — game design intent.
   Never contradict them. `MAIN.md` is a merged snapshot of DESIGN_BIBLE + MATCH + CALENDAR.
2. `docs/CALENDAR.md` — the Calendar Simulator spec: day-tick loop, flashpoint
   arbitration, conflict resolution, season boundary pipeline, RNG stream forking.
   Authoritative for `goat-calendar` and any crate that registers as a subsystem.
3. `docs/MATCH.md` — beat engine design, headspace, discipline rules.
4. `docs/TRAINING.md` — training subsystem spec (the `goat-training` crate target).
5. `docs/TRAITS.md` — traits & mastery appendix for `goat-traits`.
6. `docs/CLIENT-IMPL.md` — `goat-bridge` public API reference for renderer authors.
7. `docs/BEATS-AUTHORING-GUIDE.md` — how to author beats in `beats.json`.
8. `ROADMAP.md` — the phase plan. Work happens one phase at a time, in order.
9. The existing crates (`crates/goat-rng`, `crates/goat-fixed`) — their public APIs and
   golden values are **frozen**. Read them; never guess their signatures.

If this file ever disagrees with the docs above, those docs win —
flag the discrepancy to the user instead of silently picking one.

## Non-negotiable rules

These are load-bearing. Violating any of them is a bug even if all tests pass.

- **Determinism is sacred.** Same seed + same inputs = same universe, bit-for-bit,
  on every platform.
  - All randomness flows through the injected `goat-rng` source. Never `rand::thread_rng()`,
    never `HashMap`/`HashSet` iteration order feeding into simulation results
    (use `BTreeMap`/`Vec` or sort first), never wall-clock/time-of-day reads inside core.
  - **No `f32`/`f64` anywhere in simulation state or logic.** All sim math goes through
    `goat-fixed`. Floats are allowed only in renderer-side display formatting that never
    feeds back into core state or intents.
- **`#![forbid(unsafe_code)]`** in every crate below the FFI bridge.
- **Headless core.** Core crates have no I/O, no logging side effects in logic paths,
  no UI types, no network, no `println!`. The core exposes pure state + reduce/step
  functions; renderers send intents and read state. **All player-facing text lives in
  core data as template + slot** (so any future renderer gets it for free); the TUI
  only formats and prints.
- **The renderer is dumb.** `goat-tui` contains zero simulation logic, zero rules, zero
  randomness. If you find yourself computing a game outcome in the TUI, stop — that
  code belongs in core. The litmus test: deleting `goat-tui` must lose no game logic.
- **Frozen golden values.** Existing golden-seed tests must never be "fixed" by updating
  expected values. If a change breaks a golden test, the change is wrong. New behavior
  gets *new* golden tests. (Exception: a phase task may explicitly say a value is not
  yet frozen pending user approval.)
- **Struct-of-arrays for populations.** World players are columnar data (parallel `Vec`s),
  not heap objects per player. Player identity is an index/id into columns.
- **Talent ceiling is law.** Nothing may push a current attribute above its potential.
- **LLMs at authoring time only.** Runtime text is template + slot from baked data.
  Never add a runtime network or model dependency.
- **Tiny saves.** Persist results, records, and path-dependent state only. Fixtures,
  background players, and history details are recomputed from
  `seed (+ season, league, birth data, date)`. Before persisting any new field, ask:
  can this be derived? If yes, derive it.

## Workspace layout

```
goat/
  Cargo.toml            # workspace
  ROADMAP.md            # phase plan + playable gates
  beats.json            # beat library (runtime-loaded authoring data)
  beats_test.json       # beats used by tests
  tasks/                # one paste-ready task file per phase
  scripts/
    test.sh             # full quality gate (fmt + clippy + tests + career-sim)
    match-sim.sh        # beat-engine match simulation harness
    world-sim.sh        # whole-world simulation harness
  crates/
    goat-rng/           # seeded, injectable RNG — FROZEN API + golden values
    goat-fixed/         # fixed-point math — FROZEN API + golden values
    goat-core/          # domain model, player, roles, week loop, reduce()
                        #   Key public surface: WorldState, Intent, reduce(), PlayerStore
    goat-match/         # beat engine, headspace, discipline
                        #   Key public surface: BeatLibrary, MatchSetup, start_match, advance_beat
    goat-world/         # genesis, fixtures, tiered sim, transfers, rival
    goat-meta/          # legacy, pantheon, reputation, pundits, contracts, life, money
    goat-save/          # tiny-save serialization — persist path-dependent state only
    goat-traits/        # traits & mastery system (MasteryTier, TraitId, PlayerTraits)
    goat-calendar/      # time orchestrator: day-tick loop, flashpoint arbitration,
                        #   conflict resolution, season boundary pipeline, RNG forking
    goat-bridge/        # FFI bridge for Flutter via flutter_rust_bridge 2.9.0
                        #   (static lib; see docs/CLIENT-IMPL.md)
    goat-tui/           # text renderer BINARY — the playable game. No sim logic.
                        #   Two binaries: goat-tui (interactive) and career-sim (headless harness)
  docs/
    DESIGN_BIBLE.md     # game design intent — never contradict
    DESIGN_BIBLE_APP_A.md # traits & mastery appendix
    CALENDAR.md         # calendar engine spec — authoritative for goat-calendar
    MATCH.md            # match engine design
    TRAINING.md         # training subsystem spec
    TRAITS.md           # traits & mastery appendix (same content as DESIGN_BIBLE_APP_A)
    CLIENT-IMPL.md      # goat-bridge API for renderer authors
    BEATS-AUTHORING-GUIDE.md  # how to write beats in beats.json
    MAIN.md             # merged snapshot of DESIGN_BIBLE + MATCH + CALENDAR
```

Note: `goat-training` (training routines, intensity, growth, energy) is a planned crate
per the roadmap but is not yet in the workspace — training logic currently lives inline
in `goat-core/src/week.rs`. See ROADMAP.md Phase 3.5 for the wiring plan.

Crate boundaries may be refined by the docs — they win. But the core/renderer split
is absolute.

## Architecture: how the pieces connect

The simulation follows a strict unidirectional data flow:

```
Renderer (goat-tui)
  ↓ sends Intent
goat-core::reduce(WorldState, Intent, &mut RngSource) → WorldState
  ↑ reads state (no mutation)
```

`goat-core::state::reduce()` is the **only** mutation point. It dispatches to:
- `week.rs` — `advance_week()` for training/energy/events
- `calendar_loop.rs` — bridges to `goat-calendar`
- `generation.rs` — player creation pipeline

`goat-match` is invoked by `goat-tui` directly (not by `reduce()`); the renderer drives
beat-by-beat match play and reports the `MatchResult` back via an Intent.

`goat-save` persists only the fields that cannot be rederived from the world seed.
The save format is v4; see `goat-save/src/save.rs` for the `SaveData` struct.

Beat authoring happens via `beats.json` (loaded at runtime by `goat-match`). To add
beats: edit `beats.json` following `docs/BEATS-AUTHORING-GUIDE.md`, then run
`cargo test -p goat-match` to validate the library loads.

## Workflow: phases and gates

- Work proceeds **one phase at a time** per `ROADMAP.md`, using the matching `tasks/TASK-NN` file.
- Every phase ends with a **playable gate**: a thing the user can do in `goat-tui`
  (`cargo run -p goat-tui`). A phase is not done until its gate is playable.
- Within a phase, follow the task file's steps and **pause for user review** where marked.
- Do not start a later phase early. Do not gold-plate beyond the phase's scope —
  out-of-scope items are listed per task and deferred on purpose.
- When a design question is genuinely open (bible marks it deferred/parked), **ask —
  don't decide**. Parked: goalkeeper career, final tuning numbers, art/graphical UX,
  beat-library volume beyond the starter set, deeper relationship web.
- Never refactor `goat-rng` / `goat-fixed` as a side effect of other work.

## Coding conventions

- Don't bump toolchain or add dependencies without asking — the dependency budget is
  deliberately near-zero. `goat-tui` v1 is plain stdin/stdout (no ratatui/crossterm
  unless the user approves later). `flutter_rust_bridge = "=2.9.0"` is pinned in
  the workspace — do not bump it without asking.
- `cargo fmt` + `cargo clippy -D warnings` clean before declaring any step done.
- Public API gets doc comments explaining *why* (the design rule it implements).
- Naming mirrors the bible glossary: beat, headspace, familiarity, orbit, lazy-promote,
  batch-tick, genesis, pantheon, school. Don't invent synonyms.
- All tunable numbers (weights, multipliers, energy costs, probabilities) are named
  constants in each crate's `tuning` module — never inline magic numbers. Bible numbers
  are placeholders illustrating shape; implement them as the starting values.

## Test discipline

- **Golden-seed tests first.** Every deterministic pipeline gets a test asserting exact
  outputs for fixed seeds. These are the project's spine.
- Property tests where cheap: `current <= potential` always; `role_rating` monotonic in
  key attributes; familiarity ordering preserved; attributes always in 1–99.
- Long-horizon sanity: headless fast-forward of full careers/seasons must not panic and
  must keep invariants. Use `cargo run -p goat-tui --bin career-sim -- --seed N`.
- The TUI gets at least a smoke test: scripted stdin → expected stdout fragments,
  run against a fixed seed.
- Tests must not depend on each other or on execution order.

## Definition of done (any task)

1. `cargo test` green across the workspace — including all pre-existing golden tests
   with their original expected values.
2. `cargo fmt --check` and `cargo clippy -D warnings` clean.
3. New deterministic behavior covered by at least one golden-seed test.
4. The phase's playable gate works: state the exact `cargo run -p goat-tui` flow to try.
5. No new dependencies, no floats in sim, no unsafe, no I/O in core, no logic in TUI.
6. Short summary: what changed, which bible/tech-doc section it implements.

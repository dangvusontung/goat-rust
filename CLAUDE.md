# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

Single-career football life-sim. Headless deterministic simulation core in Rust. The real,
shipping renderer is **Flutter mobile**, built against `goat-bridge` (FFI via
flutter_rust_bridge; see `docs/CLIENT-IMPL.md` + `docs/FLUTTER-APP-GUIDE.md`). `goat-tui`
(terminal) and `goat-web` (WASM browser demo) are **internal dev/testing harnesses** —
they exercise the same core through the same intent/state contract a real renderer would,
which is how they double as manual playability checks during development, but they are not
the target product. The core never knows the difference between any of the three. 100%
offline at runtime.

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

# Build + serve the goat-web browser demo (from repo root)
wasm-pack build crates/goat-web --target web --out-dir ../../web/pkg
cd web && python3 -m http.server 8000   # wasm ES modules don't load over file://
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
- **The renderer is dumb.** This applies to every renderer — the shipping Flutter app
  (via `goat-bridge`) and the `goat-tui`/`goat-web` dev harnesses alike — zero simulation
  logic, zero rules, zero randomness. If you find yourself computing a game outcome in a
  renderer, stop — that code belongs in core. The litmus test: deleting any one of them
  must lose no game logic.
- **Golden values are NOT frozen** (changed 2026-08-30 — this rule previously said the
  opposite; earlier task files and code comments still reflect the old rule). A design
  change that legitimately moves a golden value is fine: update the expected value and
  leave a comment saying *what* changed it. Precedent for that style is already in
  `crates/goat-core/tests/golden_week.rs` (the BL7 durability re-freeze note).
  - What is still non-negotiable is **determinism** (above): the same seed on the same
    code must produce the same output, every run, every platform. Golden tests remain the
    tripwire for *accidental* drift — so when one fails, first work out whether your change
    should have moved it. Update it deliberately, never reflexively to get to green.
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
    goat-bridge/        # FFI bridge for the real Flutter app, via flutter_rust_bridge 2.9.0
                        #   (static lib; see docs/CLIENT-IMPL.md + docs/FLUTTER-APP-GUIDE.md).
                        #   This is the shipping renderer's integration point.
    goat-training/      # training routines/intensity/growth (Phase 3.5 target — in the
                        #   workspace but not yet wired into goat-core's week loop; see below)
    goat-tui/           # text renderer BINARY — dev/testing harness, not the shipping game.
                        #   No sim logic. Binaries: goat-tui (interactive, doubles as a manual
                        #   playability check), career-sim (headless harness), bl3-sim
                        #   (manual club-economy season-tick check, not in test.sh)
    goat-web/           # WASM renderer (wasm-bindgen) for the browser demo in web/ — another
                        #   dev/testing harness, not the shipping game. Talks to the core
                        #   crates directly (not through goat-bridge's FFI layer).
  web/                  # goat-web's browser front-end (index.html/main.js) + node smoke test
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

Note: `goat-training` (training routines, intensity, growth, energy) is a workspace member
with real logic, but nothing calls it yet — training still lives inline in
`goat-core/src/week.rs`, and neither `goat-tui` nor `goat-web` depend on the crate. See
ROADMAP.md Phase 3.5 for the wiring plan (dep-cycle fix, unify `PlayerStore` ownership,
week-vs-day-tick reconciliation) before touching this.

Crate boundaries may be refined by the docs — they win. But the core/renderer split
is absolute.

## Architecture: how the pieces connect

The simulation follows a strict unidirectional data flow:

```
Renderer (Flutter via goat-bridge — shipping; goat-tui / goat-web — dev harnesses)
  ↓ sends Intent
goat-core::reduce(WorldState, Intent, &mut RngSource) → WorldState
  ↑ reads state (no mutation)
```

`goat-core::state::reduce()` is the **only** mutation point. It dispatches to:
- `week.rs` — `advance_week()` for training/energy/events
- `calendar_loop.rs` — bridges to `goat-calendar`
- `generation.rs` — player creation pipeline

`goat-match` is invoked by the renderer directly (not by `reduce()`); the renderer drives
beat-by-beat match play and reports the `MatchResult` back via an Intent.

`goat-web` links `goat-core`/`goat-match`/`goat-world`/`goat-meta`/`goat-calendar` straight
into a `wasm-bindgen` cdylib and exposes a JSON-string session API to `web/main.js` — it does
not go through `goat-bridge`, which is the separate flutter_rust_bridge FFI surface the real
Flutter app builds against. New core functionality should stay reachable from the shipping
path (`goat-bridge`); `goat-tui`/`goat-web` don't need to track it in lockstep since they're
test harnesses, but keep them close enough to still be useful for manual verification — per
`docs/FLUTTER-APP-GUIDE.md` §0, `goat-bridge`'s surface already lags several design rounds
behind the core (see `tasks/TASK-BRIDGE-refresh.md`), which is the actual integration debt
to track, not a TUI/web gap.

`goat-save` persists only the fields that cannot be rederived from the world seed.
The save format is v4; see `goat-save/src/save.rs` for the `SaveData` struct.

Beat authoring happens via `beats.json` (loaded at runtime by `goat-match`). To add
beats: edit `beats.json` following `docs/BEATS-AUTHORING-GUIDE.md`, then run
`cargo test -p goat-match` to validate the library loads.

## Workflow: phases and gates

- Work proceeds **one phase at a time** per `ROADMAP.md`, using the matching `tasks/TASK-NN` file.
- Every phase ends with a **playable gate**: a thing the user can do in `goat-tui`
  (`cargo run -p goat-tui`), used as the dev-time proof the phase's core logic actually
  works end-to-end. A phase is not done until its gate is playable. This is a verification
  step, not a claim that `goat-tui` is the shipping game — see Project overview.
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

## Git hygiene

- `.gitignore`'s `/target/` only matches the workspace root. Anything under `spikes/`
  or other nested Cargo projects gets its own `target/` — verify `**/target/` (already
  present) actually covers it before adding a new spike, and run `git status` after
  the first build to confirm nothing under a nested `target/` is offered up to `git add`.
- Before staging, `git status` and eyeball the list for build output, IDE files, or
  anything that isn't source — don't `git add -A`/`git add .` blindly in a directory
  you haven't just run `cargo build` in.
- If a large or generated file ever does get committed, don't just delete it in a new
  commit — that keeps the blob in history. Flag it to the user; removing it for real
  needs `git rm --cached` (untrack going forward) and, separately, a history rewrite
  (`git filter-repo`) if the blob already bloated `.git` — the latter needs explicit
  user sign-off since it force-pushes and breaks other clones.

## Test discipline

- **Golden-seed tests first.** Every deterministic pipeline gets a test asserting exact
  outputs for fixed seeds. They catch accidental drift — but their values are not frozen
  (see Non-negotiable rules); a deliberate design change may update them, with a note.
- Property tests where cheap: `current <= potential` always; `role_rating` monotonic in
  key attributes; familiarity ordering preserved; attributes always in 1–99.
- Long-horizon sanity: headless fast-forward of full careers/seasons must not panic and
  must keep invariants. Use `cargo run -p goat-tui --bin career-sim -- --seed N`.
- The TUI gets at least a smoke test: scripted stdin → expected stdout fragments,
  run against a fixed seed.
- Tests must not depend on each other or on execution order.

## Definition of done (any task)

1. `cargo test` green across the workspace. Golden values may be updated when a change
   legitimately moves them — say which ones moved and why in the summary (#6).
2. `cargo fmt --check` and `cargo clippy -D warnings` clean.
3. New deterministic behavior covered by at least one golden-seed test.
4. The phase's playable gate works: state the exact `cargo run -p goat-tui` flow to try.
5. No new dependencies, no floats in sim, no unsafe, no I/O in core, no logic in TUI.
6. Short summary: what changed, which bible/tech-doc section it implements.

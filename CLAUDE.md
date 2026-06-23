# CLAUDE.md — Become the GOAT (Rust core + text renderer)

Single-career football life-sim. Headless deterministic simulation core in Rust with a
**playable text-based renderer** (`goat-tui`) — the first proof of the swappable-renderer
architecture. Mobile (Flutter) renderer comes later; the core never knows the difference.
100% offline at runtime.

## Source of truth (read in this order before any non-trivial change)

1. `docs/BecomeTheGOAT-RustCore-TechDoc.md` — the *mechanisms*: stack, RNG design,
   fixed-point, save format, module map, build order. **This doc wins all conflicts.**
2. `docs/BecomeTheGOAT-TechnicalDesign.md` — architecture rationale and trade-offs.
3. `docs/Become-the-GOAT-Design-Bible.md` — game design *intent*. Never contradict it.
4. `docs/CALENDAR.md` — the Calendar Simulator spec: day-tick loop, flashpoint
   arbitration, conflict resolution, season boundary pipeline, RNG stream forking.
   Authoritative for `goat-calendar` and any crate that registers as a subsystem.
5. `ROADMAP.md` — the phase plan. Work happens one phase at a time, in order.
6. The existing crates (`crates/goat-rng`, `crates/goat-fixed`) — their public APIs and
   golden values are **frozen**. Read them; never guess their signatures.

If this file ever disagrees with the tech doc or the calendar spec, those docs win —
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
  tasks/                # one paste-ready task file per phase
  crates/
    goat-rng/           # seeded, injectable RNG — FROZEN API + golden values
    goat-fixed/         # fixed-point math — FROZEN API + golden values
    goat-core/          # domain model, player, roles, week loop, reduce()
    goat-match/         # beat engine, headspace, discipline
    goat-world/         # genesis, fixtures, tiered sim, transfers, rival
    goat-meta/          # legacy, pantheon, reputation, pundits, contracts, life, money
    goat-save/          # tiny-save serialization (v4 format)
    goat-training/      # training routines, intensity, growth, energy — extracted from core
    goat-calendar/      # time orchestrator: day-tick loop, flashpoint arbitration,
                        #   conflict resolution, season boundary pipeline, RNG forking
    goat-bridge/        # FFI bridge for Flutter via flutter_rust_bridge 2.9.0
                        #   (static lib; see docs/CALENDAR.md appendix for task file)
    goat-tui/           # text renderer BINARY — the playable game. No sim logic.
  docs/
```

Crate boundaries may be refined by the tech doc's module map — tech doc wins. But the
core/renderer split is absolute.

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
  must keep invariants.
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

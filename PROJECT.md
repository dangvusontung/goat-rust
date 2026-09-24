# Become the GOAT — Project Context Document

> **Purpose:** This file is a self-contained briefing for working on this project in
> web-based AI coding environments (Kimi Code Web, Claude Code on the web, or any
> cloud agent session). Import/paste it as project knowledge or instructions. It
> summarizes the local `CLAUDE.md`, `ROADMAP.md`, and the docs — when you have repo
> access, those files remain the source of truth and win any conflict.

---

## 1. What this project is

**Become the GOAT** is a single-career football (soccer) life-sim. You play one
footballer from academy at 16 to retirement. The goal is not winning trophies but
**building a legacy case** that in-game pundits and "schools of greatness" argue
about forever.

- **Headless, deterministic simulation core in Rust**, 100% offline at runtime.
- **Swappable renderers:** the core never knows what renders it. The first renderer
  is a plain text TUI (`goat-tui`). A Flutter mobile renderer comes later via an FFI
  bridge (`goat-bridge`).
- Same seed + same inputs = same universe, **bit-for-bit, on every platform**.

## 2. Repository layout

```
Cargo.toml              # workspace (edition 2021, resolver 2)
CLAUDE.md               # agent instructions — source of truth for rules
ROADMAP.md              # 10-phase build plan with playable gates
tasks/                  # paste-ready task files, one per phase
scripts/                # test.sh, match-sim.sh, world-sim.sh
crates/
  goat-rng/             # seeded injectable RNG — FROZEN API + golden values
  goat-fixed/           # fixed-point math — FROZEN API + golden values
  goat-core/            # domain model: 30 attributes (SoA), roles, week loop, reduce()
  goat-match/           # beat engine, contests, headspace, discipline, starter beats
  goat-world/           # genesis, fixtures, tiered sim, history, transfers, rival
  goat-meta/            # legacy axes, pantheon schools, awards, reputation, pundits,
                        #   contracts, life & money
  goat-save/            # tiny-save serialization
  goat-calendar/        # day-tick time orchestrator, flashpoint arbitration, RNG forking
  goat-traits/          # traits & mastery
  goat-bridge/          # FFI bridge for Flutter (flutter_rust_bridge =2.9.0, pinned)
  goat-tui/             # text renderer BINARY — the playable game + dev harnesses
docs/
  DESIGN_BIBLE.md       # game design intent — never contradict it
  MAIN.md               # merged design docs (bible + match + calendar + training + traits)
  MATCH.md              # match deep-dive: beats, layout, output
  CALENDAR.md           # calendar simulator spec — authoritative for goat-calendar
  TRAINING.md           # training subsystem spec
  TRAITS.md             # traits & mastery spec
  PLAYER_RATING.md      # player rating model
  BEATS-AUTHORING-GUIDE.md / FLUTTER-APP-GUIDE.md / CLIENT-IMPL.md
  sim-analysis.md / sim-fixes.md   # simulation tuning analysis
```

## 3. Non-negotiable rules (load-bearing — violating them is a bug)

- **Determinism is sacred.** All randomness flows through the injected `goat-rng`
  source. Never `rand::thread_rng()`, never `HashMap`/`HashSet` iteration order
  feeding sim results (use `BTreeMap`/`Vec` or sort first), never wall-clock reads
  in core.
- **No `f32`/`f64` in simulation state or logic.** All sim math goes through
  `goat-fixed`. Floats only in renderer-side display formatting.
- **`#![forbid(unsafe_code)]`** in every crate below the FFI bridge.
- **Headless core.** No I/O, no logging side effects, no UI types, no network, no
  `println!` in core crates. Core exposes pure state + reduce/step functions.
  Player-facing text lives in core data as **template + slot**.
- **The renderer is dumb.** `goat-tui` contains zero sim logic, zero rules, zero
  randomness. Litmus test: deleting `goat-tui` must lose no game logic.
- **Frozen golden values.** Never "fix" a golden-seed test by updating expected
  values — if it breaks, the change is wrong. New behavior gets *new* golden tests.
- **Struct-of-arrays for populations.** World players are columnar data (parallel
  `Vec`s), not heap objects. Player identity is an index/id into columns.
- **Talent ceiling is law.** Nothing pushes a current attribute above its potential.
- **LLMs at authoring time only.** Runtime text is template + slot from baked data.
  Never add a runtime network/model dependency.
- **Tiny saves.** Persist only results, records, and path-dependent state. Anything
  derivable from `seed (+ season, league, birth data, date)` is recomputed, not saved.
- **Dependency budget near zero.** Don't add dependencies or bump toolchain without
  asking. `flutter_rust_bridge = "=2.9.0"` is pinned.

## 4. Coding conventions

- `cargo fmt` + `cargo clippy -D warnings` clean before declaring any step done.
- Public API gets doc comments explaining *why* (the design rule it implements).
- Naming mirrors the bible glossary: beat, headspace, familiarity, orbit,
  lazy-promote, batch-tick, genesis, pantheon, school. Don't invent synonyms.
- All tunable numbers are named constants in each crate's `tuning` module — never
  inline magic numbers.

## 5. Test discipline

- **Golden-seed tests first** — every deterministic pipeline asserts exact outputs
  for fixed seeds. These are the project's spine.
- Property tests where cheap: `current <= potential`; `role_rating` monotonic;
  attributes always in 1–99.
- Long-horizon sanity: headless full-career fast-forward must not panic and must
  keep invariants.
- TUI smoke tests: scripted stdin → expected stdout fragments, fixed seed.
- Tests must not depend on each other or on execution order.

## 6. Commands

```bash
cargo test --workspace        # full test suite (golden tests included)
cargo fmt --check
cargo clippy --workspace -- -D warnings
scripts/test.sh               # fmt → clippy → tests → career-sim sanity (one-shot gate)
cargo run -p goat-tui         # the playable game
scripts/match-sim.sh          # match harness: beat-engine output vs results
scripts/world-sim.sh          # world/season sim harness
```

## 7. Current status

All 10 roadmap phases have landed (through Phase 10 — life & money, retirement
verdict, full-career loop). The full game is playable in `goat-tui` start to finish:
academy at 16 → weeks/training → beat-driven matches → seasons → transfers/contracts
→ legacy & pantheon → retirement verdict. Phase 3.5 (calendar + training wired into
the live loop) is also done. Remaining work is tuning, beat-library volume, and the
Flutter client (`goat-bridge`).

**Explicitly out of scope (parked, do not build unprompted):** goalkeeper career,
graphical renderers, final tuning numbers, beat-library volume beyond the starter
set, deeper relationship web.

## 8. Definition of done (any task)

1. `cargo test` green across the workspace, including all pre-existing golden tests
   with their original expected values.
2. `cargo fmt --check` and `cargo clippy -D warnings` clean.
3. New deterministic behavior covered by at least one golden-seed test.
4. No new dependencies, no floats in sim, no unsafe, no I/O in core, no logic in TUI.
5. Short summary: what changed, which bible/tech-doc section it implements.

## 9. How to work on this project (for the agent)

1. Read `CLAUDE.md` and the relevant doc(s) before any non-trivial change. Order of
   authority: design docs (`docs/`) > `CLAUDE.md` > this file. If docs disagree,
   flag it — don't silently pick one.
2. Never refactor `goat-rng` / `goat-fixed` as a side effect of other work.
3. When a design question is genuinely open (marked deferred/parked in the bible),
   ask — don't decide.
4. If a change requires deviating from the design docs, say so out loud and wait
   for the user's call.
5. Keep changes minimal and scoped; match existing code style.

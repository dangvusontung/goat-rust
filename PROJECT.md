# Become the GOAT — Project Context Document

Local resume update: [derived-state checkpoint](docs/LOCAL-RESUME-CHECKPOINT.md) —
optional exact cache in save layout 28, unchanged SIM13; canonical journals and
fallback reconstruction retained. Long-career file measurements are recorded there.

Latest core update: [v13 runtime journals and reactive NPC matches](docs/CORE-STEPS-2-3.md) —
lossless runtime storage, exact replay, score/fatigue substitutions, NPC discipline
and named keeper coverage; save layout 27/SIM13. Core roadmap milestones 2 and 3
are implemented for autonomous dated league fixtures; full competition calendar is next.

Previous core update: [v11 five-season league coefficients](docs/LEAGUE-COEFFICIENTS.md) — normalized
continental results, distinct cup entrants and ranked deep selection; save 25/SIM11.

Latest implementation: [v10 ranked deep leagues](docs/DEEP-LEAGUES.md) — PC league plus
top 5, named NPC fixture minutes/credits, authoritative standings replay; save 25.

Latest checkpoint (2026-10-06): [implemented v9 season chronology](docs/SEASON-CHRONOLOGY.md),
15 August–30 June, continuous training/health and dated replay; save layout 24.
Legacy factory descriptions below remain applicable to their compatibility paths.

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

**Development priority (2026-10-05): core simulation realism first.** The TUI
is a testing adapter, not a presentation/product priority. Complete shared
headless behavior before enhancing interfaces. See
[`docs/SIMULATION-AUDIT.md`](docs/SIMULATION-AUDIT.md) for mechanics, evidence,
realism gaps and the proposed core backlog.

The career spine is playable in `goat-tui`: academy/first-team start at 16 →
training → roster-driven league matches → seasons → contracts/transfers →
legacy/pantheon → retirement. Calendar progression and pre-season are live;
canonical training growth remains in `goat-core/week.rs`. The separate
`goat-training` day-based model has its own tests but is not called by the TUI.
Do not treat those independent training tests as live-loop integration evidence.

**World target: 150 countries** (confirmed by the user on 2026-10-05).
After the September merge, the active career path uses `WorldGenesis` with
20 countries, 3 tiers each and 20 clubs per tier. The earlier 50-country
`layout`/`worldgen` implementation remains in the repository but does not drive
the career loop. Neither number represents the final target; expanding to 150
requires an implementation task covering nation data, fixtures, saves and tests.

The current continuation checkpoint and validation evidence are in
[`docs/DEVELOPMENT-STATUS.md`](docs/DEVELOPMENT-STATUS.md). Remaining priorities
are shared core orchestration, consistent accounting, representative headless
season measurement and realism calibration;
the web demo and Flutter bridge do not yet imply feature parity with the TUI.

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

Current simulation update (2026-10-05): shared opportunity/conversion and PC/NPC
attribute development are implemented via versioned entry points (SIM_VERSION 6, including full-time red-card continuation, PC/NPC opportunity accounting and dated development history).
See [implementation report](docs/SHARED-SIMULATION-RUN.md) for validated runs,
performance measurements, compatibility and remaining fidelity gaps.

Controlled calibration: [400,000-fixture comparison](docs/MATCH-CALIBRATION.md)
finds NPC resolutions close in mean goals, with PC participation still governed
by separate scoring stakes. Shared PC/NPC opportunity accounting is now implemented in simulation version 5.

Completed: [v5 shared PC/NPC opportunities](docs/PC-NPC-OPPORTUNITIES.md),
460,000-fixture comparison, semantic non-goal narration, role/chain/policy
ablations. [PC/NPC dated development history](docs/DEVELOPMENT-HISTORY.md) is now implemented in v6 (save layout 22), including incremental NPC projection. Mobile session caching and consistent season chronology are next; 150 countries remains deferred.

Current update (2026-10-06): [v7 individual NPC training/health](docs/NPC-INDIVIDUAL-LIFE.md),
SIM_VERSION 7, layout 22. Five-drill weekly policy, energy/recovery and seeded
injury episodes feed promotion, selection and season availability. Core gate
564 passed; whole-world 20-season cloud replay median 9.21 s, peak native RSS
about 41.9 MiB. Mobile cache/device task and realistic fixture workload remain next.

Current update (2026-10-06): [v8 dated NPC fixture workload](docs/NPC-FIXTURE-WORKLOAD.md),
SIM_VERSION 8 / save layout 23. Observed engine minutes override background
plans; rest gaps and congestion affect energy and injury risk. Weekly training
and 364/365-day clock differences remain documented limitations.

Latest: [retained deep/light session](docs/DEEP-LIGHT-SESSION.md) implemented in core and live
league adapter. Incremental journal synchronization preserves v8 fresh replay;
no save-layout or simulation-version bump. Orbit policy, chronology and 200k
capacity validation remain pending.

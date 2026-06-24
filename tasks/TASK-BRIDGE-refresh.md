# TASK BRIDGE — Refresh goat-bridge to the current core (prereq for the Flutter UI)

Prereq: Phases 0–10 complete (headless core done). Read CLAUDE.md and
`docs/CALENDAR.md` appendix (bridge task notes). `flutter_rust_bridge = "=2.9.0"` is
PINNED — do not bump without asking.

The Flutter UI talks to the Rust core **only through `goat-bridge`**. The bridge was built
around Phase 8 and then parked, so it is **stale**: it exposes none of Phases 3.5, 9, or 10.
This task brings it current. It is a **catch-up, not a rewrite** — the pattern already works.

## Where the bridge is today (the pattern to extend)
- `crates/goat-bridge/src/api.rs` (~55k) — the hand-written surface:
  - `GoatGameState` + `*Dto` structs: flattened, FFI-safe mirrors of core state.
  - One `pub fn` per action (`new_game`, `advance_week`, `set_routine`, `play_round`,
    `apply_season_end`, …): each calls `reduce(state, Intent::…)` on a global `WorldState`
    and returns DTOs. Read-only `get_*` fns return DTOs.
- `frb_generated.rs` (~106k) — **codegen** output of `flutter_rust_bridge_codegen` from
  `api.rs`. Never hand-edit it; it is regenerated.
- **Gap:** zero references to economy/sponsors/life/retirement/genesis/rival/history/
  calendar. It mirrors ~Phase 8.

## ⚑ Decisions to confirm (pause before Slice 1)
- **D1 — Screen-text strategy.** Player-facing text for screens currently lives in
  `goat-tui` render fns, not core. (a) Lift NEW Phase 9/10 strings into core as
  template+slot as we expose them (Flutter + TUI share; true to the architecture); or
  (b) each renderer owns its strings (faster, but duplicated). *Recommend (a) for new
  surface; don't retro-migrate Phases 1–8 text in this task.*
- **D2 — Codegen / Dart workflow (CORRECTED — this is a hard prerequisite).**
  `frb_generated.rs` is codegen and is **field-locked**: it constructs
  `GoatGameState { … }` field-by-field, so changing ANY DTO breaks its compilation. It can
  only be fixed by regenerating with `flutter_rust_bridge_codegen`, which (a) is **not
  installed** and (b) needs a **Dart target** (a Flutter project + frb config) that **does
  not exist**. Flutter/Dart SDKs ARE installed. So a minimal Dart target + the codegen
  toolchain must be stood up FIRST (new Slice B.0) — you cannot edit the FFI structs
  before then. Hand-editing `frb_generated.rs` is forbidden.
- **D3 — World-data exposure granularity.** *Recommend read-only summaries* (current
  standings/top scorers per league, pantheon canon, rival verdict, history slice) — never
  the raw 20–30k population over FFI.

## TDD anchor (the load-bearing one)
`crates/goat-bridge/tests/spec_bridge_parity.rs`:
- `bridge_path_matches_direct_reduce` — a scripted sequence driven through the bridge
  `pub fn`s produces a `GoatGameState` whose fields equal those from the same intents
  applied via direct `reduce`. **Determinism must survive the FFI boundary** (same seed ⇒
  same state); this is the spine of the whole task.
- Per-DTO completeness: every new state field appears in `GoatGameState`/a `*Dto`.

## Slices (ordered, each ships green)
### B.0 — Toolchain + minimal Dart target + baseline regen (PREREQUISITE)
Install `flutter_rust_bridge_codegen` =2.9.0 (matches the pinned lib). Create a minimal
Flutter/Dart project + `flutter_rust_bridge.yaml` pointing at it as the codegen output.
Regenerate `frb_generated.rs` from the CURRENT `api.rs` with **no API changes** and confirm
it is byte-stable (or trivially equivalent) + still compiles — proving the regen loop works
before we change anything. Without this, slices B.1–B.4 cannot compile.
Gate: `flutter_rust_bridge_codegen generate` runs clean; `cargo build -p goat-bridge` green.

### B.1 — Economy + sponsors + life actions & state
Extend `GoatGameState` with the Phase-10 scalars (savings, business_value, bankrupt,
dev_invest_level, marketability, sponsor_tier, relationships[3], character_rep). Add
`pub fn`s: `invest_in_business`, `set_dev_investment`, `sign_sponsor`, `set_marketability`,
`apply_life_event`, `respond_to_media`, `settle_season_economy`. Parity test green.

### B.2 — Calendar surfacing
Expose `pc_epoch_day` + the week's `last_week_flashpoints` (a `FlashpointDto`) so the UI
can render calendar windows interrupting a week.

### B.3 — World read-models (Phase 9)
Read-only DTOs + fns: `get_pantheon_canon` (from `backfill_history`), `get_rival_verdict`
(`crystallise_rival`), `get_world_standings`/top-scorers (batch-tick summaries),
`get_world_fingerprint` (debug). Summaries only (D3).

### B.4 — Retirement + final verdict
`should_retire` query; a `VerdictDto` (the schools' disagreeing placements via
`all_rankings` with the Phase-10 `icon_axis` Icon), exposed as `get_final_verdict`.

### B.5 — Codegen + parity hardening
Regenerate `frb_generated.rs` from `api.rs` (pinned 2.9.0). Confirm it compiles and the
parity test still passes. Document the regen command. Dart bindings deferred to the
Flutter-scaffold task (D2).

## ⏸ Pauses
Before Slice 1 (confirm D1–D3). Before regenerating `frb_generated.rs` (it is large/codegen
— review the diff).

## Definition of done
1. `cargo build -p goat-bridge` + `cargo test -p goat-bridge` green; workspace still green.
2. `GoatGameState`/DTOs expose every Phase 3.5/9/10 state field a UI needs.
3. A `pub fn` exists for every player action across the live game.
4. Bridge-vs-direct parity test green (determinism survives FFI).
5. `frb_generated.rs` regenerated from `api.rs`, consistent, compiles.
6. `fmt` + `clippy -D warnings` clean; `#![forbid(unsafe_code)]` holds above the FFI line;
   no new deps; flutter_rust_bridge stays pinned at =2.9.0.
7. Per-slice summary of what was exposed.

## Out of scope (separate tasks)
The Flutter app scaffold + Dart UI (next task after this). Retro-migrating Phases 1–8
screen text. Number tuning (TASK-TUNE). Parked: GK career, deeper relationship web.

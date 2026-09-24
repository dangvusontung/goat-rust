# TASK — BL5.1: Goal/Assist split

Prereq: none. Confirmed scope, ready for Dev (no open design questions left).

Read first: `crates/goat-match/src/beats.rs` (`ScoreEvent`), `crates/goat-match/src/library.rs`
(all `score_event: Some(ScoreEvent::GoalFor)` sites), `crates/goat-core/src/state.rs`
(`pc_season_goals`/`pc_career_goals`, `Intent::ApplyRoundResult`), `crates/goat-tui/src/main.rs`
(counting + display sites), `crates/goat-tui/src/career_sim.rs` (counting sites),
`crates/goat-save/src/save.rs` (`VERSION`, the v7→v8/v8→v9 tail-append precedent for backward
compat).

## Origin

Raised by Tùng 2026-07-22 (`tasks/TASK-DESIGN-round2-world-genesis-scaleup.md`, "BL5.1" under
"Parked for a future design round"), picked up 2026-08-03. Only `goals` is tracked anywhere
today (`pc_season_goals`/`pc_career_goals`); assists don't exist as a field. `ScoreEvent::GoalFor`
is the only score-event variant; every "PC scores" outcome in `library.rs` uses it, including at
least one outcome that is narratively an assist, not a goal.

## Verified: current state (2026-08-03)

- `crates/goat-match/src/library.rs` has 22 outcomes tagged `score_event: Some(ScoreEvent::GoalFor)`.
  Grepped every one against its own outcome text: **21 are genuine PC goals** (shot/header/finish
  text ending "GOAL!"). Exactly **one is mistagged** — the outcome at (search for this exact
  string) `"The dummy worked — your teammate curls it in. 2-1!"`: the PC sets up the goal, a
  teammate scores it. This is the only beat in the whole library that needs retagging.
- `pc_season_goals`/`pc_career_goals` (`crates/goat-core/src/state.rs`) follow this pattern:
  season counter reset to 0 at season-end, accrued into the career counter at
  `ApplySeasonEndLegacy`, incremented from `Intent::ApplyRoundResult`'s `pc_goals: u32` field
  (computed by the TUI by filtering match moments for `ScoreEvent::GoalFor` before constructing
  the intent).
- Counting sites that filter on `ScoreEvent::GoalFor` and must get an assist-filter sibling:
  `crates/goat-tui/src/main.rs` (~line 918, the per-match goal count feeding `ApplyRoundResult`)
  and `crates/goat-tui/src/career_sim.rs` (4 sites: the equivalent per-match/per-season counts
  feeding its own `ApplyRoundResult` construction and its season-summary struct).
- `pc_season_goals`/`pc_career_goals` are part of the binary save format
  (`crates/goat-save/src/save.rs`, `VERSION = 14`), written unconditionally and read with
  `.unwrap_or(0)` for the career fields (added after season fields, so old saves default them to
  0). New assist fields need the same tail-append treatment: write unconditionally, read with
  `.unwrap_or(0)`, bump `VERSION` to 15.

## Decision (confirmed, small — mirror the `goals` pattern exactly, nothing more)

1. **`beats.rs`**: add `ScoreEvent::AssistFor` as a new enum variant alongside `GoalFor`.
2. **`library.rs`**: retag exactly the one mistagged outcome identified above from `GoalFor` to
   `AssistFor`. Do not touch any of the other 21 `GoalFor` sites — they're correct as-is.
3. **`state.rs`**: add `pc_season_assists: u32` / `pc_career_assists: u32`, wired identically to
   `pc_season_goals`/`pc_career_goals` — same reset point, same accrual-to-career point, same
   `Default` init (0).
4. **`Intent::ApplyRoundResult`**: add a `pc_assists: u32` field next to `pc_goals`, incrementing
   `state.pc_season_assists` the same way `pc_goals` increments `state.pc_season_goals`.
5. **Counting sites**: in `main.rs` and all 4 `career_sim.rs` sites, add an `AssistFor` filter
   parallel to the existing `GoalFor` filter, and pass the resulting count into
   `ApplyRoundResult`'s new `pc_assists` field.
6. **Save format**: bump `crates/goat-save/src/save.rs::VERSION` to 15. Add
   `pc_season_assists`/`pc_career_assists` to `SaveData`, write unconditionally, read with
   `.unwrap_or(0)` (mirrors exactly how `pc_career_goals` was added after `pc_season_goals` —
   find that precedent in the file and copy its shape). Add a backward-compat test loading an old
   fixture/bytes at a pre-15 version and asserting the new fields default to 0, per the existing
   v7→v8/v8→v9 precedent this project's `CLAUDE.md` Definition-of-Done calls for.
7. **Display**: wherever `pc_season_goals`/`pc_career_goals` are already surfaced as part of an
   existing stat line (season summary, career summary structs in `main.rs`/`career_sim.rs`),
   add the assist counterpart next to it, matching the existing formatting. Do not invent new
   screens or reflow existing ones.

## Explicitly out of scope (do not touch)

- **`crates/goat-meta/src/legacy.rs` (`LegacyEvidence`)** — do NOT wire assists into the Legacy
  scoring formula. It stays `career_goals`-only for now. This is a deliberate scope decision
  (Legacy-weighting of assists is a separate design question, not yet asked of Tùng) — leave a
  one-line comment noting it as a future hook, same idiom as BL7's "club medical quality" hook
  note in `tuning.rs`.
- **Golden Boot** (`compute_golden_boot` in `main.rs`) — real-world Golden Boot is a goals-only
  award; do not add assists to it.
- **Rival crystallization** (`crystallise_rival`, uses `pc_career_goals`) — goals-only by design,
  don't touch.
- **Any TUI bug unrelated to this** — per this project's standing rule, `goat-tui` is a
  playtest/dev harness, not the real UI. Fix only what's needed to correctly display/count
  assists; don't fix unrelated TUI issues you notice along the way.

## Definition of done

Follow `CLAUDE.md`'s standing Definition of Done exactly:
1. `cargo test --workspace` green, including all pre-existing golden tests with original expected
   values unchanged.
2. `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. At least one new golden-seed or backward-compat test covering the new assist counting/save
   round-trip.
4. Playable gate: state the exact `cargo run -p goat-tui` flow that shows an assist being counted
   (the one retagged beat, at (find its beat id / phase in `library.rs`) — play/skip a match that
   reaches it, then check season/career summary shows a nonzero assist count).
5. No new dependencies, no floats in sim, no unsafe, no I/O in core, no logic in TUI.
6. Commit with a short summary of what changed and which section of this doc it implements.

If anything above turns out to be wrong once you're reading the real code (e.g. a counting site
that doesn't exist, a struct shaped differently than described), stop and report back rather than
improvising a fix — this doc was written from a fresh read of the code on 2026-08-03 but may have
missed something.

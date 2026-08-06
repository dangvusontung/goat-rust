# TASK — Auto-advance training between match weeks

Prereq: none. This is a game-loop interaction-model fix, not a UI redesign.

Read first: `crates/goat-tui/src/main.rs` (the main input loop, `run_next_round`,
`Intent::AdvanceWeek`/`AdvanceWeeks` handlers), `crates/goat-core/src/state.rs`
(`tick_one_week`/`tick_one_rest_week`, `pc_week_training_done` flag,
`Intent::ApplyRoundResult`), `crates/goat-world/src/calendar.rs`
(`WEEK_MATCH_COUNTS`, `round_to_week`, `week_to_rounds`, `is_break_week`).

## Origin

Raised by Tùng 2026-08-03, mid architecture walkthrough of the training/week loop.
His framing: a real week can have 1-2 matches or none; days without a match should
just train (or rest) automatically off whatever routine is already set; the player
should only be interrupted for an actual match decision. This is bible §2.2
("manage by exception") applied literally to the weekly loop, which today does not
follow it — see "Verified: current state" below.

**Scope note (Tùng, explicit):** `goat-tui` is a playtest/dev harness, not the real
UI — per this project's standing rule (see `TOOLS.md`/prior memory: "TUI chỉ để test,
đừng refer vào nó"). Treat this as fixing the underlying **interaction model** (when
should the game auto-progress vs. stop and ask), not as a TUI polish task. Do not
improve TUI cosmetics, formatting, or unrelated menu items while here.

## Verified: current state (2026-08-03)

- **The player must manually press `[W]` every calendar week** to train, even though
  `Routine` (focus attrs + intensity) is a persistent setting (`state.pc_routine`,
  changed only via `[S]`) that doesn't change week to week. `[W]` → `Intent::AdvanceWeek`
  → `tick_one_week` (real training via `advance_week`) → sets `pc_week_training_done`.
  A second `[W]` in the same week is a no-op (existing guard, correct, keep it).
- **`[P]`/`[K]` (play/skip match) are NOT gated by whether a match is actually due
  this week.** They're always available once a season is active (`has_season`) and
  call `run_next_round`, which advances to whatever the next round is regardless of
  real elapsed time. The player currently free-flows between `[W]` and `[P]`/`[K]` in
  any order; `pc_week_training_done`/`week_ends` (in `Intent::ApplyRoundResult`)
  reconciles the calendar bookkeeping after the fact, however the player interleaved
  them.
- **A working "auto-advance, stop at the first noteworthy event" mechanism already
  exists**: `Intent::AdvanceWeeks { n }` (behind `[F]`) loops `tick_one_week` up to
  `n` times and breaks early "at the first noteworthy event"
  (`!state.last_week_events.is_empty()`). This does NOT currently stop for "a match is
  due" — only for training-derived events (injury, breakthrough, etc.). Confirm this
  reading is right — if `last_week_events` already includes match-due signals, the
  scope below shrinks further.
- **The 1-2-matches-per-week / break-week model already exists as real data**, unused
  by this loop: `goat-world::calendar::WEEK_MATCH_COUNTS` (0/1/2 matches per calendar
  week), `round_to_week(round)` / `week_to_rounds(week)` (round↔week mapping),
  `is_break_week(week)`. `run_next_round` already computes `round_to_week(round)` for
  its own header display (`format_week_header`), so the round↔week relationship is
  already being read at the point matches are played — it's not a new lookup to wire.

## Decision (confirmed scope, mirror manage-by-exception exactly)

Change `goat-tui`'s main loop so the player's default action is a single "continue"
input that:

1. Auto-applies the current `Routine` via the training path (same underlying call as
   today's `[W]` — `tick_one_week`/`advance_week`, **no change to the training math,
   no new golden values**) for each week that has no match due, using the existing
   `is_break_week`/`week_to_rounds`(or equivalent already-available round↔week data)
   to know which weeks those are.
2. Stops and prompts the player **only** when: a match is due this week (offer
   Play/Skip, same as today's `[P]`/`[K]`), OR a noteworthy `DevelopmentEvent` fires
   (injury, breakthrough — same trigger `AdvanceWeeks` already uses), OR a calendar
   flashpoint fires (`last_week_flashpoints` — transfer window, international break —
   already surfaced today, don't change what counts as noteworthy).
3. A week with 2 matches (`WEEK_MATCH_COUNTS` can be 2): confirm both get offered in
   sequence, not silently collapsed to one — check `week_to_rounds(week)` returns
   both round indices and both get their own Play/Skip prompt.
4. Keep `[W]`/`[F]`/`[P]`/`[K]`/`[S]` available as manual overrides for anyone who
   wants to step through by hand — this is additive (a faster default path), not a
   removal of existing controls. Existing keybindings must keep working exactly as
   they do today.

If reading `AdvanceWeeks`'s existing early-stop logic (`last_week_events`) reveals it
already covers more of the above than this doc assumes (e.g. it turns out matches are
already events), say so and shrink the change accordingly — don't build machinery
that already exists.

## Explicitly out of scope

- **No changes to `week.rs`/`advance_week`'s training math.** This task changes *when*
  training fires automatically, never *what it computes*. All existing golden tests
  must stay green with unchanged expected values.
- **No TUI cosmetic changes** (menu text polish, colors, layout) beyond what's
  mechanically required to remove the redundant manual step. Per the origin note
  above, `goat-tui` is a test harness — don't invest in making it a "nicer" UI.
- **No changes to match resolution, discipline, injury, or calendar-window logic** —
  this task only touches the *loop that decides when to call* those systems, not
  those systems themselves.
- **Congestion warnings (BL/US-05 style "you have 2 matches in 8 days")** — not part
  of this task. Auto-stopping to play both matches in a 2-match week satisfies the
  immediate ask; a proactive warning banner is a separate, later idea if wanted.

## Definition of done

Follow `CLAUDE.md`'s standing Definition of Done:
1. `cargo test --workspace` green, including all pre-existing golden tests with
   original expected values unchanged.
2. `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings`
   clean.
3. At least one new test (smoke/scripted-stdin or headless) proving: a break week
   auto-trains without a keypress, a 1-match week stops once for Play/Skip, a 2-match
   week stops twice.
4. Playable gate: state the exact `cargo run -p goat-tui` flow showing the new default
   "continue" behavior skipping straight through break weeks to the next match
   decision.
5. No new dependencies, no floats in sim, no unsafe, no I/O in core, no logic moved
   into the TUI that belongs in `goat-core` (the auto-advance *decision loop* can live
   in the TUI since it's presentation-layer sequencing, but don't duplicate any
   training/match logic there — call the same `goat-core`/`goat-world` functions the
   manual path already uses).
6. Short summary: what changed, which section of this doc it implements.

If anything above turns out to be wrong once you're reading the real code (e.g. the
round↔week mapping doesn't work the way this doc assumes, or `AdvanceWeeks` already
does more than described), stop and report back rather than improvising a fix — this
doc was written from a fresh read of the code on 2026-08-03 but may have missed
something.

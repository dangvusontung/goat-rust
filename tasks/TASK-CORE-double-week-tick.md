> **Status: DONE (2026-07-03).** Implemented as specced: `week_ends` on
> `ApplyRoundResult` (callers use `goat_world::week_ends_after_round`),
> `AdvanceWeek` no-ops when the week is already ticked. Pinned by
> `double_fixture_week_ticks_once` in the bridge spec suite. Clock-style test
> harnesses (AdvanceWeek loops with no round loop) now clear
> `pc_week_training_done` per iteration to simulate week boundaries.

# TASK CORE — Exactly one weekly tick per calendar week (fix double-fixture aging)

Follow-up to TASK-CORE-time-model-age-sync (landed 2026-07-03). Requested by the
Flutter client: in a double-fixture week the player currently ages 2 weeks in 1
calendar week.

## Where it stands today (the model that landed)

`Intent::ApplyRoundResult` ticks time **once per round**: if
`pc_week_training_done` is false it runs a rest tick, then `rest_weeks` rest
ticks for skipped break weeks, then resets the flag (`state.rs` ~L798). The
flag reset after EVERY round means the 3 double-fixture weeks (calendar weeks
1, 6, 23 — see `WEEK_MATCH_COUNTS`) get **two ticks each**:

- In-season ticks = 30 rounds + 11 skipped breaks = **41** over 38 calendar weeks.
- The +3 drift is absorbed at `StartSeason` (off-season back-fills 11 instead
  of 14 to the 52-week target), so the season-start age invariant still holds —
  but intra-season age runs up to 3 weeks fast, and the player can train TWICE
  in one calendar week (the flag reset re-enables TRAIN before the 2nd fixture).

## Desired model

**One weekly tick per calendar week, exactly.** The week's tick happens either
as a training tick (player trained) or a rest tick (first match of the week
applies it). The second round of a same-week pair passes no time and offers no
second training session. Season totals become: 27 match weeks + 11 break weeks
= 38 in-season ticks; off-season back-fill = 14. Invariant unchanged:
`age == START_AGE + (season−1)·52` at every season start.

## Implementation sketch

Repurpose `pc_week_training_done` as "**this calendar week's tick has run**"
(consider renaming to `pc_week_ticked` if save compat allows; DTO field name
`week_training_done` can stay — its UI meaning "TRAIN is spent" still holds):

1. **`Intent::AdvanceWeek` (train):** if the flag is set → no-op (return state
   unchanged; don't clobber `last_week_events`). Else run the training tick and
   set the flag. This is what enforces one training per calendar week — the
   reducer, not the UI, is the gate.
2. **`Intent::ApplyRoundResult`** gains `week_ends: bool` (alongside the
   existing `rest_weeks`), true when this round is the LAST round of its
   calendar week — caller computes:
   `round + 1 >= ROUNDS_PER_SEASON || round_to_week(round + 1) != round_to_week(round)`.
   Handler:
   - if `!flag` → rest tick; set flag (the week has now elapsed untrained).
   - if `week_ends` → run `rest_weeks` rest ticks, then `flag = false`
     (a fresh week begins for the next round).
   - if `!week_ends` (same-week second fixture upcoming) → leave flag TRUE:
     blocks a second training AND blocks a second tick at the next
     ApplyRoundResult.
3. **Callers** (bridge `play_round` + `make_beat_choice`; TUI `main.rs` normal
   + suspension paths; `career_sim.rs` both loops; test drivers in
   `golden_world.rs` / `full_sim.rs`): pass `week_ends` computed from the
   calendar. Consider a goat-world helper `week_ends_after_round(round) -> bool`
   next to `rest_weeks_after_round`.

## Tests to update / add

- `spec_time_model_age_sync.rs`: in-season totals change 41 → 38; the
  season-start invariant tests stay as-is (target unchanged). Add:
  `double_fixture_week_ticks_once` — play rounds 0..3 (covers calendar week 1's
  two fixtures) and assert age advances exactly `round_to_week(3)` weeks... i.e.
  after round 2 (both week-1 fixtures) age_weeks - START == 2, not 3.
- `spec_bridge_parity.rs`: extend `calendar_shifts_with_played_rounds` to also
  assert `week_training_done` stays true between a double week's two fixtures
  (TRAIN stays spent) and age gains 1 week, not 2, across the pair.
- Bridge DTO: no new fields needed; `week_training_done` semantics on double
  weeks change (stays true after fixture 1) — the Flutter TRAIN button already
  keys off it, so the UI needs zero changes.

## Gameplay note (intentional change — flag to designer)

Max training sessions drop 30 → 27 per season (double weeks: one session for
two matches, matching real congested-fixture rhythm). Long-horizon growth specs
and career-sim calibration may shift slightly; re-baseline where the change is
explained by 3 fewer sessions + 3 fewer age-weeks mid-season.

# TASK CORE — Sync player age (and weekly development) to the season calendar

Found via the Flutter client (2026-07-03): a player at Game Week 2, Round 2/30, with
2 matches played still shows **16y 0w**. Matches move the season calendar but pass
no time for the player.

## The gap

The game has two clocks that drift apart:

1. **Season calendar** — derived from `season_round` (30 rounds over 38 Mon–Sun
   calendar weeks incl. breaks; see `goat-world/src/calendar.rs`).
2. **Player clock** — `age_weeks` ticks ONLY inside the weekly development tick
   (`goat-core/src/week.rs` ~L114, driven by `Intent::AdvanceWeek` = training).

Consequences:
- Playing matches never ages the player. Nothing forces training, so you can play
  all 30 rounds at 16y 0w (the reported screenshot).
- Even training dutifully once per round (the intended loop — the bridge resets
  `pc_week_training_done` after every match): 30 age-weeks per season, not 38 —
  the 8 break/rest weeks never tick.
- `Intent::StartSeason` (`state.rs` ~L753) adds ZERO weeks — the ~14-week
  off-season doesn't exist for the player's body.
- Net drift: ≤30 age-weeks per 52-week season-year. After 10 seasons the world is
  in 2035 but the player is ~21–22 instead of 26. The age curves (growth window,
  peak, decline — `game_mechanics.md`) are calibrated against real age, so careers
  become absurdly long and decline barely happens. Legacy/pantheon "age at
  retirement" comparisons are similarly skewed.

## Expected model (proposal)

Time is driven by the calendar; training is a choice *within* a week, not the clock:

1. **Round transitions tick elapsed weeks.** When `season_round` advances (via
   ApplyRoundResult), tick the weekly development once per calendar week crossed
   (including skipped break weeks). Weeks where the player didn't train run a
   "rest week" variant: age + energy recovery + injury countdown + decay, no
   training growth (`pc_week_training_done` already distinguishes them).
2. **Off-season ticks at StartSeason.** 52 − 38 = 14 rest-week ticks (or tune:
   some as vacation with faster energy recovery, no decay — designer call).
3. **Invariant to pin in a spec test:** at the start of season N,
   `age_weeks == START_AGE_WEEKS + (N − 1) * 52`. A bridge-level version belongs
   in `goat-bridge/tests/spec_bridge_parity.rs` (e.g. after a full season loop,
   `age_years == 17` at the start of season 2).

## Golden safety

- `golden_week` (the per-tick growth/injury/decay math) is untouched — the fix
  changes HOW OFTEN ticks run, not what a tick does. The rest-week variant is new
  surface, not a change to the frozen path.
- Long-horizon specs (`spec_phase10_longhorizon`, career sims, peer `BatchTickPeers`
  calibration) WILL shift — peers' aging assumptions must be checked so PC and
  peers age at the same rate.

## Note for the bridge/UI (after core lands)

`advance_week` should probably become "train this week" (rejected/ignored if
already trained this calendar week) rather than "advance the clock", and
`advance_weeks(n)` becomes "simulate n calendar weeks". Bridge loop in
`play_round`/`make_beat_choice` completion then needs no special handling —
the reducer owns the clock.

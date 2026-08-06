# TASK CORE — Bank the running season when retiring mid-season

Found by `crates/goat-bridge/tests/spec_bridge_parity.rs` (bridge spec suite, 2026-07-02).

## Problem

Career totals (`pc_career_goals`, `pc_career_matches`, `pc_career_output_sum`) are
banked **only** by `Intent::ApplySeasonEndLegacy` (`goat-core/src/state.rs` ~L492).
`Intent::Retire` (~L700) just sets `pc_retired = true`:

```rust
Intent::Retire => {
    state.pc_retired = true;
    state
}
```

The Flutter hub exposes a RETIRE button at any point in the season
(`goat_flutter/lib/features/game/presentation/pages/game_loop_page.dart`). A player
who retires mid-season loses that season's goals/matches from the career totals the
retirement page renders (`retirement_page.dart` shows `careerGoals` / `careerMatches`
straight from the snapshot). The final verdict / legacy computation presumably
under-counts the same way.

## Expected

Retiring must fold the running season's stats into career totals first — either:
- (a) `Intent::Retire` in core banks `pc_season_*` into `pc_career_*` (mirroring the
  ApplySeasonEndLegacy arithmetic, minus awards/title logic), or
- (b) the bridge `retire()` fn runs the season-end banking intents before `Retire`
  (consistent with how `new_game` composes intents).

(a) is truer to "no game logic outside core".

## Also note (DTO semantics, no code change needed)

Mid-season, `GoatGameState.career_goals`/`career_matches` EXCLUDE the running season.
Any UI showing "live" career totals must display `career_* + season_*`. The bridge
spec test pins this semantics; update the test when (a)/(b) lands.

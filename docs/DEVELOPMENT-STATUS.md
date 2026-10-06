# Development checkpoint — 2026-10-05

Owner direction: **core realism first; TUI is a testing adapter**. This checkpoint
is supplemented by the [simulation audit](SIMULATION-AUDIT.md). Documentation
and two diagnostic 10,000-match samples were added after baseline stabilization;
no simulation behavior was changed by the audit.

Latest coding update: shared match and per-attribute development entry points
are implemented. See [implementation, measurements and limits](SHARED-SIMULATION-RUN.md).
PC red-card continuation is also implemented; see [regression coverage](RED-CARD-CONTINUATION.md).
The [controlled match comparison](MATCH-CALIBRATION.md) now includes 400,000
fixtures and isolates a large PC participation effect; opportunity accounting is
now implemented in [simulation version 5](PC-NPC-OPPORTUNITIES.md). Latest: [v6 PC/NPC development history](DEVELOPMENT-HISTORY.md), persistent PC
training/health/workload ledgers and sparse NPC exposure with incremental projection.
The audit below remains a record of the earlier baseline.

Latest update (2026-10-06): [v7 individual NPC life](NPC-INDIVIDUAL-LIFE.md) adds
named weekly drills, accumulated energy, dated injury/recovery episodes and
health-aware lineups/season quotas. Full gate 564 passed, 0 failed, 1 ignored.

## Last feature milestone

The September 22–24 local branch delivered academy/youth intake, staff,
roster-driven league matches (PA2 M1–M4), manager selection, NPC match residue,
substitutions, danger-man duels, debut OVR cap 65, persistent training intensity,
career-batch, and merit-based transfer offers. The last market fix, `d4a1976`,
replaced cumulative seasonal output with per-match average in scout estimates.
Merge `5ed92df` on September 25 integrated the other branch's calendar/pre-season,
web demo, competitions, club economy and legacy additions. The next commit,
`c78b04b`, only added log ignores.

## Active paths and target

- **Target: 150 countries**, confirmed by the user on 2026-10-05.
- The active TUI/career harness uses `WorldGenesis`: 20 countries, 3 tiers each,
  20 clubs per tier, 38 league rounds. `career_batch.rs` explicitly records this
  post-merge choice. The 50-country `layout`/`worldgen` path remains separate.
- Calendar progression is live. Training growth/injuries/decay remain canonical
  in `goat-core/week.rs`; `goat-training` is a separate day-based model, not a
  TUI dependency. Its original consolidation task is not a reliable done marker.
- League matches use real population squads, manager selection, substitutions
  and persisted NPC credits. Academy and several non-league match paths use
  synthetic squads; the web demo has a smaller feature surface than the TUI.
- Club economy passes are wired in `ReplayCache::advance_one_season`; this does
  not establish that every live career path invokes that full replay pipeline.

## Baseline stabilization

The old TUI creation scripts now use seed-before-nation selection and answer the
academy prompt. Tests of league actions first complete pre-season. Existing
assertions remain intact. The Clippy range-loop finding in attribute bias
calculation is fixed using mutable enumeration, preserving operation order and
simulation arithmetic. No dependency, save-version or golden-value changes.

`live_league_season_persists_roster_matches_and_legacy` exercises the actual TUI
with seed 42 in England's third tier: seven pre-season weeks, all 38 league
rounds, season-end banking, save and load. It checks manager output, reduced
playing time, persisted NPC credits and one banked season. This is an auto-play
test of the shared league setup; interactive per-beat choices are not exercised.

## Next work

1. [Extract shared headless simulation](../tasks/TASK-CORE-SHARED-SIMULATION.md)
   from renderer orchestration without altering behavior. Build a representative
   season runner using actual squads, manager decisions and the match engine.
2. Address the audit's core correctness/realism gaps: red-card continuation,
   participation/statistics accounting, actual award records, compatible
   background simulation, workload/development and shared market/replay state.
3. Measure across positions and seeds, recording bench rate, trust, minutes,
   contributions and result distributions. Synthetic career-batch output is
   not evidence of live match realism. Establish real-data calibration targets.
4. The [150-country expansion](../tasks/TASK-CORE-150-COUNTRIES.md) is saved for
   later implementation on the shared core. Presentation enhancements follow
   core completion, not the other way around.

## Reproduce validation

Validated on 2026-10-05: the complete `scripts/test.sh` gate passes, including
formatting, workspace Clippy, 532 passing tests (0 failures, 1 ignored), the
seed-42 career invariant check and seed scanner. All 22 TUI smoke tests pass,
including the new live-season check. Golden values and dependency lockfiles
remain unchanged. The ignored test is not counted as executed validation.

From the existing checkout, activate the environment tools if needed:

```sh
source /workspace/.goat-tools/activate.sh
cd /workspace/goat-rust
bash scripts/test.sh
cargo test --locked -p goat-tui --test smoke_stdin live_league_season
```

The quality gate includes formatting, Clippy, workspace tests, seed-42 career
invariants and the seed scanner. Preserve frozen golden values. Use the existing
isolated checkout; do not create a worktree unless explicitly requested.

# Dated NPC fixture workload — simulation version 8

Latest checkpoint (2026-10-06): [implemented v9 season chronology](SEASON-CHRONOLOGY.md),
15 August–30 June, continuous training/health and dated replay; save layout 24.
Legacy factory descriptions below remain applicable to their compatibility paths.

Implemented 2026-10-06. The active league adapter now uses
`genesis_scheduled`, `ReplayCache::new_scheduled` and
`rebuild_population_scheduled`. Earlier expected/shared/developed/lived factories
remain available for frozen compatibility tests. The match opportunity kernel,
PC weekly behavior, identity and innate potential draws are unchanged.

## What changed

V7 deducted the preceding season's appearance proxy evenly over 52 weeks. V8
constructs dated fixture doses from the existing 38-round league calendar:
preseason and breaks have no league dose; single-match weeks use Saturday;
double-match weeks use Tuesday and Saturday. Separate player/fixture RNG draws
choose background rotation and minutes. Regulars appear with 79% probability
for 75–90 minutes; fringe players with 21% probability for 10–30 minutes.
Exposure appearance metadata chooses the rotation category; it no longer supplies
an averaged energy deduction. These background minutes are **plans**, not
observations or a fully selected team XI.

The match engine reports named NPC starter and substitute stints and bench DNPs
with zero minutes. Opposition substitutions close the outgoing stint and open
the incoming one; recording adds no match RNG draws. The live league adapter
stores `(competition, player, fixture, date, minutes)`, using the persisted
fixture's scheduled date when available. An observation overrides the generated
fixture everywhere, even if its date moved into another week. A zero-minute
observation suppresses a planned appearance. Repeating an identical observation
is idempotent; contradictory records for the same player/fixture are rejected
by population ingestion (the core journal keeps the first accepted entry).

## Health and development

Healthy NPC weeks process match dates in order. Match cost is 20 energy per
90 minutes, proportional to minutes played; non-match days recover 6 energy,
capped at 100. The existing weekly training cost is then charged, with the shared
exhaustion downgrade, growth, ceilings, decay and injury primitives. V7's weekly
passive recovery is omitted here because daily recovery has already applied.

Weekly injury probability is the shared PC base probability multiplied by:

`1 + max(weekly_minutes - 90, 0) / 180 + 0.5 when the shortest match gap is <3 days`

Integer rounding applies and probability is capped at 999/1000. These constants
are provisional game calibration; no empirical injury model has been validated.
Weekly history exposes minutes, match counts, shortest gap and sampled risk.
Existing injury recovery skips generated fixtures and training. Authoritative
observations still contribute load when an approximate weekly injury prediction
would have excluded the player; they do not rewrite that medical prediction.

League appearance totals count positive-minute dated league doses. Cup/national
observations can add fatigue through the same API without counting as league
appearances. Goal allocation weights use these appearance totals. Existing orbit
career credits are retained; batch crediting adds only the remainder.

## Persistence and replay

Save layout **23**, `SIM_VERSION = 8`. Optional `NLOD` rows are 24 bytes each
plus an eight-byte section header. Only path-dependent observed minutes are saved;
background plans, attributes and weekly medical histories remain derived. Older
simulation versions are refused; no save migration is provided. Loading reconstructs
market/season state, sparse observations, health and attributes deterministically.

Compact observation columns link entries to player indices. Numeric caches include
last match date and this season's played league count. Adding an observation
invalidates that player's derived caches, including earlier dates if the fixture
was rescheduled. Query order does not consume another player's RNG stream.

## Validation and performance

See the recorded gate and release benchmark measurements below. Tests cover
substitution minute conservation including zero-minute late substitutes, rest /
30-minute / 90-minute / congested / consecutive-day loads, cup accounting,
injury exclusions, reschedule de-duplication, cold/warm/backward query consistency,
season replay and save corruption bounds. Frozen golden expectations are retained.

## Remaining limits

- Background rotation is independent per player, so it does not guarantee eleven
  starters or team-wide minute conservation. Background matches still use seasonal
  aggregation rather than full named-player match-engine simulation.
- The engine's own-side PC substitution uses an abstract replacement. Named NPC
  telemetry does not resolve that replacement into a complete own-side XI ledger.
- Only league fixtures are currently captured by the live adapter; cups, national
  games, travel and training-day planning need further integration.
- Injury onset and growth remain weekly, not exact events during a match. Weekly
  training is still `goat-core` behavior; this does not integrate the independent
  `goat-training` day-based subsystem.
- World life uses 52 × 7 = 364 days per season; core calendar windows use 365.
  This change preserves those existing clocks rather than unifying them.
- The [retained headless session](DEEP-LIGHT-SESSION.md) now avoids rebuilding
  completed seasons between live TUI league rounds. Native ARM64 measurements
  remain pending; cloud timings do not establish mobile performance.
- The 150-country expansion remains a separate deferred task.

## Recorded run — 2026-10-06

`bash scripts/test.sh`: **571 passed, 0 failed, 1 ignored**. Format,
Clippy with warnings denied, career invariants and the seed scanner passed.
The live league smoke test also verifies 38 distinct persisted minute journals.
[Full gate output](experiments/NPC-WORKLOAD-V8-2026-10-06-gate.txt).

Release benchmark: seed 42, 26,835 matched genesis NPCs, a facility change at
week 26 for every tenth player. Three sequential runs, no simultaneous test
processes in the retained samples. Median CPU wall times:

| Operation | V7 individual life | V8 dated workload |
|---|---:|---:|
| Cold first-year projection | 413.162 ms | 497.672 ms |
| Advance one week | 16.924 ms | 18.087 ms |
| Repeat same date | 0.168 ms | 0.191 ms |

The changed 20-season world/market replay takes **11.09 s** median,
ending with 54,933 population rows and 567,609 exposure segments. Maximum measured
child peak RSS is **53,516 KiB** (about 52.3 MiB), including the full example's
projection and replay phases; it is not isolated incremental memory. The lifetime
projection phase stops at retirement. Same-date reads use existing numeric cache.

Reproduce: `cargo run --release -p goat-world --example bench_npc_workload`.
Native cloud Linux x86_64, Rust 1.90.0; timings are not Android/iPhone measurements.
[Run 1](experiments/NPC-WORKLOAD-V8-2026-10-06-run1.txt),
[run 2](experiments/NPC-WORKLOAD-V8-2026-10-06-run2.txt),
[run 3](experiments/NPC-WORKLOAD-V8-2026-10-06-run3.txt),
[resource measurements](experiments/NPC-WORKLOAD-V8-2026-10-06-resources.json).

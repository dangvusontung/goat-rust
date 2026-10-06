# Dated training and health — simulation version 6

Historical v6 report. Current NPC enhancement: [individual weekly training and health, v7](NPC-INDIVIDUAL-LIFE.md). The PC ledger and layout 22 remain current.

Implemented 2026-10-05 following the request to track both PC and NPC development,
with core realism before renderer work. Match opportunity rules remain version 5;
background development/replay changes require `SIM_VERSION = 6`. Save layout is 22.

## PC: factual history

`WorldState.pc_development_history` contains three append-only ledgers:

- Training/rest weeks: epoch day, starting age, focus attribute bitset, requested
  and effective intensity, effective training facilities/modifiers, energy before
  and after, injury weeks before and after, total attribute change.
- Match workload: epoch day, actual energy cost received by core, energy before
  and after, injury weeks before and after. Core's existing intent does not carry
  minutes, competition or medical diagnosis; none are fabricated.
- Health events: training injury onset, match injury, recovery. Recovery is dated
  at the end of the recovery week. A new injury records the remaining duration
  used by the existing model; injury type/severity and separate overlapping
  medical conditions are not introduced.

The existing calendar guard prevents duplicate training records. Rest and
summer back-fill weeks are recorded too. Logging adds no RNG draws and does not
change training rolls. `last_week_events` remains the transient display buffer;
it is not the persistent history. Weekly growth is stored as a total, not a
30-attribute snapshot or a full breakthrough/familiarity event archive.

Binary rows cost 38 bytes per week, 24 per match and 9 per health event, plus
16 bytes for the extension header/counts. A 20-year synthetic ledger containing
1,040 weeks and 1,040 matches costs **64,496 extra bytes**, before health events.
New saves restore the history and can continue identically. Old history is never
invented from the current snapshot. Earlier simulation versions remain refused
by the normal load guard; no gameplay migration is supplied. The explicit
layout-only parser remains available for migration tooling. Historical test
fixtures omit the new extension before truncating older fields; their expected
values are unchanged.

## NPC: dated expected exposure

The version-6 factory is `population::genesis_developed`. It preserves identity,
innate potential and creation RNG. Sparse exposure changes are stored in linked
columnar arrays, with one head per player. No full player object is stored per
background NPC. An exposure records its world week, facilities, estimated energy,
intensity, focus share, lifestyle, recovery modifier and preceding season's
league appearances as a workload proxy.

Before a change, balanced intake facilities apply. Transfer replay changes
facilities from the transfer epoch onward, preserving other training inputs.
Completed seasons update future workload exposure from appearances. The initial
energy assumption is 75; subsequent season estimates are
`max(55, 90 - min(apps, 60) / 2)`. Appearances are not actual minutes or congestion
measurements. Default focus share remains 0.700 from the earlier expected NPC
model; it is not a realistic, calibrated schedule of five named weekly drills.

Health shares use the **same injury probability** as PC, including fatigue,
intensity, age, lifestyle and the hidden innate trait. The mean recovery duration
averages all PC duration outcomes after the same integer physio scaling. For
onset probability `p` and mean recovery duration `d`, stationary healthy exposure
is `(1-p)/(1+p*d)`: the injury onset week also skips development. This is an
expectation, not a sampled sequence of diagnosed injuries. `expected_health_at`
exposes risk, expected duration and healthy share without exposing the hidden
trait. Promotion uses estimated energy and does not invent an active injury.
The existing temporary NPC lineup-unavailability sampler remains separate.

Development splits each intervention interval at annual age boundaries and
uses the common fixed-point weekly growth/decay laws, including healthy exposure
on decline. It reproduces the **weekly expected-plan reference**, not the full
stochastic PC trajectory (variance, breakthroughs, focused drills and injuries).
A later club cannot rewrite previously accrued development or reroll talent.
Youth entrants use their own intake epoch.

Ranking and lazy promotion use the same attribute calculation. Same-date cache
entries are explicitly invalidated on each accepted exposure revision. Forward
queries continue from cached attributes; older queries recompute from history.
Tests compare cold and incremental results, including age decline and changes
at the same date.

## Replay and compatibility

`ReplayCache::new_developed` and `orbit::rebuild_population_developed` now share
market/season reconstruction. The match population previously replayed seasons
without the market's transfers; the version-6 path reconstructs both, including
orbit appearance/goal credits and their batch remainder. The current TUI test
adapter uses the developed path. `genesis`, `genesis_shared`, `new_shared` and
`rebuild_population_shared` retain their earlier compatible models.

Production exposure changes are derived from deterministic replay, so NPC
segments are not copied into saves. Existing orbit credits remain persisted.
Direct `record_exposure` calls are headless interventions: callers must replay
those inputs themselves; this change does not add a saved arbitrary NPC policy
editor. Past path-dependent club interventions not represented by replay inputs
cannot be reconstructed solely from today's budget/facility snapshot.

Both transfer windows still use the existing season replay epoch, not distinct
weekly market dates. Current national test adapters use the developed genesis
factory but do not reconstruct the whole domestic transfer history. Full shared
weekly orchestration and a unified season clock remain follow-up work.

## Validation and cloud performance

Run the gate with `bash scripts/test.sh` after activating the repository toolchain.
Final gate: **558 passed, 0 failed, 1 ignored**; formatting, clippy, career
invariants and seed scanner all passed. Coverage includes factual injury onset/recovery, unchanged training RNG, calendar
guards, full byte save/load/continuation, truncated/corrupt history counts,
24-year weekly projection equivalence, facility interventions, immutable innate
potential, youth intake, ranking/promotion agreement and market/orbit rebuild.
A 200,000-week seeded health reference per physio setting checks stationary
availability within one percentage point at a fixed risk/energy/age.

Release reproduction: `cargo run --release -p goat-world --example bench_history`.
Seed 42; 26,835 initial NPCs; 10% receive a facility change at week 26. One cloud
run, not a mobile benchmark or distribution of latency measurements:

| Workload | Frozen constant plan | Dated plan with incremental cache |
| --- | ---: | ---: |
| Week 52 first read | 118.30 ms | 158.33 ms |
| Week 53 after caching week 52 | 129.31 ms | 27.24 ms |
| Week 1040 after the earlier reads | 333.39 ms | 304.03 ms |
| Same-date cached read, week 53 | 0.093 ms | 0.086 ms |

The sequential 20-season world replay fell from **9.31 s to 3.38 s** after
incremental projection, with identical final population/segment counts and
benchmark OVR sums. It ends with 54,955 identities and 99,579 exposure segments;
retired identities remain in the existing population. Benchmark process peak RSS
was **21,216 KiB** (about 20.7 MiB), measured using Linux child-process resource
usage; this excludes a renderer, app bridge and actual device constraints.

[Raw timings](experiments/DEVELOPMENT-HISTORY-V6-2026-10-05.txt) and
[process resources](experiments/DEVELOPMENT-HISTORY-V6-2026-10-05-resources.json).
Cold rebuilds remain too large for a mobile UI frame. Retain the session replay
cache and execute simulation off the UI thread. The TUI currently reconstructs
completed seasons for each league match; its repeated cold rebuild cost is not
removed by this core implementation. Device benchmarks and session cache
integration are the next performance task. The 150-country expansion remains
separate and deferred.

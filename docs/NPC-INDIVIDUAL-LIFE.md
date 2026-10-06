# Individual NPC training and health — simulation version 7

Implemented 2026-10-06. The version-6 expected background health model remains
available, while the current model samples individual NPC weeks after world
entry. PC weekly behavior and the match opportunity kernel are unchanged.
`SIM_VERSION = 7`; save layout stays 22. Existing v6 saves are refused by the
simulation guard; no migration is supplied.

## What is simulated

Each NPC starts individual simulation at `intake_week` (0 for genesis players).
Older genesis players retain expected pre-entry attribute development; no medical
history before world entry is invented. Innate potential and identity draws are
unchanged. Simulation stops at the existing retirement age of 38.

Healthy weeks train at most five unique named attributes: three position-specific
drills plus rotating general drills. Defender preferences are StandingTackle,
Marking and Interceptions; midfield preferences ShortPassing, Vision and
BallControl; forward preferences Finishing, AttPositioning and Composure.
The legacy exposure `focus_share` requests a number of drills, rounded upward
from `30 * share` and capped at five; it no longer makes all 30 attributes gain
0.700 of a training session every week. This is a simple policy, not a manager
or role-specialist planner.

Energy accumulates, rather than resetting to the annual estimated value. Shared
PC training costs, passive recovery, exhaustion downgrade and injured recovery
apply. Match workload drains `apps * 20 / 52` energy per week, where `apps` is the
preceding season's recorded appearance count. The 20-energy assumption and uniform
weekly distribution are **proxies**, not actual minutes or fixture congestion.
The exposure's energy estimate initializes entry state; later energy estimates do
not teleport a player's accumulated energy.

Health uses the PC injury probability, hidden innate trait and integer-scaled
physio duration. Onset skips training; remaining recovery weeks decrement by one,
recover energy and skip growth/decline, matching the PC recovery law. Separate
RNG streams are keyed by player seed and absolute week for health and training
variance. Query order, cache hits and other players cannot consume these streams.
A physio/facility change affects subsequent weeks; it does not reroll an earlier
injury or shorten the duration of one already in progress.

Healthy development uses PC growth, variance, commitment/lifestyle ceilings and
age decline. Lifestyle growth modifiers are included. NPC breakthroughs and
weekly familiarity/role XP are not implemented by this path; the conditional
healthy growth reference test prevents claiming equivalence to the whole PC loop.

## History and match consequences

`Population::training_history(index, from, to)` replays concrete weeks with focus
bitsets, effective intensity, energy before/after, injury remaining before/after
and onset flags. Dates use half-open world-week intervals. `injury_history`
returns onset, duration, predicted return date and an actual recovered date only
when the requested history has reached recovery before retirement. A recovery
week is onset + 1 + duration, matching PC's onset-plus-recovery convention.
No injury type, severity or anatomical diagnosis is invented.

Promoted views carry actual simulated energy and injury remaining. Current
lineup and formation selection exclude injured NPCs. PC selection uses those
same injuries instead of the separate weekly outage roll for v7. Version 6 and
older paths retain their original sampler.

Season appearance quotas decrease with the fraction of healthy weeks during the
preceding 52-week interval. Batch goal-attribution weights also reflect that
availability. Orbit credits remain real recorded appearances/goals, and the batch
only fills their remainder. This is still a season abstraction: exact fixture-date
availability, actual match injury moments and individual minutes are not simulated
by the season batch resolver.

## Replay, storage and cache

Current factories are `genesis_lived`, `ReplayCache::new_lived` and
`orbit::rebuild_population_lived`. Earlier factories remain compatible. The TUI
league test adapter and its direct national population factories use v7; national
factories still lack full domestic market reconstruction. The TUI remains an
adapter, with no new renderer screens in this change.

No per-NPC heap player object or permanent full weekly journal is stored. Numeric
attribute caches are paired with small health state and a rolling 52-bit healthy
week mask. History is replayed only when requested. Exposure changes retain a
cached historical prefix if its date is at/before the change; caches containing
later affected weeks are invalidated. Entry-state changes invalidate initialization.
The healthy-week mask supplies season availability without replaying each whole
career for every quota calculation.

Production exposure inputs are reconstructed by the existing market/season/orbit
replay. NPC episodes and training variance therefore add no save rows. Arbitrary
headless interventions still require the caller to replay their inputs; an editor
for persisted NPC medical/training overrides is not introduced. The existing two
transfer windows still share the season replay epoch. Retired identities and
sparse annual exposure segments remain in the population.

## Validation

`bash scripts/test.sh`: **564 passed, 0 failed, 1 ignored**. Formatting, clippy,
career invariants and seed scanner passed. Frozen expected values were not changed.
New coverage checks unique/rotating drills; no growth or decline during injury;
conditional healthy energy/growth against the PC weekly loop; individual onset
and recovery; injured lineup exclusion; season quotas against replayed weekly
records; cold/incremental/backward query agreement; unchanged past under dated
interventions; immutable potential; and market/orbit reconstruction of individual
history. The existing byte save/load and PC-history checks remain green.

## Performance

Release reproduction:
`cargo run --release -p goat-world --example bench_npc_life`.
Three direct binary runs on cloud, seed 42, same initial 26,835 identities and the
same facility intervention for 10% of players at week 26 in both models. Medians:

| Workload | v6 expected model | v7 individual model |
| --- | ---: | ---: |
| First population ratings, week 52 | 156.53 ms | 405.39 ms |
| Week 53 after cached week 52 | 27.36 ms | 16.39 ms |
| Same-date cached read, week 53 | — | 0.188 ms |
| Jump from week 53 to 1040 | — | 3,473.25 ms |

V7 sequential replay of 20 whole-world seasons takes **9.21 s** median. It ends
with 54,941 identities and 497,086 sparse exposure segments. Maximum native child
process RSS across these runs is **42,920 KiB**, about 41.9 MiB, excluding renderer
and device-specific overhead. The earlier v6 whole-world report measured 3.38 s;
that historical run evolves a different world under different rules.

The first v7 implementation took 58.54 s for 20 seasons. Retaining unaffected
cache prefixes and rolling availability reduced this to about 9.2 s while
preserving the v7 benchmark OVR sums, population and segment counts. Cold long
career jumps remain expensive; do not run them on a mobile UI thread. These are
cloud timings, not ARM/device certification. Session-cache integration and real
mobile measurements remain the next performance task; the current TUI still
rebuilds completed seasons for each league match.

Raw measurements: [run 1](experiments/NPC-LIFE-V7-2026-10-06-run1.txt),
[run 2](experiments/NPC-LIFE-V7-2026-10-06-run2.txt),
[run 3](experiments/NPC-LIFE-V7-2026-10-06-run3.txt),
[resources](experiments/NPC-LIFE-V7-2026-10-06-resources.json).
The 150-country expansion remains a separate deferred task.

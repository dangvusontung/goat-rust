# Runtime journals and reactive NPC matches — v13

Subsequent storage work: [local resume checkpoint](LOCAL-RESUME-CHECKPOINT.md),
save layout 28 / unchanged SIM13. The tables below remain the journal-only baseline.

Implemented 2026-10-06. Save layout 27, simulation version 13. Core roadmap
milestones 2 and 3 are implemented for the current dated league simulation.
The top-five-plus-PC deep budget and deferred 150-country task are unchanged.

## Exact medical/history storage (milestone 2)

The v12 medical model is retained: dated PC recovery uses elapsed time; sampled
NPCs share the training/health model and expose observed injury recovery and a
two-week return phase. A cached last-return week avoids replaying an entire medical
history every time a manager selects a player. Weekly injury generation remains an
approximation; the match model does not add a second independent injury roll.

The shared `goat_core::journal` codec losslessly packs fixture dictionaries, player
identity deltas, minutes and credits. Completed-season snapshots in the retained
session now use these bytes. Only current-season minute and credit inputs remain
as expanded cache copies. Past inputs are compared by a streaming exact decoder,
not a hash or approximate career totals. A changed completed input rebuilds the
canonical population. Existing dated/legacy replay APIs retain their old semantics.

Population workloads store one fixture/date/competition dictionary and compact
player row indices, minutes and per-player sorted lookup indices. Duplicate lookup
is binary search; planned fixture dates use a retained map instead of scanning all
historical rounds. Youth intake counts club membership and goalkeeper coverage in
one pass rather than rescanning the entire population for each club. No RNG draws
or legacy player identity fields were changed by this optimization.

This is the compressed-runtime alternative in the checkpoint task. It does not
persist a derived population snapshot: identity, annual training, transfers,
managers, promotion and coefficients still reconstruct from seed plus authoritative
journals. Cold resume still replays annual history. WorldState keeps full canonical
minute/credit vectors; they cannot be discarded without a further journal-backed
API. RAM and cold time therefore still grow with career length.

## Reactive NPC football (milestone 3)

New dated TUI careers enable the versioned realistic NPC model. Headless callers
use `Intent::EnableRealisticNpc` before their first deep fixture, or construct an
explicit realistic state. Existing explicit legacy factories remain unchanged.
The autonomous NPC engine simulates 18 five-minute intervals through minute 90,
including when the PC is absent or suspended.

- Select eleven named players, including a keeper, by current ability, fitness,
  injury availability, return phase and suspension. Derive formation/style from
  the club tactical profile. Missing roles use available backups.
- Energy decreases with minutes, stamina and pressing; actual earlier fixtures
  in the same week contribute congestion before kickoff. Returning starters are
  penalized and replaced around minute 30 when a legal backup is available.
- Up to five substitutions in three non-halftime windows, plus halftime. Normal
  opportunities are 45/60/75/85; return management and keeper dismissal can use an
  earlier window. A fit player above energy 75 stays on unless return management
  or a late score-dependent tactical change warrants replacement. Used players
  cannot reenter. Losing sides prefer extra attacking
  cover; leading sides prefer defensive cover.
- Chase a deficit from minute 55 (attack +10%, midfield +3%, defense -10%);
  protect a lead from minute 65 (attack -8%, defense +8%). These are tunable
  integer heuristics, not a spatial tactical engine.
- The shared possession/chance/conversion kernel receives the actual active lines
  and manpower. Goals and assists belong to players present at that interval.
  Every squad member gets a workload observation, including explicit zero DNPs.
- A dismissal removes the player immediately for remaining intervals. Team minutes
  total 990 minus each dismissed player's remaining minutes. Scores continue to
  full time. A keeper dismissal substitutes a backup keeper for an outfielder if
  capacity permits; otherwise an outfielder becomes the emergency keeper.

### Discipline

Typed card events persist season, competition, fixture, date, player, minute and
kind. First yellow, second-yellow dismissal and direct red are distinct. The
current league rule is five accumulated yellows for one missed fixture, a second
yellow for one, and direct red for three. Other competition IDs currently use a
two-yellow threshold; actual dated cup/national fixtures belong to milestone 4.
These are explicit game rules, not universal real-world regulations. The current league
layer uses one league competition ID across its generated divisions; cross-country
regulatory jurisdiction is not modeled. Distinct dated competition identities and
rule tables are part of milestone 4.

A ban is served by the player's club fixtures in the same competition, including
DNP/light matches and actual date overrides. Days off and summer do not serve it.
Outstanding bans carry over a season boundary; accumulated yellows reset. The
first caution in a second-yellow dismissal is rescinded from the accumulation,
avoiding a simultaneous threshold ban for that same caution. Cup cards do not
serve a league ban. The generated league integration indexes current/previous
season events rather than scanning every player's full card journal at selection.

### Goalkeeper coverage and limits

Each initial club has two deterministic keeper specialists; youth intake replenishes
that coverage. This flag is derived and does not consume extra RNG. Keeping strength
is an explicit proxy: 50% reactions, 20% agility, 15% jumping, 15% composure.
It contributes 20% to defense; an emergency keeper uses 60% of the proxy.
The legacy outfield position remains for compatibility, and the aggregate market
still recruits by those outfield roles. A keeper-aware dated market belongs to
milestone 5. Specialized handling,
positioning/diving attributes and a playable PC goalkeeper career are deferred.

The new engine covers autonomous deep league fixtures. The interactive PC beat
engine still uses its existing NPC/abstract-keeper path; it has not acquired this
new per-NPC card/substitution journal. Full competition calendars, dated cup
continuation, that adapter integration and transient opponent deepening are the
next milestone. Light fixtures retain aggregate behavior. Match injury events,
extra time, stoppage time, VAR, and administrative termination below seven players
are not implemented by this five-minute NPC engine.

## Compatibility and verification

Layout 27 persists the optional NPCD model/card extension; minute and credit
packing remains lossless. The loader bounds counts, rejects invalid model flags,
card kinds/minutes, unordered/duplicate cards and truncations. SIM12 and earlier saves are rejected by the
normal loader; no silent simulation migration is provided. Layout-only diagnostic
inspection and frozen legacy goldens remain available.

Tests compare retained and fresh replay across a new season; changes to completed
minutes/credits force rebuilds. Realistic saves replay the next fixture exactly,
including cards, minutes, credits, scores and career counters. Match checks cover
minutes and scorer conservation, sub/window limits, no banned participation,
return load, keeper dismissal, score-dependent roles and strength/keeper effects.
Medical/training/career equality is checked after native cold resume.

Native performance and statistical calibration are recorded below.
These x86_64 measurements do not establish ARM64/mobile throughput. The current
population and deep budget are held fixed; 150 countries remain a separate task.

## Statistical calibration

Release fixture-only diagnostic: five configurations, 20,000 seeds each (100,000
matches total). Home squad rating 70, energy 90, stamina 70; the two seeded club
profiles stay fixed. This is a controlled sensitivity check, not a fitted dataset
of real league matches. Card-disabled cases isolate the availability/line effects.

| Away condition | Home goals/match | Away goals/match | Home win % |
| --- | ---: | ---: | ---: |
| Equal squad, cards enabled | 1.4462 | 1.4005 | 38.58 |
| Rating 40, cards disabled | 2.3466 | 0.7335 | 74.26 |
| Energy 35, cards disabled | 1.7238 | 1.1200 | 51.80 |
| Keeper proxy 1, cards disabled | 1.6057 | 1.4060 | 42.44 |
| Equal squad, cards disabled | 1.4470 | 1.4070 | 38.44 |

Equal squads produce 2.8467 total goals/match, 25.285% draws and 4.645% scoreless
matches. There are 2.7233 first cautions and 0.1948 dismissals/match, including
0.03725 direct reds; second-yellow dismissals are counted separately from first
cautions. Substitutions average 8.00005 across both teams in this configuration.
A fresh high-stamina player is tested to stay on at minute 60 without a tactical
reason. These directional effects are accepted as the first calibration; card
rates, tactical heuristics and keeping proxy still need empirical league targets.
[Raw calibration](experiments/NPC-MATCH-V13-2026-10-06-calibration.txt).

## Native measurement procedure

Build with `cargo build --release -p goat-save --example bench_journal -p goat-world
--example bench_npc_match`. Run `target/release/examples/bench_journal --years N
--realistic --final-codec` separately for N=3,10,20. This seed-42/base-2023 scenario
has six deep leagues (top five plus PC league 59); the PC is absent, so every fixture
uses autonomous NPC resolution. It excludes the interactive PC beat engine, renderer
and dated cup/national fixtures. All elapsed league rounds actually run.

`--final-codec` delays serialization until the last season. The logged
`season_N_before_codec` VmRSS/VmHWM therefore describes one simulation session plus
canonical WorldState, including allocator high-water marks from normal reducers,
and excludes diagnostic SaveData/decoded clones. The following phase serializes and
decodes with SaveData/base/decoded clones, reports exact row equality, and drops
temporary clones. The cold phase adds a second session and loaded WorldState,
compares career fingerprints and full training histories/medical readouts for
indices 0,100,1000. These are genesis players; by season 20 those samples are
retired, so this sample is not an exhaustive check of active youth medical state.
The new-season integration test additionally checks active youth training/medical
state, keeper identity and subsequent
fixture equality across rebuilt and retained populations. These
three memory phases must not be mistaken for the same gameplay footprint.

VmHWM is cumulative for that process; a lower current VmRSS does not reset its peak.
Run through `python3 scripts/measure-native.py OUTPUT COMMAND ARGS` to report
per-child `wait4` peak RSS, CPU and wall time independently. This avoids inherited
or cumulative `RUSAGE_CHILDREN` peaks when a shell executes its final command.
The earlier 3/10/20 logs also include each child’s own `/proc/self/status` peaks.
Runs are single samples on x86_64 Linux, not performance guarantees or ARM64 results.
The compact minute/credit payload is compared with the exact old field widths for
the same rows; card/model/score data remains in whole-save size. Runtime compressed
snapshots do not replace annual reconstruction, and the remaining long-career cold
replay cost is explicitly reported.

## Long-career storage results

| Seasons | Minute rows | Credits | Cards | Old minute/credit bytes | Compact minute/credit bytes | Whole save bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 3 | 364,762 | 218,813 | 19,872 | 10,294,879 | 2,014,304 | 2,750,980 |
| 10 | 1,352,496 | 729,472 | 66,355 | 37,595,856 | 8,153,650 | 10,609,912 |
| 20 | 3,211,988 | 1,458,988 | 132,180 | 87,359,916 | 19,002,386 | 23,900,138 |

At 20 seasons, the same-row minute/credit payload is **78.25% smaller** than the
old fixed-width representation. Full save size is about 22.79 MiB. Every round
actually executes; this is not a synthetic repetition of one season's rows.
SIM13's eleven named slots and substitutions produce different journal counts
than frozen SIM12's ten named outfield slots, so cross-model row counts are not
used as a compression comparison.

## Native time and memory results

| Seasons | One-session resident MiB before codec | One-session peak MiB before codec | Diagnostic peak MiB with codec/second session | Encode ms | Decode ms | Cold rebuild + career comparison s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 3 | 54.18 | 59.13 | 84.93 | 13.175 | 9.299 | 1.561 |
| 10 | 184.05 | 184.05 | 252.15 | 57.835 | 34.720 | 43.254 |
| 20 | 338.56 | 387.11 | 581.74 | 158.476 | 83.419 | 175.884 |

All round trips and retained/cold assertions passed. The complete 3/10/20-season
simulation-plus-final-codec phases took 14.013/180.201/759.571 seconds respectively;
these are cumulative whole-career runs, not the latency of one fixture. No claim
of instant long-career/mobile resume is made. Native fixture-only calibration is
cheap; canonical journal processing and historical/annual player progression still
scale with history. Further profiling must separate those costs before optimizing.

The same frozen football/three-season journal scenario as v12 was rerun with the
runtime optimization: 364,306 minute rows and 177,422 credits, identical 1,845,282-byte
compact payload. Its two-session diagnostic peak was about 80.97 MiB by child
VmHWM versus the earlier v12 141.88 MiB sample; cold comparison was 1.555 seconds
versus 3.085 seconds. This controls row/model differences when evaluating runtime
storage, while the main table above measures the new realistic model. These are
single cloud samples; they do not prove a stable speedup on another machine.

The compressed-journal implementation completes milestone 2's storage alternative,
but **a 176-second cold replay and 387 MiB single-session peak at 20 seasons are
not a mobile acceptance gate**. An [exact derived resume checkpoint](../tasks/TASK-CORE-DERIVED-RESUME-CHECKPOINT.md)
is recorded as a capacity/mobile follow-up. Avoid increasing the deep budget or
integrating 150 countries before that work and native ARM64 measurements.

Raw runs: [3 seasons](experiments/CORE-V13-2026-10-06-native-3.txt),
[10 seasons](experiments/CORE-V13-2026-10-06-native-10.txt),
[20 seasons](experiments/CORE-V13-2026-10-06-native-20.txt),
[frozen-model comparison](experiments/CORE-V13-2026-10-06-legacy-comparison.txt),
[structured metrics](experiments/CORE-V13-2026-10-06-metrics.json).

## Final gate

Final workspace: **618 passed, 0 failed, 1 ignored**. Formatter,
Clippy with warnings denied, career invariants and the seed scanner passed.
All frozen legacy expectations remained unchanged. The final gate includes the
new-season keeper/active-youth/next-fixture comparison and malformed card tests.
[Full gate](experiments/CORE-STEPS-2-3-V13-2026-10-06-gate.txt).

Next core milestone: [4 — dated competition calendar](../tasks/TASK-CORE-DATED-COMPETITIONS.md).
Interactive PC adapter integration remains in that milestone; long-career resume
checkpoint is subsequently implemented in [layout 28](LOCAL-RESUME-CHECKPOINT.md);
native ARM64 certification remains capacity/mobile work. Changes are local;
no commit or push was requested for this implementation.

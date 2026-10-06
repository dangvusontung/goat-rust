# Task — Deep simulation and light simulation

Status: step 1 implemented 2026-10-06; steps 2–5 pending.
Report: [retained simulation session](../docs/DEEP-LIGHT-SESSION.md).
Core first; TUI is a testing adapter. Follow DESIGN_BIBLE §7.1.
The 150-country registry remains a separate task; 200,000 NPCs is a capacity
scenario, not an approved identical league pyramid for every country.

## Scope and existing behavior

Deep sim: the PC's played fixture runs the named-player match engine, including
substitutions, full-time continuation after a PC dismissal and observed minute
journals. Other fixtures in the current division already have cheaper team-level
round simulation; they are not all full beat-engine matches.

Light sim: distant leagues use seasonal results, standings, individual credits,
market/economy, promotion/relegation and youth intake. V8 dated NPC health and
training are queried lazily and cached, but first-time projection still iterates
individual weeks and season aggregation can request many players. Existing tiering
does not yet establish a 200k mobile performance budget.

## Desired boundary

1. The PC fixture receives full match-engine simulation. Current club/league and
   imminent cup/national opponents receive fixture-level treatment as needed.
   Team-level round simulation remains a cheaper option within the orbit.
2. Rivals and scouting/transfer candidates receive detailed attributes and health
   on contact. Merely viewing a player must not require simulating all his matches
   or reroll any already established result.
3. Distant leagues retain season-level processing and lazy individual realization.
   They still produce coherent players, records, transfers, injuries and growth.
   Do not advance the entire population with a mandatory weekly full-world pass.
4. A headless policy selects the active orbit from stable IDs, date and explicit
   career events. Rendering, wall-clock timing and query order cannot decide
   football outcomes. Bound the active set; numerical budgets follow measurement.

## Transition invariants

- Player identity, talent, club membership, elapsed development and established
  medical/career history survive light-to-deep and deep-to-light transitions.
- Promote by reconstructing canonical state at the date; never generate a fresh
  player. V8 generated minutes remain labeled as estimates rather than turning
  into fictitious observed engine minutes.
- Previously established match results and credits are immutable. Detailed future
  simulation can differ from a hypothetical light run; it must preserve accounting
  and use the same football laws, not promise identical outcomes at both depths.
- One fixture/player credit is applied once. Observed minutes replace its generated
  dose, including zero-minute DNPs and rescheduling; seasonal remainder subtraction
  prevents duplicated appearances/goals. Relevant cup/national doses affect health.
- Cache/fresh replay and save/load agree for the same model, inputs and tier decisions.
  Changing a past authoritative input invalidates affected projections.
- A single chronology must reconcile the existing 364/365-day season clocks before
  expanding to heterogeneous international schedules.

## Implementation order and acceptance

1. Retain the world/population session cache across rounds. Extend the existing
   ReplayCache through a renderer-independent API; see the mobile performance task.
2. Unify chronological mapping and implement/test stable orbit transitions, using
   named fixture identities and existing sparse observation/history storage.
3. Improve background lineup/rotation and minute accounting: eleven-player/team
   minute conservation where applicable, injured exclusions, substitutions and
   non-league exposure. Independent per-player V8 plans are the current baseline.
4. Benchmark a clearly labeled 200k-capacity scenario before 150-country integration:
   cold resume, weekly active-orbit progression, season boundary, peak memory and
   long-career restore; separate light cost from detailed active-set cost.
5. Verify a transfer into/out of the orbit, scouting-only access, opponent promotion,
   cup/national overlap, past moved fixture, youth intake, retirement and save/load.
   Reading/scouting in a different order must not change the world. Keep RNG/fixed
   golden expectations and version changed behavior explicitly.

Measure native ARM64 before claiming mobile support. No new dependencies, no
renderer rules or per-NPC heap objects. Persist only path-dependent decisions and
observations; derive background state. Country expansion stays deferred.


Owner calendar decision: [season chronology](../docs/SEASON-CHRONOLOGY.md),
15 August–30 June; 1 July–14 August summer rest/national/friendly activity.
Step 2 must implement explicit season frames and continuous medical/training days,
not replace a 364 constant with 365 and retain a 52-week rollover.

Implementation checkpoint (2026-10-06): step 1 retained session is complete.
Step 2 chronology is implemented in v9, including calendar cache keys and save/load;
automatic stable orbit selection/transitions remain pending. Steps 3–5 remain open.
See the chronology document for measurements and remaining adapter limitations.

## Owner refinement — deep league scope (2026-10-06)

The permanent deep set includes all NPCs in the PC's current league, plus NPCs
in the highest-ranked leagues. Remaining leagues use light simulation; imminent
cup/national opponents can still receive temporary detailed treatment.

The number of highest-ranked leagues is configurable (proposed initial budget:
5, pending owner choice). Deduplicate the PC league if it is already in that set.
Resolve equal ranking scores by stable LeagueId. Refresh membership at season
opening and immediately after a PC transfer; preserve player histories across
transitions. Ranking should eventually use multi-season competition coefficients;
current static nation power is only a bootstrap proxy, not a live league ranking.
This policy is recorded here; automatic selection and execution are not yet wired.

## Implemented refinement — v10

Permanent automatic league scope is now wired headlessly and into dated TUI play:
PC league plus top 5 bootstrap-ranked leagues, with deduplication and dated transfer
transitions. Named fixture simulation, health-aware selection, substitutions, DNP
loads and authoritative season score replay are implemented; save layout 25/SIM10.
See [deep league implementation](../docs/DEEP-LEAGUES.md).

Step 2's permanent league selection/transitions are complete. A results-based
coefficient table and transient cup/national opponent policy remain open. Step 3 has
an initial team-conserved 10-outfield-slot implementation; explicit keepers, NPC
suspensions and tactical/substitution refinement remain open. Steps 4–5's 200k/ARM64
capacity checks and broader international overlap scenarios remain pending.

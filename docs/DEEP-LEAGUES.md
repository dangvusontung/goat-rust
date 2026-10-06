# Ranked deep leagues — simulation version 10

Latest: [v13 reactive NPC matches and compact runtime journals](CORE-STEPS-2-3.md).
The abstract keeper and fixed substitutions described below are the frozen v10
model, retained for explicit legacy APIs. New dated careers enable the v13 model.

Update: [v11 rolling coefficients](LEAGUE-COEFFICIENTS.md) replaces the bootstrap
ranking after completed seasons. The v10 implementation and measurements below
remain the historical checkpoint.

Implemented 2026-10-06. The owner selected the PC's league and the highest-ranked
leagues as the permanent deep set. The initial budget is **top 5 plus the PC league**;
if the PC league is in that top 5, it is counted once. All other leagues remain light.
The 150-country expansion stays deferred.

## Selection and transitions

`goat_world::deep::select_leagues` accepts league scores and an explicit top-N
budget. Scores sort descending, equal scores use stable LeagueId, and the PC league
is mandatory. The active adapter uses `DEFAULT_TOP_LEAGUES = 5`.
Bootstrap scores use the generated world's seeded national stature and division
level. They represent strength in this generated universe, not real-world UEFA
coefficients. A multi-season results-based coefficient system is still pending;
the selection API already accepts its future score table.

`SimulationSession::advance_deep` is headless. It establishes a scope at season
preparation, refreshes on a PC league change and only resolves elapsed fixture dates.
Joining a league mid-season does not reroll its earlier light fixtures.
Historical scope spans keep continuously selected leagues caught up even when a
transfer occurs between two advance calls. Dated scope
events record both PC club and league; established scores, individual credits and authoritative minutes survive
leaving the orbit and save/load. Scouting/query order does not select tiers.
A suspended/academy PC leaves his club's NPC fixture running through the separate
`advance_deep_without_pc` entry point.

## Detailed NPC fixture resolution

Selected clubs use their replayed season membership, with the PC nation's live
membership overlaid. Selection filters injury and retirement and ranks by current
attributes, accumulated form and energy. A balanced 4-3-3 is filled from available
backups when a position group lacks players. The existing engine models an abstract
keeper: **10 named outfield slots plus the keeper**. NPC minutes conserve those
10 slots; the abstract goalkeeper has no individual workload row.

The fixture uses the shared chance/conversion kernel and health-adjusted team lines,
with independent seeded streams per league/round/club pairing. This is full-match
fixture resolution, not an interactive beat sequence for every NPC. Three scheduled
substitution opportunities (60/70/80) replace tired starters with available backups
of the same position. Named scorers are weighted by position, ability and minutes;
assisters must overlap the scorer's time on the field. Explicit zero-minute DNPs
replace the background workload proxy. Only participants receive an appearance.

Scores feed both the PC's live table and season replay: standings, champions,
manager match points and promotion cannot overwrite played games with light scores.
The live dated season closes through `finish_deep_season`, using the same promotion
and membership as cold replay; the next deep schedule uses that canonical membership.
Missing fixtures retain the existing light RNG stream. Aggregated round credits
merge by player ID; repeated calls do not apply a second appearance or goal.
The retained session accepts sorted minute appends and same-round credit additions
without a cold rebuild. Changed past authoritative data still invalidates replay.

Normal dated minute observations preserve cached medical/development state before
the affected week. A reschedule uses the earliest of its original and actual dates;
it invalidates projections that already consumed the original planned fixture.

## Save and remaining realism work

Save layout **25**, simulation version **10**. Scope events and detailed scorelines
are a tagged optional tail; existing NPC minute and orbit journals carry named
participation. Incompatible older simulation saves are rejected; no migration is
implemented. Legacy factories and explicit `--legacy-calendar` scenarios remain
available with their original golden expectations.

Detailed fixture journals grow with the selected leagues and elapsed seasons.
This implementation does not yet compact old authoritative minute/credit records.
The prototype still has an abstract keeper, scheduled rather than score-reactive
NPC substitutions, weekly clinical events and no full NPC competition suspension
ledger. Annual NPC transfers remain aggregate passes. Cup/national calendar and
persistent tournament progress, transient opponent policy, team tactics refinement,
200k capacity and native ARM64 measurements remain separate work.
Web/bridge adapters have not been migrated to the dated deep loop.

## Verification and measurement

Coverage includes deterministic ranking ties and fixed-seed scores, mandatory PC
league/deduplication, elapsed-only fixture processing, injury exclusions, minute and
goal conservation, idempotent fixture execution, transfer without past rerolls,
cache/fresh medical equivalence, rescheduled past loads, season table replay,
transfer between advances and canonical next-season promoted membership,
save/load continuation and the live TUI's ranked-league fixture execution.

Native release measurements use seed 42, year 2023, PC league 59 and six leagues.
`cargo run --release -p goat-world --example bench_deep_leagues` resolves all selected
NPC league fixtures, treating the PC as absent. It excludes the PC beat engine,
rendering, save serialization and international/cup fixtures. The boundary includes
the full world's annual replay, market, promotion and youth intake.

Raw measurements: [run 1](experiments/DEEP-LEAGUES-V10-2026-10-06-run1.txt),
[run 2](experiments/DEEP-LEAGUES-V10-2026-10-06-run2.txt),
[run 3](experiments/DEEP-LEAGUES-V10-2026-10-06-run3.txt),
[resource observations](experiments/DEEP-LEAGUES-V10-2026-10-06-resources.json).
The peak RSS observation is cumulative across the three sequential child runs.
Full checks: [workspace gate](experiments/DEEP-LEAGUES-V10-2026-10-06-gate.txt).
This is a desktop baseline, not proof of mobile performance.

Design coverage: DESIGN_BIBLE §7.2/§7.3 tiered world and lazy promotion, shared match
kernel, the owner deep-scope refinement and v9's continuous season chronology.

Three-run release baseline: **2,280 fixtures**, 113,088 authoritative dose rows,
median round **27.782 ms**, slowest observed round **57.787 ms**, full season
median **1.070 s**. Annual boundary median **0.773 s**; next population 29,235 rows.
The retained world rebuilt once (cold initialization). Maximum observed child RSS:
**35,888 KiB (~35.0 MiB)**. A round here means ten fixtures in each of six leagues,
not a single match or a full PC play session. Explicit minutes alone occupy about
2.6 MiB in the current save encoding for this season; journal compaction remains open.

Final gate: **591 passed, 0 failed, 1 ignored**. Formatting, Clippy,
career invariants and seed scanning all passed. Frozen golden expectations unchanged.

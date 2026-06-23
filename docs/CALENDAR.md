# Calendar Simulator — Core Spec

**Project:** BECOME THE GOAT
**Scope:** Headless core only. No renderer, no UI, no art. The time-orchestration spine.
**Status:** Design-locked. Numbers are illustrative placeholders.

---

## Overview

The Calendar Simulator is the **time-orchestrator** of the headless core. It is not a feature the player sees directly — it is the clock that drives the entire simulation. Its single job: decide what happens on the next in-game day, run every subsystem that has something to do, and interrupt the player *only* at moments that matter. Every other system (training, match, transfer, media, life, injury, international) registers as a **listener** on this clock.

Two pillars meet here mechanically. §2.2 (manage-by-exception) is implemented by the flashpoint arbitration loop. §2.3 (seed-determinism) is implemented by injected RNG and ephemeral, regenerated fixtures.

Design stance, locked: **the day is the tick, but the week is the player's loop unit.** The core advances one day at a time (so fixture congestion, energy recovery, and suspensions count exactly), but the player is only woken at flashpoints. A normal day ticks silently.

This spec assumes the three decisions already made: **day-tick granularity**, **multi-competition with conflict resolution**, and **manage-by-exception advance**.

---

## Assumptions

- `[ASSUMED]` Every `Date` in the core is an **in-game date** (epoch = save's start day), never wall-clock. The core must never call `DateTime.now()` — that violates §9 (determinism is sacred).
- `[ASSUMED]` A "season" is a football season frame (e.g. Aug→May) plus transfer windows and an off-season gap — not a calendar year.
- `[ASSUMED]` A year is a fixed **365 days** in-game. No leap years, no real-weekday mapping. The player does not care whether a match is on a Tuesday; they care about gaps between matches. This keeps arithmetic deterministic and trivial. `[DECISION NEEDED]` confirm.
- `[ASSUMED]` The calendar deep-ticks day-by-day for the **orbit only** (player's club + competitions). The outer world batch-ticks at season granularity (§7.1) and is reconciled at season boundaries.
- `[ASSUMED]` Multi-competition = league + 1–2 domestic cups + (nationality-dependent) continental + international windows. All orbit-relevant fixtures merge into **one** unified timeline.
- `[ASSUMED]` Suspensions count by **matches of that specific competition**, not by days — surviving fixture reschedules.

---

## User Stories

### Epic: Time advancement

```
US-01: Advance through dead time
As a player,
I want to press "advance" and have the game skip days where nothing needs me,
So that I don't click through 365 days a year to reach the moments that matter.
Priority: P0 | Size: L

US-02: Preview the week ahead
As a player,
I want to see what the upcoming week holds (fixtures, known events) before I commit,
So that I can set my training routine and rotation with full information.
Priority: P0 | Size: M

US-03: Skip a long stretch safely
As a player,
I want to skip a long period (e.g. a 6-week injury layoff),
So that I don't manually advance through recovery — but I still get stopped if an unexpected flashpoint fires.
Priority: P0 | Size: M
```

### Epic: Fixture orchestration

```
US-04: Unified fixture view
As a player,
I want all my competitions (league, cups, continental, international) merged into one schedule,
So that I see a single timeline of what's next rather than juggling separate calendars.
Priority: P0 | Size: M

US-05: Congestion warning
As a player,
I want the game to flag when fixtures pile up (e.g. 3 matches in 8 days),
So that I can plan rotation and rest before I burn out or get injured.
Priority: P1 | Size: S
```

### Epic: System orchestration (system-as-actor)

```
US-06: Deterministic fast-forward
As the core,
I need to advance N days producing identical results for the same (seed, intent stream),
So that I can headless-sim 20 seasons in tests and assert on outcomes.
Priority: P0 | Size: M

US-07: Fixture conflict resolution
As the core,
I need to resolve two orbit fixtures landing too close or on the same day,
So that the schedule stays playable and deterministic without random rescheduling.
Priority: P0 | Size: L

US-08: Season boundary orchestration
As the core,
I need to run an ordered pipeline when crossing a season boundary,
So that the old season is settled, the world ages and batch-ticks, and the new season's fixtures are regenerated — all in a fixed order.
Priority: P0 | Size: L
```

---

## Acceptance Criteria

```
AC-01 for US-01: Advance stops at the first flashpoint

Scenario: A flashpoint exists ahead
  Given a fixed seed and a clean save mid-season
  When the player calls advanceUntilFlashpoint()
  Then the clock stops on the first day where at least one subsystem reports requiresDecision = true
  And the returned StopResult contains that day's stop reports

Scenario: No flashpoint before the next hard-stop
  Given no subsystem will report a flashpoint for 12 days
  And a match (hard-stop) is 12 days away
  When the player calls advanceUntilFlashpoint()
  Then the clock advances silently 12 days
  And stops on the match day
  And no intermediate day produced a player interruption
```

```
AC-02 for US-06: Determinism is byte-identical

Scenario: Same seed + same intents = same state
  Given two fresh saves created from the same seed
  When the identical intent stream is applied to both
  Then a state snapshot of both is byte-identical
  And this holds whether a match was played or skipped
```

```
AC-03 for US-03: Bounded skip breaks on hard-stop

Scenario: Injury layoff interrupted by a transfer offer
  Given the player is injured for 42 days
  When the player calls restDays(42)
  And on day 19 the club receives a transfer offer for the player (hard-stop)
  Then the clock stops on day 19
  And the returned StopResult contains the transfer hard-stop
  And the remaining rest is not auto-consumed

Scenario: Quiet layoff runs to completion
  Given the player is injured for 42 days
  And no hard-stop fires in that window
  When the player calls restDays(42)
  Then the clock advances exactly 42 days
  And energy/injury recovery is ticked once per day (42 ticks)
```

```
AC-04 for US-07: Conflict resolution is deterministic

Scenario: Two orbit fixtures on the same day
  Given a league match and a cup quarter-final both scheduled on day D for the player's club
  When resolveFixturesForDay(D) runs
  Then the higher-priority fixture (cup knockout) keeps day D
  And the league match is rescheduled to the next legal slot
  And the same seed always produces the same rescheduled day
  And the rescheduled fixture records its originalDay for audit
```

```
AC-05 for US-08: Season boundary runs in fixed order

Scenario: Crossing the season boundary
  Given the clock is on the last day of a season
  When tickOneDay() advances past endDay
  Then runSeasonBoundary() executes these steps in exactly this order:
    1. settleSeason (tables, top scorers, titles)
    2. awardCeremonies
    3. ageTickPopulation (all 20-30k players +1 year, apply age curves)
    4. batchTickOuterWorld
    5. promoteRelegateClubs
    6. openWindow(transferSummer)
    7. genesisFixtures for the new season
    8. discardFixtures of the old season
  And the event log records them in this order
```

```
AC-06 for US-04: Suspension counts by match, not day

Scenario: A suspended player's league match is postponed
  Given the player has 1 league match remaining on a suspension ledger
  And the next scheduled league match is rescheduled by conflict resolution
  When the rescheduled league match is actually played
  Then matchesRemaining decrements to 0
  And no non-league match in between affects the league suspension count
```

```
AC-07 for US-04: International break pulls the player from the club

Scenario: Player is called up during an international window
  Given an international window is active
  And the player is selected for the national team
  When the clock ticks into the window
  Then orbit club fixtures in the window are rescheduled out
  And the player's days are filled with international duty

Scenario: Player not called up
  Given an international window is active
  And the player is NOT selected
  When the clock ticks into the window
  Then those days become rest/training days
  And no international hard-stop fires
```

```
NFR-01: Tick performance
  Given a headless full-career sim (≈7,300 day-ticks over 20 seasons)
  When run on a mid-range mobile device
  Then a silent day-tick (no fixtures, no flashpoints) completes in negligible time
  And the dominant cost is match resolution and season batch-tick, not the day loop itself

NFR-02: Save size
  Given any save at any point in a career
  When serialized
  Then it stores only results, records, and the current-season materialized state
  And past-season fixtures are NOT stored (regenerated from seed)
  And deserialization of a save completes in under 1 second
```

---

## Data Models

```
Entity: GameClock
- epochDay: int                  // day 0 = save start; THE single time axis
- currentSeason: SeasonId
Persistence: save file (tiny)
Source of truth: local (core)

Entity: Season
- id: SeasonId
- startDay: int                  // epochDay
- endDay: int
- windows: List<CalendarWindow>  // summer/winter transfer, international breaks, off-season
- competitionIds: List<CompetitionId>   // orbit competitions active this season
Persistence: current season materialized; past seasons keep results only
Source of truth: regenerated from seed (ephemeral), except current

Entity: CalendarWindow
- type: WindowType
- startDay: int
- endDay: int

Entity: Competition
- id: CompetitionId
- kind: CompetitionKind          // league | domesticCup | continental | international
- priority: int                  // for conflict resolution; higher wins
- isOrbit: bool                   // relevant to the player's club / nation
Persistence: regenerated from seed

Entity: Fixture
- id: FixtureId                   // deterministic = hash(seed, competition, season, round, slot)
- competitionId: CompetitionId
- scheduledDay: int              // epochDay; may be reschedule-shifted
- originalDay: int               // audit trail for conflict resolution
- homeClub: ClubId
- awayClub: ClubId
- importance: FixtureImportance
- legForId: FixtureId?           // for 2-legged ties; must reschedule together
- isOrbit: bool
Persistence: current-season only; past = discard fixtures, keep results
Source of truth: regenerated from seed

Entity: DayContext               // published read-only to subsystems each tick
- epochDay: int
- season: SeasonId
- todaysFixtures: List<Fixture>  // already conflict-resolved
- activeWindows: List<WindowType>
- daysUntilNextFixture: int
- congestionScore: double        // fixtures in a rolling 10-day window
- rngStream: RngStream           // injected sub-stream "calendar"

Entity: DayReport                // each subsystem returns this after handling a day
- source: SubsystemId
- stopClass: StopClass           // silent | softFlashpoint | hardStop
- payload: EventPayload?         // event data for the renderer
- mutations: List<StateMutation> // applied deterministically in registration order

Entity: StopResult               // returned to the renderer when advance halts
- day: int
- stops: List<DayReport>         // the reports that caused the halt
- pending: List<DayReport>       // buffered soft flashpoints flushed alongside

Entity: SuspensionLedger
- playerId: PlayerId
- competitionId: CompetitionId   // suspension is scoped per competition
- matchesRemaining: int          // decrements only when a match of THIS comp is played
Persistence: save

Relations:
- Season has many Fixture
- Season has many CalendarWindow
- GameClock belongs to one currentSeason
- Fixture belongs to one Competition
- SuspensionLedger belongs to Player, scoped per Competition

Enums:
enum WindowType        { transferSummer, transferWinter, internationalBreak, offSeason }
enum CompetitionKind   { league, domesticCup, continental, international }
enum FixtureImportance { deadRubber, league, derby, cupKnockout, continental, final }
enum StopClass         { silent, softFlashpoint, hardStop }
enum AdvanceMode       { tickOne, untilFlashpoint, restDays, advanceToDate, simSeasonHeadless }
enum SubsystemId       { match, training, transfer, media, life, injury, international, contract }
```

---

## Core Loop — Pseudo-code

### Intent entry point (from renderer)

```
// The renderer only sends intents. The core decides everything.
function handleIntent(intent):
    switch intent.type:
        ADVANCE        -> return advanceUntilFlashpoint()
        REST(n)        -> return advanceBounded(n, allowBreak = true)
        SKIP_TO(date)  -> return advanceBounded(date - clock.epochDay, allowBreak = true)
        PLAY_MATCH(id) -> return delegateToMatchEngine(id)   // not the calendar's job
```

### Atomic tick — the ONLY function allowed to mutate the clock

```
function tickOneDay() -> List<DayReport>:
    day = clock.epochDay

    // 1. Resolve fixtures for today (conflict resolution applied)
    fixtures = resolveFixturesForDay(day)
    ctx = buildDayContext(day, fixtures)

    // 2. Poll subsystems in FIXED registration order (determinism-critical)
    reports = []
    for sys in REGISTERED_SUBSYSTEMS:        // ordered list, never a hash-map iteration
        report = sys.onDay(ctx)              // subsystem runs its own rules
        applyMutations(report.mutations)     // mutate immediately, in order
        reports.append(report)

    // 3. Decrement match-scoped counters for matches actually played today
    decrementCountersForPlayedMatches(ctx, reports)

    // 4. Advance the clock
    clock.epochDay += 1

    // 5. Season boundary?
    if clock.epochDay > clock.currentSeason.endDay:
        runSeasonBoundary(clock.currentSeason)

    return reports
```

### advanceUntilFlashpoint — the heart of manage-by-exception

```
function advanceUntilFlashpoint() -> StopResult:
    softBuffer = []                          // accumulate soft flashpoints to batch
    loop:
        reports = tickOneDay()

        hard = reports.filter(r => r.stopClass == hardStop)
        soft = reports.filter(r => r.stopClass == softFlashpoint)
        softBuffer.extend(soft)

        if hard.notEmpty():
            // must stop NOW (match day, window open, call-up, serious injury, offer)
            return StopResult(day = clock.epochDay, stops = hard, pending = softBuffer)

        if shouldFlushSoft(softBuffer, clock):
            // enough soft events / heavy enough / hit a week boundary -> surface them
            return StopResult(day = clock.epochDay, stops = softBuffer, pending = [])

        // nothing -> day passed silently, loop continues
```

### advanceBounded — rest / injury / skip with a cap

```
function advanceBounded(maxDays, allowBreak) -> StopResult:
    target = clock.epochDay + maxDays
    while clock.epochDay < target:
        reports = tickOneDay()
        if allowBreak and reports.any(r => r.stopClass == hardStop):
            return StopResult(day = clock.epochDay, stops = hardStops(reports), pending = [])
    return StopResult(day = clock.epochDay, stops = [], pending = [])
```

### simSeasonHeadless — for tests + outer-world (NEVER stops for flashpoints)

```
function simSeasonHeadless(seasonId):
    // Same code path as live; only difference is autoResolve instead of stopping.
    while not seasonEnded(seasonId):
        reports = tickOneDay()
        for r in reports where r.stopClass != silent:
            autoResolve(r)        // default policy: skip match, keep routine, decline offers
```

### resolveFixturesForDay — deterministic conflict resolution, NO random

```
function resolveFixturesForDay(day) -> List<Fixture>:
    raw = mergeAllOrbitFixtures(day)         // gather from every orbit competition
    if raw.length <= 1:
        return raw

    // Two or more orbit fixtures on the same day -> conflict.
    // Fixed-priority policy. No dice.
    sort raw by (competition.priority DESC, importance DESC, fixtureId ASC)

    keep   = raw[0]                          // highest-priority fixture holds the day
    bumped = raw[1..]

    for f in bumped:
        // international windows always win club fixtures (FIFA-style):
        // if `keep` is international, club fixtures here were already bumped upstream.
        newDay = nextLegalSlot(f,
                               after = day,
                               avoid = [keep.scheduledDay] + windowDays + restMinGap)
        if f.legForId != null:
            rescheduleTieTogether(f, f.legForId, newDay)   // keep 2-leg ties paired
        else:
            reschedule(f, newDay)            // records originalDay

    return [keep]
```

### runSeasonBoundary — fixed-order pipeline (order is load-bearing)

```
function runSeasonBoundary(oldSeason):
    // DO NOT REORDER. Each step depends on the previous.
    settleSeason(oldSeason)                  // 1. final tables, top scorers, titles
    awardCeremonies(oldSeason)               // 2. -> media engine (8.7), legacy axes (8.1)
    ageTickPopulation()                      // 3. all 20-30k players +1yr, apply age curves (5.1)
    batchTickOuterWorld(oldSeason)           // 4. non-orbit leagues advance, season granularity (7.1)
    promoteRelegateClubs()                   // 5. update the pyramid
    openWindow(transferSummer)               // 6. open summer window + transfer sagas (7.3)
    newSeason = genesisFixtures(seed, oldSeason.id + 1)   // 7. regen new season schedule (ephemeral)
    discardFixtures(oldSeason)               // 8. drop old fixtures, keep results (9: tiny save)
    clock.currentSeason = newSeason
```

### Determinism plumbing (§9 — sacred)

```
// RNG is split into independent streams per domain.
// The calendar must NOT draw from the same stream as the match engine,
// or "play vs skip a match" would shift the RNG of transfers/injuries.

rootRng     = seededRng(saveSeed)
calendarRng = rootRng.fork("calendar")
matchRng    = rootRng.fork("match")
transferRng = rootRng.fork("transfer")
injuryRng   = rootRng.fork("injury")

// Fixture ids and reschedule slots derive from calendarRng (or pure hashing),
// so the same seed always yields the same schedule, even after regeneration.
```

**Hard rules for the code reviewer:**
- Banned in the core: `DateTime.now()`, global `Random()`, `System.currentTimeMillis()`, any wall-clock read.
- All randomness flows through the `RngStream` injected via `DayContext`.
- Subsystem poll order is a **fixed ordered list** — never iterate an unordered `Map` or `Set`.

---

## Feature Breakdown

### Phase 1 — MVP (single competition, no conflict)

| # | Task | Layer | Notes |
|---|------|-------|-------|
| 1 | `GameClock` + epochDay arithmetic (365-day year) | Domain | no wall-clock; pure int math |
| 2 | `Season` + `CalendarWindow` models | Domain | windows as day-ranges |
| 3 | Deterministic `RngStream` with `fork(name)` | Infra | foundation for everything |
| 4 | `Fixture` model + deterministic id from seed | Domain | id = hash(seed, comp, season, round, slot) |
| 5 | `genesisFixtures(seed, season)` — single league | Domain | ephemeral; regen on reach |
| 6 | `DayContext` builder | Domain | computes daysUntilNextFixture, congestion |
| 7 | Subsystem interface + fixed registry | Domain | `onDay(ctx) -> DayReport` |
| 8 | `tickOneDay()` | Domain | the only clock-mutator |
| 9 | `advanceUntilFlashpoint()` + `shouldFlushSoft()` | Domain | core player loop |
| 10 | `advanceBounded()` (rest/skip) | Domain | injury layoffs |
| 11 | `simSeasonHeadless()` + `autoResolve()` | Domain | test harness |
| 12 | Save/load: serialize clock + current season only | Data | regen the rest |

> ⚠️ Tasks 8–11 depend on 6–7. Task 5 depends on 3–4. Task 3 first — everything leans on it.

### Phase 2 — Multi-competition + conflict

| # | Task | Layer | Notes |
|---|------|-------|-------|
| 13 | `Competition` model + priority field | Domain | league/cup/continental/intl |
| 14 | `mergeAllOrbitFixtures(day)` | Domain | unified timeline |
| 15 | `resolveFixturesForDay()` conflict resolution | Domain | fixed-priority, deterministic |
| 16 | `nextLegalSlot()` reschedule with avoid-set | Domain | respects windows + min rest gap |
| 17 | 2-leg tie pairing (`rescheduleTieTogether`) | Domain | legForId stays coupled |
| 18 | `SuspensionLedger` per competition | Domain | count by match, not day |
| 19 | `decrementCountersForPlayedMatches()` | Domain | survives reschedules |
| 20 | International window subsystem | Domain | hard-stop; pulls player from club |
| 21 | Congestion score (rolling 10-day) | Domain | feeds soft-flashpoint warning |

### Phase 3 — Season boundary + world reconciliation

| # | Task | Layer | Notes |
|---|------|-------|-------|
| 22 | `runSeasonBoundary()` ordered pipeline | Domain | order is load-bearing |
| 23 | `ageTickPopulation()` — struct-of-arrays | Infra | 20-30k players, columnar (9) |
| 24 | `batchTickOuterWorld()` hook | Domain | season-granularity tick (7.1) |
| 25 | `promoteRelegateClubs()` | Domain | pyramid update |
| 26 | `discardFixtures()` + results retention | Data | keep records, drop fixtures |
| 27 | Season-review flashpoint (optional stop) | Domain | player toggle |

> ⚠️ Phase 3 depends on Phase 2. Task 23 is the perf-critical one — see Tech Notes.

---

## Tech Notes & Gotchas

### Time & determinism
- **Day-tick is cheaper than it sounds.** 365 ticks/year × 20 years = ~7,300 ticks per career. Most days are silent and early-return. The real cost is match resolution and the season batch-tick — not the day loop. Do not micro-optimize the loop before profiling.
- **Banned in the core:** `DateTime.now()`, global `Random()`, wall-clock reads. Inject `RngStream` through `DayContext` only. This is the single most important rule — one stray `now()` and §2.3 is dead.
- **Subsystem registration order is an ABI.** Once shipped, reordering the subsystem list breaks determinism for every existing save. Lock it as a versioned enum. If you must add a subsystem, append it and bump a `simVersion`.
- **Split RNG streams per domain.** If the calendar and match engine share a stream, then playing vs skipping a match shifts every downstream roll (transfers, injuries). Fork per domain.

### Fixtures & conflict
- **Don't store fixtures.** Regenerate from `(seed, season, league)` every time. The save holds results + records + current-season materialized state only (§9). This is what keeps load under 1s.
- **Suspension counts by match, not day** (AC-06). If you count by day, a postponed match wrongly serves the ban. The counter only decrements when a match of *that exact competition* is actually played.
- **2-leg ties reschedule together.** When bumping a first leg, the `legForId` second leg moves with it — don't shift one and orphan the other.
- **International windows are an external interrupt.** They both pull the player out (national duty) and bump club fixtures. Treat `international` as a first-class subsystem with hard-stop authority, not a special case inside the league logic.

### Performance
- **Struct-of-arrays for `ageTickPopulation`** (§9). Aging 30k players each season boundary with 30k heap objects → ARC/GC spike → frame stutter exactly when crossing seasons. Store the population columnar (parallel typed arrays), not as 30k reference objects.
- **Headless sim must be the same code path.** `simSeasonHeadless` differs from live only in calling `autoResolve` instead of returning a `StopResult`. If you write two code paths (one for "play", one for "test"), they will diverge and your tests stop meaning anything.

### UX-adjacent (lives in core but shapes feel)
- **`shouldFlushSoft()` is where the game feels good or annoying.** Stop on every minor event → the player rages. Buffer soft flashpoints and flush as a cluster (by count threshold, by cumulative weight, or at a week boundary). This is the single most-tuned function in the system — expect to iterate against a prototype.
- `[DECISION NEEDED]` Soft-flush policy: count-threshold, weight-threshold, or fixed cadence (every Monday)? Affects the entire feel of manage-by-exception.

### Decisions still open
- `[DECISION NEEDED]` Fixed 365-day year vs real calendar mapping. Recommendation: fixed 365 — simpler, deterministic, players don't care about weekdays.
- `[DECISION NEEDED]` Competition priority table for conflicts. Proposed ladder: `final > continental > cupKnockout > derby > league > deadRubber`. Confirm whether priority can shift by nationality/era.
- `[DECISION NEEDED]` Do reschedules propagate to the outer (batch-ticked) world, or are orbit reschedules local-only and invisible to AI clubs? This affects league-table consistency.

---

## ⚠️ Risks & Open Questions

- **International break stacked on club congestion = double-whammy.** The player is simultaneously fatigued and pulled away. The energy/injury model must receive the right signal from the calendar, or you get an absurd "inexplicably exhausted player" bug. Wire the congestion + intl-duty signals explicitly.
- **`shouldFlushSoft` tuning is unbounded scope.** It's a feel problem, not a correctness problem — it cannot be "finished" on paper. Budget prototype iteration time, don't try to nail it in the spec.
- **Reschedule ripple into the outer world** (the open decision above) can quietly break league-table consistency if orbit reschedules aren't reflected where they should be. Decide the boundary before coding Phase 3.
- **GK career is parked (§11)** but the calendar must stay player-type-agnostic from day one. Don't hard-code outfield assumptions into `DayContext` or fixture handling — adding GK later should touch beats/flashpoints, not the calendar.
- **Save migration vs subsystem ABI.** The moment you ship and later add/reorder a subsystem, old saves desync. You need a `simVersion` and a migration story before the first public build, not after.

---
---

# APPENDIX — `TASK-0X-goat-calendar.md` (paste-ready for Claude Code)

> This is the Claude Code task file derived from the spec above. It follows the same
> convention as `TASK-01-goat-core`: read source-of-truth first, work in reviewable
> steps with pauses, frozen golden values, determinism non-negotiable.
>
> **Prereqs before pasting this:**
> - `TASK-01-goat-core` is merged and `cargo test --workspace` is green.
> - `goat-core` exposes the player/attribute/role model the calendar will poll against.
> - `CLAUDE.md` is in the repo root and the tech doc is current.
>
> **Phasing note:** This task covers Phase 1 of the calendar (single-competition,
> deterministic tick loop). Multi-competition + conflict resolution and the season
> boundary pipeline become `TASK-0X+1` and `TASK-0X+2` — do not pull them forward.

---

Read CLAUDE.md, then docs/BecomeTheGOAT-RustCore-TechDoc.md (the module map, RNG design,
and build order), then the design-bible sections §2.2, §2.3, §5.4, §6.1, §7.1, §9, and
finally the public APIs of crates/goat-rng, crates/goat-fixed, and crates/goat-core.
Do not write any code until you've read all of them.

If anything in this task contradicts the tech doc, STOP and flag it — the tech doc wins.

Then build the `goat-calendar` crate, Phase 1 (single competition, deterministic tick),
in these steps — pause for my review after each step:

## Step 1 — Time core + season model
- New workspace member `crates/goat-calendar` (`#![forbid(unsafe_code)]`, deps limited
  to goat-rng, goat-fixed, goat-core).
- `GameClock { epoch_day: u32, current_season: SeasonId }`. The in-game year is a fixed
  365 days — no leap years, no wall-clock. epoch_day is the single time axis.
- `Season { id, start_day, end_day, windows: Vec<CalendarWindow>, competition_ids }`.
- `CalendarWindow { kind: WindowKind, start_day, end_day }` with WindowKind =
  { TransferSummer, TransferWinter, InternationalBreak, OffSeason }.
- All day arithmetic is pure integer math. No `std::time`, no `chrono`, no `SystemTime`.
- Property test: epoch_day → (season-relative day) round-trips; windows never overlap
  illegally within a season.

## Step 2 — Subsystem registry + DayContext + tick_one_day
- Define the `Subsystem` trait: `fn on_day(&mut self, ctx: &DayContext) -> DayReport`.
- `DayReport { source: SubsystemId, stop_class: StopClass, payload, mutations }` with
  StopClass = { Silent, SoftFlashpoint, HardStop }.
- The registry is a FIXED-ORDER `Vec`, never a HashMap iteration. Document that this
  order is an ABI: reordering breaks save determinism. Gate it behind a `SIM_VERSION`
  const.
- `DayContext` carries epoch_day, season id, today's fixtures, active windows,
  days_until_next_fixture, congestion_score, and an injected RNG sub-stream
  (`rng.fork("calendar")` — never the global or the match stream).
- `tick_one_day(&mut self) -> Vec<DayReport>`: the ONLY function permitted to mutate
  `epoch_day`. Build context → poll subsystems in order, applying each report's
  mutations immediately → decrement match-scoped counters for matches played today →
  `epoch_day += 1` → if past season end_day, call the (stubbed for now) season-boundary
  hook.
- For Phase 1, provide 1–2 trivial stub subsystems (e.g. a training stub and a match
  stub) so the loop is exercisable. Real subsystems land in their own crates later.

## Step 3 — advance loop + golden-seed test (calendar test #1)
- `advance_until_flashpoint(&mut self) -> StopResult`: loop tick_one_day; stop on the
  first HardStop, or when `should_flush_soft(buffer, clock)` says the buffered
  SoftFlashpoints are worth surfacing; silent days loop. `StopResult { day, stops,
  pending }`.
- `advance_bounded(&mut self, max_days, allow_break) -> StopResult`: tick up to max_days,
  breaking early on a HardStop only if allow_break. This is rest/injury/skip.
- `should_flush_soft` for Phase 1: simplest defensible rule (e.g. flush when buffer
  length >= N, N a named constant in `tuning`). Mark it clearly as TUNABLE — the real
  policy is deferred (bible-style open question), so leave a doc comment, do not invent
  a clever final rule.
- Golden-seed test: fixed seed + fixed intent sequence (a mix of ADVANCE and REST over,
  say, 60 days with the stub subsystems firing scripted reports) → assert the exact
  sequence of stop days and the exact final clock state. These expected values become
  FROZEN once I approve.
- Determinism test: run the same 60-day sequence twice, assert byte-identical state
  snapshots (use `insta` if it's already in the workspace, otherwise a manual
  serialize-and-compare).

## Rules reminders (from CLAUDE.md — these override convenience)
- No floats in sim. No std HashMap iteration feeding results. RNG only via injection,
  forked per domain — the calendar stream must be independent of the match stream.
- Do NOT touch goat-rng, goat-fixed, or goat-core. If their API seems insufficient,
  stop and ask — do not refactor them as a side effect.
- All pre-existing golden tests must stay green with their ORIGINAL expected values.
  Never "fix" a failing test by editing the expected value.
- `epoch_day` mutation lives in exactly one place (`tick_one_day`). If you find yourself
  incrementing it anywhere else, stop — that's a design smell this task forbids.
- Season-boundary pipeline is OUT OF SCOPE for this task (stub the hook only). Do not
  implement settle/age-tick/batch-tick here.
- Multi-competition and conflict resolution are OUT OF SCOPE. Phase 1 is single-comp.
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` clean before each
  pause.

At each pause: show me the file tree of what you added, the key type definitions
(GameClock, Season, DayContext, DayReport, the Subsystem trait), and the test output.

## Definition of done (this task)
1. `cargo test --workspace` green — including all pre-existing golden tests at their
   original expected values (goat-rng 9, goat-fixed 6, plus goat-core's).
2. `cargo fmt --check` and `cargo clippy -D warnings` clean.
3. The tick loop's deterministic behavior is covered by the golden-seed test AND the
   byte-identical determinism test.
4. No new heavy deps (insta is fine if already present), no floats in sim, no unsafe,
   no I/O, no wall-clock reads anywhere in the crate.
5. `grep -rn "now()\|SystemTime\|Instant\|chrono\|f32\|f64" crates/goat-calendar/src`
   returns nothing.
6. Short summary of what changed and which bible/tech-doc sections it implements
   (expected: §2.2, §2.3, §5.4, §9).
# TASK DESIGN ROUND 4 — Competitions: domestic cup, continental club tiers, World Cup & continental championships

Prereq: none to *start this design*, but this doc's numbers assume the round-2 world-genesis
scale-up (`tasks/TASK-DESIGN-round2-world-genesis-scaleup.md`, Doc A: 20 nations × 3 tiers ×
20 clubs) and the round-2 national-team/tactical-identity layer
(`tasks/TASK-DESIGN-round2-national-team-tactical-identity.md`, Doc B: call-ups, caps,
`TacticalIdentity`) as already shipped or in-flight — **verified against real code, 2026-07-22:
both are already implemented** (see "Verified" below), so this doc is not blocked on them the
way it would have been if written before Doc A/B landed.

This is backlog item #2 from the parked list at the bottom of
`tasks/TASK-DESIGN-round2-world-genesis-scaleup.md` ("Multi-competition: domestic cups +
continental competitions"), designed properly per Tùng's 2026-07-22 interactive session. That
parked section is being replaced by a pointer to this doc (see the diff to that file alongside
this one).

Read first: `docs/MAIN.md`'s **Calendar Simulator — Core Spec** section (Data Models ~line
913–996, Core Loop pseudo-code ~line 1000–1150, Feature Breakdown Phase 2 ~line 1180–1192) —
the `Competition`/`Fixture`/`FixtureImportance`/`SuspensionLedger` shape this doc builds on;
`crates/goat-calendar/src/*.rs` (the **already-built Phase 1 scaffold** of that exact spec —
see "Verified" below, this is the foundation, not a green field); `crates/goat-world/src/{world,
fixtures,season,batch_tick,promotion}.rs` (the league/promotion-relegation machinery this doc's
new competition kinds sit alongside, not replace).

## Ground rules for this doc

- **Reuse `goat-calendar`'s existing Phase 1 scaffold; this doc is that crate's Phase 2, not a
  parallel system.** `CompetitionId`, `Fixture` (with `competition_id`/`scheduled_day`/
  `original_day`/`is_orbit`), `DayContext`, `DayReport`, `StopClass`, `SubsystemId` (already has
  an `International` variant), and `CalendarEngine` (`tick_one_day`/`advance_until_flashpoint`/
  `advance_bounded`/`sim_season_headless`) already exist and already match the bible's shape
  closely. `engine.rs:145` has a literal comment: `// Phase 1 stub: SuspensionLedger (per-
  competition, counts by match not day) added in Phase 2.` **This doc is the design for that
  comment's Phase 2.** Do not propose a new calendar/fixture engine from scratch.
- **No change to the existing league round-robin simulation itself.** `fixtures.rs`'s
  `generate_fixtures`/`round_fixtures` (circle-method round-robin) and `season.rs`'s `Table`
  keep working exactly as they do today for the league competition kind — this doc adds
  *other* competition kinds alongside the league, it does not touch how the league itself is
  simulated.
- **No change to promotion/relegation.** `promotion.rs`'s `ReplayCache`/`apply_season_end`
  (replay-from-seed, `PROMO_RELEGATION_N = 3`) is untouched. Continental qualification (Slice 3)
  *reads* final league position, it does not feed back into promotion/relegation.
- **No change to player generation, attribute storage, or match-engine internals.** New
  competition kinds are new *fixtures* fed through the existing match-resolution path
  (`sim_team_match` for AI-vs-AI, the real match engine `goat-match` for PC-orbit fixtures) —
  not a new way of simulating a match.
- **Reuse `TacticalIdentity` for national teams — it already exists.** `GeneratedNation` already
  carries `tactical_identity: TacticalIdentity` (`world.rs:49-53`) and `stature: u8`
  (`world.rs:44-48`, range 25–95, `world.rs:213`). Slice 4 (national-team competitions) does
  not generate anything new for "what a national team is" — that's already Doc B's job, done.
  This slice only adds *fixtures and a simulated result* for that already-existing entity.
- **"Generated but consistent," same pattern as everywhere else in this codebase.** Every new
  RNG-driven decision (cup draw, bye assignment, tournament group draw) must be a pure function
  of `world_seed` (+ competition/season/round indices), on its own forked RNG stream, per bible
  §9's sacred determinism plumbing — never sharing a stream with match/transfer/injury/calendar
  RNG.
- **Flag every number Tùng didn't explicitly give**, per the same discipline `TASK-DESIGN-
  round1`/`round2` used. This doc invents several concrete numbers (cup entry-round scheme,
  continental slot formula, qualifying-match count) that were sketched at a conceptual level in
  conversation but not given exact values — every one is called out below and collected in
  "Decisions Design needs from Tùng," not silently assumed settled.

## Verified: what already exists vs. what's genuinely new

**World-genesis scale-up (Doc A) and national-team layer (Doc B) are both already implemented**
— this changes this doc's starting point from what the round-2 parked note assumed ("zero
`Competition`/cup/continental code exists"). Specifically, verified 2026-07-22:

- `crates/goat-world/src/world.rs:95-102` — `NUM_NATIONS = 20`, `TIERS_PER_NATION = 3`,
  `CLUBS_PER_DIV = 20`, `NUM_DIVISIONS = 60`, `NUM_CLUBS = 1,200`, `PROMO_RELEGATION_N = 3`.
  `GeneratedNation.stature: u8` (`world.rs:44-48`, rolled `25..95` at `world.rs:213`) and
  `GeneratedNation.tactical_identity` / `Club.tactical_identity` (`world.rs:49-53,64-66`) all
  exist and are genesis-generated today.
- `crates/goat-world/src/promotion.rs` — `ReplayCache`/`apply_season_end`/`PromoRelegationEvent`
  fully implemented (replay-from-seed, `PROMO_RELEGATION_N` top/bottom cut, idempotent).
- `crates/goat-core/src/state.rs:95-104,368,811-817` — `Intent::NationalTeamCallUp`,
  `pc_career_caps`/`pc_season_caps`/`pc_career_international_goals` all exist and are folded
  into legacy evidence at season-end, per Doc B's shipped design.
- `crates/goat-meta/src/legacy.rs:10-41` — `LegacyEvidence::career_caps` /
  `career_international_goals` exist. **`league_titles: u32` exists (`legacy.rs:23`) but there
  is no `cup_titles`, `continental_titles`, or `world_cups_won`/`continental_championships_won`
  field anywhere.** This is the same gap Doc B flagged for caps, now recurring for every new
  trophy this doc introduces — see Slice 5.

**`goat-calendar`'s Phase 1 scaffold already implements most of the bible's Calendar Simulator
data model** — this is the single biggest fact grounding this doc, since it means Slice 1 below
is "finish Phase 2 of an existing plan," not "design a calendar system from scratch":

- `crates/goat-calendar/src/clock.rs:7-10,87-98` — `CompetitionId` exists as a `pub type
  CompetitionId = u32` (a bare alias, **no `Competition` struct with `kind`/`priority` yet** —
  this is exactly what Slice 1 adds). `Fixture` already has `competition_id: CompetitionId`,
  `scheduled_day`/`original_day` (reschedule audit trail), and `is_orbit: bool` — **but no
  `importance: FixtureImportance` field and no `leg_for_id` for two-legged ties** (both bible-
  spec fields, both missing today — Slice 1 adds them).
- `crates/goat-calendar/src/subsystem.rs:17-26` — `SubsystemId` already has an `International`
  variant (doc comment: "Must interrupt immediately: match day, transfer offer, serious injury,
  call-up") — the *slot* for national-team fixtures already exists in the subsystem registry;
  nothing has been built behind it yet (same pattern Doc B found for the call-up layer before
  it shipped).
- `crates/goat-calendar/src/engine.rs:71-116,144-145` — `fixtures_for_day` only filters by
  `is_orbit && scheduled_day == day`; **there is no `resolveFixturesForDay`-style conflict
  resolution at all today** — if two orbit fixtures land on the same day, nothing picks a
  winner or reschedules the loser. `engine.rs:145`'s comment confirms `SuspensionLedger` is a
  deliberately-deferred Phase 2 item, not forgotten.
- **`goat-calendar` is *not yet fully wired to the real league fixtures.***
  `crates/goat-core/src/calendar_loop.rs:86-110`'s `advance_calendar_week` constructs
  `CalendarEngine::new(world_seed, season, Vec::new())` — an **empty fixture list** — the
  `CalendarEngine` today only drives window-detection flavor (international break/transfer
  window soft-flashpoints via `WindowWatch`), it does not yet carry the real league round-robin
  fixtures from `goat-world::fixtures`. This doc's Slice 1 is also where that wiring gap gets
  closed — competitions (plural) only make sense once the calendar actually carries real
  fixtures from more than one source.
- `crates/goat-core/src/state.rs:903-905` — `pc_suspension_weeks: u32` is **one single global
  scalar**, decremented once per resolved round with no competition scoping at all. This is the
  "before" state Slice 5's `SuspensionLedger` replaces — today, a red card in *any* competition
  would (if this were wired to more than the league) incorrectly serve out across all of them.
- `crates/goat-world/src/calendar.rs:1-31` — the existing season week-grid (`SEASON_CALENDAR_WEEKS
  = 38`, `BASE_CAREER_YEAR = 2025`, Aug 15-anchored) already has 3 break weeks per season (week
  4 international, week 21 winter, week 32 spring) and the season itself only spans roughly
  mid-August to mid-May (38 weeks × 7 ≈ 266 days from the Aug 15 anchor) — **leaving a genuine
  ~3-month off-season gap every year that today has zero simulated content in it.** This gap is
  load-bearing for Slice 4's World Cup/continental-championship design (see below) — it already
  exists, unused, in the current calendar; this doc's national-team finals tournaments are the
  first thing proposed to actually live there.

**Confirmed still true: no tournament simulation of any kind exists.** Doc B explicitly scoped
"simulating actual international tournament outcomes" as out of its round — verified again
today, no code anywhere computes a domestic cup / continental / World Cup *winner*. This doc is
that follow-up.

---

## Slice 1 — `Competition` entity, `FixtureImportance` ladder, and wiring real fixtures through `goat-calendar`

**Why first:** every other slice produces fixtures that need somewhere to live. This slice
brings the bible's `Competition` entity into code (today only a bare `CompetitionId` alias
exists) and closes the "real league fixtures aren't fed into `CalendarEngine`" gap — the
foundation every later slice's fixtures are scheduled through.

### 1.1 — `Competition` struct + `CompetitionKind`

```rust
// crates/goat-calendar/src/clock.rs (extends the existing module)

pub struct Competition {
    pub id: CompetitionId,
    pub kind: CompetitionKind,
    /// Conflict-resolution priority; higher wins a same-day clash (see 1.2).
    pub priority: i32,
    /// Relevant to the PC's club/nation this season.
    pub is_orbit: bool,
}

pub enum CompetitionKind {
    League,
    DomesticCup,
    ContinentalTier1,   // Champions-League-equivalent
    ContinentalTier2,   // Europa-equivalent
    ContinentalTier3,   // Conference-equivalent
    WorldCup,
    ContinentalChampionship,   // Euro/Copa América/Asian Cup/etc. — one enum value, many nations' instances
}
```

This is **7 kinds**, not the bible's original 4 (`league | domesticCup | continental |
international`) — `continental` splits into the 3 club tiers Slice 3 needs, and
`international` splits into `WorldCup` vs `ContinentalChampionship` since Slice 4 gives them
different cadence/offset (see below) and they need to be distinguishable for legacy-evidence
purposes ("won a World Cup" and "won a continental championship" are different trophies).

`Fixture` (`clock.rs:87-98`) gains the two bible fields it's missing:

```rust
pub struct Fixture {
    // ...existing fields unchanged...
    pub importance: FixtureImportance,
    pub leg_for_id: Option<FixtureId>,   // two-legged ties (continental knockout, some cup rounds)
}
```

### 1.2 — `FixtureImportance` ladder for the now-larger competition set

The bible's original proposed ladder (`docs/MAIN.md:1233`) was `deadRubber < league < derby <
cupKnockout < continental < final` — a single flat `continental` bucket. With 3 continental
club tiers plus 2 national-team competition kinds, that collapses too much distinct information
to resolve same-day clashes correctly. Proposed ladder, ascending (higher wins a clash):

```rust
pub enum FixtureImportance {
    DeadRubber,
    League,
    Derby,                    // a league match, rivalry-flagged
    ContinentalTier3,
    DomesticCupEarly,         // rounds before the tier-1 entry round (Slice 2)
    ContinentalTier2,
    DomesticCupLate,          // tier-1-entry round onward, incl. semis
    ContinentalTier1,
    DomesticCupFinal,
    ContinentalTier1Final,
}
```

**National-team fixtures (`WorldCup`/`ContinentalChampionship`) are deliberately NOT in this
ladder at all.** Per the bible's own pseudocode (`docs/MAIN.md:1106-1107`: "international
windows always win club fixtures (FIFA-style): if `keep` is international, club fixtures here
were already bumped upstream") — national-team competitions don't win a same-day *priority*
contest against club fixtures, they **exclude club fixtures from being scheduled in their window
at all**, one level higher than the importance ladder. This is a hard calendar-level rule (an
international window makes every date inside it unavailable to club fixture generation), not a
same-day tie-break. Slice 4 details how this plays out given the *existing* international-break
window is far too short for a whole World Cup.

**[DECISION NEEDED — flag for Tùng]:** the exact ladder above is Design's proposal, not a
value Tùng specified. In particular: does `ContinentalTier1` (group/knockout stage) really
outrank `DomesticCupLate` (semifinal)? Real football scheduling gives UCL matches priority for
rest-day allocation over domestic cup replays, which is the reasoning behind this ordering, but
it's a judgment call, not a stated decision — confirm before Dev starts.

### 1.3 — Wire real league fixtures through `CalendarEngine` (closes the `Vec::new()` gap)

`calendar_loop.rs:91`'s `CalendarEngine::new(world_seed, season, Vec::new())` must start passing
the PC's orbit fixtures (today: just their league fixtures from `goat_world::fixtures`; once
Slices 2–4 ship: cup/continental/national-team fixtures too) instead of an empty vec. This is
mechanical (`fixtures_for_club`/equivalent already exists in `goat-world`, just needs threading
into the `Fixture` shape `goat-calendar` expects — `competition_id`, `importance`,
`scheduled_day` computed from `goat_world::calendar`'s existing week-grid) but is the concrete
step that makes "multiple competitions" a real, testable thing instead of a type that exists but
carries no data.

### 1.4 — `resolveFixturesForDay`-equivalent conflict resolution

Implement the bible's pseudocode (`docs/MAIN.md:1090-1117`) against `goat-calendar`'s
`CalendarEngine`: when `fixtures_for_day` returns more than one orbit fixture for a day, sort by
`(competition.priority DESC, importance DESC, fixture.id ASC)`, keep the winner, reschedule the
rest to the next legal slot (avoiding other scheduled days, active windows, and a minimum rest
gap — the existing `days_until_next_fixture`/`congestion_score` context already computed by
`build_context` gives the inputs this needs). Two-legged ties (`leg_for_id`) reschedule together,
never independently.

- TDD: `crates/goat-calendar/tests/` new module `conflict_resolution` —
  `higher_priority_competition_wins_same_day_clash`, `bumped_fixture_reschedules_forward_not_
  backward`, `two_legged_tie_reschedules_together` (mirroring the bible's own AC set and
  `promotion.rs`'s existing idempotency-test style).
- Playable gate: `cargo run -p goat-tui` → a season where the PC's club has both a league match
  and a cup match on the same calendar day (synthetic/seeded) → the resolved fixture list shows
  exactly one of them on that day, the other moved with a visible reschedule note.

**Size: large, risk: medium-high.** Not because any one piece is hard, but because it's the
slice everything else depends on, and it touches the one already-shipped-and-tested crate
(`goat-calendar`) rather than adding a new one — regressions here ripple into every existing
`goat-calendar`/`calendar_loop.rs` test. Sequence this **strictly first**, fully landed and
tested, before starting Slices 2–5.

---

## Slice 2 — Domestic cup: single-elimination, tier-staggered entry, round-by-round random redraw

**Decision (verbatim, item 1 confirmed by Tùng):** every club in a nation is eligible, FA-Cup
style — lower-tier clubs enter earliest, top-tier clubs enter later via byes.

### 2.1 — Entry-by-tier scheme (concrete numbers — [DECISION NEEDED], Design's proposal)

At the confirmed Doc A scale (60 clubs/nation: 20 Tier-1 + 20 Tier-2 + 20 Tier-3), a
staggered-entry bracket that avoids awkward byes as long as possible:

| Round | Entrants this round | Total in the round | Byes | Winners advance |
|---|---|---|---|---|
| R1 | Tier 3 (20 clubs) | 20 | 0 | 10 |
| R2 | Tier 2 (20 clubs) + R1's 10 winners | 30 | 0 | 15 |
| R3 | Tier 1 (20 clubs) + R2's 15 winners | 35 | 1 | 17 + 1 bye = 18 |
| R4 | — | 18 | 0 | 9 |
| R5 | — | 9 | 1 | 4 + 1 bye = 5 |
| R6 | — | 5 | 1 | 2 + 1 bye = 3 |
| R7 (semifinal-equivalent) | — | 3 | 1 | 1 + 1 bye = 2 |
| R8 (Final) | — | 2 | 0 | 1 (champion) |

Two rounds (R2, R4) land on even numbers and need no byes at all — a happy consequence of
20+20+20 tier sizes, not engineered. **8 rounds total** (R1–R6, then a semifinal-equivalent and
a final) mirrors the real FA Cup's 8 named "proper" rounds (R1–R5, QF, SF, F) closely enough to
read as authentic, even though club counts per round differ from the real competition.

**Why Tier 3 enters first, Tier 2 second, Tier 1 last (not, say, all three simultaneously):**
mirrors the real FA Cup's staggered entry (lower-league clubs play more rounds before meeting
the top flight) and gives lower-tier clubs a real, distinct "cup run" narrative arc (several
rounds of pure lower-tier action before facing a big club) rather than every club having an
identical-length cup campaign regardless of tier — this is explicitly part of what makes the
domestic cup feel different from the league, not a redundant second league.

**Bye assignment rule:** when a round has an odd number of clubs, one club is drawn at random
(same draw RNG as the round's pairing draw, see 2.2) to receive the bye — **not** given
preferentially to a higher-tier or higher-strength club. This follows directly from ground rule
#2 (Tùng: "cứ real mà quất," no protective seeding) — a bye is exactly as "unseeded" as a
pairing; nothing in the real FA Cup privileges byes to bigger clubs either (byes there come from
qualifying-round attrition, not design).

**[DECISION NEEDED — flag explicitly]:** the exact entry-round assignment (Tier 3→R1, Tier
2→R2, Tier 1→R3) and the resulting 8-round bracket shape above are Design's proposal, grounded
in the confirmed 20/20/20 club counts but not a number Tùng gave directly — confirm before Dev
starts, particularly whether 8 rounds (a full ~2-month-plus cup campaign for a Tier-3 club) is
the right amount of in-season fixture congestion to add on top of the existing 38-round league
season (see Slice 5's congestion-score interaction).

### 2.2 — Random draw, redrawn every round, seed-deterministic

Each round's pairings are drawn fresh — **not** a fixed bracket set once at the top (ground
rule confirmed explicitly by Tùng: "cứ real mà quất," matching the real FA Cup's round-by-round
redraw, not a single-elimination bracket generated once like a knockout tournament seed sheet).
Two Tier-1 clubs entering the same round (R3 onward) **can** draw each other — no protective
separation.

```rust
// crates/goat-world/src/domestic_cup.rs (new)
fn draw_round(world_seed: u64, season: u32, nation: NationId, round: usize, clubs: &[ClubId]) -> Vec<(ClubId, ClubId)> {
    let mut rng = GoatRng::new(world_seed ^ domestic_cup_salt(nation) ^ (season as u64) ^ (round as u64));
    // Fisher-Yates shuffle of `clubs` (bye-holder removed first if odd), then pair sequentially.
    // Home/away assignment: a second independent roll per pair (or fixed by draw order —
    // [DECISION NEEDED], Design has no strong preference here; real FA Cup draws home/away
    // as part of the same ball-draw, so pairing order deciding home/away is a defensible default).
}
```

New forked RNG stream, `domestic_cup`, independent of `calendar`/`match`/`transfer`/`injury`
(bible §9's sacred stream-independence rule) — drawing a different round must not perturb any
other subsystem's randomness.

- TDD: `domestic_cup_draw_is_deterministic_per_seed` (same seed+season+round → identical
  pairings), `two_tier1_clubs_can_meet_in_their_entry_round` (statistical: over many seeds,
  at least one instance of two Tier-1 clubs drawn together in R3 — proves the "no protective
  seeding" property holds, not just that it's theoretically possible), `bracket_converges_to_
  one_champion_per_nation_per_season` (structural, mirrors `promotion.rs`'s invariant-style
  tests), `bye_recipient_is_uniformly_random_not_tier_biased` (statistical, over many seeds no
  systematic tier favored for byes).
- Playable gate: `cargo run -p goat-tui` → advance through a season with the PC's club in the
  domestic cup → cup fixtures appear interleaved with league fixtures on the calendar,
  elimination is visible after a loss, a champion is crowned by season's end.

**Size: large, risk: medium.** The draw/bracket logic itself is a well-understood algorithm
(nothing novel), the risk is entirely in Slice 1's conflict-resolution/calendar-congestion
interaction (an extra ~8-round competition materially increases fixture density for clubs that
go deep) — sequence after Slice 1 is solid, not in parallel.

---

## Slice 3 — Continental club competitions: 3 tiers, stature-ranked qualification

**Decision (verbatim, item 3 confirmed by Tùng):** 3 tiers (Champions-League/Europa/Conference
shape), top 4-5 clubs per nation qualify with slot count varying by federation ranking.

### 3.1 — Qualification pool: each nation's Tier-1 (top-flight) final league position only

**Scoping clarification, not explicit in the task brief but load-bearing — [DECISION NEEDED,
flag for confirmation]:** continental qualification is read off final position in each nation's
**Tier-1 domestic league only** (the 20-club top flight), not "top N clubs across all 3
domestic tiers of a nation." This matches real UEFA competition qualification exactly (Europa/
Conference slots go to top-flight clubs finishing outside the Champions League places, e.g.
5th–7th — never to 2nd-tier clubs) and is the only reading consistent with "clubs just outside
the top-tier cutoff drop to tier 2, etc." (a Tier-2-*continental*-competition slot going to a
Tier-1-*domestic*-league club that just missed Tier-1-continental, not to an actual Tier-2-
domestic club). A domestic-cup-winner-grants-continental-berth mechanic (real football does
this too, e.g. FA Cup winner → Europa League) is **not** included — see "Out of scope."

### 3.2 — Slot-count formula, ranked by `GeneratedNation.stature` (not an absolute threshold)

Reuses the already-generated `stature: u8` field (`world.rs:44-48`, range 25–95) directly —
**no new per-nation quality metric is generated**, since `stature` already *is* "how strong is
this footballing nation" (it already drives the club-strength distribution within that nation,
`world.rs:234`). Rank the 20 nations by `stature` descending → `nation_rank` 1 (strongest) to 20
(weakest) — **rank position, not a raw `stature` threshold**, because real UEFA-coefficient
slot allocation is itself rank-based (a federation's slot count depends on its *relative*
standing among all member federations, not an absolute score) — this keeps the allocation
correctly proportional regardless of how a given `world_seed` happens to distribute the 20
stature rolls (an absolute-threshold scheme would break if an unusual seed rolled many nations
into a narrow high band).

| `nation_rank` band | Tier-1 slots (positions) | Tier-2 slots (positions) | Tier-3 slots (positions) | Total continental clubs |
|---|---|---|---|---|
| 1–4 (top quartile) | 5 (1–5) | 3 (6–8) | 2 (9–10) | 10 of 20 |
| 5–9 | 4 (1–4) | 3 (5–7) | 2 (8–9) | 9 of 20 |
| 10–14 | 3 (1–3) | 2 (4–5) | 2 (6–7) | 7 of 20 |
| 15–20 (bottom third) | 2 (1–2) | 1 (3) | 1 (4) | 4 of 20 |

This gives Tier-1 slot counts of **2–5**, matching the task's headline "top 4-5 clubs" for
stronger nations while extending down to 2 for genuine minnows — an explicit extension beyond
the literal "4-5" framing, made because the task itself asked for slot count to *vary by
federation ranking*, which a flat 4-5-for-everyone scheme wouldn't actually do.
**[DECISION NEEDED — this whole table, bands and counts, is Design's proposal; the only
Tùng-confirmed facts are "3 tiers," "top 4-5ish," and "varies by federation ranking." Every
number in this table needs sign-off before Dev starts.]**

### 3.3 — Fixture format per tier

**[DECISION NEEDED — not addressed in the task brief at all, needs Tùng's input]:** does each
continental tier run a group stage before knockout (matching real UCL/Europa/Conference
shape more closely) or single-elimination only (simpler, closer to this doc's domestic-cup
design)? Design's recommendation for a first pass: **single-elimination with two-legged ties**
(reuses `leg_for_id` from Slice 1, no new group-table type needed, far less new code than a
group stage requiring its own mini-`Table` per continental group) — a group stage is a
reasonable enhancement to flag for a later round once single-elimination continental football
is proven out, not this round's default. This recommendation trades authenticity (real UCL's
group/league phase is a big part of its identity) for scope control, consistent with this doc's
"already extra-large" sizing — flag for explicit confirmation, do not silently build the bigger
version.

Draw: same random-redraw-per-round, seed-deterministic pattern as Slice 2 (own forked RNG
stream, `continental_tier{1,2,3}`), across all qualified clubs from all 20 nations for that
tier — i.e., a Tier-1-continental club from a powerhouse nation can draw a Tier-1-continental
club from a minnow nation in any round, same "no protective seeding" rule as the domestic cup.

- TDD: `continental_slots_match_nation_rank_band` (structural, asserts table membership per
  3.2's bands), `continental_draw_deterministic_per_seed`, `continental_champion_per_tier_per_
  season` (one champion crowned per tier, mirrors `promotion.rs`'s invariant style).
- Playable gate: `cargo run -p goat-tui` → PC's club finishes in a continental qualification
  position → next season's fixture list includes continental fixtures against clubs from other
  nations.

**Size: extra-large, risk: high.** Three parallel competitions (not one), a genuinely new
cross-nation qualification computation (needs full previous-season table data for all 20
nations' Tier-1 leagues, which today is only ever computed on-demand for flavor screens — this
needs to become a real per-season-boundary computation, similar in shape to how `promotion.rs`'s
`ReplayCache` already extends incrementally), and a new fixture-format decision (3.3) not yet
settled. Recommend Tùng treat this as its own dedicated implementation round, same framing Doc A
gave its own extra-large slice.

---

## Slice 4 — National team competitions: World Cup + continental championships

**Decision (verbatim, item 4 confirmed by Tùng):** World Cup every 4 in-game years (real FIFA
cadence), continental championships (Euro/Copa América/Asian Cup/etc.) also on real-world
cadence, staggered from the World Cup by the same offset real tournaments use.

### 4.1 — Cadence: World Cup seasons 1, 5, 9, 13…; continental championship seasons 3, 7, 11, 15…

Matches the real 4-year cycle with a 2-year stagger (World Cup and Euro/Copa/etc. never land in
the same calendar year in reality) — `season_number % 4 == 1` → World Cup year,
`season_number % 4 == 3` → continental-championship year. **[DECISION NEEDED — Design picked
season 1 as the anchor World Cup year (so the PC's very first season already has a World Cup)
somewhat arbitrarily; confirm whether the PC's career should start in a World Cup year, a
continental-championship year, or a "neither" year — this shifts what a new career's early
seasons look like and wasn't specified.]**

### 4.2 — Where a tournament actually happens on the calendar: the existing off-season gap

**This is the load-bearing finding of this slice.** `goat-world::calendar`'s existing 38-week
season grid, anchored at Aug 15 (`calendar.rs:14,17`), runs roughly mid-August to mid-May —
leaving a **~3-month gap every year (roughly June–August) that today has zero simulated
content in it.** Real World Cups and continental championships are themselves played in
exactly that window (June/July), specifically *because* that's when domestic seasons pause in
the real world too. **This means the tournament finals (the actual month-long event) can be
scheduled entirely inside the existing off-season gap, colliding with zero club fixtures by
construction** — no `FixtureImportance`-ladder conflict resolution is even needed for the
tournament itself, only for its *qualifiers* (4.3).

This is a much better fit than trying to route the tournament through the existing
`WindowKind::InternationalBreak` window (`calendar_loop.rs:39-42`, currently ~15 days at
`start_day: 30`) — that window is sized for a single round of call-ups (Doc B's shipped
design), nowhere near long enough for a multi-week final tournament. **Recommendation: World
Cup/continental-championship finals are a new calendar concept — an "off-season tournament
window" distinct from the in-season `WindowKind::InternationalBreak` — not a scaled-up version
of the existing break window.** [DECISION NEEDED: confirm this reading rather than trying to
stretch the existing international-break window to tournament length.]

### 4.3 — Qualifying: spread across the 3 non-tournament seasons' existing international-break windows

Real World Cup qualifying is a long campaign (~8-10 matches per team over ~18 months); this
game's existing calendar has exactly **one** ~15-day `InternationalBreak` window per season
(`calendar.rs:22`, week 4). To fit a qualifying campaign into the existing calendar shape
without adding new in-season break weeks (a bigger, separate calendar-layout change this doc
does not propose):

**[DECISION NEEDED — concrete number, Design's placeholder only]:** 2 qualifying fixtures per
international-break window, across the 3 non-tournament seasons preceding a tournament season
(a World Cup in season 5 means seasons 2, 3, 4 each carry 2 qualifiers = 6 total qualifying
matches per cycle). This is a deliberately modest scale for a life-sim rather than a full
football-manager-grade qualifying simulation (real campaigns run 2-3x this many matches) —
flag explicitly: is 6 qualifiers per cycle enough to feel like "qualifying," or does Tùng want
more (which would require the bigger, separate change of adding more in-season international-
break windows per `calendar.rs`'s `WEEK_MATCH_COUNTS` layout)?

Qualifying fixtures **do** need Slice 1's conflict resolution — they land inside the existing
in-season `InternationalBreak` window, which (per the bible's own pseudocode, `docs/MAIN.md:
1106-1107`) already always wins against any club fixture that would otherwise land there. This
is the one place this slice touches the priority ladder: confirming that international-window
exclusivity, not a same-day `FixtureImportance` comparison, is what already keeps qualifiers
clash-free with league play (no new mechanism needed here beyond what Slice 1 wires up).

### 4.4 — Simulated result, not just call-ups: this is the "future doc" Doc B flagged

Doc B explicitly scoped "simulating actual international tournament outcomes" as **out of its
round**, flagging it as a probable future task (`TASK-DESIGN-round2-national-team-tactical-
identity.md`'s B.4 and "Out of scope" section). **This is that future doc — tournament
simulation is explicitly IN scope here**, since the task brief names "World Cup" as a
first-class competition kind expected to produce a real champion, not just a call-up/caps
flavor event. Concretely: qualifying groups (regional, seed-deterministic draw, same pattern as
Slices 2-3) produce a small number of finalist nations; the finals tournament is simulated as a
group stage + knockout (or single-elimination only, matching Slice 3.3's fixture-format
decision — **[DECISION NEEDED, same open question as 3.3, decide once for both]**) using
national-team strength derived from `TacticalIdentity`/`stature`-weighted eligible-population
quality (reusing Doc B's existing "recompute eligibility fresh each window, no persisted
roster" pattern — B.2's shipped design — rather than inventing a second national-squad
concept).

### 4.5 — Legacy consequence: new `LegacyEvidence` fields, following the exact `career_caps` precedent

**Verified gap, same shape Doc B already flagged and partially closed:** `LegacyEvidence`
(`crates/goat-meta/src/legacy.rs:10-41`) has `career_caps`/`career_international_goals` but
**no field for "won a World Cup" or "won a continental championship" at all.** Shipping
tournament simulation with zero legacy consequence would repeat exactly the gap Doc B called
out for caps — a player could captain their nation to a World Cup win and it would have zero
effect on any `School::score` ranking. **Recommendation, mirroring Doc B's own precedent
exactly:** add `career_world_cups_won: u32` and `career_continental_championships_won: u32` as
new raw `LegacyEvidence` counters (no new school-weighting logic required this round, same
"cheap, mechanical, raw evidence for a future axis pass" framing Doc B used for caps) — folded
at `ApplySeasonEndLegacy`, same pipeline position as every other counter.

- TDD: `crates/goat-core/tests/` new test mirroring the existing `pc_career_caps` folding test
  exactly, for the two new counters. `crates/goat-world/src/` new module (`world_cup.rs` or
  similar) — `qualifying_group_draw_deterministic_per_seed`, `tournament_produces_exactly_one_
  champion`, `world_cup_and_continental_championship_never_land_in_the_same_season` (structural
  invariant on the 4-year/2-year-offset cadence from 4.1).
- Playable gate: `cargo run -p goat-tui` → advance a career through a World-Cup season → a
  qualifying-window flashpoint appears (reusing Doc B's shipped call-up UI, not a new screen)
  → advancing through the off-season shows the tournament resolve → if the PC's nation wins,
  a legacy counter increments, visible from the existing [G] Legacy screen.

**Size: extra-large, risk: high.** This is arguably the single riskiest slice in the whole doc:
it needs a genuinely new tournament-bracket engine (group stage + knockout, a shape nothing in
the codebase has today — even Slice 2/3's single-elimination brackets are simpler), a new
calendar concept (the off-season tournament window, 4.2), and a cadence-correctness invariant
that must hold for the entire length of a career (20-40+ seasons). Strongly recommend building
this **after** Slices 1-3 are solid — the single-elimination bracket machinery Slices 2/3 build
is a direct prerequisite/precedent for this slice's knockout stage, so building this first would
mean building bracket logic twice.

---

## Slice 5 — `SuspensionLedger` scoping + final conflict-resolution wiring across all 7 competition kinds

**Why last:** finalizing the real `FixtureImportance` ordering and `SuspensionLedger` scoping
needs every competition kind from Slices 1-4 to actually exist first — this slice is the
integration pass, not new mechanics of its own.

### 5.1 — `SuspensionLedger`, replacing the single global `pc_suspension_weeks` scalar

```rust
// crates/goat-calendar/src/clock.rs or a new suspension.rs
pub struct SuspensionLedger {
    pub player_id: PlayerId,   // or a bare index, matching however goat-core identifies players
    pub competition_id: CompetitionId,   // scoped per competition — a cup ban doesn't bleed into league
    pub matches_remaining: u32,          // decrements only when a match of THIS competition is played
}
```

`crates/goat-core/src/state.rs:903-905`'s `pc_suspension_weeks: u32` (currently one scalar,
decremented once per resolved round with zero competition awareness) is replaced by a small
`Vec<SuspensionLedger>` (in practice tiny — the PC is rarely suspended in more than one
competition simultaneously, but the type must support it correctly). **Suspension counts by
match actually played in that exact competition, not by elapsed calendar days or rounds of any
other competition** (bible AC-06, already correctly implemented today for the single-
competition case at `state.rs:901-905` — this slice generalizes that existing correct logic
across competitions, it doesn't fix a bug in it).

### 5.2 — `goat-save::VERSION` bump

Adding `Vec<SuspensionLedger>` (replacing a bare `u32`) is a save-format change —
`goat-save::save::VERSION` (currently 10) bumps to 11, with a backward-compat test reading an
old single-scalar `pc_suspension_weeks` save and converting it into a single league-scoped
`SuspensionLedger` entry, following the exact v9→v10 precedent already in `save.rs`.

### 5.3 — Congestion-score sanity check across the full competition set

`goat-calendar::engine::congestion_score` (`engine.rs:91-96`, currently a simple 10-day-window
fixture count) needs re-checking once a PC's club can simultaneously be in League + Domestic Cup
+ up to 3 rounds deep in a continental tier, while the PC personally might also have a
national-team call-up — this is the point where fixture density is genuinely at its highest in
the whole game and `shouldFlushSoft`'s soft-flashpoint-batching feel (`engine.rs:266-268`,
currently a bare count threshold, `SOFT_FLUSH_THRESHOLD = 3`) most needs real playtesting, not
just unit tests. **Not a new number to invent here — flagging that this is where the existing
`TASK-TUNE` convention (first-pass placeholder constants, tune later against a prototype) most
needs to be applied, once all of Slices 1-4 are actually playable together.**

- TDD: `crates/goat-core/tests/` — `suspension_in_one_competition_does_not_block_availability_
  in_another` (the concrete regression this slice exists to prevent), a save-roundtrip test in
  `crates/goat-save/tests/save_roundtrip.rs` for the v10→v11 migration.
- Playable gate: `cargo run -p goat-tui` → PC picks up a domestic-cup suspension → next league
  fixture still selects the PC normally → next domestic-cup fixture correctly benches them.

**Size: medium, risk: medium.** Smaller than Slices 1-4 individually, but it's the slice that
proves the other four actually compose correctly, which is exactly the kind of integration risk
that doesn't show up until it's attempted — do not skip real playtesting here in favor of unit
tests alone.

---

## Out of scope (do not fold into this doc)

- **Continental-cup-via-domestic-cup-winner** (a real-football mechanic: e.g. an FA Cup winner
  who missed continental qualification on league position gets a Europa/Conference berth
  anyway) — not requested, adds a timing dependency between the cup final and continental
  qualification cutoff that this doc does not resolve. Flag as a possible future enhancement.
- **Group-stage continental/World-Cup format** — Slices 3.3/4.4 recommend single-elimination
  (with two-legged ties) as the first-pass default over a full group-stage format, pending
  Tùng's explicit confirmation either way; this doc does not design the bigger group-stage
  version.
- **Variable qualifying-campaign size / extra in-season international-break windows** — 4.3's
  "6 qualifiers per cycle" fits inside the *existing* single in-season break window; a larger,
  more realistic qualifying campaign would require changing `goat-world::calendar`'s
  `WEEK_MATCH_COUNTS` layout itself (more break weeks per season), which is out of this doc's
  scope — flagged as a real possibility, not designed here.
- **Youth/age-group national tournaments, women's competitions, or any second parallel
  national-team structure** — out of scope, matching Doc B's own "singular senior team per
  nation" scoping; nothing here revisits that.
- **Rebalancing continental-slot bands (3.2) as the world's stature distribution is observed
  over real playtesting** — `TASK-TUNE` territory once this ships, not a design decision here.
- **Club finances/prize money for winning any of these competitions** — bible §7.3's AI-club-
  economy item is separately parked (round-2's parked list); this doc adds fixtures and a
  simulated result, not any money flow from winning them.
- **Media/pundit coverage of cup runs or tournament outcomes** — bible §8.7's pundit-credibility
  item is separately parked; unaffected by this doc.

## Decisions Design needs from Tùng before Dev starts (collected from above)

1. **1.2**: the proposed `FixtureImportance` ladder (10 variants, national-team fixtures
   excluded entirely and handled by window-exclusivity instead) — confirm the relative order,
   especially `ContinentalTier1` vs `DomesticCupLate`.
2. **2.1**: the exact entry-by-tier round assignment and resulting 8-round bracket shape (Tier
   3→R1, Tier 2→R2, Tier 1→R3, byes at R3/R5/R6/R7) — Design's proposal from the confirmed
   20/20/20 club counts, not a number Tùng gave directly.
3. **3.1**: confirm continental qualification reads *Tier-1 domestic league position only* (not
   "top N clubs across all 3 tiers of a nation") — the reading Design believes is correct by
   analogy to real UEFA competitions, but stated nowhere explicitly in the task brief.
4. **3.2**: the stature-rank-band → slot-count table (4 bands, Tier-1 slots 2-5) — Tùng
   confirmed "3 tiers, top 4-5, varies by ranking" conceptually; every concrete number in the
   table is Design's proposal.
5. **3.3 / 4.4 (same question, decide once for both)**: single-elimination-with-two-legged-ties
   (Design's recommendation, less new code) vs. a full group-stage-then-knockout format (more
   authentic to real UCL/World Cup, meaningfully bigger scope) for continental tiers and
   national-team tournaments.
6. **4.1**: does the PC's career start in a World-Cup year, a continental-championship year, or
   neither? Design picked season-1-is-a-World-Cup-year arbitrarily.
7. **4.2**: confirm the off-season-gap tournament window is a new calendar concept, separate
   from the existing in-season `WindowKind::InternationalBreak`, rather than an attempt to
   stretch the existing short window to tournament length.
8. **4.3**: qualifying-campaign size — Design's placeholder is 2 qualifiers per international
   window across the 3 non-tournament seasons (6 total per cycle); confirm this is enough, or
   flag that a bigger campaign requires a separate change to `goat-world::calendar`'s break-week
   layout.
9. **4.5**: confirm `career_world_cups_won`/`career_continental_championships_won` as new raw
   `LegacyEvidence` counters is in scope this round (mirrors Doc B's own `career_caps`
   precedent) — without this, tournament simulation ships with zero legacy consequence, the
   same gap Doc B flagged and this doc would otherwise repeat.
10. **Sequencing**: this doc explicitly recommends Slices 1 → 2/3 (can run somewhat in parallel
    once 1 lands, both build straightforward single-elimination brackets) → 4 (reuses 2/3's
    bracket machinery, adds the harder cadence/calendar-window design) → 5 (integration pass,
    needs 1-4 all present to test real composition). Given this doc's overall size, strongly
    recommend treating it as **multiple separate implementation rounds**, not one Dev pass, the
    same way Doc A recommended for its own extra-large A2 slice.

## Definition of done (once Dev implements)

1. `cargo test --workspace` green, including new tests per each slice's TDD anchor and updated
   `goat-calendar`/`calendar_loop.rs` tests for the Slice 1 conflict-resolution wiring.
2. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. No new dependencies without an explicit, separately-confirmed exception.
4. `goat-save::save::VERSION` bumps to 11 (Slice 5's `SuspensionLedger` change), with a
   backward-compat test per the existing v9→v10 precedent.
5. Playable gates for Slices 1-5 all pass via `cargo run -p goat-tui`.
6. A real perf check once Slice 3/4 land: qualification computation across 20 nations' Tier-1
   tables plus a World-Cup-cycle simulation both need timing, same discipline Doc A's A2.5/A3.1
   established for genesis/replay cost — not estimated blind here.
7. No floats in sim state/logic, no unsafe — existing project-wide constraints.

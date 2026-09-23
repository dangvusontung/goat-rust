# TASK-PA2 — World → Match: Roster-Driven Matches (M1 + M1.5)

**Project:** BECOME THE GOAT
**Scope (this task):** milestones **M1** (done) and **M1.5** (this round) — roster-derived
tactical profiles + the manager/selection model. Milestones M2/M3/M4 are listed under
"Roadmap" for context but are NOT this task.
**Status:** M1 ✅ (commit `b9361ed`) · M1.5 ✅ (`f1ef256`, formation fix `2005f3a`) ·
M2 ✅ IMPLEMENTED (2026-09-23) — SquadSheet + matchup-driven contests (A.5) +
real-name commentary; golden seed 42 re-frozen 52/2-2 → 57/1-1 (justified in
`golden_match.rs` + sim-analysis); `scripts/test.sh` green; match-batch re-validated.
**Relationship to prior design:** PA2 design thread (2026-09-23, Tùng + assistant). Engine tuning rounds 2/3 (commits `177dc98`, `09869f1`, `f8f7e6c`) are orthogonal — M1 must not invalidate them.

---

## Origin

Matches are currently the PC + 21 anonymous NPCs: individual teammate/opponent attributes
have zero influence — a single static scalar (`world.clubs[id].strength`, set once at
worldgen) feeds `TacticalProfile::derive`. The 63,600-player roster built in Phase A/B/C
exists only for pantheon/rival at career end. Tùng approved the 5-milestone plan
(M1 → M1.5 → M2 → M3 → M4) to wire the real world into matches; M1 is the low-risk first step.

## Verified current state (read before touching anything)

- `MatchSetup` (`crates/goat-match/src/sim.rs:410`) takes `own_profile`/`opp_profile:
  TacticalProfile` — the engine reads ONLY the 3 line scalars + 4 style weights. No NPC
  attribute is ever read. Do not change this in M1.
- `TacticalProfile::derive(strength, club_id, world_seed)` (`crates/goat-core/src/tactical.rs:45`):
  lines = strength ± jitter; styles are seed-derived and strength-INDEPENDENT.
- Live match path: `crates/goat-tui/src/main.rs:622` `make_setup` — own/opp profiles from
  static club strength. Academy path (`main.rs:967`) is out of scope.
- Roster IS queryable per player: `Population::promote(idx, elapsed_weeks, name, world) ->
  Option<PlayerView>` (`crates/goat-world/src/population.rs:211`) — full 30 attrs, deterministic,
  refuses retired players. `current_ovr` (`population.rs:196`) is the cheap ranking key.
- The live loop holds no `Population`; the pantheon path (`main.rs:2061`) shows the house
  pattern: rebuild via `genesis(world_seed)` + replay `batch_tick_season` per completed season
  (youth intake keeps squads at 25 forever — without replay, late-career squads would be all
  retirees). Cost is trivial; determinism is the spine.
- `elapsed_weeks` for the population = `state.pc_epoch_day / 7` (persisted since save v6).
- Attr groups for line computation: `SHOOTING_ATTRS`, `DRIBBLING_ATTRS`, `PASSING_ATTRS`,
  `DEFENDING_ATTRS` (`crates/goat-core/src/attrs.rs:210-213`).
- NPC names exist: `history::name_from_seed(seed)` (`crates/goat-world/src/history.rs:85`).

## Decision (locked by Tùng, 2026-09-23)

M1 scope, exactly:

1. **Lineup, both teams: simple top-11 by `current_ovr`** at the current date, skipping
   retired. The PC always starts (current behaviour) — the PC occupies one of the 11 slots,
   so the own-team profile averages top-10 NPCs + the PC's real attrs (a strong PC finally
   lifts his team's profile — addresses sim-analysis issue #6).
2. **`TacticalProfile::from_squad`** (new, goat-core): lines = mean of the lineup's actual
   attribute groups (attack = SHOOTING+DRIBBLING, midfield = PASSING, defense = DEFENDING);
   styles keep coming from `derive(50, club_id, world_seed)` (club identity, seed-stable).
3. **Population in the live loop**: rebuilt per match week via genesis + season replay
   (pantheon pattern).
4. **Engine, golden tests, save format: UNCHANGED.** `MatchSetup` unchanged,
   `golden_match.rs` untouched, save stays v10. Harnesses (`match-batch`, `career-sim`)
   keep their explicit controlled profiles — they are measurement tools, not the game.

## Out of scope (later milestones)

- **M1.5** — manager profile (personality/nationality/age derived from club seed),
  `manager_trust` + `manager_favor` (save v11), deep PC-team lineup selection (PC can be
  benched), formation from club style, thin hooks into wage/transfer/media.
- **M2** — individuals in the contest (MATCH.md A.5), real names in commentary, SquadSheet.
- **M3** — NPC stat accumulation from orbit matches, real NPC form, save v12.
- **M4** — substitutions (PC subbed on/off mid-match; side-stream RNG so goldens survive;
  `minutes_played` weighting for rating/form). **Tùng's note, do not lose:** a PC returning
  from injury ("mới què dậy") may be thrown on for the final few minutes to regain match
  fitness — because he plays few minutes, the weight on outcome/rating must be SMALL,
  matching M4's minutes_played weighting design.

## DoD (M1 — done, commit `b9361ed`)

- [x] `TacticalProfile::from_squad` + unit tests (lines reflect roster attrs; styles
      deterministic per club; lines clamp 1..=99).
- [x] Lineup picker in goat-world (top-11 OVR, skips retired, deterministic) + unit tests.
- [x] Live match path builds both profiles from real squads (own includes PC attrs).
- [x] `scripts/test.sh` fully green (fmt, clippy -D warnings, workspace tests, career-sim
      invariants + seed scan). No golden re-freeze needed — engine untouched.
- [x] Diff contains no changes to `crates/goat-match/` or `crates/goat-save/`.

---

# M1.5 — Manager & Selection Model

**Locked by Tùng (2026-09-23):** save bumps are free (dev phase, no careful migration);
commentary with real names is M2 (not here); opponent lineup stays simple top-11 OVR;
the PC's club selection must be deep ("as real as possible"); the PC CAN be benched for
a whole match (auto-sim, output 0, no trust/form update when not playing); manager
personality derives from club seed and MAY modulate trust; formation from club style is
in M1.5, not deferred to M3.

## Verified current state (M1.5 additions)

- `goat-core` is headless: it must NOT depend on `goat-world`. Manager profiles live in
  `goat-world`; trust/favor *values* live in `WorldState`; base values are computed in the
  TUI and passed into core via intent fields (the `InitWorld`/`ExecuteTransfer` pattern).
- Direct field mutation from the TUI has precedent (`state.pc_discipline_rep` at
  `main.rs:763`), but trust/favor updates go through a new intent for testability.
- Injury does NOT currently block playing (`run_next_round` checks only
  `pc_suspension_weeks`). M1.5 makes injury a hard lineup exclusion (locked design:
  "injury/suspension loại cứng").
- `Intent::RespondToMedia` is orphaned (only tests call it). M1.5 wires it after red cards.
- NPCs have no wages: the "top earner" selection factor is proxied by PC wage vs a
  club-scale band (`pc_wage_annual >= club_strength * 3`).
- `best_role_for_position` (`main.rs:1124`) maps PC position → role for rating.
- Harnesses (`match-batch`, `career-sim`) build their own controlled `MatchSetup`s and do
  NOT go through `run_next_round` — selection changes cannot perturb them.

## Design (locked)

### Manager profile — `goat-world/src/manager.rs` (new)

Derived from `(world_seed, club_id)`, never saved (tiny-saves rule):

- `ManagerPersonality`: `Strict | Balanced | StarLover` (rng % 3), in the spirit of
  `RefPersonality::from_rng` (`goat-match/src/discipline.rs:29`).
- Nationality: 60% the club's nation, else uniform over `NUM_NATIONS`.
- Age: 38 + rng(0..23) → 38–60.
- Name: `history::name_from_seed` on the manager seed.
- Seed: `world_seed ^ MANAGER_SALT * (club_id + 1)`; fixed draw order
  (personality, nation-roll, [nation], age).

### `pc_manager_trust` / `pc_manager_favor` — i32 0–100, path-dependent (save v11)

**Trust = professional judgement** (performance/attitude):
- Base on arrival (new game, transfer): `50`, +6 if `pc_age + 12 >= mgr_age` (veteran
  respect); if `pc_age <= 20`: Strict −5 / StarLover +3.
- Per match played: `raw = ((output − 50) / 8).clamp(−4, 4)`; if `raw < 0`: Strict ×3/2,
  StarLover ×1/2 (personality modulation, approved).
- Training attitude (per round, from `pc_week_training_done` + `Routine.intensity`):
  not trained → Strict −5 / Balanced −3 / StarLover −2; trained: High +2, Medium +1, Low 0.
- Benched week: NO trust change (locked), NO form update (output 0 path already skips the
  EMA in `ApplyRoundResult`).

**Favor = personal bias/politics** (independent of performance):
- Base (recomputed as drift target each round):
  `50` + same-nation PC↔manager +8
  + marketability ≥ 70: StarLover +8 / Strict −8 (sign flips with personality)
  + `((fan_rep − 50)/10).clamp(−5, 5)` + `((character_rep − 50)/10).clamp(−4, 4)`
  + discipline_rep ≥ 70 → −6, ≤ 25 → +3
  + lifestyle: Flashy → Strict −10 / StarLover +5; Professional → Strict +6.
- Drift: each round favor moves ±1 toward the recomputed base (sticky, path-dependent).
- Event shocks: `AgitateForTransfer` → trust −5, favor −3; defiant media response after a
  red card with a Strict manager → favor −4.
- "Same agent" source: DROPPED (no agent identity in game). Locker-room conflict: proxy via
  `power_ladder` (real conflict system deferred).

### Lineup selection — PC's club (deep) / opponent (top-11 OVR, unchanged)

- **Formation from club style (in M1.5):** dominant style of `TacticalProfile::derive(50,
  club_id, world_seed)` → outfield slot split (D/M/F). The population has NO goalkeeper
  entity (positions are D/M/F only), so slots follow the real football convention and
  **sum to 10** — the 11th man is an implicit abstract GK who contributes nothing to the
  line means (they are averages; n=10 stays comparable to the opponent's top-11-OVR mean):
  Pressing (4,3,3) · Possession (3,5,2) · Counter (5,4,1) · WingPlay (3,4,3).
  *(Fixed after review: the first cut summed to 11, implicitly fielding 11 outfielders.)*
- **NPC score** = `current_ovr` + seeded weekly form noise ±10; seeded ~3%/week
  availability exclusion (injury/suspension abstraction; nothing saved).
- **PC score** =
  `role_rating` (best role for position, familiarity already inside)
  + `(form − 50)*3/10` + `(trust − 50)*2/5`
  + fit by familiarity tier: Awkward −6 / Unconvincing −2 / Competent 0 / Natural +4
  + first season at club: `(academy_hype/10).clamp(−5, 5)`
  + top-earner proxy (`wage >= strength*3`): +5
  + `(fan_rep − 50)/10` + marketability ≥ 70: +3
  + `−8 * power_ladder`
  + energy: <60 → −4, <40 → −10.
- **Ranking**: PC competes inside his position group for that group's slots. If below the
  cutoff by ≤ 5 points → borderline roll `pct = 50 + diff*10 + (favor − 50)/5`, seeded per
  week (deterministic). Below by > 5 → benched. Above → starts.
- **Benched path**: auto-sim the PC fixture from the profile means; no
  `ApplyMatchResult`/`ApplyCardResult`; `ApplyRoundResult` with `pc_output = 0` (form/stat
  skip is already gated on `pc_output > 0`).
- **Own-team profile** becomes formation-aware: slot-split lineup; when the PC starts he
  takes one slot in his group (his real attrs in the mean); when benched, 11 NPCs.
  Opponent profile unchanged (M1 top-11 OVR).

### Thin hooks (M1.5)

- Wage renewal: `new_wage += (trust − 50)/5` in `run_contract_negotiation`.
- `AgitateForTransfer` handler: also trust −5 / favor −3.
- Media: after a red card, prompt contrite/defiant → `Intent::RespondToMedia` (finally
  wired); defiant + Strict manager → favor −4.

### Hard exclusions

Injury (`injury_weeks > 0`) or suspension → PC cannot start (bench path, existing
suspension path kept for suspensions).

## Out of scope (M1.5)

- Engine/`MatchSetup`/golden tests: UNCHANGED. Diff must not touch `crates/goat-match/`.
- M2 individuals-in-contest, real-name commentary, SquadSheet.
- M3 NPC stat accumulation / real NPC form (weekly noise here is ephemeral by design).
- M4 substitutions (incl. Tùng's note: returning-from-injury PC gets late minutes to regain
  match fitness, with outcome/rating weight scaled by `minutes_played`).
- Locker-room conflict as a real system (power_ladder proxy only).

## DoD (M1.5)

- [x] `manager.rs`: profile derive deterministic; personality/nation/age bounds; trust/favor
      formula unit tests (sign flips, clamps).
- [x] Selection unit tests: strong PC starts at a weak club; weak PC can be benched at a
      strong club; borderline roll deterministic per week; formation slots sum to 10
      (outfield only — GK is abstract, no GK entity in the population);
      ~3% NPC unavailability over many weeks.
- [x] `pc_manager_trust`/`pc_manager_favor` in `WorldState` + save v11 roundtrip.
- [x] Live loop: bench path (auto-sim, output 0), trust/favor drift after matches,
      formation-aware own-team profile.
- [x] Hooks: wage renewal trust term, agitate trust/favor hit, red-card media prompt.
- [x] `scripts/test.sh` fully green; no `goat-match` diff; goldens untouched.

### Implementation notes (M1.5, as built)

- `first_season_at_club` is proxied by `pc_seasons_played == 0` — the academy-hype term
  effectively only fires for a fresh breakthrough in season 1 (academy hype only exists
  on the academy path anyway). A mid-career transfer gets his "fresh start" through the
  fan_rep reset (40) instead. If a real per-club tenure counter is wanted later, that's a
  small follow-up field.
- The red-card media prompt only fires in interactive weeks (`play_interactive`); auto-sim
  weeks (`[K]`) never prompt and never touch `RespondToMedia`.
- NPC weekly unavailability is 1-in-33 (~3%), form noise ±10 — both seeded per
  (player seed ^ week seed), ephemeral, never stored (M3 owns real NPC form).
- Smoke runs (seed 7, Forward): a 64-OVR 16-year-old at a 5★ Premier club under a Strict
  manager is benched 8/8 (trust base 45) — earning a place at an elite club is meant to be
  hard; the academy/weak-club routes are the on-ramp. At a Third Division club the same PC
  starts while training/output hold up, then loses his place when he skips training
  (trust erodes −2/round under `K`-skip). One observed dynamic to watch: a Counter-style
  club (5-4-1 after the outfield-only fix) has a single forward slot, so forwards there
  are benched more often —
  realistic, but M4 (late-game sub cameos) is the intended pressure valve.

---

# M2 — Individuals in the contest + real-name commentary

**Approved by Tùng (2026-09-23).** This IS an engine change: golden seed 42 will be
re-frozen (justification required) and match-batch re-validated. Counter stays 5-4-1
(locked). M1.5b (display-only names) is parked — M2 supersedes it.

## Verified current state (read before touching anything)

- `MatchSetup` (`goat-match/src/sim.rs:428`) has no NPC slot; the engine reads only the
  3 line scalars + 4 style weights. 9 construction sites: `golden_match.rs` (2),
  `match_batch.rs` (1), `career_sim.rs` (3), `main.rs` (2 — first-team + academy),
  `goat-bridge/api.rs` (2), `goat-tui/tests/full_sim.rs` (1).
- Contest difficulty: `scaled_difficulty` (`sim.rs:1196`) — authored base +
  `(line_stat − 50)/2` where line_stat is opp.attack/defense/mid by the action attr's
  family. No individual is ever read (violates MATCH.md A.5's locked principle).
- Generic text lives in `beats.json`: `att_assist` ("your teammate finishes it off!"),
  `auto_goal_for_header` ("your teammate rises highest"), `auto_goal_against_header`
  ("Their forward meets the cross"), plus "their forward/winger/full-back/midfielder/
  striker" in several situations and "him/he" in outcomes (`att_beat_man`, `att_won_fk`,
  `def_beaten_wide`, `def_dragged_out`).
- Texts are stored verbatim into `MomentSummary.setup_text/outcome_text` at beat-build /
  auto-beat time — the slot fill must happen there, not at render.
- `beats_test.json` is referenced only by docs — no code loads it; left alone.
- goat-match must NOT depend on goat-world (layering): squads arrive as data; the real
  names (`history::name_from_seed`) are resolved by the *caller* (goat-tui).
- Live loop already computes everything a SquadSheet needs: formation lineup indices
  (M1.5) + `Population::promote` (full attrs) + `name_from_seed`.
- Bridge (`goat-bridge/api.rs:792,1463`) builds its own static-profile setups for the
  Flutter client — out of M2's gameplay scope; it gets stub squads to compile (real
  squads there are a client milestone, see docs/CLIENT-IMPL.md).

## Design (locked)

### (c) SquadSheet — `goat-match/src/squad.rs` (new)

```rust
pub struct SquadPlayer { pub name: String, pub position: u8 /*0=D,1=M,2=F*/,
                         pub attrs: [Fixed; NUM_ATTRS], pub is_pc: bool }
pub struct SquadSheet { pub players: Vec<SquadPlayer> }  // starting XI
```

`MatchSetup` gains `own_squad: SquadSheet, opp_squad: SquadSheet` (mandatory — no
dual code path). `SquadSheet::stub(strength, seed)` gives deterministic synthetic
sheets for harnesses/tests/academy/bridge (attrs centred on strength with seeded
variance; names from a small built-in pool — goat-match stays world-independent).
The live loop (goat-tui) builds REAL sheets from the M1.5 lineup: own = formation
lineup (PC flagged `is_pc`) via `lineup_indices_formation`, opp = top-11 OVR via
`lineup_indices`; each promoted with `name_from_seed(pop.seed[idx])`.

### (a) Contest-level individuals (MATCH.md A.5)

In `build_beat`, the PC's contest now has a specific opponent on the far side:

- **Matchup pool** from `opp_squad`: side=attack → opp defenders (midfield zone →
  midfielders); side=defend → opp forwards (midfield zone → midfielders). Fallback:
  whole squad. One seeded draw per beat.
- **Counter-stat**: by the action attr's family, read off the matchup's real attrs —
  PC attacking attrs (shoot/pass/dribble) → his mean(DEFENDING); PC defending attrs →
  his mean(SHOOTING+DRIBBLING); others → mean of those groups.
- **Blend (locked 50/50):** effective stat = `(line_stat + matchup_stat) / 2`, then the
  existing `(stat − 50)/2` difficulty scaling applies unchanged. Squad quality stays
  felt through BOTH the team line and the specific man — a 90-rated CB is genuinely
  harder to beat than his 60-rated partner on the same team.
- The matchup's name also fills `{opponent}` in this beat's texts — the man you beat
  (or who beat you) is named consistently.

### (b) Template slots in beats.json

- `{opponent}` — the beat's matchup (both directions).
- `{scorer}` — goal outcomes: PC-action attack goal where the teammate finishes → own
  NPC (position-weighted pick FWD3/MID2/DEF1, PC excluded); defend-side `goal_against`
  → the matchup himself; auto `goal_for` → own NPC forward-weighted; auto
  `goal_against` → opp NPC forward-weighted.
- `{assist}` — own NPC (mid/forward pick).
- Rewritten texts: `att_assist`, `att_beat_man`, `att_won_fk`, `def_beaten_goal`,
  `def_beaten_wide`, `def_dragged_out`, all 6 `auto_goal_*`, and the situations with
  "their forward/winger/full-back/midfielder/striker".
- Filled at build/auto-beat time via a per-beat cast (matchup + scorer + assist, 3
  seeded draws), stored pre-filled in `MomentSummary`.

### Golden + validation

- Golden seed 42 re-freeze EXPECTED: new RNG draws (matchup/name picks) + the 50/50
  difficulty blend change contest math. Flow rules (possession/zone/momentum/auto-goal)
  are untouched. Justification goes in the commit message + sim-analysis.
- match-batch 100k before (already logged, post-M1.5 run) vs after — distribution must
  stay sane (W/D/L, goals, clean sheets, SiD, rating dist); mean rating may move
  slightly from matchup variance.
- Harnesses (`match-batch`, `career-sim`, `full_sim`) use `SquadSheet::stub` centred on
  their controlled strengths; `golden_match.rs` uses fixed sheets with const names.

## Out of scope (M2)

- Actor-swap contests (A.4: you pass → HIS finishing resolves) — bigger content/beat
  change, future milestone.
- NPC stat accumulation / real NPC form (M3), substitutions (M4).
- Bridge real squads (client milestone).

## DoD (M2)

- [x] `squad.rs` + `MatchSetup` fields; all 9 construction sites updated; stub sheets
      deterministic.
- [x] Matchup-driven difficulty (50/50 blend) — a strong CB measurably harder to beat
      than his weak partner (unit test `strong_matchup_harder_than_weak_partner`).
- [x] beats.json fully templated; no "your teammate"/"their forward" survives in
      commentary output; `{scorer}/{opponent}/{assist}` filled from real squads in the
      live game (TUI smoke shows real names).
- [x] Golden re-frozen with justification; `scripts/test.sh` green.
- [x] match-batch 100k before/after table in docs/sim-analysis.md.

---

# M3 — NPC stat accumulation + real NPC form (save v12)

**Approved by Tùng (2026-09-23).** Deep change across goat-match (goal credits with
identity), goat-world (orbit overlay → population columns, form-based selection),
goat-core (state + intent) and goat-save (v12). Bridge/Flutter client explicitly
parked. Engine change must be **flow-neutral**: no RNG draw added/moved, so the
golden seed 42 match must pass WITHOUT re-freeze — that is the proof.

## Verified current state (read before touching anything)

- `pc_goals` is over-credited in the live loop (`main.rs:924`): it counts EVERY
  `GoalFor` moment — auto-beat team goals (a teammate is literally named in the
  text) and `att_assist` ("{scorer} finishes it off!") all land in the PC's
  career tally. `match_batch.rs:135` / `career_sim.rs:362,504` have the same
  flaw through `is_action && GoalFor` (counts `att_assist` as a PC goal).
- beats.json score-event inventory (all 10): `att_goal_clean`/`att_goal_scramble`
  (attack success, no `{scorer}` → PC goal), `att_assist` (attack success with
  `{scorer}` → teammate goal, PC assist), `def_beaten_goal` (defend failure,
  `{scorer}` = the matchup → opponent goal), 6× `auto_goal_*` (drawn `{scorer}`
  from the possessing squad, no assist). No text uses `{assist}`.
- `SquadPlayer` has no identity — names only, so credits cannot reach the
  population. `build_beat`/`auto_beat` discard the picked `&SquadPlayer` after
  cloning the name.
- Population career columns (`career_goals/apps/titles`) are fed ONLY by
  `batch_tick_season`, which credits ALL divisions including the PC's — orbit
  individuals get season-abstraction stats regardless of what actually happened
  in deep-simmed PC matches. The deep orbit (weekly `sim_team_match` for every
  division fixture) produces no individual residue at all.
- NPC "form" in selection is `NPC_FORM_NOISE` ±10 seeded ephemeral
  (`population.rs:414`) — nothing persists, no feedback loop.
  `lineup_indices_formation` is pure top-OVR per group (deterministic).
- Population is rebuilt pantheon-style at two sites (`main.rs:646`, `main.rs:2442`)
  from genesis + `batch_tick_season(1..season)`. Save stores only `world_seed`
  for the world side.

## Design (locked)

### 1. Engine: goal credits with identity (goat-match) — flow-neutral

- `SquadPlayer` gains `id: Option<u32>` — population index when the sheet is
  built from the real world; `None` for stubs and for the PC entry.
- New types (beats.rs): `GoalActor { Pc, Npc(Option<u32>) }`,
  `GoalCredit { event: ScoreEvent, scorer: GoalActor, assist: Option<GoalActor> }`.
- `GeneratedChoice` gains `success_credit`/`failure_credit: Option<GoalCredit>`,
  computed in `build_beat` from the RAW outcome text (before slot fill):
  `goal_for` + text contains `{scorer}` → scorer = drawn teammate, assist = Pc;
  `goal_for` without `{scorer}` → scorer = Pc; `goal_against` → scorer = the
  matchup. `auto_beat` credits the drawn scorer of the possessing squad.
- `ActiveMatchState` accumulates `goal_credits: Vec<GoalCredit>`;
  `MatchResult` exposes it. **No RNG draw is added, removed or reordered** —
  credits are pure bookkeeping over draws that already happen for commentary.
- Golden seed 42 must pass byte-identical (no re-freeze). match-batch 100k
  before/after must be identical on engine stats; the PC-goal-derived columns
  (pc_goals_hist, SiD, carried) SHIFT because the measurement becomes honest
  (assists no longer count as PC goals) — logged as a measurement fix.

### 2. Orbit overlay (goat-world, new `orbit.rs`)

- Record types live in goat-core (headless-safe: only u32/u8 fields):
  `NpcMatchCredit { pop_idx, goals: u8, assists: u8, result: i8 }`,
  `OrbitMatchRecord { season, round, div: u8, credits: Vec<NpcMatchCredit> }`.
  One credit per starter per PC match (an appearance is one record line).
- `Population` gains `form: Vec<i16>` (default 50; orbit players only).
  Per record: `career_apps += 1`, `career_goals += goals`, and form EMA
  `form = form*65/100 + rating*35/100` with synthetic rating
  `(58 + 14*goals + 9*assists + 6*result).clamp(30, 95)` — deterministic,
  no extra RNG.
- **No double counting**: `batch_tick_season` gains an orbit overlay parameter
  (new `batch_tick_season_orbit`, old signature delegates with `None`). When
  crediting a division the PC played in that season, per-player batch credits
  become the REMAINDER: `apps += max(0, season_apps − orbit_apps)`,
  `goals += share − orbit_goals (min 0)`. Totals stay consistent with the old
  abstraction (a starter ends ~30 apps/season) but the goals land on the real
  scorers. Titles still credit normally.
- Replay: `rebuild_population(world_seed, season, records)` =
  genesis → for s in 1..season { apply records@s; batch_tick_season_orbit(s,
  overlay@s) } → apply records@current season. Records are grouped by their
  stored `div`, so PC transfers across divisions stay correct.
- Only NPCs who actually appeared in a PC match ever get orbit records —
  the rest of the 63,600 population stays pure derive. This is the persist-vs-
  derive line: records are path-dependent (match RNG decided the scorers) so
  they MUST be saved; everything else stays replayed.

### 3. Real form in selection (goat-world)

- `select_pc`: NPC candidate score = `current_ovr + (form − 50)*3/10 + noise`,
  with `NPC_FORM_NOISE` 10 → **5** (form now carries the real signal; a small
  seeded term keeps week-to-week selection alive). Same form weight the PC
  formula uses. Closes the loop: play well → form ↑ → selected more → more
  records. NPCs untouched by the orbit keep form 50 = old behaviour ±5.
- `lineup_indices_formation` (own club only — opponent stays locked top-11 OVR
  via `lineup_indices`) ranks by `ovr + (form − 50)*3/10`, deterministic (no
  noise) so the sheet/profile stay stable within a week.

### 4. Core + save v12

- `WorldState` gains `orbit_records: Vec<OrbitMatchRecord>` (append-only) +
  `Intent::RecordOrbitMatch { record }` (pushes; nothing else).
- Save v12: appended orbit blob (count, then per record season/round/div +
  credit lines). v11 and older load with an empty overlay = pure M1.5 world.
- Live loop (`main.rs`): both population rebuilds go through
  `rebuild_population`; after every PC match (interactive, auto, AND bench
  quick-sim — starters get appearances + result-based form there too, goals
  unknown → 0) the TUI sends `RecordOrbitMatch` with credits built from
  `result.goal_credits` (Some(id) scorers only) over both lineups.
- `pc_goals` honesty fix everywhere: count `goal_credits` with `scorer == Pc`
  (live loop, career_sim ×2, match_batch) instead of counting GoalFor moments.

## Out of scope (M3)

- Assists column in Population (assists feed form only; no `career_assists`).
- Opponent lineup selection beyond top-11 OVR (locked since M1.5).
- Substitutions / minutes weighting (M4 — incl. the post-injury "few minutes
  to find his feet" cameo Tùng specified: small minutes ⇒ small rating/outcome
  weight via `minutes_played`).
- Bridge/Flutter real squads (client milestone).
- Actor-swap contests (A.4).

## DoD (M3)

- [x] `goal_credits` in `MatchResult`; golden seed 42 passes UNCHANGED
      (flow-neutrality proof); unit tests: att_assist → teammate goal + PC
      assist; auto goal → named NPC credit; PC goal → Pc actor.
- [x] `rebuild_population` + orbit-aware batch tick: no double counting
      (starter apps ≈ 30/season with full orbit coverage), real scorer's
      career_goals reflect his deep-simmed goals (unit test).
- [x] `select_pc`/`lineup_indices_formation` use the form column; feedback
      loop covered by a multi-week unit test (hot NPC rises, cold NPC sinks).
- [x] Save v12 round-trip incl. orbit blob; v11 save loads with empty overlay.
- [x] Live loop records every PC match (all 3 paths); `pc_goals` counts only
      real PC goals.
- [x] `scripts/test.sh` green; match-batch 100k + career-sim before/after
      logged in docs/sim-analysis.md (engine stats identical, PC-goal
      measurement shift explained).

---

# M4 — substitutions (pc_on_pitch, sub on/off, minutes weighting)

**Approved by Tùng (2026-09-23).** Final PA2 milestone. Golden seed 42 must stay
UNFROZEN — the sub decision runs on a SIDE-STREAM RNG that never touches the
match RNG (precedent: `RefPersonality::from_rng(match_seed ^ 0xBADCAFE)`), and
harness setups opt out via `sub_context: None`.

## Verified current state (read before touching anything)

- `tick()` (`sim.rs:706`): clock → momentum decay → possession → zone drift →
  force_reckless → `involved()` → build_beat, else `auto_beat`. No concept of
  the PC being off the pitch — he is always involved-eligible.
- M1.5 bench path (`main.rs`): a benched week is quick-simmed by
  `sim_team_match` on profile means — no engine, no scorers, output 0.
- `advance_beat` runs red-mist + frustration injection + headspace tick after
  each resolved beat (only reachable when a beat was pending, i.e. on-pitch).
- `MatchResult` has no minutes concept; `player_output` accumulates beat deltas
  from 50 regardless of how long the PC played.
- Manager personality (Strict/Balanced/StarLover) + `pc_manager_trust` exist
  (M1.5). Injury: `view.injury_weeks > 0` benches the PC; nothing records WHEN
  an injury ended (needed for the post-injury cameo rule).

## Design (locked)

### 1. `pc_on_pitch` + `SubContext` (goat-match)

```rust
pub struct SubContext {
    pub seed: u64,                    // side-stream seed (caller: match_seed ^ salt)
    pub pc_starts_on_bench: bool,
    pub manager_trust: i32,           // 0-100
    pub manager_patience: i32,        // 0-100 (Strict 30 / Balanced 55 / StarLover 75)
    pub pc_returning_from_injury: bool,
}
MatchSetup::sub_context: Option<SubContext>   // None = plays 90, never subbed (all harnesses)
```

`ActiveMatchState` gains `pc_on_pitch`, `pc_started_match`, `sub_exhausted`,
`minutes_played`, `last_on_minute`, `sub_rng: Option<GoatRng>` (side stream,
created from `SubContext.seed` — the match RNG is NEVER consumed for subs).
`tick()` gates force_reckless + `involved()`/build_beat on `pc_on_pitch`;
off-pitch ticks are pure `auto_beat`. `advance_beat` guards red-mist on the flag.
A red card closes the PC's minutes (he is off, permanently).

### 2. Sub rules (side-stream rolls, evaluated per tick)

- **Sub-on** (benched start), from minute 50: `chance = 12 + 8×goals_behind(cap 3)
  + (trust−50)/5` percent/tick, halved when leading by 2+ (rest him). Guarantees:
  trailing at 72' → on; level at 78' → on; anything but a big lead at 84' → on.
  A big lead to the end can mean a DNP (minutes_played = 0) — the M1.5
  benched-whole-match outcome survives. Window lands ~55–70' when trailing.
- **Hook** (starting PC only — a sub who came on stays), from minute 55:
  `output < 45 + (50−patience)/10 − (trust−50)/10` → 12%/tick → ~60–75'. Strict
  managers hook at ≤47, StarLovers only below ~40; high trust protects.
  Once hooked, `sub_exhausted` — no re-entry.
- **Post-injury cameo** (Tùng's locked note): `pc_returning_from_injury` and
  not selected to start → thrown on at 80'+ for closing minutes regardless of
  scoreline ("find his legs"). Few minutes ⇒ tiny rating/trust weight via §3.

### 3. `minutes_played` + linear opportunity weighting

`MatchResult.minutes_played`. When `sub_context` is present and minutes < 90:
`player_output = 50 + (raw − 50) × minutes/90` — you can only move your rating
while on the pitch; the minutes you missed blend you back toward the neutral
baseline. Linear (not sqrt): a 25-minute cameo SHOULD count ~3× less than a
full shift — this is exactly the "đá ít phút → trọng số nhỏ" rule for the
post-injury cameo. Form EMA and manager trust read the normalised output
unchanged. `minutes_played == 0` keeps M1.5 semantics: pc_output = 0 → no form
gain, no match counted, trust untouched, no energy cost. Goals scored in a
cameo still count full (a sub's goal is a goal). Normalisation is gated on
`sub_context.is_some()` so harness/golden setups are byte-identical.

### 4. Commentary (3 texts, pushed as non-action moments)

- Sub-on: "The board goes up — your number. You're on." 
- Cameo: "Gentle minutes to find your legs again — you're on for the closing stages."
- Hook: "Your number goes up. The manager has seen enough — you're coming off."

### 5. Live loop (goat-tui)

- The M1.5 quick-sim bench path is REPLACED by the engine: every week runs the
  real match with `sub_context: Some(...)` (bench-start when benched). Bonus:
  orbit records now get REAL scorers even when the PC is benched.
- Interactive "watching from the bench": render commentary moments only, no
  beat prompt while `current_beat()` is `None`; prompts begin the moment he is
  subbed on. Auto (K) path needs nothing new.
- `WorldState.pc_injury_return_week: Option<u32>` (save **v13**, appended):
  set when `injury_weeks` ticks down to 0; the live loop treats a return within
  the last week as `pc_returning_from_injury` (bench-start only — if the
  manager picks him to start, he starts).
- Energy cost scales with minutes (`25 × minutes/90`).

## Out of scope (M4)

- NPC substitutions / tactical subs for the AI sides (sheets stay static).
- Multiple subs, injury-time subs, extra time.
- Rating normalisation inside pure harnesses (sub_context None there).

## DoD (M4)

- [x] `pc_on_pitch` gating + side-stream sub RNG; golden seed 42 UNCHANGED
      (sub_context None everywhere in harnesses; no match-RNG consumption).
- [x] Scenario test: benched → subbed on (~55–70') → can score; hook test
      (strict manager, low output → off 60–75'); cameo test (80'+, small
      minutes → output pulled toward 50).
- [x] Interactive bench-watching branch; 3 commentary texts render.
- [x] Save v13 round-trip with `pc_injury_return_week`.
- [x] `scripts/test.sh` green; match-batch 100k byte-identical on engine stats
      (sub_context None) — logged in docs/sim-analysis.md.

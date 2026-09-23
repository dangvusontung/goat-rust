# TASK-PA2 — World → Match: Roster-Driven Matches (M1 + M1.5)

**Project:** BECOME THE GOAT
**Scope (this task):** milestones **M1** (done) and **M1.5** (this round) — roster-derived
tactical profiles + the manager/selection model. Milestones M2/M3/M4 are listed under
"Roadmap" for context but are NOT this task.
**Status:** M1 ✅ (2026-09-23, commit `b9361ed`). M1.5 ✅ IMPLEMENTED (2026-09-23) —
manager derive + trust/favor (save v11) + formation + selection/bench path + hooks;
`scripts/test.sh` fully green; no `goat-match`/golden diff.
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

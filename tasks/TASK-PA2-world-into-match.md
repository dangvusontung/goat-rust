# TASK-PA2 — World → Match: Roster-Driven Matches (M1)

**Project:** BECOME THE GOAT
**Scope (this task):** milestone **M1** only — roster-derived tactical profiles for the PC's match. Milestones M1.5/M2/M3/M4 are listed under "Roadmap" for context but are NOT this task.
**Status:** ✅ M1 IMPLEMENTED (2026-09-23) — `TacticalProfile::from_squad`
(`goat-core/src/tactical.rs`), `Population::lineup_indices`/`squad_avg_attrs`
(`goat-world/src/population.rs`), live path wired in `goat-tui/src/main.rs`.
No `goat-match`/`goat-save` changes; goldens untouched; `scripts/test.sh` green.
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

## DoD

- [ ] `TacticalProfile::from_squad` + unit tests (lines reflect roster attrs; styles
      deterministic per club; lines clamp 1..=99).
- [ ] Lineup picker in goat-world (top-11 OVR, skips retired, deterministic) + unit tests.
- [ ] Live match path builds both profiles from real squads (own includes PC attrs).
- [ ] `scripts/test.sh` fully green (fmt, clippy -D warnings, workspace tests, career-sim
      invariants + seed scan). No golden re-freeze needed — engine untouched.
- [ ] Diff contains no changes to `crates/goat-match/` or `crates/goat-save/`.

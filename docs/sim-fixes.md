# Sim Fix Proposals

Based on `docs/sim-analysis.md`. Each section names the root cause, the exact change, and the file/constant affected.

---

## Fix 1 (HIGH) — OVR plateau: rotate training routine when attrs cap out

**Root cause.** `career_sim.rs` hard-codes one routine for the entire career:
```
focus_attrs: [Finishing, Dribbling, ShortPassing], intensity: High
```
Once those three attrs hit their ceilings (age 20–22), every `AdvanceWeek` tick produces zero gain. Mental attrs (Vis, Rea, BCo, Com) have their peak trainability at age 26–33 per `goat-training/src/tuning.rs`, but they are never focused.

**Fix.** In `career_sim.rs`, after each season, inspect headroom for each focused attr and rotate out capped ones for mental attrs:

```
age 16–22 → focus [Fin, Dri, ShPa]       (current)
age 23–27 → focus [Fin, Dri, Vision]      (once Fin/Dri close to ceiling, swap Pas→Vis)
age 28–33 → focus [Vision, Composure, Reaction]  (mental peak — replaces topped attrs)
age 34+   → focus [Composure, Reaction, ShPa]    (maintenance)
```

Concrete rule: if `current >= potential - 3`, drop that attr from the focus list and add the next mental attr in priority order `[AttrId::Vision, AttrId::Composure, AttrId::Reaction, AttrId::BallControl]`.

**Expected effect.** OVR continues rising through the 20s via familiarity gain and mental attr growth. Peak should shift from age 27–28 to 29–32.

**Files.** `crates/goat-tui/src/career_sim.rs` — routine construction and season loop.

---

## Fix 2 (MEDIUM) — Form spiral: add mean-reversion floor

**Root cause.** The EMA in `state.rs:614`:
```rust
state.pc_form = state.pc_form * Fixed::raw(850) + out_fixed * Fixed::raw(150);
```
Has no floor. Once form drifts below 20 (bad runs of form), the 0.85 × self-reference keeps it low regardless of later performance. A veteran playing 30 games/season at form=12 is unrealistic.

**Fix.** Change the EMA to include a tiny pull toward a baseline floor (30):

```rust
// Blend: 80% EMA, 15% this output, 5% pull toward floor.
// The floor term prevents indefinite basement-level form.
let floor = Fixed::from_int(30);
state.pc_form = state.pc_form * Fixed::raw(800)
    + out_fixed * Fixed::raw(150)
    + floor * Fixed::raw(50);
```

This is still pure EMA arithmetic, no branching. At steady-state with output=30, form converges to 30. At output=80, form converges to 74 (versus 80 before). Players can still have bad form but can't get permanently stuck below 27.

**Files.** `crates/goat-core/src/state.rs` (ApplyRoundResult handler). Add a `FORM_FLOOR_WEIGHT` constant to `crates/goat-core/src/tuning.rs`.

---

## Fix 3 (MEDIUM) — Goal variance: blend deterministic rate with stochastic roll

**Root cause.** The current goal attribution (per-team-goal Finishing roll) is a binomial over ~30–45 team goals/season. Even at Fin=96 this produces large swings: at 24% hit rate over 45 tries, the standard deviation is ≈3 goals (range roughly 4–16 in a bad/good season).

**Fix.** Split the attribution 50/50 between a deterministic rate and the stochastic roll:

```rust
// Deterministic half: guaranteed rate based on Finishing.
let det_goals = (pc_gf as u64 * finishing as u64 / 800) as u32;

// Stochastic half: per-goal roll for the remaining team goals.
let stoch_goals = (0..pc_gf)
    .filter(|_| goal_rng.next_range_u32(0, 799) < finishing)
    .count() as u32;

let player_goals = det_goals + stoch_goals;
```

The denominator 800 means Fin 96 ≈ 12% guaranteed per team goal, plus another stochastic roll with the same rate. Combined expectation is same; variance is halved.

**Files.** `crates/goat-tui/src/career_sim.rs` (goal attribution block, line 205).

---

## Fix 4 (HIGH) — OVR ceiling: raise W_KEY to make elite attrs dominate

**Root cause.** With `W_KEY=1.0 / W_IMP=0.5 / W_BAS=0.25`, a role like CompleteForward has 5 KEY attrs and 11 IMP attrs. A player with Fin=97 but Dri=43 gets heavily dragged by the low Dri even though Fin is exceptional. OVR peaks at 68 across 20 seeds; the GOAT threshold needs 80+.

**Fix A — raise W_KEY weight tier (minimal, safe change):**

In `crates/goat-core/src/tuning.rs`:
```rust
/// Key attribute weight: raised from 1.000 → 1.500 so elite key attrs dominate.
pub const W_KEY: Fixed = Fixed::raw(1_500);
/// Important attribute weight: 0.500 (unchanged).
pub const W_IMP: Fixed = Fixed::raw(500);
/// Baseline attribute weight: 0.200 (reduced slightly to widen the spread).
pub const W_BAS: Fixed = Fixed::raw(200);
```

Because `role_rating` normalises by Σweights, this does not change the [0,99] range. It changes the relative contribution: at W_KEY=1.5, a single KEY attr at 97 vs 43 costs fewer points from low IMP attrs. Expected peak OVR increase: ~8–12 points.

**Fix B — add a Poacher role (more invasive, better specialist arc):**

A Poacher role with only `[Fin(KEY), ShP(KEY), LSh(IMP), Pen(IMP), Pos(BAS)]` would give Fin=97 players an OVR of ~90. This needs a new `RoleId::Poacher` entry in `roles.rs` and the weight table, plus forward-position familiarity seeding.

**Recommendation:** Start with Fix A (one constant change, re-freeze goldens). Evaluate whether Poacher is needed after seeing the new peak distribution.

**Golden test impact.** Both fixes break the existing golden-seed expected values — they must be re-frozen after the change. Per CLAUDE.md these are "new behavior" goldens, not fixes to broken tests.

---

## Fix 5 (LOW) — Age 23 skipped: record age at season start

**Root cause.** The snapshot records `age_weeks / 52` after 60 training weeks per season. Season 6 ends at age 22.9 (rounds to 22), season 7 ends at 24.1 (rounds to 24) — age 23 is never a snapshot boundary.

**Fix.** Record age at the start of each season (before `AdvanceWeek` calls), not at the end. In `career_sim.rs`:

```rust
// Capture age before the season loop, not after.
let season_start_age_weeks = state.players.get_age_weeks(pc_id);

// ... season loop ...

snaps.push(SeasonSnap {
    age: season_start_age_weeks / 52,
    ...
});
```

This shifts the snapshot to represent "age entering the season" rather than "age leaving it." Ages 16–35 all appear naturally.

**Files.** `crates/goat-tui/src/career_sim.rs`.

---

## Fix 6 (HIGH) — Player quality decoupled from results: OVR-based team modifier

**Root cause.** All matches use `sim_team_match(CLUBS[f.home].strength, CLUBS[f.away].strength, ...)`. The PC's OVR has zero influence on the team's results. Seed 15 (OVR 40) wins 7 titles purely because Leeds has a high `strength` constant.

**Fix.** When simming the PC's team's match, apply a small OVR-based modifier to the team strength:

```rust
// OVR modifier: ±0.3 strength per OVR point above/below 60 baseline.
// Capped at ±15 to avoid dominating club quality.
const OVR_STRENGTH_SCALE: i32 = 3; // tenths of a point per OVR above 60
let pc_ovr = ovr(&view.current, &view.familiarity).to_int();
let ovr_bonus = ((pc_ovr - 60) * OVR_STRENGTH_SCALE / 10).clamp(-15, 15);

for f in &all_fixtures {
    let (home_str, away_str) = if f.home == season_pc_club {
        (CLUBS[f.home].strength + ovr_bonus, CLUBS[f.away].strength)
    } else if f.away == season_pc_club {
        (CLUBS[f.home].strength, CLUBS[f.away].strength + ovr_bonus)
    } else {
        (CLUBS[f.home].strength, CLUBS[f.away].strength)
    };
    let (gf, ga) = sim_team_match(home_str, away_str, &mut sim_rng);
    ...
}
```

At OVR 70 the PC adds +3 strength; at OVR 40 they subtract 6. This is enough to shift ~1–2 league places without making the player a complete one-man team.

**Files.** `crates/goat-tui/src/career_sim.rs` (fixture simulation block, line 170–188). Add `OVR_STRENGTH_SCALE` as a named constant at the top of the file.

---

## Fix 7 (LOW) — Pac freezes: lower physical start percentage

**Root cause.** `PHYSICAL_START_PCT = 850` (85% of potential). With only 15% headroom and the career sim not training Acceleration/Speed, Pac fills the gap in the first season and is frozen for 10+ years.

**Fix.** Lower `PHYSICAL_START_PCT` from `850` to `750` in `crates/goat-core/src/tuning.rs`. This gives ~25% headroom for early-career physical growth, producing a visible Pac arc age 16–22 before it tops out. Alternatively, add `AttrId::Acceleration` to the early-career focus block (Fix 1) so it's explicitly trained alongside Fin/Dri during the youth phase.

**Golden test impact.** This changes all `generate_player` golden values — hold for a phase where goldens are already being re-frozen (e.g., after Fix 4).

---

## Fix 8 (MEDIUM) — OVR/Fin disconnect: addressed by Fix 4 + Poacher role

Issues 4 and 8 share the same root (W_KEY too weak). Raising W_KEY (Fix 4A) also fixes the Fin=97 → OVR=57 case, because Fin being KEY for most Forward roles will now dominate over the IMP drag from Str/Hed.

If after Fix 4A a Fin=97 player still shows OVR below 70, that's the signal to add the Poacher role (Fix 4B).

---

## Suggested implementation order

| Priority | Fix | Change size | Breaks goldens? |
|----------|-----|-------------|-----------------|
| 1 | Fix 4A — raise W_KEY | 1 constant | Yes — re-freeze |
| 2 | Fix 1 — rotate training routine | ~30 lines in career_sim | No |
| 3 | Fix 6 — OVR modifier for team results | ~15 lines in career_sim | No |
| 4 | Fix 2 — form EMA floor | 3 lines + 1 constant | No (not goldened) |
| 5 | Fix 3 — goal variance blend | ~5 lines in career_sim | No |
| 6 | Fix 5 — age 23 snapshot | 3 lines in career_sim | No |
| 7 | Fix 7 — PHYSICAL_START_PCT | 1 constant | Yes — bundle with Fix 4 re-freeze |

Fix 4A and Fix 7 should be applied in the same PR since both require re-freezing goldens.

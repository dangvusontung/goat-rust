# TASK 10A — Lifestyle consequence: the longevity/injury fork (TDD slice)

Prereq: Phase 8 playable; `pc_lifestyle` already plumbed (`SetLifestyle`, growth ±10%).
Read CLAUDE.md + bible §8.6 (off-pitch life) and §2.4 (talent ceiling is law).
This is a carve-out of Phase 10 Step 1 — shipped first because it is small, fully
test-specced, and unblocks the toxic-lifestyle scenario the rest of life/money builds on.
**This doc does not introduce relationships, economy, sponsors, or media** — those stay
in TASK-10. Scope is exactly: make lifestyle *cost* something.

## Why (empirical motivation — do not delete, this is the bug)

`career-sim` (Forward, seeds 0–19, 20 seasons), measured before this task:

```
PRO   (Professional/High): Peak OVR 93 | Injured weeks 113
TOXIC (Flashy/High)      : Peak OVR 93 | Injured weeks 113   ← identical to PRO
TOXIC (Flashy/Low)       : Peak OVR 93 | Injured weeks   7   ← laziest = healthiest
```

Two defects: (1) lifestyle is **not** an input to injury risk, so Flashy and
Professional are indistinguishable; (2) lifestyle only scales growth *speed* (±10%),
which fully washes out over a career — peak OVR is identical. A toxic lifestyle must
change the **destination**, not just the rate of arrival. Bible §8.6 requires the
identity fork (quiet long legacy vs flashy burn) to be a real trade-off you "cannot
fully have both."

## Acceptance spec (already written, currently RED)

`crates/goat-core/tests/spec_phase10_lifestyle.rs` — turn these green:
- `flashy_lifestyle_increases_injuries` — Flashy > Professional injured-weeks, fixed seed/intensity.
- `professional_lifestyle_reaches_higher_peak` — Pro peak OVR > Flashy peak over a full career.
- `lifestyle_never_breaks_talent_ceiling` — stays GREEN (regression tripwire: `current ≤ potential`).

Remove the `#[ignore]` on the first two as each goes green. Do **not** weaken the
assertions to pass; implement the behavior.

## Step 1 — Lifestyle feeds injury risk (`goat-core`)
- Add a `lifestyle_injury_mult` to `injury_prob` in `crates/goat-core/src/week.rs`.
  Signature gains the lifestyle (pass `state.pc_lifestyle`, or thread it through the
  week tick — keep the function pure, no global reads).
- New named constants in `crates/goat-core/src/tuning.rs` (no inline magic numbers):
  `INJURY_LIFESTYLE_X10_PRO`, `_BALANCED`, `_FLASHY` (illustrative shape: Pro 0.8×,
  Balanced 1.0×, Flashy 1.4× — these are placeholders, frozen only on user approval).
- Keep the existing fatigue × intensity × age factors; lifestyle multiplies alongside.
- **Determinism:** all fixed-point, all through the injected RNG. No floats.

## Step 2 — Lifestyle bends the longevity curve (`goat-core`)
The fork must alter the ceiling actually reached and/or the decline timing:
- Flashy: earlier/steeper physical decline (pull the age-decay curve forward) and a
  small effective-ceiling haircut on physical attrs; Professional: later/shallower decline.
- Implement via the age-curve / `apply_passive_decay` path — **never by raising any
  `current` above its `potential`** (§2.4). Lower the *reached* peak by decaying sooner,
  not by exceeding the cap.
- Constants in `tuning.rs`: `LIFESTYLE_DECAY_SHIFT_*`, `LIFESTYLE_PHYS_CEILING_*`.
- Decide explicitly (and comment): does the existing ±10% growth multiplier stay, fold
  into this, or get removed? Default: keep it — speed and destination are different levers.

## Step 3 — Golden coverage
- New golden-seed test: fixed seed + Flashy vs Professional over N weeks → exact frozen
  injured-week counts and exact frozen peak OVRs (new values, user-approved → frozen).
- Property test: monotonicity — `injuries(Flashy) ≥ injuries(Balanced) ≥ injuries(Pro)`
  and `peak(Pro) ≥ peak(Balanced) ≥ peak(Flashy)` across a seed sweep.

## ⏸ PAUSE — freeze numbers with the user
Before declaring done, show the user the new `tuning.rs` placeholders and the resulting
`career-sim` batch deltas. Golden values freeze **only** on explicit approval (per CLAUDE.md).

## Playable gate
`cargo run -p goat-tui --bin career-sim -- --seed 3 --lifestyle professional` vs
`--lifestyle flashy` (flags added in this branch's career-sim): the Professional career
visibly outlasts and out-peaks the Flashy one — fewer injured weeks, higher sustained
OVR, later decline. The toxic run finally has teeth.

## Definition of done
1. `cargo test --workspace` green, including all pre-existing golden tests at original values.
2. The two spec tests un-`#[ignore]`d and passing; tripwire still green.
3. `cargo fmt --check` + `cargo clippy --all-targets -D warnings` clean
   (also clear the two pre-existing lints: `golden_week.rs` let_and_return,
   `week.rs` unused `NUM_ROLES` import).
4. New golden + property coverage as above; numbers user-approved.
5. No floats in sim, no unsafe, no I/O in core, no logic in TUI.
6. Summary: what changed + which bible section (§8.6, §2.4) it implements.

## Out of scope (stays in TASK-10)
Relationships/scandals, economy/bankruptcy, sponsors/marketability tiers, media
flashpoints, retirement verdict. Lifestyle's *only* new effects here are injury risk
and longevity.

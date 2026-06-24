# TASK 10B — Life & money: execution scoping (carves the rest of Phase 10 into slices)

Companion to `TASK-10-life-money-ship.md` (design intent). Phase 10 Step 1's
lifestyle↔longevity fork already shipped as **TASK-10A** (done). This file scopes the
REMAINING Phase 10 into golden-safe slices and surfaces the decisions to confirm first.
Read CLAUDE.md, bible §8.5/§8.6/§8.7/§8.8, §2.4 (ceiling-capped advantages), §11 (parked).

## What's left in Phase 10 (after 10A)
- Off-pitch life: a *few* relationship threads + scandals (NOT a dating sim; the deep
  relationship web is **parked** per §11).
- Economy: wages/bonuses in, spend/invest out, **bankruptcy possible**; money buys
  development/longevity advantage but **capped by potential** (§2.4), irrelevant early.
- Sponsors gated by **marketability** (local→national→global); obligations cost
  time/energy; over-commercialising dents reputation (closes the Icon axis loop).
- Media flashpoints (§8.7): interactive at key moments (presser after a red, transfer
  saga) → ripple into reputation facets + pundit narratives.
- Retirement → **the final verdict**: the schools deliver disagreeing placements; the
  career enters the save's canon. No win screen — the debate is the ending.
- Hardening: save audit at full scale, long-horizon career sweep, TUI manage-by-exception.

## ⚑ Decisions to confirm (pause before Slice 1)
- **D1 — Economy depth.** (a) Minimal: wage in, lifestyle/upkeep out, savings, bankruptcy
  flag; or (b) + an investment/business P&L thread feeding the Icon axis (bible §8.8).
  *Recommend (a) first, (b) as a follow-on slice* — keep it ceiling-capped and irrelevant
  early either way.
- **D2 — Relationship depth.** How many threads (partner / family / close friend) and are
  events surfaced purely by exception? *Recommend 2–3 threads, exception-only, scandal-on-
  rupture* — honoring the §11 "parked deeper web".
- **D3 — Does money affect the sim, or only flavor/legacy?** If money buys
  trainers/nutrition → development speed (capped at potential), it touches the growth path
  (golden-sensitive). *Recommend: money modifies a facilities-style multiplier, never the
  ceiling; gate behind a flag so Balanced/no-spend stays byte-identical to today's goldens.*

## Slices (ordered, each ships green + has a gate)
### 10B.1 — Economy core (goat-meta or goat-core scalars)
Wages/bonuses in, lifestyle upkeep + optional spend out, savings, bankruptcy state.
Ceiling-capped "invest in development" multiplier behind a flag (neutral by default →
goldens unmoved). Golden: fixed-seed cashflow over a career.
Gate: a money panel; run a contract down, overspend, risk bankruptcy.

### 10B.2 — Sponsors + marketability tiers
Marketability (already a reputation scalar) → local/national/global tiers; obligations
cost energy (the training resource); over-commercialising dents reputation. Golden:
tier thresholds + rep deltas.
Gate: sign sponsors as marketability climbs; feel the time/rep trade-off.

### 10B.3 — Relationships + scandals + media flashpoints
2–3 exception-only threads; scandals hit Character rep; media flashpoints at red cards /
transfer sagas with choices that ripple into reputation facets + pundit lines (reuse the
Phase-7 pundit engine). Golden: deterministic flashpoint + rep ripple for a seed.
Gate: a presser choice after a red card visibly moves your reputation.

### 10B.4 — Retirement + the final verdict
Retirement trigger (decline / offers drying up / player choice) → career retrospective →
the **schools' disagreeing placements** (reuse `goat_meta::pantheon` SCHOOLS/all_rankings,
ideally over the seeded canon from TASK-09B) → career enters canon. Golden: verdict for a
fixed career.
Gate: retire and watch four schools argue your legacy — the game's ending.

### 10B.5 — Hardening
Save audit at full scale (tiny-save: nothing derivable persisted), long-horizon headless
career sweep (16→retirement, invariants, perf budget, save round-trip each season), TUI
manage-by-exception pass.
Gate: a full career 16→retirement playable start to finish.

## ⏸ Pauses
Before Slice 1 (confirm D1–D3). Before freezing any economy/sponsor/verdict golden.

## Definition of done (whole phase)
1. `cargo test --workspace` green incl. ALL pre-existing goldens at original values
   (the no-spend/Balanced path must stay byte-identical).
2. Each slice's golden + invariants green; new values user-approved.
3. Money/longevity advantages ceiling-capped (§2.4); economy can bankrupt but never
   pushes past potential.
4. `fmt` + `clippy -D warnings` clean; no floats in sim, no unsafe, no I/O in core,
   no logic in TUI; determinism bit-for-bit.
5. The whole game is playable: academy at 16 → retirement → the schools' verdict.

## Out of scope (parked §11)
Deeper relationship web, goalkeeper career, graphical renderers + goat-bridge FFI.

# TASK 10 — Life & money + full-career hardening: the whole game

Prereq: Phase 9 playable. Read CLAUDE.md + bible §8.5, §8.6, §8.8, §2.4 guardrails.
This phase completes the design; the gate is a full playable career, 16 → retirement.

## Step 1 — Off-pitch life (goat-meta, bible §8.6)
- A few tracked relationship threads (partner, family, close friends) + events —
  medium depth, surfaced by exception, not a dating sim.
- **Lifestyle ↔ longevity:** professional lifestyle extends the peak (bends the age
  curves' decline); partying burns out early but feeds marketability + scandal risk.
  The identity fork (quiet long legacy vs flashy icon burn) must be a real trade-off —
  you cannot fully have both.
- Scandals hit Character rep; stable life feeds focus → form.

## Step 2 — Economy (bible §8.8)
Money as a real resource: wages/bonuses in, spending/investments out, **bankruptcy
possible**. Money buys gameplay advantage (private trainers, nutrition, recovery →
development speed, longevity) but is **capped by potential — never past the ceiling**
(pillar §2.4), counterweighted by bankruptcy risk, and irrelevant early (you start
poor). Investment/business layer: simple P&L threads → post-career empire or bankrupt
ex-star, feeding the Icon axis.

## Step 3 — Sponsors + media flashpoints
- Sponsors gated by Marketability: local → national → global tiers; obligations cost
  time/energy (the same resource training needs); over-commercializing dents reputation.
  This completes the Icon axis loop from Phase 7.
- Media becomes interactive at flashpoints (bible §8.7): presser after a red card,
  transfer-saga statement — choices ripple into the reputation facets and pundit
  narratives. The world auto-reports everything else.

## Step 4 — Retirement + the final verdict
Retirement decision (decline, offers drying up, or your call) → career retrospective →
the schools deliver their final, *disagreeing* placements → your career enters the
canon of the save's universe. Still no win screen — the debate is the ending.

## Step 5 — Hardening
- Save format final pass: version field, tiny-save audit (anything derivable removed).
- Long-horizon suite: full 16→retirement careers headless across a seed sweep —
  invariants, performance budget, save round-trips at every season boundary.
- TUI pass: consistent navigation, manage-by-exception defaults everywhere
  (auto-run + interrupt), a "season in 5 minutes" flow actually achievable.

## Playable gate — THE game
`cargo run -p goat-tui`: create a kid, live the whole career — train, play, transfer,
feud, cash in or stay loyal, burn bright or last forever — retire, and watch four
schools argue about you forever. Ship-shaped, text-rendered, fully offline.

## Out of scope, permanently parked (bible §11)
Goalkeeper career, graphical renderers + goat-bridge FFI, final tuning numbers,
beat-library volume scaling, deeper relationship web.

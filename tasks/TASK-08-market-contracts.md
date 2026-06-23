# TASK 08 — Career market: contracts, transfers, loans, player power

Prereq: Phase 7 playable. Read CLAUDE.md + bible §4.2, §5.4 (loans), §7.3, §8.3, §8.4.

## Step 1 — Contracts (goat-meta)
Wage, length, signing + loyalty bonus, release clause, performance bonuses, image
rights (inert until Phase 10). **Squad-status/role promises are recorded and
enforceable** — a broken "you'll start" promise is legitimate grounds to agitate.
Contract length is the spine of leverage: running it down flips power to the player;
age bends value continuously. Negotiate yourself or delegate to an agent — agent
quality matters (simple tiered agents).

## Step 2 — AI clubs as agents (goat-world)
Each club in the small world gets: strategy, finances/budget, a squad-building plan,
a manager. Windows run as part of the season tick: AI clubs buy/sell from each other,
your teammates arrive and leave. Interest in *you* emerges from form, Output,
reputation, age, contract situation. Windows surface **by exception**: quiet windows
pass silently; sagas interrupt.

## Step 3 — Loans + player power
- Loans resolve develop-vs-minutes (§5.4): big-club parent + facilities multiplier,
  loan club + real minutes; return developed and match-sharp.
- The escalation ladder (§8.3): quiet request → transfer request → media agitation →
  skip training → strike/AWOL. Each rung raises sale pressure, burns Character rep,
  and risks retaliation (freeze-out). Leverage requires stature; resolution driven by
  contract years left, form, squad importance.

## Step 4 — TUI
Offer/negotiation screens (terms, counteroffers, promises), transfer-saga event flow,
loan decisions, the escalation ladder as explicit choices with visible rep costs,
squad-status view (promised vs actual minutes).

## Tests
Golden: seeded multi-window scenario → exact offers, AI transfers, negotiation outcomes.
Property: longer remaining contract → higher fee pressure; broken promises unlock
agitation; escalation burns Character monotonically; a squad player's leverage fails.
Long-horizon: 10 seasons of windows headless, finances and squads stay coherent.

## Out of scope
Sponsors/image-rights payouts (Phase 10), international transfers beyond the small
world (Phase 9 scales it), money as a spendable resource (Phase 10).

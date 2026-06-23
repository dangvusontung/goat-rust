# TASK 07 — The spine: legacy, pantheon, pundits, awards, reputation

Prereq: Phase 6 playable. Read CLAUDE.md + bible §8.1, §8.2, §8.7. This phase is the
carrot that replaces a win condition — the game's whole point becomes visible here.

## Step 1 — Legacy axes (goat-meta)
Track the evidence continuously from existing systems: Winning (trophies + decisive
contributions), Accolades, Output (career performance record), Longevity, Decisive
Moments (finals, late winners — detectable from match context + beat ripples), Loyalty
(club history), Icon (stub until Phase 10 marketability), head-to-head (stub until
Phase 9 rival). Scoring is debate material and a progress readout — **never a terminal
state, never a "you win" screen** (pillar §2.1).

## Step 2 — The pantheon
- A **seeded canon** of past greats generated at genesis with plausible career records
  (mini-history for now; deep history arrives Phase 9).
- 3–4 **schools** that weight the axes differently and never agree by design: the
  trophy-counter, the eye-test romantic, the stats purist, the loyalty-traditionalist.
  Each ranks you against the canon its own way, live, updating as you play.
- Divergence is a feature: tests must show realistic careers where schools disagree
  on your placement.

## Step 3 — Reputation, 4 facets (bible §8.2)
Sporting / Marketability / Character / Club-fan as independent values with their own
inputs: Output + trophies → Sporting; flair + decisive moments → Marketability (full
loop in Phase 10); dirty play (Phase 6 scalar) + scandals → Character; loyalty +
performances → Club-fan. Wire Character into Phase 6 officiating.

## Step 4 — Awards + pundits
- Awards nights: league + world player-of-the-year, computed from world season data,
  awarded to you *or AI players* — losing to a rival must be possible. These are the
  retention peaks: give them ceremony in the TUI.
- Pundits: 3–5 **named, recurring characters**, each mapped to a school, with bias and
  personality, following your whole career — the doubter, the champion, the stats nerd.
  Template + slot text baked in core data; they react to seasons, awards, milestones.

## Step 5 — TUI
Pantheon screen (per-school rankings + trajectory), legacy case sheet (evidence per
axis), award night sequence, pundit feed after notable events, reputation panel.

## Tests
Golden: a scripted multi-season career → exact axis values, exact per-school rankings,
exact award outcomes. Property: schools provably diverge on mixed-profile careers;
no code path declares a terminal win.

## Out of scope
Emergent rival + head-to-head live data (Phase 9), Icon/marketability loop, sponsors,
media flashpoint *interactions* (Phase 10 — pundit feed here is read-only).

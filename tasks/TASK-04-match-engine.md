# TASK 04 — Match engine v1: beats, headspace, output ≠ result

Prereq: Phase 3 playable. Read CLAUDE.md + bible §6 (all of it) + tech doc.
This is the heart of the game — go slow, pause after every step.

## Step 1 — goat-match crate: the beat data model
A beat = trigger conditions → setup (template + slot text) → 2–4 attribute/trait-gated
choices → contest resolution (your attrs vs difficulty + RNG) → transitions (next
beat/phase) → ripple consequences (headspace, momentum, scoreline, form).
- Beats are **data**, organized by phase of play + tags + context; a context-weighted
  selector picks what fires next.
- All beat text is template + slot, baked into the crate as data (handwritten for now —
  the LLM mass-generation pipeline is a later authoring concern). **No text logic in
  the TUI; slots are filled in core.**

## Step 2 — Match flow
- A match = sequence of beats strung by momentum, stamina, and phase transitions —
  including off-ball positioning calls, not just shots.
- Two axes simulated separately (pillar §2.5): **your Output** resolved through beats;
  **the team result** from a separate stochastic team-strength sim (stub team strengths
  for now). "Hat-trick but lost 3–2" must be reachable.
- **Play or skip:** skip auto-resolves your Output from attributes + form via the same
  engine (auto-picked choices), so skipped and played careers stay comparable.
- Stamina drains through the match and feeds contest odds; match minutes feed
  familiarity growth (playing a role > training it).

## Step 3 — Headspace v1
Confidence / Nerves / Frustration / Flow as live in-match axes. Beats ripple into them;
they feed contest odds, which choices appear, and which beats can trigger. Composure
governs volatility and recovery speed. Form (slow, season-long baseline from Phase 5 —
stub a constant for now) vs headspace (fast in-match deviation).

## Step 4 — Starter beat library
~25–30 handwritten beats covering: open play (attack/defense), one-on-one, set pieces,
a penalty, off-ball positioning, a last-minute chance with a deeper hand-authored
mini-tree. Enough variety that two matches feel different.

## Step 5 — Playable gate in the TUI
Pre-match: play or skip. Playing: setup text → choices → resolution → ripple, beat by
beat; HUD line with score, minute, stamina, headspace. Post-match: your rating (Output)
vs the result, key moments recap.

## Tests
- Golden: fixed seed + scripted choice sequence → exact beat sequence, contest results,
  final Output rating and scoreline.
- Property: skip and play draw from the same resolution engine; headspace stays in
  bounds; higher relevant attributes → higher contest win rate over many seeded trials
  (deterministic seed sweep, not statistical flakiness).
- Output and team result are demonstrably decoupled (a seed exists in tests where high
  Output coincides with a loss).

## Out of scope
Cards/discipline depth (Phase 6 — a minimal foul outcome may exist as a beat consequence),
real opponents/league context (Phase 5), rivalry flavor (Phase 9).

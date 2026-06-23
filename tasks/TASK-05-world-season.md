# TASK 05 — Small world v1: seasons, fixtures, tables, form, save/load

Prereq: Phase 4 playable. Read CLAUDE.md + bible §7.1–7.2 + tech doc (especially the
save format and fixtures-are-ephemeral rules).

## Step 1 — Mini-genesis (goat-world)
A deliberately small, seed-derived world: 2–3 nations across the powerhouse↔minnow
spectrum, 2 divisions each, ~16 clubs/division, ~500–1000 players total — generated
through the same pipeline shape as the real thing (structure → clubs with identity
stubs → people via the Phase 2 generation pipeline), **stored SoA from day one**.
Scale comes in Phase 9; the rules must not change then, only the volume.
Replace the Phase 2 stub nationality/club lists with this world.

## Step 2 — Fixtures + season tick
- Fixtures generated deterministically from (seed + season + league) and **never stored**
  for past seasons — past seasons keep only results/records (tech doc rule).
- Your club's season: your matches go through the Phase 4 engine (play/skip);
  selection/minutes depend on your standing at the club (simple squad-status model:
  bench at a big club vs starter at a small one — the develop-vs-minutes dial, §4.2).
- Other matches in your league: team-strength result sim. League tables, top scorers,
  basic records accumulate as path-dependent state.

## Step 3 — Form
The slow season-long baseline (bible §6.2): driven by recent Output and minutes; feeds
match engine (replacing the Phase 4 stub), selection, and later reputation. Headspace
deviates around it.

## Step 4 — Minimal save/load (goat-save)
Tiny-save per the tech doc format: seed + creation choices + path-dependent state
(career history, tables, records, current-season materialized state). Everything else
recomputes. Round-trip golden test: save → load → identical state hash; and a
re-derivation test: a fresh world from the same seed matches the saved world's derived
data.

## Step 5 — Playable gate in the TUI
Full season loop: fixture list, play or skip each match, league table and scorer charts,
form tracker, end-of-season review (your stats, team finish). Save from the main menu,
quit, resume.

## Tests
Golden: fixed seed → exact mini-world (clubs, key players), exact fixture list, and an
auto-played season's exact final table. Long-horizon: 5 headless seasons, invariants
hold. Save/load round-trip. TUI smoke updated.

## Out of scope
Transfers/contracts (Phase 8), full-scale world + history + lazy-promote (Phase 9),
awards/legacy (Phase 7), international football (arrives with Phase 9 world).

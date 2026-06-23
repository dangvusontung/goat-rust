# TASK 09 — Full-scale world: genesis, history, tiered sim, the emergent rival

Prereq: Phase 8 playable. Read CLAUDE.md + bible §7 (all), §7.4, and the tech doc's
performance sections. This phase changes scale, not rules — game logic from Phases 1–8
must survive untouched.

## Step 1 — Full genesis (goat-world)
- Structure: many nations across the powerhouse↔minnow spectrum with full pyramids;
  nationality now truly is the difficulty/story dial (§4.1) — international football
  (qualifiers, continental + world tournaments) enters the season tick.
- Clubs with rich identity: history, rivalries, philosophy, stature, finances.
- People: 20–30k full-fidelity players + youth pools, **strictly SoA columns** —
  no per-player heap objects. Budget genesis at ~1–3s with lazy generation, on the
  reduce path's background-friendly structure (renderer shows a loading sequence).
- Decades of fake history by running the batch-tick backwards/forwards so past Ballon
  d'Ors, records, and the Phase 7 canon are internally consistent, not random labels.

## Step 2 — Tiered simulation
- **Deep-sim the orbit:** your club, league, direct rivals — match-by-match.
- **Batch-tick the rest** at season granularity: tables, top scorers, AI transfers,
  records (path-dependent state only).
- **Lazy-promote on contact:** fully realize a background player the moment he becomes
  relevant (you face him, transfer link).
- **Formula-driven background growth:** non-orbit current attributes computed on demand
  from (seed + birth data + date), never stored/stepped weekly.
- Save stays tiny (tech doc): verify save size stays within budget at full scale.

## Step 3 — Youth regeneration + the emergent rival (bible §7.4)
- World regenerates youth each season through the genesis pipeline.
- At your genesis, a **cohort of peers** is rolled and grows in parallel via the cheap
  tick — variance, injuries, busts, breakouts.
- **Rivalry crystallizes retroactively:** if a peer keeps pace at the top over years,
  media/pundits (Phase 7 engine) start framing the rivalry; head-to-head feeds the
  legacy axis. **Sometimes nobody keeps up** — you reign alone and the harsher schools
  apply the weak-era asterisk. No scripted nemesis, no guaranteed rival.
- Rivalry-flavored matches: charged beats/context tags when paths cross.

## Step 4 — TUI
Seeded-universe new game (share/replay a seed), loading sequence with flavor, world
screens (other leagues at season granularity), international windows, rival/generation
tracker, history browser (past winners, records, canon).

## Tests
Golden: fixed seed → exact genesis fingerprint (hash of world columns), exact slice of
generated history. Determinism: re-deriving any background player at any date matches
across runs. Performance: genesis + a fast-forwarded season within budget (document
measured times). Long-horizon: 20 headless seasons at full scale — invariants, no
panics, rivalry sometimes emerges and sometimes doesn't across a seed sweep.

## Out of scope
Lifestyle/economy/sponsors (Phase 10). GK career stays parked.

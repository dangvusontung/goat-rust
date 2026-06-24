# TASK 09A — Full-scale world: execution scoping (carves TASK-09 into golden-safe slices)

Companion to `TASK-09-full-world-rival.md` (the design intent). This file grounds Phase 9
in the **current** codebase, orders the work into independently-reviewable slices each
with a TDD anchor, and surfaces the architectural decisions that need a call before the
big build. Read CLAUDE.md, bible §7 + §241–247 (tiered sim), CALENDAR.md §7.1
(batch-tick outer world). **This phase changes scale, not rules** — Phases 1–8 logic and
their frozen goldens stay untouched.

## Where the code is today (the starting line)

- `goat-world` is **64 static clubs** + fixtures + team-strength season sim. There is **no
  background player population** at all.
- The only "other players" are the **8-peer batch-tick stub**: `PeerState` in `goat-core`
  (`InitPeers` / `BatchTickPeers` / `DeclareRival`), seeded by `build_peer_cohort` in the
  TUI, with `find_rival_candidate` crystallising a rival from those 8 after season 5.
- So Phase 9 is two things: (a) **build the real 20–30k SoA population + history** that
  doesn't exist yet, and (b) **generalise the existing 8-peer rival** into a rival that
  crystallises from the real generation cohort (the stub is the seam to grow, not replace).

## Architecture (recommended — confirm the ⚑ decisions below first)

- **Tiered depth, full-fidelity stats (bible §241–247).** Everyone has stats; only
  *simulation depth* is tiered:
  - **Deep-sim the orbit** (PC club + competitions) — the existing match/week path.
  - **Batch-tick the rest** at season granularity (tables, scorers, AI transfers, records).
  - **Lazy-promote on contact** — a background player gets a real `PlayerStore` row only
    when he becomes relevant (you face him / a transfer links him).
  - **Background growth is formula-driven** — non-orbit current attrs are *computed on
    demand* from `(seed + birth data + date)`, never stored or stepped weekly
    (TRAINING.md §302: do NOT allocate per-day energy for 20–30k — the §9 SoA/perf trap).
- **Tiny saves hold records only.** Background players, fixtures, and history are
  recomputed from `seed (+ season, league, birth data, date)`. Persist league tables,
  records, Ballon d'Or canon, transfers — the path-dependent residue — nothing derivable.
- **SoA columns, no per-player heap.** The population is parallel `Vec`s keyed by an
  index/id (same discipline as `PlayerStore`).

## ⚑ Decisions to confirm (pause before Slice 1)

- **D1 — Where the population lives.** New columnar store in `goat-world` (the "outer
  world", keeps core lean) vs. extend `goat-core::PlayerStore`. *Recommend:* new
  `goat-world` module — orbit players promote *into* `PlayerStore`, background stay in the
  outer store. Keeps the headless-core/world split clean.
- **D2 — History depth + what's persisted.** How many decades of backfilled canon, and
  is it (a) generated once at genesis and stored as records, or (b) recomputed on demand
  from seed? *Recommend:* backfill N≈20–40 seasons, persist **records only** (winners,
  Ballon d'Ors, top-scorer lines); recompute everything else.
- **D3 — Rival crystallisation rule.** Cohort size, "keeps pace" threshold, retroactive
  window, and the **weak-era asterisk** threshold (when NOBODY qualifies). *Recommend:*
  generalise `find_rival_candidate`'s shape; pick thresholds, then freeze via golden.

## TDD anchors (write RED first, per slice)

`crates/goat-world/tests/spec_phase9_*.rs` (+ a rival spec in `goat-core`):
- `genesis_fingerprint_is_stable` — fixed seed → exact hash of the world's SoA columns
  (sorted) and a frozen slice of backfilled history. Same seed ⇒ identical universe,
  bit-for-bit (the spine golden for this phase).
- `background_player_rederive_is_deterministic` — re-deriving any background player's
  attrs at any date matches across runs and never exceeds potential (§2.4).
- `lazy_promote_never_resurrects_retired` — promotion-on-contact respects birth/retire
  dates; a retired id can't reappear as active.
- `batch_tick_totals_are_monotonic` — season batch-tick never decreases career
  goals/apps/titles; tables conserve played/points.
- `rival_crystallises_or_doesnt` (goat-core) — deterministic across a seed sweep; the
  **nobody / weak-era** branch is reachable and frozen (no guaranteed nemesis).
- `genesis_within_budget` — genesis + one fast-forwarded season under the documented
  time/alloc budget (bible: ~1–3s genesis; CALENDAR.md §520: don't micro-opt the day loop).

## Slices (ordered, each ships green + has a gate)

### 9A.1 — SoA population store + genesis (the foundation)
Columnar outer-world population (identity, nationality, birth data, club, potential
seed). Deterministic genesis from `world_seed`. RED→GREEN `genesis_fingerprint_is_stable`.
Gate: a debug `--scan`-style dump prints a stable fingerprint for a seed.

### 9A.2 — Formula-driven background growth + lazy-promote
On-demand attr derivation from `(seed + birth + date)`; promote-on-contact into
`PlayerStore`. Invariants: re-derive determinism, `current ≤ potential`, no per-day state.
Gate: face a generated opponent; he's realised with sane, stable stats.

### 9A.3 — Season batch-tick of the outer world
Non-orbit leagues advance at season granularity (tables, scorers, AI transfers, records).
Reconcile at season boundary (CALENDAR.md §7.1 / pipeline step 4). Monotonicity golden.
Gate: a world screen shows other leagues' tables + top scorers advancing each season.

### 9A.4 — Seeded history backfill + canon
Backfill decades of internally-consistent past winners / Ballon d'Ors / records feeding
the Phase 7 pantheon. Persist records only. Golden: exact history slice for a seed.
Gate: a history browser shows past greats; the pantheon cites real backfilled canon.

### 9A.5 — Generalise the emergent rival (bible §7.4)
Replace the 8-peer stub's cohort with a view into the real generation cohort; retroactive
crystallisation; head-to-head feeds legacy; **weak-era asterisk when nobody keeps pace**.
Golden: rival emerges / doesn't deterministically across seeds. Keep `DeclareRival`/
`pc_rival_idx` plumbing; widen the candidate source.
Gate: years in, the media names your rival — or crowns you alone with the asterisk.

### 9A.6 — TUI: world screens
Seeded-universe new game (shareable seed), genesis loading sequence, other-leagues
screens, international windows, generation/rival tracker, history browser. No sim logic.
Gate: the roadmap's Phase 9 gate — "boot the full universe in seconds; meet your
generation; years later the media names your rival — or crowns you alone."

## ⏸ Pauses
Before Slice 1 (confirm D1–D3). Before freezing any genesis/history/rival golden value
(approve the numbers; once frozen they're sacred).

## Definition of done (whole phase)
1. `cargo test --workspace` green incl. **all** Phase 1–8 goldens at original values.
2. Every TDD anchor green; genesis/history/rival goldens approved + frozen.
3. Genesis + fast-forward within the documented budget (state measured numbers).
4. Tiny-save audit at full scale: save size within budget; nothing derivable persisted.
5. SoA columns (no per-player heap); determinism bit-for-bit cross-platform; no floats in
   sim; no unsafe; no I/O in core; no logic in TUI.
6. Summary per slice: what changed + which bible/CALENDAR.md section it implements.

## Out of scope
Lifestyle / economy / sponsors / media flashpoints (Phase 10). Goalkeeper career (parked).
Deeper relationship web (parked). Match-engine changes (Phases 4/6 are frozen here).

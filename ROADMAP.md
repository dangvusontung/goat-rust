# ROADMAP — Become the GOAT

The whole game, built in 10 phases. Each phase has a **playable gate** — something you
can do in `goat-tui` when it lands. `goat-tui` (and, added later, `goat-web`) exist as
dev/testing harnesses for exercising and manually verifying the core; the actual shipping
renderer is the Flutter mobile app built against `goat-bridge` (see CLAUDE.md's Project
overview, and `docs/CLIENT-IMPL.md` / `docs/FLUTTER-APP-GUIDE.md`). Claude Code works one
phase per task file (`tasks/TASK-NN-*.md`), in order, with review pauses inside each.

Principles baked into the ordering:
- **Playable from Phase 2 onward.** The TUI arrives early and grows with every phase —
  no six-month tunnel before you can touch the game.
- **Small world first, full world later.** Phases 2–8 run on a deliberately tiny world
  (a few nations, ~500–1000 players) so every system is testable fast. Phase 9 scales
  genesis to the real 20–30k SoA world + decades of history without changing game rules.
- **Vertical before horizontal.** The career spine (player → weeks → matches → seasons →
  legacy) lands before the wide meta (transfers, life, money).

| Phase | Crates touched | What lands | Playable gate (in goat-tui) |
|---|---|---|---|
| 0 ✅ | goat-rng, goat-fixed | Deterministic RNG + fixed-point math, golden tests | — (foundation) |
| 1 | goat-core | Domain model: 30 attributes (SoA), 6 families, age-curve archetypes, 14 roles, familiarity, `derive_attrs`, `role_rating`, `reduce()` skeleton + golden test #1 | — (headless; gate is tests) |
| 2 | goat-core, goat-tui | Player generation pipeline (bible §5.3) + TUI shell | Create a player (name/position/nationality/club; talent rolled), inspect attributes, roles, OVR. Re-roll with new seed. |
| 3 | goat-core, goat-tui | The week loop: training routine + intensity, energy/fatigue, growth gated by age curves, injuries & dev events by exception, familiarity training | Live week to week: set routine, train, rest, get interrupted by events, watch attributes grow. Fast-forward N weeks. |
| 4 | goat-match, goat-tui | Beat engine v1: phases/tags/context selector, contests (attrs vs difficulty + RNG), starter beat library (~25–30 authored beats, template+slot), headspace v1, play-or-skip, **Output rating separate from team result** | Play a full match beat-by-beat or skip it; see your rating vs the scoreline ("hat-trick, lost 3–2" must be possible). |
| 5 | goat-world, goat-save, goat-tui | Small world v1: mini-genesis (2–3 nations, 2 divisions each, ~500 players), deterministic fixtures, league tables, team-strength result sim, form, season summary. Minimal save/load (tiny-save format per tech doc) | Play a full season: skip/play matches, climb the table, see top scorers, end-of-season review. Save and resume. |
| 6 | goat-match, goat-core, goat-tui | Match depth: full headspace loop (confidence/nerves/frustration/flow, composure as damper), discipline (cards from choices + frustration + aggression + ref personality), suspensions, dirty/clean reputation feeding officiating | Feel the difference between an ice-man and a mercurial talent; pick up a red, serve the ban, watch your rep tighten refs around you. |
| 7 | goat-meta, goat-tui | Legacy spine: the 7+1 axes, pantheon with 3–4 never-agreeing schools, seeded canon of past greats, awards nights, 4-facet reputation, named recurring pundits (template+slot debate) | Open the pantheon screen; see four schools rank you differently; win (or lose) an award night; read pundits argue about you. |
| 8 | goat-meta, goat-world, goat-tui | Career market: contracts (length/clauses/promises), negotiation + agent option, transfers & AI club agents, loans (develop-vs-minutes resolved), player-power ladder | Get offers, negotiate, force a move (and burn Character rep), go on loan, run a contract down. |
| 9 | goat-world, goat-tui | Full-scale world: genesis at 20–30k players SoA, decades of seeded history + consistent canon, tiered sim (deep orbit / batch-tick / lazy-promote / formula-driven background growth), **emergent rival** (cohort, retroactive crystallization, sometimes nobody — weak-era asterisk) | New save boots the full universe in seconds; meet your generation; years later the media names your rival — or crowns you alone. |
| 10 | goat-meta, goat-save, goat-tui | Life & money: lifestyle ↔ longevity fork, relationships (few threads), sponsors/marketability tiers, economy with bankruptcy risk + ceiling-capped advantages, media flashpoints. Save hardening + full-career long-horizon tests | **The whole game:** academy at 16 → retirement, every system live, final legacy verdict argued by the schools. Full career playable start to finish. |

## Planned: Phase 3.5 — CalendarEngine & Training integration

`goat-calendar` and `goat-training` landed in Phase 1 as standalone crates but are not
yet wired into the live game loop. Three architectural blockers must be resolved first:

1. **Circular dependency.** `goat-training` depends on `goat-core` (needs `PlayerStore`,
   `Attrs`, age curves). `goat-core`'s `reduce()` / `advance_week` cannot import
   `goat-training` without creating a cycle. Fix: extract a `TrainingIntentProcessor`
   trait in `goat-core`, implemented in `goat-training` and injected by `goat-tui`.

2. **Dual `PlayerStore` ownership.** `TrainingSubsystem` holds its own copy of player
   data. The week loop in `WorldState` is the single source of truth; training must
   operate on borrowed slices of it, not an owned copy.

3. **Granularity mismatch.** The current loop is week-based (`advance_week`); the
   `CalendarEngine` spec (CALENDAR.md) is day-tick. Phase 3.5 will resolve the
   week-vs-day boundary: either CalendarEngine batches 7 day-ticks per advance_week call,
   or advance_week is rewritten to delegate to CalendarEngine.

**Phase 3.5 scope:**
- Resolve the dep cycle (trait injection or crate restructure)
- Unify PlayerStore ownership
- Wire `goat-calendar`'s `advance_until_flashpoint` into the week loop
- Wire `goat-training`'s routines so training output feeds real attribute growth
- Golden-seed test: full-season headless sim with calendar + training produces same
  final stats for a fixed seed

Playable gate: `[W] Train` in goat-tui applies the training crate's routine logic
rather than the inline fallback in `advance_week`.

## How to run this with Claude Code

1. Put `CLAUDE.md`, `ROADMAP.md`, and `tasks/` in the repo root.
2. One phase per Claude Code session (or `/clear` between phases): paste the task file's
   prompt, let it read the docs + existing code first.
3. Review at every pause point inside a task. Approve golden values explicitly —
   once approved they freeze.
4. Commit at every green pause. Tag at every phase gate (`phase-2-playable`, …).
5. If Claude Code proposes deviating from the tech doc or bible, it must say so out loud
   and wait for your call. Silence = follow the docs.

## Explicitly out of scope (entire roadmap)

Goalkeeper career, final tuning numbers (placeholders throughout, centralized in `tuning`
modules), beat-library volume beyond the starter set, deeper relationship web. All parked
per bible §11.

**No longer out of scope, superseding the above:** the Flutter renderer and `goat-bridge`
FFI were originally parked for this roadmap but are now actively under construction — see
`docs/CLIENT-IMPL.md` (bridge API reference) and `docs/FLUTTER-APP-GUIDE.md` (screen-by-
screen build guide, which tracks how far `goat-bridge` lags the core via
`tasks/TASK-BRIDGE-refresh.md`). `goat-web` (a WASM browser dev harness, `crates/goat-web`
+ `web/`) was also added after this roadmap was written and isn't listed in the phase table
above; treat it the same as `goat-tui` — a testing surface, not a phase deliverable.

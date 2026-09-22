# World Scale-Up — Phase A: Procedural Clubs & Nations

**Project:** BECOME THE GOAT
**Scope:** `goat-world` crate — world genesis (`world.rs`, `population.rs`).
**Status:** ✅ IMPLEMENTED (2026-09-22, commits `141429c` + `94d23cf`). The sections below are the original design doc, kept for context; see "Implementation notes" for where the code deviates from it.
**Relationship to the Bible:** this is not a new system — it's implementing Design Bible §7.2 ("nations and leagues across the powerhouse ↔ minnow spectrum, with full pyramids", "procedural + templates, LLMs at authoring time"), which was never built. Current code only has 2 nations / 64 hand-authored clubs as a Phase 5 placeholder (see `world.rs` header comment).
**Depends on:** nothing new — pure `goat-world` change. Does NOT touch `goat-match` (Match Flow, just tuned tonight, is orthogonal — `TacticalProfile::derive(strength, club_id, world_seed)` keeps working unchanged, just fed a bigger/richer `strength`).

---

## Implementation notes (what actually landed, and deviations from the design below)

**Landed:** 50 nations with hand-tiered `nation_power` (England 95 → Vietnam 48), 159 divisions / 2,544 clubs / 63,600 background players, deterministic `generate_world(world_seed)`, pyramid strength (quadratic elite skew inside each division, division drops [8, 28, 44, 58]), seed-picked club names from per-nation city pools × style patterns, data-driven rosters (`div_clubs`/`div_index`, `NationId(u8)` replacing the `Nation` enum), TUI picker (seed → nation → division → club), save format v8. Phase-9 goldens re-frozen.

**Deviations (decisions taken during implementation):**

1. **World structure is compile-time fixed after all.** Decision 3 (fixed 50-nation list) plus fixed per-tier division counts make club ids and division rosters seed-INDEPENDENT, so `NUM_CLUBS = 2544` / `NUM_DIVISIONS = 159` / `CLUBS_PER_DIV = 16` remain consts — asserted against the `NATIONS` table by tests. Only club names/strengths are seed-derived. This dissolves two of the three "structural blockers" without a variable-length save format; `table_raw: [u32; 80]` survives because every division has exactly 16 clubs.
2. **Generated world is NOT persisted in the save.** The doc said "generate once, cache hard into the save" to guarantee zero runtime LLM dependency — but generation turned out to be pure Rust (no LLM call, see note 3), so re-deriving from `world_seed` at load is equivalent and keeps the tiny-saves discipline ("derive, don't store"). The save only bumps to VERSION 8; v7 saves are rejected because their club indices referred to the 64-club world.
3. **`club_names_agent` is not callable from a Rust process**, so names come from a baked dataset (`nations.rs`: city pool + pattern family per nation) — the "authoring-time data" of Bible §7.2. A future offline script may regenerate that dataset via the agent without any runtime change.
4. **Peer cohorts / historic greats** use the generic `name_from_seed` pool for player names (per-nation player-name pools are a possible Phase-B-flavour follow-up); great nationalities are power-weighted so legends cluster in strong nations.
5. **Rival-verdict golden recalibrated** (200g/5t → 250g/6t): 159 divisions award 159 titles per season, so the old bar gave every seed a rival (zero variance).

---

## Overview

Grow the world from 2 nations / 64 clubs to **50 nations, 3-4 divisions per nation, ~16 clubs per division** (~2,400-3,200 clubs total, ~60-80k background players — same order of magnitude the Bible's §7.2/§9 already designed for at 20-30k, just bigger). Club strength should follow real football's shape: a handful of elite nations (England, Spain, Germany, Italy, France, Brazil, Argentina, Portugal, Netherlands...) sit far above weaker footballing nations, and within each nation a pyramid (few elite clubs at the top division, more mid-tier clubs lower down) — so a top club from a weak nation can be weaker than a mid-table club from a strong nation (real UEFA-coefficient-style effect).

This phase does **not** touch youth/scout (Phase B) or coaching staff (Phase C) — those are designed (see `memory` notes referenced below) but sequenced after this lands and is reviewed.

---

## Current state (read before touching anything)

- `crates/goat-world/src/world.rs`: `CLUBS: [Club; 64]` is a hand-authored `const` array (macro `club!(...)`), 2 nations (England, Brazil) × 2 `DivLevel` (Top/Second) × 16 clubs/division. `DIV_CLUBS: [[ClubId; 16]; 4]` is a compile-time-fixed array of division rosters. `Club::strength: u8` is gassed by hand per club.
- `crates/goat-world/src/population.rs`: `genesis(world_seed)` already loops `for club in CLUBS { for slot in 0..SQUAD_SIZE }` and is written to scale — `POP_SIZE = NUM_CLUBS * SQUAD_SIZE` is already dynamic on `NUM_CLUBS`. No change needed here beyond `NUM_CLUBS` growing.
- `Club::facilities_mult()` derives a development multiplier from `strength` — reused later in Phase C for coach/staff quality, no change needed now beyond `strength` itself changing shape.

---

## Decisions (locked, do not re-litigate — ask Tùng only if genuinely blocked)

1. **Clubs are procedurally generated, not hand-authored.** Use the `club_names_agent` MCP tool (already registered in the workspace, see `~/.claude/agents` / MCP config) to generate club names + flavor per nation/division. This matches Bible §7.2's "procedural + templates, LLMs at authoring time for flavor" — do this at **genesis time only** (when a new save/world is created), never at runtime/load.
2. **Generate once, cache hard into the save.** Club names/data are generated a single time at world genesis and persisted (not re-generated on every load — no LLM calls at runtime, per Bible §9 "zero runtime model dependency").
3. **50 nations, fixed list** — not seed-driven. Real country names (fine, geographic data isn't IP — unlike club names/branding, which is why those ARE procedural instead of using real club names).
4. **`nation_power: u8`** (new field, 1-99) — one per nation, **hand-assigned by tier**, matching real football's UEFA-coefficient-style hierarchy (England/Spain/Germany/Italy/France top tier, Brazil/Argentina/Portugal/Netherlands next tier, tapering down for smaller footballing nations). Not seed-driven — same tiers in every world.
5. **Club strength formula:**
   ```
   club_strength = nation_power + pyramid_offset(division_rank, club_rank_within_division) + small_jitter
   ```
   `pyramid_offset`: top division gets a small number of high-strength clubs and a long tail of mid-tier clubs going down the divisions — same shape as an income/wealth pyramid, not a flat distribution. Exact curve is a `[DECISION NEEDED]` for whoever implements — pick something monotonic and clamp to `1..=99`, write a unit test asserting top-division average > lower-division average per nation and that no club exceeds 99 or drops below 1.

## Known structural blockers (must fix, not optional)

- `DIV_CLUBS: [[ClubId; CLUBS_PER_DIV]; NUM_DIVISIONS]` is a compile-time-fixed 2D array. This cannot scale to a variable number of nations/divisions/clubs-per-division. Replace with a data-driven structure (e.g. `Vec<Division>` where `Division { nation: Nation, level: DivLevel, clubs: Vec<ClubId> }`), generated at genesis and persisted in the save, not a `const`.
- `Nation` enum (`England = 0, Brazil = 1`) is a 2-variant compile-time enum. 50 nations can't be a hardcoded Rust enum with hand-named variants sanely — consider a `NationId(u8)` newtype backed by generated/static metadata (name, tier) in a `Vec`/array sized `NUM_NATIONS`, not an enum with 50 hand-typed variants.
- `NUM_CLUBS`, `CLUBS_PER_DIV`, `NUM_DIVISIONS` are all `const` today. These become **generated values stored in the save**, not compile-time constants — `Population::genesis` and everything downstream needs to read club/division counts from generated world data, not from `const`.

---

## Out of scope for this task (deliberately deferred)

- **Youth + scout system** (breakthrough arc, U21→first-team promotion, scout noise) — designed, see `memory/2026-09-22-goat-rust-world-scale-design.md` section B on the assistant's side (Tùng has it). Separate task, after this lands.
- **Coaching/staff system** (head coach, fitness coach, club psychologist, physio, set-piece/GK coaches, hireable personal staff + agent) — designed, section C of the same notes. Reuses `facilities_mult` once this phase's `nation_power`/pyramid strength exists. Separate task, after B.
- Promotion/relegation gameplay logic, fixtures/calendar scaling to the new club count — touch only if genesis/data structures block on it; otherwise leave `fixtures.rs`/`calendar.rs`/`season.rs` alone for this task.

---

## Acceptance / gates

- `cargo test --workspace` stays green, including existing `genesis_is_deterministic` / `genesis_is_full_and_columnar` / `background_current_never_exceeds_potential` tests in `population.rs` (adapt them to the new club/nation counts, don't delete the invariants they check).
- New test: same `world_seed` → bit-for-bit identical generated world (clubs, nations, strengths) across runs — same determinism discipline as the rest of the codebase.
- New test: strength distribution sanity — pyramid shape holds (top division stronger on average than lower divisions, within a nation), and cross-nation the `nation_power` ordering is respected (no low-tier nation's average club strength exceeds a top-tier nation's, modulo pyramid/jitter noise at the extremes).
- `cargo fmt --all --check` + `cargo clippy --workspace --all-targets -- -D warnings` clean, same as always.
- Report back: club/nation counts generated, a few sample clubs from different tiers/divisions to sanity-check the pyramid by eye, and confirmation the `club_names_agent` integration only fires at genesis (not on save load).

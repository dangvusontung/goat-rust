# TASK DESIGN ROUND 2, DOC A — World genesis scale-up: generated clubs, ~20 nations, promotion/relegation

Prereq: none — this is a pre-implementation design spec for a scope expansion Tùng approved
verbally on 2026-07-22 (not previously written down). Covers items 1–3 of that approval.
Item 4 (national-team layer + tactical identity) is written up separately in
`tasks/TASK-DESIGN-round2-national-team-tactical-identity.md` — see "Why split into two
docs" below.

Read first: `docs/MAIN.md` §3 (System Architecture — headless core, pure/no-IO), §7.1–7.2
(Living World & Simulation Strategy, World Genesis), §9 (Engineering & Performance Notes —
tiny-save, struct-of-arrays, seed-is-the-universe), `crates/goat-world/src/world.rs` (current
hardcoded club/division data), `crates/goat-world/src/history.rs` (the seeded-generation
pattern this task must follow for club/nation identity too).

## Why split into two docs

Tùng's approved scope was one conversation but is architecturally two unrelated pieces of
work:

- Items 1–3 (this doc) are all the **same subsystem**: static compile-time world data
  (`crates/goat-world/src/world.rs`'s `CLUBS`/`DIV_CLUBS`/`NUM_DIVISIONS` consts) becoming
  seed-derived generated data, at increasing scale, with season-to-season mutation
  (promotion/relegation) layered on top. They share one architectural transformation and one
  ripple surface (every `CLUBS[id]`/`DIV_CLUBS[div]` call site — **~117 of them**, counted
  below).
- Item 4 (national teams + tactical identity) touches **none** of that. It's a brand-new data
  axis on `crates/goat-core/src/roles.rs`'s role-fit system and a brand-new selection
  subsystem with no existing code to extend. It doesn't share a single file with items 1–3
  except incidentally (`Nation` as an identifier).

One document covering both would either bury the national-team design under 3 slices of
"replace this const array" mechanics, or the reverse. Splitting lets each doc get its own
effort/risk read and be sequenced independently — Tùng could ship this doc's items 1–3 and
leave item 4 for a later round, or vice versa, without re-deriving which parts depend on
which.

## Ground rules for this doc

- **No change to player generation.** `crates/goat-world/src/population.rs`'s genesis
  pipeline (attribute rolls, potentials, role DNA) is untouched. This task only changes what
  *club* a generated player belongs to and how many clubs/nations exist — not how players
  themselves are generated.
- **No change to the past-greats canon's hand-authored flavor.** `crates/goat-meta`'s
  `PastGreat`/`CANON` (the pantheon, `crates/goat-meta/src/pantheon.rs:64-139`, per
  `TASK-DESIGN-round1-pantheon-saves.md` §2.5) stays hand-authored fictional data, untouched.
  `crates/goat-world/src/history.rs`'s *backfilled* greats (`HistoricGreat`, generated,
  already fictional/safe) DO ripple slightly — see A2.4 — because they currently roll
  `nationality: u8` against a hardcoded 0..1 range that must track the new nation count.
- **Struct-of-arrays discipline extends to the new club/nation data too.** The bible's SoA
  mandate (§9) is about the 20-30k *player* population (`PlayerStore`,
  `crates/goat-core/src/player.rs:20-41` — parallel `Vec<T>` columns, not `Vec<Struct>`).
  Club/nation/league *structure* is a much smaller count (~1,000 clubs, ~20 nations,
  ~60 divisions even at full scale) where a `Vec<Club>` of small owned structs is completely
  fine — this is not the 20-30k hot path SoA is protecting. Do not over-apply columnar
  storage here; it would be a premature abstraction for a dataset 20-30x smaller than the one
  the bible's SoA note is actually about.
- **"Generated but consistent," same pattern as `history.rs`.** Every new generated
  identity (nation name, club name, nation stature, league name) must be a pure function of
  `world_seed` (+ a small per-entity index), reproducible, with no runtime LLM call (bible
  §7.2, §9 — "LLMs at authoring time only").

## Verified: the current architecture this task must change

`crates/goat-world/src/world.rs:62-105` — `NUM_CLUBS: usize = 64`, `CLUBS_PER_DIV: usize =
16`, `NUM_DIVISIONS: usize = 4`, `DIV_CLUBS: [[ClubId; 16]; 4]`, `DIV_NATIONS`, `DIV_LEVELS`
are all `const` — fixed at **compile time**, not generated. `CLUBS: [Club; 64]`
(`world.rs:119-194`) hardcodes 64 real club names (Manchester City, Liverpool, Flamengo, …)
with hand-picked strength values. `Nation` (`world.rs:12-32`) is a 2-variant enum
(England/Brazil), not a generated set.

**Current population scale is nowhere near the bible's 20-30k target.**
`crates/goat-world/src/population.rs:25,28` — `SQUAD_SIZE: usize = 25`,
`POP_SIZE: usize = NUM_CLUBS * SQUAD_SIZE` = 64 × 25 = **1,600 players**. This task is
literally the work of reaching the bible §7.2/§9 population target, not a separate future
concern — the "~20-30k players" line in the bible has never been implemented, only declared.

**Ripple size, counted by grep, 2026-07-22:**
- `crates/goat-tui/src/main.rs` — 28 occurrences of `CLUBS`/`DIV_CLUBS`/`DIV_NAMES`/
  `Nation::`/`DivLevel::`.
- `crates/goat-bridge/src/api.rs` — 31 occurrences.
- `crates/goat-world/src/{batch_tick,history,fixtures,season,population,world,lib}.rs` — 58
  occurrences combined.
- Total: **~117 call sites** across a 3,897-line combined `main.rs`+`api.rs` surface. This is
  the single biggest reason this doc rates "extra-large" overall (see sizing at the end) —
  not conceptual difficulty, but sheer mechanical ripple.

**A fixed-size array is baked into the save format, not just `goat-world`:**
`crates/goat-core/src/state.rs:58` / `crates/goat-save/src/save.rs:46` —
`table_raw: [u32; 80]` (`= 5 × CLUBS_PER_DIV`, i.e. hardcoded to 16 clubs/division) is the
**PC's own division's** league table, persisted in the save. This is small in scope (it's
one division, not the whole world) but its fixed size is load-bearing for the current binary
save format (`crates/goat-save/src/save.rs:433,531` write/read this as a flat 80-`u32` block)
— see A2.5 for why this doc's recommended design keeps it a fixed size rather than turning it
into a `Vec`.

**Where non-PC leagues are (and aren't) actually simulated today** — this matters for A3:
`goat_world::batch_tick::batch_tick_season` (`crates/goat-world/src/batch_tick.rs:65-100`)
does run a real, full round-robin table sim (via `season::Table` + `fixtures::round_fixtures`)
for **every** division — but it is only ever invoked **on demand**, replaying from season 1
every time (`crates/goat-tui/src/main.rs:1857-1860`, inside the history-browser/rival screen:
`for s in 1..=seasons { batch_tick_season(&mut pop, seed, s, s*52) }`), never incrementally
persisted season-to-season. The season-by-season `Intent::BatchTickPeers` reducer that *does*
run every season (`crates/goat-core/src/state.rs:753-768`) is a much cheaper, unrelated
mechanism — a handful of named rival "peers" random-walking flavor stats, not a real league
table for the other divisions. **There is currently no code path that maintains a persisted,
incrementally-updated league table for any division except the PC's own.** This is the key
fact A3's design leans on.

---

## Slice A1 — Generated club identity (names), same 2-nation/4-division/64-club shape

**Decision (verbatim, item 1):** "Generated club names instead of hardcoded real ones... no
license for real club names, so everything must be generated/fictional... deterministic per
world_seed, following the same 'generated but consistent' pattern as background player
names/the seeded historic canon."

**Scope for this slice specifically:** replace only the 64 `name: &'static str` fields in
`CLUBS` (`world.rs:119-194`) with generated fictional names. Do **not** change `NUM_CLUBS`,
`NUM_DIVISIONS`, `CLUBS_PER_DIV`, `Nation`, or any strength value in this slice — that's A2.
Shipping A1 alone first gives Tùng a low-risk, independently-releasable slice (pure content
swap, zero architecture change) before committing to A2's much bigger rewrite.

### A1.1 — Why this can't stay `&'static str`

`Club.name: &'static str` (`world.rs:45`) is a compile-time string literal. A generated name
is owned (`String`), computed once at genesis from `world_seed`. This means `Club` becomes a
non-`Copy`, non-`const`-constructible struct (`#[derive(Debug, Clone, Copy)]` at `world.rs:42`
must drop `Copy`), and `CLUBS: [Club; NUM_CLUBS]` can no longer be a `const` — it becomes a
value built once at genesis time and threaded through, exactly like `History` already is
(`crates/goat-world/src/history.rs:66-70`, built by `backfill_history(seed, n)` and passed
around by reference/clone, not a global const).

This is the load-bearing architecture decision this whole doc pivots on: **club/league/nation
structure moves from `const` data to a `WorldGenesis` value, generated once per save from
`world_seed`, held in memory for the session** (not necessarily persisted — see A2.5). A1 is
the smallest possible slice that forces this shift, which is why it's sequenced first even
though "just swap 64 names" sounds trivial — the mechanical ripple (Copy → Clone, `CLUBS[i]`
global lookup → `world.clubs[i]` field access) is the real work, not the name generator
itself.

### A1.2 — Name-generation approach

Follow `history.rs:76-85`'s exact pattern (`make_name`/`name_from_seed`): two small
`&'static str` word-bank arrays (e.g. a `CLUB_PREFIX`/`CLUB_SUFFIX` or
`CITY_STEM`/`CLUB_NOUN` pair — e.g. "Ashford", "Brackwell", "Solmoor" × "United", "Athletic",
"Rovers", "Town", "City" — real football-club-naming conventions, fictional place names) and
a seeded RNG draw per club: `GoatRng::new(world_seed ^ club_seed(club_id))` →
`format!("{city} {suffix}")`. This is a genuinely new word-bank (place-name stems), not reuse
of `FIRST_NAMES`/`LAST_NAMES` (those are person names, wrong register for a club name) — a
new small const array, hand-authored fictional place-name fragments (same register as
`history.rs`'s existing hand-authored `FIRST_NAMES`/`LAST_NAMES` — no LLM call, no license
risk, since these are invented word fragments, not real place/club names).

**Underspecified — needs Tùng's input, not a Design guess:** how large should the word banks
be, and should club naming vary by nation (e.g. Brazilian-register vs English-register club
names) to preserve the current game's flavor of "this is clearly an English club" vs "this is
clearly a Brazilian club"? The current 64 hardcoded names lean hard on real-world naming
convention differences (English: "Town/United/City/Rovers"; Brazilian: single-word club
identities like "Flamengo"/"Corinthians"). A flat single word-bank across all nations would
homogenize that flavor distinction, which may or may not be an acceptable loss — **flag for
Tùng**, don't silently pick "one generic word bank" or "N per-nation word banks" without
confirmation, since the second option is real extra authoring work (multiple word banks) that
only pays off if nation-flavored naming matters to him.

### A1.3 — Ripple (this slice only, before A2's bigger rewrite)

Mechanical, not conceptual: every one of the ~117 call sites above that reads `CLUBS[id].name`
still works (the field still exists, still `&str` via `.as_str()`/`&club.name`), but the
*value* `CLUBS` becomes a genesis-time-built value, not a global const — so every reader needs
that value threaded to it (function parameter or a field on whatever state/context object
already flows through `goat-tui`/`goat-bridge`, most likely alongside `world_seed` itself).
`crates/goat-tui/src/main.rs` and `crates/goat-bridge/src/api.rs` both already carry
`world_seed`/`state.world_seed` everywhere `CLUBS` is read — the natural home is building
`WorldGenesis` once at genesis/load time (same moment `History::backfill_history` already
runs) and passing/storing it alongside the existing `state`.

- TDD: `crates/goat-world/src/world.rs` gets a new test module —
  `club_names_deterministic_per_seed` (two `generate_clubs(seed)` calls with the same seed
  produce identical names) and `club_names_differ_across_seeds` (different seeds produce at
  least one different name), mirroring `history.rs:206-216`'s
  `history_is_deterministic`/(implicit) cross-seed-difference pattern.
- Playable gate: `cargo run -p goat-tui` → new game → league table / transfer screens show
  generated names, not "Manchester City"/"Flamengo" etc.

**Size: medium, risk: low.** Small conceptually, but touches the `Copy`→`Clone` derive change
and the threading-through of a non-const value — the mechanical foundation A2 builds on.
Doing this first, alone, means A2's much bigger diff isn't also debugging "does the const→value
conversion work" at the same time as "does 20-nation genesis work."

---

## Slice A2 — Scale genesis: ~20 nations, multiple leagues/tiers per nation

**Decision (verbatim, item 2):** "Scale up world genesis: ~20 nations, multiple leagues per
nation, multiple levels (tiers) per league... procedurally generated at genesis time
(seed-derived nation names/club names/league structure), while staying consistent with the
existing 'whole universe from one seed' pillar (bible §2.3) and the SoA performance
architecture for the 20-30k player population (bible §9)."

### A2.1 — Concrete numbers: a grounded proposal, not a guess, but still needs Tùng's sign-off

The bible already sets one hard target this task can anchor to: **~20,000-30,000 players**
(`docs/MAIN.md` §7.2 point 3, §9). Combined with the *already-implemented*
`SQUAD_SIZE = 25` (`population.rs:25`, unchanged by this task per the ground rules), the
population target directly constrains total club count:

```
target_players / SQUAD_SIZE = target_clubs
20,000 / 25 = 800 clubs        30,000 / 25 = 1,200 clubs
```

At 20 nations, that's **40-60 clubs per nation**. A natural, round split consistent with the
current game's existing "top flight + second tier" 2-level shape
(`world.rs:36-39`, `DivLevel::Top`/`Second`) extended by one more tier:

**Confirmed by Tùng, 2026-07-22: 20 clubs per tier** (not 16 — Design's first pass proposed 16
to reuse `CLUBS_PER_DIV` unchanged; Tùng explicitly overrode this). Reworked numbers: **3
tiers per nation × 20 clubs per tier = 60 clubs/nation × 20 nations = 1,200 clubs → 30,000
players** — top edge of the bible's 20-30k band, still inside it.

Because `CLUBS_PER_DIV` is changing from 16 to 20 anyway, this slice is **not** the
zero-touch "just add more instances of the same fixed-size division" case the first pass
described — `CLUBS_PER_DIV = 16` (`world.rs:63`) itself needs to become 20, which ripples into
`fixtures.rs`'s `ROUNDS_PER_SEASON = (CLUBS_PER_DIV - 1) * 2` (`fixtures.rs:10`, becomes 38 —
matching real-world 20-club leagues' 38-round season) and `season::Table`'s
`[TableEntry; CLUBS_PER_DIV]` fixed-size array (`season.rs:57-58`, becomes 20-wide). Both are
still **uniform, fixed-size-per-tier changes** (every tier in every nation stays exactly 20
clubs) — not the larger variable-per-tier-sizing rewrite described below, which remains
unconfirmed and separate.

Still open, and *not* resolved by the club-count confirmation above: variable club-counts-per-
tier (e.g., a smaller top flight, bigger lower tiers — closer to how real football pyramids
actually taper) is realistic and arguably more authentic, but turns `[TableEntry; CLUBS_PER_DIV]`,
`table_raw: [u32; 80]`, and `ROUNDS_PER_SEASON` from fixed consts into per-division values,
rippling into the save format (A2.5) and `fixtures.rs`'s round-generation math. **Flag for
Tùng: uniform 20-clubs-per-tier (low risk, reuses the same shape, less realistic) vs. variable
per-tier sizing (more realistic, meaningfully bigger rewrite of
`season.rs`/`fixtures.rs`/the save format's `table_raw`)** — do not silently pick one.

Also needs Tùng's confirmation, not a Design default: **should every nation get exactly 3
tiers**, or should the powerhouse-vs-minnow spectrum (bible §4.1, §7.2 point 1: "nations and
leagues across the powerhouse ↔ minnow spectrum, with full pyramids") mean some nations get
fewer tiers (a minnow nation with 1 shallow league) and others more (a powerhouse with 4+)?
The bible text leans toward the latter ("full pyramids" implies variable depth), but that's a
bigger generation-logic and promotion/relegation-topology problem (A3) than uniform 3-tiers-
everywhere. **Recommendation: uniform 3 tiers/nation for this round** (bounds the scope of
A2+A3 to something shippable), with variable-depth pyramids flagged as an explicit future
enhancement once uniform tiers are proven out — but this is a recommendation, not a decision
already made; confirm before Dev starts.

**Data-shape note (added post-review, 2026-07-22):** regardless of how the uniform-vs-variable
clubs-per-tier question above is resolved, `League.clubs` (A2.3's refined struct, below) is a
`Vec<ClubId>` with a `max_clubs: u8` field, not a `[ClubId; N]`-shaped const array — so this
decision only ever changes a *number* stored in `max_clubs`, it never forces a type-level
rewrite of `season.rs`/`fixtures.rs` either way. That said, `ROUNDS_PER_SEASON`/`table_raw`'s
*sizes* still depend on which club-count is chosen — now confirmed as 20, not 16 (see A2.1) —
the refinement decouples the type shape from the number, it does not make the number itself
free to change without touching those two.

### A2.2 — Nation generation

**SUPERSEDED (2026-07-23) — nation *naming* only.** Tùng reversed the "fictional word-bank"
call below for nations specifically: **nations must be real-world countries** (a fixed,
hand-authored list of ~20 real country names — England, Brazil, Germany, etc. — not
seed-generated fictional names). Clubs (A1.2) and player names (`history.rs` `FIRST_NAMES`/
`LAST_NAMES`) stay fictional/procedurally generated exactly as designed — only the nation
identity itself changes from generated to real. The stature/quality-band mechanism below
(seed-derived club-strength spread per nation) is unaffected and still applies — it just now
shifts each *real* country's generated clubs, instead of a *fictional* one's. Actual code
(`NATION_PREFIX`/`NATION_SUFFIX` word banks and the generation call in `world.rs:242-243`)
has **not** been updated to match yet — this is a real-country name list swap-in, tracked as
follow-up implementation work, not done by this doc edit alone.

`Nation` (`world.rs:12-32`) goes from a 2-variant enum to a generated `Vec<GeneratedNation>` (or
similar), each with:
- A generated name (same word-bank-and-seed approach as A1.2, a new fictional-country word
  bank — e.g. syllable-combination generation, since 20 invented country names is a bigger ask
  than 16-per-tier invented club names and a flat prefix/suffix bank will look repetitive
  faster; **flag for Tùng**: is a simple syllable-combinator good enough, or does this deserve
  the same hand-authored-word-bank treatment as `history.rs`'s person names? A larger
  hand-authored bank of ~20-40 fictional country-name fragments is cheap to write and much
  less repetitive than a 2-3-syllable combinator — Design's recommendation is the hand-authored
  bank, but flag it since it's additional authoring work, not free).
- A **stature/quality band**, seed-derived — this is the mechanism that preserves bible
  §4.1's nationality-as-difficulty-dial ("Powerhouse nation... Minnow nation... this single
  choice tilts which legacy axes are even available") and §7.2's explicit "powerhouse ↔
  minnow spectrum" requirement once England/Brazil's hand-tuned strength numbers
  (`world.rs:121-193`, ranging 90 down to 32) are no longer hardcoded. Concretely: a
  `stature: u8` or similar scalar that shifts the mean/spread of that nation's generated club
  strengths (mirroring how England's clubs today run 90→45 and Brazil's run 82→32 — a
  powerhouse nation's *floor* is still respectable; a minnow's *ceiling* is capped). This is
  the one genuinely new generated attribute this slice needs beyond "names" — it's what makes
  a randomly-rolled minnow nation actually *play* like a minnow (the George Best run,
  bible §4.1) rather than just *sound* like one.

### A2.3 — League/division generation

`DIV_CLUBS`/`DIV_NAMES`/`DIV_NATIONS`/`DIV_LEVELS` (`world.rs:72-105`) become generated: for
each of the ~20 nations, 3 tiers (per A2.1's proposal), each tier a division with a generated
league name (reuse the fictional-place-name word bank from A1.2 for "the ___ League" style
naming, or a simpler ordinal scheme — top flight gets the nation-flavored name, e.g. "the
Ashford Premier Division," lower tiers "Division Two"/"Division Three" — **this is a small,
low-stakes content choice Design can make without asking Tùng**, unlike A2.1/A2.2's numbers).
`NUM_DIVISIONS` becomes `nations × tiers_per_nation` (60 at the proposed 20×3).

`clubs_for_nation`/`club_division`/`club_div_pos` (`world.rs:212-215`, `197-210`) keep their
current signatures and logic — they already iterate/search rather than hardcode indices, so
they port over unchanged once `CLUBS`/`DIV_CLUBS` are runtime values instead of consts (same
"port the logic, change the storage" shape as A1).

**Refinement (post-review, 2026-07-22): `League.clubs` is a `Vec<ClubId>`, not a fixed-size
array.** The generated per-division data above is best modeled as one `League` struct per
division — replacing the four parallel `DIV_CLUBS`/`DIV_NAMES`/`DIV_NATIONS`/`DIV_LEVELS`
arrays with a single `Vec<League>` — where:

```rust
struct League {
    id: LeagueId,       // was: index into DIV_CLUBS/DIV_NAMES/DIV_NATIONS/DIV_LEVELS
    nation: NationId,    // was: DIV_NATIONS[div]
    tier: DivLevel,      // was: DIV_LEVELS[div]
    name: String,        // was: DIV_NAMES[div], generated per this section
    clubs: Vec<ClubId>,  // was: DIV_CLUBS[div]: [ClubId; CLUBS_PER_DIV]
    max_clubs: u8,       // NEW — the cap is data, not baked into the type
}
```

`clubs` must be `Vec<ClubId>`, not `[ClubId; N]`, regardless of what N is settled at — A3
already requires `League.clubs` to be *mutated* season-to-season (a relegated club is removed
from one league's `clubs` and inserted into another's, see A3.1/A3.3), so a fixed-size array
was never actually viable once promotion/relegation exists. This refinement doesn't add a new
capability; it just stops the type shape from implying a fixed roster that A3 would have had to
work around anyway — a near-zero-cost change, since `Vec<ClubId>` at ~16-20 entries is a tiny
heap allocation done once at genesis (or once per promotion/relegation transition), nowhere
near the SoA-sensitive 20-30k player population this doc's ground rules already carve out as a
different concern.

`max_clubs: u8` moves per-league capacity from the type system (today's `CLUBS_PER_DIV` const,
reused everywhere via array-size generics) into a plain field on the struct. This is what
actually buys future flexibility: a later round adding MLS-style no-relegation conferences, a
36-team Champions-League-style league phase, or smaller 10-18-club domestic leagues only needs
different `max_clubs` values per `League`, not a new type per league shape. **This round's own
numbers are unaffected by this refinement** — see A2.1's decision #1 for what the actual
per-tier club count is; `max_clubs` just holds whichever number Tùng confirms there, as data
instead of a type parameter.

**Resolved 2026-07-22 (was flagged as a mismatch, now confirmed):** the earlier pass's proposal
of 16 clubs/tier and this refinement pass's brief saying "already decided at 20" were in fact
the same decision at two different points in time — Tùng confirmed 20 explicitly, overriding
Design's 16 recommendation. A2.1 has been re-derived to 20/tier × 3 tiers × 20 nations = 1,200
clubs → 30,000 players (top edge of the bible's 20-30k band). `ROUNDS_PER_SEASON`/`table_raw`
sizing (under the uniform-per-tier path) now target 20, not 16 — see A2.1.

### A2.4 — `history.rs` ripple (backfilled canon must track the new nation count)

**Verified, mechanical, easy to miss:** `crates/goat-world/src/history.rs:96` —
`let nationality = rng.next_range_u32(0, 1) as u8;` is hardcoded to a 2-nation range (England
=0/Brazil=1). This must become `rng.next_range_u32(0, num_nations - 1)`. Similarly
`great_nation_name` (`history.rs:196-200`) calls `Nation::from_idx` — once `Nation` is a
generated `Vec` rather than a 2-variant enum, this becomes an index into that vec (or a
lookup by id) rather than an enum match. `history.rs:118-129`'s champion-resolution loop
already iterates `DIV_CLUBS` generically (no hardcoded division count), so it needs no logic
change beyond `DIV_CLUBS` being a runtime value — same shape as A1/A2.3.

### A2.5 — Genesis-time cost & the "regenerate vs persist" call

**Verified performance baseline (bible §7.2):** "~3-10s naive, ~1-3s with lazy generation" is
the bible's own estimate for the *current* 64-club/1,600-player genesis, dominated by the
history batch-sim. At ~15x the club count (64 → 960) and ~15x the player count (1,600 →
24,000), the history backfill (`backfill_history`, `history.rs:88-156` — one pass per
backfilled season, resolving champions across every division) scales roughly linearly with
division count × clubs/division, since `sim_team_match` (`season.rs:8-30`) is a handful of
integer ops and RNG draws per match, not expensive per-call. A rough order-of-magnitude
estimate: if the current 4-division/64-club genesis sits at the bible's own quoted 1-10s,
scaling division count ~15x (4→60) without any other change plausibly pushes worst-case
genesis toward **10s-60s** if done naively single-threaded — still one-time, background-thread,
loading-screen-hidden work (bible §7.2's own framing), but enough to be worth confirming
rather than assuming "still fine." **Flag for Tùng and Dev:** this needs an actual benchmark
once A2 is implemented (`cargo run` genesis timing with `--release`), not just this estimate —
if it lands past ~10-15s even backgrounded, that's worth a design conversation about trimming
`NUM_GREATS`/backfilled-season-count or parallelizing the per-division backfill loop (each
division's champion resolution is already independent — `history.rs:118-129`'s `.map()` over
`DIV_CLUBS` is trivially parallelizable with e.g. `rayon` if it becomes the bottleneck, though
that would be a **new dependency**, which the existing "no new dependencies" ground rule
(`TASK-DESIGN-round1-pantheon-saves.md`'s Definition of Done) would need an explicit
exception for — don't add it preemptively, only if the benchmark says it's needed).

**Regenerate-from-seed vs persist:** following bible §9's "the seed is the universe" /
"recompute the rest" principle, and mirroring `History` (never persisted, always
recomputed from `world_seed` on load, per `history.rs`'s own doc comment at line 6-8), this
task's default position is: **`WorldGenesis` (nations/leagues/clubs, minus promotion/
relegation's path-dependent membership, see A3) is NOT persisted in `SaveData` — it is
recomputed from `world_seed` every time a save loads**, exactly like `History` already is.
This keeps the save format's `table_raw: [u32; 80]` (A1.2's finding) as the *only* other
world-shape-adjacent persisted field, and it stays a fine fixed size under the uniform-16-
clubs-per-division proposal (A2.1). No `SaveData::VERSION` bump needed for A2 itself.

- TDD: `crates/goat-world/src/world.rs` (or wherever `generate_world(seed)` lands) gets
  `world_deterministic_per_seed` / `world_differs_across_seeds` (same shape as A1's name
  tests, but asserting on the whole generated structure: nation count, clubs-per-nation,
  total club count matches the agreed number). `history.rs`'s existing
  `history_is_deterministic`/`backfill_is_internally_consistent` tests
  (`history.rs:206-237`) need updating for the new nation/division counts but should assert
  the same invariants (exactly-one-Ballon-d'Or-per-season, champions-are-real-clubs-in-their-
  division) — these are strong existing regression anchors, keep them passing, don't weaken
  the assertions to "pass with any number."
- Playable gate: `cargo run -p goat-tui` → new game → nation-select screen shows ~20 generated
  nations with varying stature; genesis completes in an acceptable time (manually timed, see
  above); world/transfer-market screens show the expanded structure.

**Size: extra-large, risk: high.** This is the doc's biggest slice by far — the ~117-call-site
ripple (mostly mechanical, `CLUBS[i]` → `world.clubs[i]`-shaped edits) plus the genuinely new
generation logic (nation stature, league naming) plus an unverified perf question that needs a
real benchmark, not just this estimate. Recommend Tùng treat A2 as its own dedicated
implementation round, not a single Dev slice alongside A1/A3.

---

## Slice A3 — Promotion & relegation between tiers

**Decision (verbatim, item 3):** "Promotion/relegation between tiers... real season-end
logic: bottom-N of a tier drop, top-N of the tier below rise, club roster/players carry over
correctly, league tables regenerate for the new tier composition next season. Must interact
correctly with the existing season-end pipeline (the same one round-3's Slice 3 fix just made
idempotent — read that fix, commit history around d77170b, before touching this)."

**Verified: zero existing simulation logic.** Grepped the whole repo for
promotion/relegation: the only hits are `docs/FLUTTER-APP-GUIDE.md:191` ("mark champions/
relegation if the DTO flags them" — a UI-doc aspiration with no backing field) and
`TableRowDto` (`crates/goat-bridge/src/api.rs:204-215`) has no relegation flag at all. This
confirms the task's framing exactly — this is new logic, not a bug fix.

**Verified: "roster carries over" is close to automatic.** Squad membership is keyed by club,
not by division (`crates/goat-world/src/population.rs` — `pop.club: Vec<u16>`
(`population.rs:37`) stores a club id per player; `batch_tick.rs:43-47`'s `squads_by_club`
groups by `pop.club[idx]`, not by division). A club moving from tier 2 to tier 1 doesn't need
any player-record mutation at all — its existing squad (already keyed to the club id) simply
starts appearing in a different `DIV_CLUBS` bucket next season. The real work is entirely in
**which bucket a club id sits in**, not in touching player data.

### A3.1 — The genuinely hard question: does promotion/relegation need new persisted state?

**This is the load-bearing design decision of this slice, and it resolves cleanly against the
existing architecture — but needs Tùng's explicit sign-off since it's a real fork, not a
detail.**

Because `batch_tick_season` (`batch_tick.rs:65-100`) is **already a pure, deterministic
function of `(world_seed, season, elapsed_weeks)`** that computes a full table for every
division, and the existing rival-crystallization screen already replays it from season 1
forward on every view (`main.rs:1857-1860`, no persisted incremental state) — a club's
tier-membership-per-season can be computed the *same* way: **derive it by replaying
promotion/relegation season-by-season from genesis, purely from `world_seed` +
`season_number`, with zero new persisted fields.** This is the natural extension of the
already-established "generated but consistent" pattern (`history.rs`) to a path-dependent
quantity (this season's tier membership depends on last season's table, which depends on the
season before, etc. — same shape as `HistoricGreat.ballon_dors` already being a running
path-dependent accumulation resolved by replay, `history.rs:114-153`).

**The alternative** — persist a `club_id → current_tier` mapping, updated incrementally each
real season-end (only for the PC's own division + whatever the game actually simulates
live) — is simpler to reason about per-season (no replay cost) but breaks the "seed is the
universe, regenerate the rest" principle for the *other* ~59 divisions the player never
directly plays in, since those currently have **no season-by-season incremental persisted
state of any kind** (verified above — only `Intent::BatchTickPeers`'s unrelated peer-flavor
walk runs every season; the real per-division tables are always recomputed from scratch on
demand).

**Recommendation: replay-from-seed, no new persisted state**, consistent with the rest of
this doc's "don't persist what you can regenerate" stance (A2.5) — but flag two real costs
this creates that Tùng should weigh before confirming:

1. **Replay cost grows with career length.** Computing "what tier is club X in during season
   30" means replaying promotion/relegation for seasons 1-29 first. Unlike `History`'s
   backfill (a fixed ~20-30 season window, computed once and then the *player's own* career
   proceeds independently of it), this replay grows every season the player advances — by
   season 40, that's 40 replayed seasons, each with ~60 divisions × 30-round tables (A2.1's
   numbers). This is exactly the kind of cost the existing rival-crystallization screen
   already accepts (it's an on-demand, occasionally-viewed screen) — but if promotion/
   relegation membership needs to be checked *every season* (to know the PC's own league
   table opponents next season, if the PC's club or its rivals get promoted/relegated), this
   moves from "occasionally replayed for a flavor screen" to "replayed every single season
   transition," which is a materially different cost profile. **Needs a benchmark once A2 is
   built, same as A2.5's genesis-time question — flag as linked, don't estimate blind.**
2. **An in-memory (not persisted) cache of "current tier membership as of the last computed
   season" is almost certainly necessary** to avoid literally re-replaying from season 1 on
   every single week-tick — cache the replay result for the current session, keyed by the
   highest season number computed so far, extend it incrementally as `season_number`
   advances (recompute one more season's promotion/relegation delta on top of the cached
   state, not the whole history). This is a concrete, buildable design (it's exactly how an
   incremental fold works), but it's a caching-strategy decision for **Dev to implement**, not
   something this doc needs to over-specify further — flagging it here so Dev doesn't
   discover the O(seasons) replay cost mid-implementation and improvise a fix under time
   pressure.

**If Tùng would rather have the simpler, persisted-incremental-state design instead** (accept
a small new persisted field — e.g. a `Vec<u16>` "current division index per club," a few KB at
1,200 clubs, cheap by tiny-save standards — updated at each real season-end alongside the
existing table/legacy pipeline, and accept that this means the ~59 non-orbit divisions now get
a small new per-season write where previously they had none), **that's a legitimate simpler
alternative** — flag it as the explicit fork; either is buildable, but they have different
season-end pipeline shapes and this doc's other numbers (perf estimates) assume the replay
approach. **This is the single most important open decision in this doc — do not let Dev
silently pick one.**

### A3.2 — Promotion/relegation rule (bottom-N drop, top-N rise)

**Underspecified — needs a number from Tùng, not a Design guess:** "bottom-N... top-N" was
stated as the shape but not the value of N. A conventional football pyramid uses 3
(3 relegated, 3 promoted, sometimes with playoffs for a 4th spot) at 16-20 clubs/tier size —
**Design's recommendation is N=3** (standard, well-understood, no playoff mechanic needed for
a first pass) but this is exactly the kind of specific number the round-1 ground rule ("don't
silently invent numbers for things he didn't actually decide") warns against — confirm before
Dev starts. Related, also unconfirmed: **should the bottom tier have relegation at all** (no
tier below it to drop to — presumably no, but confirm no crash/edge-case is expected there)
and **does the top tier have promotion** (no, symmetric reasoning) — Design assumes both are
simply no-ops at the pyramid's edges, which should be uncontroversial, but is worth stating
explicitly as an assumption rather than leaving it implicit.

**Refinement (post-review, 2026-07-22): the per-club outcome is a typed enum, not a `bool`.**
Whichever of A3.1's replay-vs-persist fork Tùng picks, the per-club result of a season boundary
is represented as:

```rust
enum TransitionType {
    DirectPromotion,
    DirectRelegation,
}

struct PromoRelegationEvent {
    club: ClubId,
    season: u32,
    from_league: LeagueId,
    to_league: LeagueId,
    transition: TransitionType,
}
```

Exactly 2 variants, matching exactly what this round's rule implements: top-N of a tier rises
(`DirectPromotion`), bottom-N drops (`DirectRelegation`), nothing else. This is deliberately
**not** `isPromotion: bool` — a bool can only ever mean "up or down," while an enum leaves room
for a later round to add variants without rewriting every call site that currently
pattern-matches a bool into an `if`/`else`.

**Explicitly not being added this round:** no `PlayoffPromotion` variant (no playoff mechanic
exists or is being built — this section's proposed N is a clean top-N/bottom-N cut) and no
`AdministrativeRelegation`/club-dissolution variant (no bankruptcy/dissolution mechanic exists
or is being built anywhere in this doc). The enum's 2-variant shape is future-proofing the
*call-site pattern* — every place that currently would have written `if event.is_promotion`
instead pattern-matches on `transition`, so adding a third variant later touches only the
match arms that care, not every caller — it is not a commitment to build either of those
mechanics. See "Out of scope" below.

### A3.3 — Interaction with the season-end pipeline (the d77170b idempotency fix)

**Verified:** `d77170b` (`git show d77170b`) fixed a bug where re-viewing [G] Legacy at the
end-of-season gate re-ran the *entire* season-end pipeline (`CollectWage`,
`ApplySeasonEndLegacy`, `BatchTickPeers`, transfer window, contract renewal) because nothing
tracked "have I already run this season's pipeline." The fix added
`season_end_done_for: Option<u32>` (`crates/goat-tui/src/main.rs:293,323,387`) gating the
pipeline body so it runs exactly once per season boundary while the read-only [G]/[Y]/[Z]/[Q]
menu still redisplays freely.

**Promotion/relegation must be wired into that same gated block, not a separate trigger.**
Concretely: wherever `Intent::BatchTickPeers`/`ApplySeasonEndLegacy` currently fire inside the
`season_end_done_for != Some(state.season_number)` guard (`main.rs:323-387`), add the
promotion/relegation resolution (whichever of A3.1's two designs is chosen) to that same
gated, run-once block. Getting this wrong reproduces exactly d77170b's bug class — a second
view of the season-end screen re-promoting/re-relegating clubs, double-moving them, or (worse,
if using the persisted-mapping alternative) corrupting the persisted tier assignment on a
second run. **This is the concrete reason the task brief called out reading that commit before
touching this slice** — confirmed necessary, not just due diligence.

The resolution step should be modeled as producing an ordered `Vec<PromoRelegationEvent>`
(A3.2's refined type) for the season, applied atomically inside that gated block. This makes
the idempotency requirement precise and testable: viewing [G] Legacy twice must produce the
identical event list, applied exactly once — not recomputed-and-reapplied on the second view.

- TDD: a new scripted-stdin test in `crates/goat-tui/tests/smoke_stdin.rs`, same style as
  d77170b's own added test (`smoke_stdin.rs` additions from that commit) — seed a save at a
  season-end gate with a known bottom-of-table position for the PC's club (or a tracked
  rival club), view [G] Legacy **twice**, assert the promotion/relegation outcome (whichever
  screen surfaces it) is identical both times, not double-applied. A second test:
  bottom-N clubs of a tier (synthetic/seeded) end up in the lower tier's `DIV_CLUBS` bucket
  next season and top-N of the tier below end up promoted — an inequality/membership
  assertion (following the existing "assert invariants, not frozen exact values, for new
  behavior" convention from `TASK-DESIGN-round1-pantheon-saves.md`), against
  `crates/goat-world/src/history.rs`-style backfill-consistency tests
  (`history.rs:218-237`'s pattern: assert every champion is a real club in its division →
  same shape, assert every promoted/relegated club actually moved tiers).
- Playable gate: `cargo run -p goat-tui` → play (or fast-forward) a season where the PC's club
  finishes bottom-3 (or a tracked rival does) → season-end screen reflects relegation → next
  season's fixture list/table shows the club in its new tier, opponents changed accordingly.

**Size: large, risk: high** — not from the promotion/relegation *rule* itself (bottom-N/top-N
is simple), but from A3.1's architecture fork (replay vs. persist) needing to be settled
first, and from the idempotency interaction with an already-once-buggy pipeline. Strongly
sequence this **after** A2 is fully built and benchmarked, not in parallel — A3.1's estimate
depends on A2's actual genesis/replay cost being measured, not guessed.

---

## Out of scope (do not fold into this doc)

- **Item 4, national teams + tactical identity** — separate doc,
  `tasks/TASK-DESIGN-round2-national-team-tactical-identity.md`.
- **Variable club-count-per-tier / variable tiers-per-nation** (the "full pyramids," realistic
  taper the bible §7.2 gestures at) — flagged as a real possibility in A2.1/A2.3 but scoped
  out of this round's default recommendation (uniform 20-clubs/3-tiers) pending Tùng's
  confirmation either way.
- **Promotion/relegation playoffs** (a 4th promotion/relegation spot decided by a mini-
  tournament, common in real football) — A3.2's proposed N=3 is a clean top-N/bottom-N cut,
  no playoff mechanic. `PromoRelegationEvent`'s `TransitionType` enum (A3.2) does **not** grow
  a `PlayoffPromotion` variant this round — its 2-variant shape is future-proofing the
  call-site pattern, not a commitment to build this. Flag as a future enhancement if Tùng wants
  it, not this round.
- **Administrative relegation / club dissolution** (a club forcibly relegated or removed for
  financial reasons, rather than by league position — a bankruptcy mechanic) — no such
  mechanic exists or is proposed anywhere in this doc. `TransitionType` stays at exactly
  `DirectPromotion`/`DirectRelegation` this round, not a placeholder for this.
- **Rebalancing the pantheon/legacy-axis math for a 20-nation world** — the powerhouse/minnow
  nationality dial (bible §4.1) already exists conceptually and this doc's nation-stature
  generation (A2.2) is designed to keep it *functioning*, but retuning exactly how stature
  maps to legacy-axis reachability numbers is `TASK-TUNE` territory once this ships, not a
  design decision to make here.
- **Any new dependency** (e.g. `rayon` for parallelizing genesis, flagged as a *possible*
  mitigation in A2.5) — only add if a real post-implementation benchmark shows it's needed,
  and only with an explicit exception to the existing no-new-deps rule.
- **`goat-bridge`/Flutter DTO wiring for the new nation/tier picker UI** — this doc covers the
  `goat-core`/`goat-world` domain model and `goat-tui` text-UI ripple; a Flutter-side
  multi-nation nation-select/pyramid-browser UI is a follow-on `goat-bridge` task once the
  domain model lands (same reasoning as round-1's Slice 3 "out of scope" note re: Flutter
  multi-slot wiring).

## Parked for a future design round

**Player-driven dynamic club strength — superseded, now designed in full.**

What was parked here 2026-07-22 (a one-paragraph placeholder: replacing `Club.strength` with a
value computed from an actual roster, plus promoting `Match`/`Fixture` to first-class entities)
has since been designed in a full follow-up round, same day:
`tasks/TASK-DESIGN-round3-player-driven-club-strength.md`. That doc covers, against the real
current code (not the placeholder's guesses): per-club `squad_size` replacing the global
`SQUAD_SIZE` constant, the genesis anchor formula kept intact with a rare outlier roll added on
top, live club strength computed from the current roster for match simulation (discovering
along the way that this already exists for the background-league path, `batch_tick.rs`'s
`club_strength`, and just needs promoting to a public API), season-end youth-academy
replenishment with a floating squad-size band, and tactical-identity-biased lazy-promote
potentials coordinated with Doc B's `TacticalIdentity`.

The `Match`/`Fixture`-as-first-class-persisted-entities half of this placeholder was **not**
carried into the round-3 doc — Tùng's round-3 conversation scoped it out explicitly (see that
doc's "Out of scope"). `season.rs`'s existing ephemeral fixture/table simulation stays exactly
as this doc (A3) already designed it. This paragraph is superseded; see the round-3 doc for the
actual design, not this placeholder.

**Multi-competition: domestic cups + continental competitions — now designed in its own doc,
`tasks/TASK-DESIGN-round4-competitions.md` (2026-07-22).**

Tùng also asked about cup competitions (domestic cups, continental cups like a
Champions-League-style tournament), World Cup / continental national-team championships, and
schedule-conflict handling between competitions. This was originally parked here as a short
placeholder pointing at the bible's already-sketched `Competition`/`Fixture`/
`FixtureImportance`/`SuspensionLedger` shape — it has since been designed properly in its own
round (round 4), covering: domestic cup structure (single-elimination, tier-staggered entry,
random round-by-round redraw), 3-tier continental club competitions (stature-ranked
qualification), World Cup + continental national-team championships (real-world cadence,
scheduled in the existing off-season calendar gap), and the `FixtureImportance`/
`SuspensionLedger` wiring that ties all of it together. See
`tasks/TASK-DESIGN-round4-competitions.md` for the full spec, numbered slices, and the list of
numbers still needing Tùng's sign-off before Dev starts.

**AI-run club economy: transfer market between AI clubs, managers, club finances — now designed
in its own doc, `tasks/TASK-DESIGN-round5-club-economy.md` (2026-07-22).**

Bible §7.3 ("Transfer Market & AI Clubs") already commits to AI clubs being "deep agents, not
backdrops": each with its own strategy, finances/budget, squad-building plan, and manager, with
clubs trading *each other* (not just trading with the PC) so teammates arrive/leave and shift
squad chemistry around the player. Bible §7.2 item 2 also wants clubs to carry "rich identity:
history, rivalries, philosophy, stature, finances" beyond today's name+strength-number shape.
This was originally parked here as a short placeholder noting that `crates/goat-core/src/
state.rs`'s "Phase 8" transfer/contract machinery is entirely PC-facing and that no `Manager`
type or club-level finance/budget exists anywhere in the workspace — it has since been designed
properly in its own round (round 5), covering: a single-number club budget fed by an additive
income-contributor abstraction (today: tier/strength-derived baseline, built to accept
sponsorship/matchday/shirt-sales/prize-money contributors later without a spending-side
rewrite), a deterministic seeded ascending-round transfer auction with three competing spend
lanes (fill-weakest-position, gem-hunting off round-3's outlier mechanic, own-youth-academy
investment composing with round-3's intake formula), and a new lightweight `Manager` entity
type with a tactical-identity blend on appointment (reusing round-2 Doc B's `TacticalIdentity`)
and a rolling-form-based firing/rehire cycle. See `tasks/TASK-DESIGN-round5-club-economy.md`
for the full spec, numbered slices, and the list of numbers still needing Tùng's sign-off
before Dev starts.

**Pundits & Media, credibility/influence axis per pundit — now designed in its own doc,
`tasks/TASK-DESIGN-round6-pundit-credibility.md` (2026-07-22).**

What was parked here (a short placeholder, including an incorrect claim that §8.7's pundit
system was "0% implemented in code" — it was already real and committed, `crates/goat-meta/
src/pundits.rs`/`reputation.rs`, at the time that placeholder was written) has since been
designed in full, against the real code: the existing 4-pundit/4-school mapping stays exactly
as-is (no new personalities, no new schools); each pundit gets a discrete `PunditTier`
(Rookie/Established/Legend) computed by a deliberately simple, isolated `tier_for` function
(the real tenure/proven-right growth mechanic is explicit future work behind that one function
boundary); and the tier multiplies how much a pundit's season-end comment moves the player's
Sporting reputation facet (bible §8.2), closing the gap where pundit commentary today is
rendered but has zero effect on `WorldState`. See that doc for the full slice breakdown, TDD
anchors, and the list of numbers (tier count, split percentages, multiplier magnitudes) still
needing Tùng's sign-off before Dev starts.

**BL5 — "Wasted potential" narrative (Pogba/Neymar archetype) — raised by Tùng, 2026-07-22, not
yet designed.**

Tùng's framing: a player whose ceiling (`potential`) is very high but who never actually closes
the gap to it over their career — the bittersweet "what could have been" story, distinct from
simply declining or being loyal/disloyal. Verified: no code or bible text for this exists today
(`grep`'d — only unrelated "wasted opportunity" match-flavor strings in
`crates/goat-match/src/library.rs`). The raw ingredients already exist, though: every player has
tracked `current` vs `potential`, and lifestyle already creates a real mechanism for
under-realizing potential — `lifestyle_ceiling` caps a Flashy player at 96% of potential (never
100%), and `lifestyle_decline` makes a Flashy player's attributes erode faster once decline
starts (`crates/goat-core/src/tuning.rs`, `week.rs`). What's missing is a narrative/legacy layer
that specifically recognizes a large, persistent current-vs-potential gap and gives it story
weight (a Legacy label like existing "Iconic"/"Cult Hero", and/or a new signal for Eye-Test
Romantics, who are already the school most oriented toward "moments over numbers" per round-4's
distinct-signals work). Confirmed by Tùng as a genuinely new idea (not a restatement of anything
already parked). Not designed here — needs its own round to decide: is this a new Legacy label,
a new Eye-Test Romantics input, a pundit narrative thread, or some combination; what "large,
persistent gap" threshold triggers it; does it apply to background/rival players too (they don't
currently have full attribute-level `current` tracked, only a formula-derived headline
`potential_ovr`) or PC-only.

**BL6 — Fan comments (crowd sentiment + social-feed flavor) — raised by Tùng, 2026-07-22, not
yet designed.**

Tùng's framing, clarified via two follow-up questions: (1) aggregate crowd-sentiment flavor text
("the stands are split on you") — no fixed identity, not a named character; (2) social-media-style
comments that appear after matches — random, no persistent identity per commenter (distinct from
the 4 named pundits, BL4). Confirmed no fixed-identity fan characters (option 2 from the
clarifying question was explicitly NOT chosen). Text should be **template + slot, authored with
LLMs offline** — the exact pattern bible §8.7 already specifies for pundits/beats ("never at
runtime") — so this is a content-authoring extension of an existing approach, not a new runtime
mechanic. Verified: no code or bible text for fan comments exists today — only the numeric
`club_fan` reputation facet (`crates/goat-meta/src/reputation.rs`), which is a score, not text.
Open question for a real design pass: does this purely-flavor feed feed back into any state
(e.g. `club_fan` reputation, matching how BL4 wired pundit tier into Sporting reputation), or is
it pure flavor with zero mechanical effect — Tùng did not specify this either way yet.

**BL7 — Hidden per-player injury-proneness/durability trait — raised by Tùng, 2026-07-22, not
yet designed.**

Verified: `injury_prob()` (`crates/goat-core/src/week.rs:326-348`) already multiplies fatigue
(`energy`, a real accumulating value that drains from training/matches and recovers with rest —
NOT reset weekly), training intensity, age, and lifestyle into injury risk — the "plays/trains a
lot without rest → more injuries" mechanic Tùng asked about already exists and works correctly.
What's missing: every player with identical energy/intensity/age/lifestyle has *identical*
injury risk today — no player is innately more fragile (Diego Costa-style) or more durable
(Rodri-style) than another. Confirmed design: a per-player `durability`/`injury_proneness`
value, rolled once at player generation like other attributes, multiplied into the existing
formula the same way `lifestyle_injury_x10` already is — **not a new subsystem**, one more
coefficient in an existing one. Confirmed **hidden from the player** — never displayed as a
number, only observable through outcomes (matches the existing pattern of OVR's
position-weighted formula also not being shown). Confirmed **fixed/innate for now** (not
improvable via club medical staff) — Tùng agreed to keep BL7 small; the medical/sports-science
club-investment angle that could later modify this multiplier is real-world accurate but would
couple this into `TASK-DESIGN-round5-club-economy.md`'s (BL3) budget-spending system, which
isn't built yet. Recorded as an explicit future hook: BL3 (or a later round) can multiply an
additional "club medical quality" coefficient into this same formula without redesigning it.

## Decisions Design needs from Tùng before Dev starts (collected from above)

1. **A2.1**: uniform 20-clubs-per-tier across all nations (low risk — `CLUBS_PER_DIV` moves
   from 16 to 20 but stays a single uniform constant, so `ROUNDS_PER_SEASON`/`table_raw`
   change size but not shape) vs. variable club-count-per-tier (more realistic, bigger rewrite
   of `season.rs`/`fixtures.rs`/save format). **Confirmed by Tùng, 2026-07-22: 20/tier (1,200
   clubs total, 30,000 players — top edge of the bible's 20-30k band), overriding Design's
   earlier 16/tier recommendation. Resolved — no longer open.**
2. **A2.1**: uniform 3-tiers-per-nation vs. variable pyramid depth per nation's stature (bible
   §7.2 leans toward variable — "full pyramids" — but that's a bigger scope). **Recommendation:
   uniform 3, revisit variable depth as a later enhancement.**
3. **A1.2/A2.2**: how much authoring effort to invest in word banks — flat generic vs.
   nation-flavored club naming (register difference, e.g. English- vs Brazilian-style club
   names); hand-authored country-name word bank vs. syllable-combinator for the ~20 nation
   names. **Recommendation: nation-flavored club naming + hand-authored country-name bank**,
   but this is real extra authoring work Tùng should knowingly sign off on, not a free choice.
4. **A3.1 (the big one)**: replay-from-seed with zero new persisted tier-membership state
   (matches existing architecture exactly, but has an unmeasured/growing per-season replay
   cost) vs. a small new persisted `club → current tier` field updated incrementally each
   season-end (simpler per-season cost, but is genuinely new persisted state + a new
   per-season write for divisions that currently have none). **This fork changes the shape of
   A3's implementation entirely — must be settled before Dev starts, not discovered
   mid-implementation.**
5. **A3.2**: promotion/relegation N (clubs moved per tier boundary per season).
   **Recommendation: N=3**, standard and simple, no playoff mechanic.
6. **A2.5/A3.1 perf**: genesis time and (if replay-based) per-season replay time both need a
   real `--release` benchmark once A2 is built, before committing to A3's architecture — not
   estimated further here.

## Definition of done (once Dev implements)

1. `cargo test --workspace` green, including updated `history.rs` tests (A2.4) and new
   `goat-world`/`goat-tui` tests per each slice's TDD anchor.
2. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. No new dependencies without an explicit, separately-confirmed exception (A2.5).
4. `goat-save::save::VERSION` unchanged unless A3.1 resolves toward the persisted-state
   alternative (in which case it bumps, with a backward-compat test per the existing v7→v8/
   v8→v9 precedent).
5. Playable gates for A1, A2, A3 all pass via `cargo run -p goat-tui`.
6. A real genesis-time and (if applicable) replay-time benchmark recorded, not just estimated.
7. No floats in sim state/logic, no unsafe — existing project-wide constraints, unaffected by
   this task but worth restating given the scale of new code this doc implies.

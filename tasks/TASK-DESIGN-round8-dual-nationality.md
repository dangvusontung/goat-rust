# TASK DESIGN ROUND 8 — Dual nationality ("con lai")

Prereq: depends on the real-nation-list decision superseding
`tasks/TASK-DESIGN-round2-world-genesis-scaleup.md` §A2.2 (nations become real countries, not
procedurally generated fictional names — see that doc's 2026-07-23 "SUPERSEDED" note). This
doc's correlation table (Decision 2) needs a concrete real-country list to be written against;
until that list exists, the correlation table is illustrative/pending, not final.

Read first: `crates/goat-world/src/population.rs:54-56` (`Population.nation: Vec<u8>`, the
single-nationality column this doc extends), `crates/goat-world/src/national_tournament.rs:156-173`
(`national_team_strength`, the strict single-nation eligibility filter this doc's mechanic
hooks into) and `:59` (`nation_ranks`/qualifying-group partition, unaffected by this doc),
`crates/goat-tui/src/main.rs:199-234` (career-creation nation pick, the one live UI site this
doc's Decision 5 touches), and the `names_agent` MCP tool (`~/workspace/agents/src/langchain_agents/names_agent.py`)
discussed the same night this doc was written, which currently assumes one name pool per
nation with no dual-heritage concept.

## Origin

Raised by Tùng 2026-07-23, ~02:04, while discussing the real-nation-name pivot: "con lai"
(dual-nationality/mixed-heritage players) has never been designed. Worked out **interactively
with Tùng in chat**, not delegated to a subagent — see
`memory/2026-07-23-goat-rust-design-must-be-interactive.md` for why that distinction matters
in this project. The five decisions below are Tùng's own answers from that conversation, not
Design defaults.

## Verified: current state (why this is a real gap, not a nice-to-have)

- `Population.nation: Vec<u8>` (`population.rs:56`) — exactly one nationality per background
  player, no second column, no "eligible nations" concept anywhere in the crate.
- `national_team_strength` (`national_tournament.rs:156-173`) filters a nation's eligible squad
  via `pop.nation[i] as usize == nation` (line 163) — a strict equality check. A dual-eligible
  player today would just... not exist; there's nothing to hook a second nation into.
- Career creation (`goat-tui/src/main.rs:199-234`) offers a single "Pick a nation" prompt with
  no dual-nationality branch.
- No cap-tie/lock state exists anywhere (there is no concept of "has this player already
  appeared for a senior national team," since there's only ever one nation to appear for).

## Decisions (final, from the 2026-07-23 chat — not re-litigated as options)

**1. FIFA-style cap-tie rule.** A dual-eligible player may represent either eligible nation
until they play in an official **senior** national-team match for one of them (World Cup or
Continental Championship qualifying/finals, or a senior friendly if the engine ever adds
those — currently the only senior-cap-generating fixtures are `national_tournament.rs`'s
qualifying/group/knockout matches). The moment they're selected into a squad that actually
plays a senior fixture, they become **cap-tied** to that nation permanently — the second
eligibility is discarded from that point on. This mirrors real FIFA eligibility rules exactly;
Tùng was explicit that this is not a house-rule simplification, it's the real rule.

**2. Second nation: random, with correlation weighting.** Not uniform-random over all nations,
and not a hand-curated fixed pairing table either — a **weighted random draw**, where a small
number of real-world-plausible nation pairs (colonial ties, major historical migration
corridors — e.g. France↔Senegal/Algeria/Morocco, Portugal↔Brazil/Angola, England↔Nigeria/
Jamaica, Spain↔Argentina/Mexico) get a higher weight than an arbitrary unrelated pair, but any
pair remains possible. **Depends on the pending real-nation list** (see Prereq) — this doc
proposes the *mechanism* (a weight table keyed by nation pairs, default weight 1.0, elevated
weight e.g. 4.0-6.0 for flagged historical pairs), not the final table contents, since the
final ~20-real-country roster isn't chosen yet.

**3. Rate: ~10-15% of background players, seed-rolled at genesis.** Tùng's answer was "random"
to the exact number — read as "don't hand-pick an exact figure, a plausible probabilistic rate
is fine." This doc recommends **12%** as the concrete constant (`DUAL_NATIONALITY_CHANCE_PCT:
u32 = 12`), a single round number in the middle of Tùng's implied 10-15% range, seed-derived
per-player at genesis exactly like every other per-player roll in this codebase (`potential_ovr`,
`intake_week`, etc. — same `GoatRng::new(seed_mix(...))` pattern, no new persisted state: a
player's dual-eligibility is a pure function of their own `seed`, recomputed on demand like
everything else in `Population`).

**4. Names reflect the dual heritage, per-slot independent draw.** For a dual-eligible player,
`FIRST_NAMES` and `LAST_NAMES` (or per-nation pools, once `names_agent`'s nation-aware mode is
wired into the actual const arrays — separate follow-up, not this doc's job) are each
independently seed-rolled from **either** of the player's two eligible nations' pools — so a
dual-national player might get a first name from nation A and a last name from nation B, or
both from the same one; every combination is a valid, equally-random outcome. This is Tùng's
explicit answer ("Có" / yes) to "should dual-nationality show up in the name," and matches this
codebase's existing "generated but consistent" seed-derived-draw architecture rather than
inventing a new deterministic pairing rule.

**5. The player-created character can also choose dual nationality at creation.** Not
background-players-only. `goat-tui/src/main.rs`'s "Pick a nation" step (line 220) needs a
follow-up "pick a second eligible nation? (optional)" branch, using the same weighted-pair
mechanism as Decision 2 (or a free pick, since the human player is choosing deliberately rather
than being randomly rolled — **flagged below**, not decided in chat). This interacts with
bible §4.1's "nationality is the difficulty/story dial": a dual-national player's *own* choice
of which senior cap to eventually accept becomes part of that dial (e.g. picking the harder
nation for the underdog story, or keeping both open longer for flexibility) — this doc doesn't
redesign §4.1's dial itself, just notes the two systems now interact.

## Flagged for Tùng (not decided in the live chat — needs sign-off before Dev, not a Design guess)

- **Correlation table contents** — genuinely blocked on the pending real-nation-list decision
  (Prereq). Once that list exists, the specific elevated-weight pairs need Tùng's eyes (this
  doc's proposed pairs above are illustrative examples, not a vetted final list — picking which
  real-world historical ties to represent is a content/sensitivity call, not a mechanical one).
- **At career creation, is the second-nation pick free (any nation) or also weighted/limited
  like Decision 2's background-player mechanism?** Decision 5 above notes this is open — a
  human deliberately picking is a different UX than a random roll, and "any of ~20 nations" vs.
  "only weighted-plausible pairs" changes the creation-screen's complexity.
- **Effort size:** **medium-large**, bigger than a typical single-slice round. Touches: a new
  `Population` column (or derived-not-stored field, matching `seed`-derivability), the cap-tie
  state (this IS new persisted state — the one exception to "pure function of seed," since
  which nation a player ultimately committed to is genuinely path-dependent on which fixture
  they were picked into, same category as `career_goals`/other batch-tick accumulators),
  `national_team_strength`'s eligibility filter, squad-selection logic to actually surface a
  dual-eligible player to BOTH nations' candidate pools before cap-tie, the career-creation UI
  branch, and the name-draw change. Recommend slicing like round4/round5 were (e.g. Slice 1:
  data model + eligibility/cap-tie mechanism + tests; Slice 2: name-draw integration; Slice 3:
  career-creation UI) rather than one large implementation pass — **flagging the slicing plan
  itself for Tùng's sign-off**, not deciding it unilaterally here.

## TDD anchors (for whichever Dev slice implements this)

- `dual_eligibility_rate_is_within_target_band`: over a large generated population, the
  fraction with a second eligible nation lands near 12% (statistical, not exact-count).
- `dual_eligible_player_counts_toward_both_nations_squad_pools_before_cap_tie`: a dual-eligible
  player appears in `national_team_strength`'s eligible-population filter for EITHER nation
  until capped.
- `senior_cap_locks_nationality_permanently`: once a dual-eligible player is selected into a
  senior fixture squad for nation A, they no longer appear in nation B's eligible pool for any
  later fixture, same seed/save.
- `name_draw_is_independent_per_slot_across_eligible_nations`: a dual-eligible player's first
  and last name can each independently come from either eligible nation's pool (not force-paired
  from the same one).
- `dual_nationality_is_deterministic_per_seed`: same `world_seed` → same set of dual-eligible
  players, same second-nation assignment, same eventual cap-tie outcome given the same career
  played out.

## Playable gate

`cargo run -p goat-tui` → new game → nation-select screen offers an optional second-nation pick;
a full-career playtest shows at least one dual-eligible background player getting capped for one
of their two nations during a World Cup/Continental Championship cycle, and confirms they no
longer show up as a call-up candidate for the other nation afterward.

## Out of scope (this round)

- The final real-nation list itself (separate, already-flagged pending decision).
- Redesigning bible §4.1's nationality-difficulty-dial mechanism itself — this doc only notes
  the interaction exists for a dual-national player-character.
- Any UI/flavor text (bio lines mentioning heritage, pundit commentary about a player's
  nationality switch, etc.) — that's content-gen/template work, not this doc's mechanical scope.
- Retroactively changing already-generated saves' populations (existing saves keep whatever the
  pre-this-doc genesis produced; only new genesis runs get dual-eligible players, same
  precedent as every other genesis-time addition this codebase has made before).

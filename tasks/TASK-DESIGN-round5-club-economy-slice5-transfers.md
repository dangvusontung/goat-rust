# TASK DESIGN ROUND 5, SLICE 5 — Deterministic bidding-round auction & transfer execution

**Split-file note (read this first):** this file is 1 of 6 that together replace
`tasks/TASK-DESIGN-round5-club-economy.md` (now a short pointer doc). Sibling files:
`-slice1-2-foundation.md`, `-slice3-4-scouting.md`, `-slice6-academy.md`,
`-slice7-8-managers.md`, `-slice9-integration.md`. This file is fully self-contained —
implement it without reading the others or the original doc. **This is the doc's own most
novel/highest-risk piece** (a real design choice: ascending-round auction, not sealed-bid or
instant-highest-bidder) — kept isolated in its own file rather than folded into a bigger slice.

Prereq: **both `TASK-DESIGN-round5-club-economy-slice1-2-foundation.md` and
`TASK-DESIGN-round5-club-economy-slice3-4-scouting.md` must be landed and committed first (hard
prereq).** This file calls: `Club.budget` and `market_valuation` (foundation slice);
`weakest_position_target`, `gem_hunt_target`, `candidates_by_position`, `gem_targets_by_position`
(scouting slice). It does not need to re-read either sibling file — the functions exist on disk
by the time this slice starts.

Read first: `crates/goat-world/src/world.rs:140-144` (`seed_mix`, the seed-mixing idiom this
file's `auction_seed` follows) and `population.rs:109-113`/`461-467` (`player_seed`/
`intake_player_seed`, the same idiom's other instances — no shared "seed util" module exists in
this codebase, each module keeps its own private copy; this file's `auction_seed` adds its own
rather than introducing a new cross-module dependency).

## Ground rules for this file

- **Season-granularity architecture stays intact — two transfer-window *passes* per
  season-tick, not real calendar-day events for background clubs.** `calendar_loop.rs`'s
  `TransferWinter`/`TransferSummer` windows already exist for the **orbit** (PC-facing,
  day-tick) calendar, but `batch_tick_season` (the background-league path this market lives in)
  simulates a whole season's matches in one call, with no sub-season day resolution. Splitting
  that call into two match-halves so a literal mid-season window could interleave is a bigger
  structural change than this round asked for. Both windows run as two back-to-back passes at
  the same season-tick boundary — see `TASK-DESIGN-round5-club-economy-slice9-integration.md`
  for the exact ordering and why.
- **"Generated but consistent."** The auction's tie-break RNG (5.4) is its own forked stream,
  seeded from `(world_seed, season, window, player_seed)` — never sharing a stream with
  match/transfer-search/injury/calendar RNG.
- **Every invented number is flagged** — see "Decisions" below.

## Verified: grounding for this file

- **The seed-mixing convention this file's deterministic auction draws follow.**
  `world.rs:140-144` (`seed_mix(world_seed, salt, idx)`, XOR of two golden-ratio-constant
  multiplies) and `population.rs:109-113`/`461-467` (`player_seed`/`intake_player_seed`, the
  same idea). This file's `auction_seed` (5.4) adds its own local copy of the same idiom,
  matching existing precedent.
- `Club.budget: i64` (foundation slice) can be negative — this file's `bid_ceiling` (5.2)
  clamps a lane's willingness-to-pay to `club_budget.max(0)`, so a distressed club naturally
  drops out of bidding without any special-case branch.

## Slice 5 — Deterministic bidding-round auction & transfer execution

### 5.1 — Lane caps: three shares of one real budget, spent in priority order

```rust
const LANE_WEAKEST_POSITION_PCT: i64 = 50;
const LANE_GEM_HUNT_PCT: i64 = 30;
const LANE_YOUTH_INVESTMENT_PCT: i64 = 20; // spent by the academy slice, not here

fn lane_cap(club_budget: i64, pct: i64) -> i64 {
    (club_budget.max(0) * pct) / 100
}
```

Lane caps are **not** separate reserved sub-ledgers — they're a cap on how much of the club's
*current* `budget` each lane's `bid_ceiling` (5.2) is allowed to draw on, computed fresh at the
moment each lane runs. Passes execute strictly in order — **weakest-position, then gem-hunt,
then youth-investment (a sibling file)** — so a club that spends in the first pass automatically
has a smaller `budget` (and therefore smaller caps) for the next pass within the same window.
This gives "fill the real gap" priority over "chase upside" without any double-booking risk, and
without needing a second ledger.

### 5.2 — Bid ceiling: budget + need + quality, one club's max willingness to pay

```rust
/// How much one club is willing to bid for one target this lane. Never exceeds the club's
/// actual (post-earlier-lane) budget, regardless of how much the need multiplier wants to add.
fn bid_ceiling(lane_budget: i64, need_mult_pct: i64, club_budget: i64) -> i64 {
    (lane_budget * need_mult_pct / 100).min(club_budget.max(0))
}

const NEED_MULT_WEAKEST_POSITION_PCT: i64 = 130; // a confirmed gap is worth overpaying 30% for
const NEED_MULT_GEM_HUNT_PCT: i64 = 110;         // opportunistic, smaller premium
```

### 5.3 — One pass, one snapshot: order-independence (a real determinism trap)

Every club in a pass computes its target (scouting slice's weakest-position or gem-hunt search)
**against the pass's opening state** — no club's search result depends on another club's search
happening "before" or "after" it in whatever order clubs happen to be iterated. Transfers are
then resolved and applied as one batch at the pass's end. This matters for the same reason bible
§9 treats determinism as sacred: without this rule, iterating clubs `0..NUM_CLUBS` vs. any other
order would silently produce different results from the *same seed*, breaking the "generated but
consistent" guarantee every other subsystem in this codebase relies on.

```rust
pub enum TransferLane {
    WeakestPosition,
    GemHunt,
}

impl TransferLane {
    fn pct(&self) -> i64 {
        match self {
            TransferLane::WeakestPosition => LANE_WEAKEST_POSITION_PCT,
            TransferLane::GemHunt => LANE_GEM_HUNT_PCT,
        }
    }
    fn need_mult_pct(&self) -> i64 {
        match self {
            TransferLane::WeakestPosition => NEED_MULT_WEAKEST_POSITION_PCT,
            TransferLane::GemHunt => NEED_MULT_GEM_HUNT_PCT,
        }
    }
}

/// One full pass: every club's target (computed off the shared opening snapshot), grouped by
/// contested player, each contested player's auction resolved independently, all resulting
/// transfers applied together at the end.
fn run_transfer_pass(
    pop: &mut Population,
    world: &WorldGenesis,
    world_seed: u64,
    season: u32,
    window: u8, // 0 = winter, 1 = summer — folded into the auction seed (5.4)
    lane: TransferLane,
) {
    let elapsed_weeks = season * 52;
    let candidates = candidates_by_position(pop, elapsed_weeks);
    let gem_lists = gem_targets_by_position(pop, &candidates, elapsed_weeks);
    let squads = squads_by_club(pop, world.clubs.len());

    let mut targets: HashMap<usize /* player idx */, Vec<ClubId>> = HashMap::new();
    for club in &world.clubs {
        let cap = lane_cap(club.budget, lane.pct());
        let ceiling = bid_ceiling(cap, lane.need_mult_pct(), club.budget);
        let target = match lane {
            TransferLane::WeakestPosition => weakest_position_target(
                club.id, pop, &squads[club.id], &candidates, ceiling, elapsed_weeks,
            ),
            TransferLane::GemHunt => gem_hunt_target(
                club.id, pop, &gem_lists, ceiling, elapsed_weeks,
            ),
        };
        if let Some(idx) = target {
            targets.entry(idx).or_default().push(club.id);
        }
    }

    // Resolve each contested player independently, apply all transfers as one batch.
    let mut transfers = Vec::new();
    for (player_idx, bidder_clubs) in targets {
        let valuation = market_valuation(
            pop.current_ovr(player_idx, elapsed_weeks),
            pop.potential_ovr[player_idx],
            pop.age_years_at(player_idx, elapsed_weeks),
        );
        let bidders: Vec<(ClubId, i64)> = bidder_clubs
            .iter()
            .map(|&c| (c, bid_ceiling(lane_cap(world.clubs[c].budget, lane.pct()),
                                        lane.need_mult_pct(), world.clubs[c].budget)))
            .filter(|&(_, ceiling)| ceiling >= valuation)
            .collect();
        let mut rng = GoatRng::new(auction_seed(world_seed, season, window, pop.seed[player_idx]));
        if let Some((winner, fee)) = resolve_auction(&bidders, valuation, &mut rng) {
            transfers.push((player_idx, winner, fee));
        }
    }
    for (player_idx, winner, fee) in transfers {
        let seller = pop.club[player_idx] as usize;
        pop.club[player_idx] = winner as u16;
        // conservation: money moves within the closed club economy, never created/destroyed
        // by a transfer fee (only `total_income`, foundation slice 1.2, creates new money) —
        // the fee literally sums to zero across (buyer, seller), a natural invariant test (5.5).
    }
}
```

*(The mutable `world.clubs`/`Club.budget` debit/credit lines are elided above for readability —
Dev applies `world.clubs[winner].budget -= fee; world.clubs[seller].budget += fee;` inside the
final loop, after computing both indices, same batch-at-end timing.)*

`squads_by_club(pop, num_clubs) -> Vec<Vec<usize>>` is a small helper (index players by
`pop.club[i]`) — write it locally here if no shared helper exists yet.

### 5.4 — Auction resolution: deterministic ascending rounds, not instant highest-bidder

```rust
fn auction_seed(world_seed: u64, season: u32, window: u8, player_seed: u64) -> u64 {
    world_seed
        ^ (season as u64).rotate_left(17).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (window as u64).rotate_left(29).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ player_seed.rotate_left(41).wrapping_mul(0x1656_667B_19E3_779F)
}

const AUCTION_RAISE_PCT: i64 = 8; // price climbs 8% per contested round

/// `interested`: (club, bid_ceiling) pairs already filtered to `ceiling >= valuation`.
/// Returns `(winning_club, final_fee)`, or `None` if nobody clears the valuation.
fn resolve_auction(
    interested: &[(ClubId, i64)],
    valuation: i64,
    rng: &mut GoatRng,
) -> Option<(ClubId, i64)> {
    match interested.len() {
        0 => None,
        1 => Some((interested[0].0, valuation)), // uncontested — pays the ask, no reason to
                                                   // bid against yourself
        _ => {
            let mut price = valuation;
            let mut remaining: Vec<(ClubId, i64)> = interested.to_vec();
            loop {
                remaining.retain(|&(_, ceiling)| ceiling >= price);
                if remaining.len() <= 1 {
                    break;
                }
                price += (price * AUCTION_RAISE_PCT) / 100 + 1; // +1 guards a zero-valuation stall
            }
            if remaining.is_empty() {
                // Everyone dropped in the same round (ceilings clustered exactly below the
                // final raise) — deterministic seeded tie-break among the original bidders,
                // at the price band they all cleared one round earlier.
                let idx = rng.next_range_u32(0, interested.len() as u32 - 1) as usize;
                Some((interested[idx].0, price))
            } else {
                Some((remaining[0].0, price))
            }
        }
    }
}
```

This is exactly the "auction/bidding mechanism, not simple need-filling" and "price driven up
by competition" behavior Tùng specified: an uncontested target costs exactly its valuation; a
target two or more clubs want costs strictly more, in visible, replayable increments, with a
seeded (not order-dependent) resolution when ceilings tie.

### 5.5 — Not designed: seller-side reluctance

Every player is transferable at the right price this round — a club never refuses an offer
above `market_valuation` regardless of how much it needs that specific player. A real
"reluctance premium" for a club's own key players is a plausible future refinement, not built
here (see "Out of scope").

### TDD anchor (Slice 5)

- `uncontested_target_pays_exactly_valuation`: one interested bidder → `fee == valuation`
  exactly.
- `contested_target_pays_strictly_more_than_valuation`: two-plus interested bidders with
  ceilings above valuation → `fee > valuation`, and `fee` increases monotonically with the
  number of competing bidders at a fixed valuation (more competition → higher price).
- `auction_result_is_order_independent`: shuffling the `interested` slice's input order before
  calling `resolve_auction` (holding the seed fixed) produces the identical `(winner, fee)` —
  the direct regression for 5.3's determinism claim.
- `auction_is_deterministic_per_seed`: same `(world_seed, season, window, player_seed)` twice →
  identical result; different `player_seed` → a different (not necessarily different-valued,
  but independently-derived) tie-break stream.
- `budget_conservation_across_a_transfer`: after `run_transfer_pass`, `buyer.budget +
  seller.budget` post-transfer equals their pre-transfer sum minus the fee's zero-sum transfer
  (i.e. `buyer_before - fee + (seller_before + fee) == buyer_before + seller_before`) — the
  "money moves, isn't created" invariant from 5.3.
- `losing_bidders_budget_unaffected`: a club that participates in a contested auction but
  doesn't win has an unchanged `budget` afterward (money only commits on winning).

## Out of scope (this file)

- **Seller-side reluctance / squad-retention logic** — every player transfers at the right
  price this round (5.5). A club refusing to sell its own most-important player even for a huge
  offer is a real, plausible future refinement, not designed here.
- **PC-facing transfer-market participation** — the PC's own club is not yet a bidder in this
  market. A future round could let the PC's own club participate as one more bidder (the PC's
  club would simply be one more club iterated in `run_transfer_pass`) — not wired here.
- Youth-academy investment, managers, season-tick wiring — other sibling files.

## Decisions Design made as judgment calls — flag for Tùng's explicit sign-off

6. **Slice 5.1**: the `50/30/20` lane split and pass ordering (weakest-position first) —
   Design's instantiation of "balanced against a single finite budget," matching Tùng's stated
   priority ("prioritize buying a replacement" for the weakest position) but the exact
   percentages are Design's pick.
7. **Slice 5.2**: `NEED_MULT_WEAKEST_POSITION_PCT = 130`, `NEED_MULT_GEM_HUNT_PCT = 110` —
   Design's own overpay-willingness numbers.
8. **Slice 5.4**: the entire ascending-round auction mechanism — `AUCTION_RAISE_PCT = 8`, the
   round structure, and the RNG-seeded tie-break rule — is Design's concrete answer to Tùng's
   explicit "you decide... consider a bidding-round structure" instruction. The *shape*
   (ascending rounds, not sealed-bid, not instant-highest-bidder) is a real design choice worth
   Tùng's sign-off before Dev locks it into golden tests, not just the numbers.

(Numbering preserved from the original doc's full "Decisions" list.) These are first-pass
numbers for a later `TASK-TUNE` pass once playtested — not blocking items Tùng needs to approve
before Dev starts.

## Definition of done (Slice 5)

1. `cargo test --workspace` green, including every TDD-anchor test listed above.
2. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. No new dependencies. No floats in sim state/logic, no unsafe.
4. No new persisted fields in this file — no save-version bump needed here (transfers only
   mutate existing `Club.budget`/`Population.club`).
5. A `--release` benchmark of `run_transfer_pass` across all 1,200 clubs is a reasonable smoke
   check but not required to pass this slice alone — the full-scale benchmark is the
   integration slice's Definition of Done item.
6. No new failures beyond the **10 pre-existing `goat-tui` `smoke_stdin` failures** (verified
   2026-07-22, out of scope, unrelated to this work, caused by a `generate_club_name()` bug in
   `crates/goat-world/src/world.rs`): `confirm_screen_blank_enter_reprompts_instead_of_
   discarding_character`, `double_w_in_same_round_shows_message_not_silent_noop`,
   `game_sheet_and_player_sheet_boxes_close_for_short_and_long_club_names`, `key_moments_lines_
   close_with_ellipsis_not_ragged_cutoff`, `legacy_screen_notes_mid_season_batching`,
   `main_loop_unrecognized_command_messages_and_continues`, `player_sheet_explains_ovr_is_
   position_weighted`, `save_overwrite_requires_explicit_confirmation`, `save_to_empty_slot_
   succeeds_without_confirmation`, `status_header_shows_energy_percent_and_labeled_discipline_
   count`.
7. **Commit this slice before starting `-slice6-academy.md`.**

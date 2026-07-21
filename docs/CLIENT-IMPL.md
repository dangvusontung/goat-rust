# GOAT — Client Implementation Reference

**Audience:** Renderer authors (Flutter mobile app, future 2D/3D). Describes the full
`goat-bridge` public API, the expected call sequence, and all data transfer objects.

**What this is not:** game design rationale (see `MAIN.md`), Rust internals
(read the crate source), or the TUI implementation (see `crates/goat-tui/src/main.rs`).

---

## 1. Architecture Contract

```
┌──────────────────────────────────────────────────────────┐
│  Renderer (Flutter Dart / goat-tui / future 2D–3D)       │
│                                                          │
│  • Reads flat DTOs returned from bridge functions        │
│  • Sends intents by calling named bridge functions       │
│  • Owns UI state only — zero simulation logic            │
└────────────────────────┬─────────────────────────────────┘
                         │  flat scalars + DTO structs
┌────────────────────────▼─────────────────────────────────┐
│  goat-bridge  (crates/goat-bridge/src/api.rs)            │
│                                                          │
│  • Singleton WorldState behind a Mutex                   │
│  • Translates Dart-friendly scalars ↔ core types         │
│  • Builds GoatGameState snapshot after every mutation    │
│  • No simulation logic — calls reduce() + world helpers  │
└────────────────────────┬─────────────────────────────────┘
                         │  WorldState, Intent, goat_* crates
┌────────────────────────▼─────────────────────────────────┐
│  Headless core (goat-core, goat-match, goat-world, …)    │
│                                                          │
│  • Pure: no I/O, no wall-clock, deterministic            │
│  • All randomness through injected GoatRng               │
└──────────────────────────────────────────────────────────┘
```

**Rule:** the renderer never computes a game result. If you find yourself writing
simulation logic in Dart, that code belongs in core. The litmus test: could you delete
the renderer without losing any game rule? The answer must always be yes.

---

## 2. Bootstrap Sequence

Call these once when the app starts, before anything else.

```
1. load_beat_library(json)   // pass bundled beats.json asset string; returns bool (ok)
2. list_clubs()              // populate the new-game club picker
```

If `load_beat_library` returns false, the bundled fallback inside the binary is used
automatically — you do not need to handle this case explicitly; the bridge self-heals.

---

## 3. Game Lifecycle State Machine

```
                       ┌─────────────────────────────┐
                       │  App Start                  │
                       │  load_beat_library()        │
                       │  list_clubs()               │
                       └──────┬──────────────────────┘
                              │
             ┌────────────────▼────────────────┐
             │  Main Menu                       │
             │  has_active_game() → false       │
             └────┬─────────────────────────────┘
                  │ new_game() OR load_game()
             ┌────▼─────────────────────────────────────────┐
             │  WEEKLY LOOP  (season_round < rounds/season) │
             │                                              │
             │  advance_week()    — train (once/cal-week)   │
             │  advance_weeks(n)  — fast-forward            │
             │  set_routine()     — change training focus   │
             │  play_round()      — auto-play a match       │
             │  start_interactive_match()                   │
             │    └─ make_beat_choice() × N beats           │
             │  get_table()                                 │
             │  get_attributes() / get_families()           │
             │  get_roles()                                 │
             │  get_legacy()                                │
             │  get_peers()                                 │
             └──────────────┬───────────────────────────────┘
                            │  season_round >= ROUNDS_PER_SEASON
             ┌──────────────▼───────────────────────────────┐
             │  SEASON END                                  │
             │                                              │
             │  get_season_awards()   — render awards night │
             │  get_transfer_offers() — render transfer UI  │
             │  accept_transfer() OR agitate_for_transfer() │
             │  apply_season_end()    — settle legacy/rep   │
             └──────────────┬───────────────────────────────┘
                            │
             ┌──────────────▼───────────────────────────────┐
             │  is_retired?                                 │
             │   true  → retirement screen (get_legacy())   │
             │   false → start_next_season() → WEEKLY LOOP  │
             └──────────────────────────────────────────────┘
```

**`GoatGameState.season_round >= GoatGameState.rounds_per_season`** is the gate that
signals the season is over and the season-end flow should begin.

**`GoatGameState.is_retired`** signals the game is over; render the retirement screen
then return to the main menu.

---

## 4. All Public Bridge Functions

### 4.1 Bootstrap & Introspection

#### `has_active_game() → bool`
Returns true if a `WorldState` is loaded. Use to decide whether to show "Continue" on
the main menu before calling `get_state()`.

#### `load_beat_library(json: String) → bool`
Loads the beat library from a JSON string. Call once at startup with the bundled asset.
Returns false only if the JSON is malformed; the binary fallback fires automatically.

#### `get_state() → Option<GoatGameState>`
Read-only snapshot of the current game state. Returns `None` if no game is loaded.
Use for refreshing UI without triggering any mutation.

---

### 4.2 New Game / Load

#### `list_clubs() → Vec<ClubDto>`
Returns all clubs across all divisions. Use to populate the club picker during character
creation. See `ClubDto`.

#### `new_game(player_name, position, club_id, seed, lifestyle) → GoatGameState`

| Param | Type | Values |
|---|---|---|
| `player_name` | `String` | Any non-empty string |
| `position` | `u8` | `0..7` — one of the 8 specific `PrimaryPosition`s (widened from the old `0..2` broad-family range): `0`=ST `1`=W `2`=WM `3`=CAM `4`=CM `5`=DM `6`=FB `7`=CB. Out-of-range values default to ST. |
| `club_id` | `u32` | `ClubDto.club_id` from `list_clubs()` |
| `seed` | `u64` | Any; use `0` for a random feel (pass system time nanos) |
| `lifestyle` | `u8` | **Ignored.** Kept only for FFI binary compatibility with the existing `frb_generated.rs` signature. Lifestyle is now derived, not chosen — see the note below. |

Creates a new `WorldState`, generates the player from the seed, initialises the world
and peer cohort, and starts Season 1. Returns the initial `GoatGameState`.

**Note:** `nationality` is derived from the club's division; the client does not pass it
directly.

**Note on lifestyle:** lifestyle (Professional/Balanced/Flashy) is no longer picked at
creation. It is an emergent readout built up over the career from training intensity,
dev-investment level, and sponsor-tier choices (bible §8.5/§8.6). There is no
`set_lifestyle` call — read the current tier from the existing `lifestyle` field on
`GoatGameState` (unchanged: `0`=Professional `1`=Balanced `2`=Flashy) and render it as a
read-only status, not a picker.

#### `save_game(path: String) → bool`
Serializes the current game to disk. Returns true on success. Recommended call point:
before exiting, at season end, and whenever the user explicitly saves.

#### `load_game(path: String) → Option<GoatGameState>`
Deserializes from disk and sets the singleton. Returns `None` on failure. On success,
the returned snapshot is equivalent to `get_state()`.

---

### 4.3 Weekly Loop

The week is the core loop unit. A season has `rounds_per_season` match rounds, but not
every week has a match. Use `GoatGameState.is_break_week` and `week_fixtures` to decide
what the current calendar week holds.

#### `advance_week() → GoatGameState`
Runs one training week (energy cost, attribute growth, possible dev events). Marks
`week_training_done = true`. Call once per calendar week — subsequent calls in the same
week still advance time but without additional training benefit.

Check `GoatGameState.last_events` after this call for any interrupts (injury,
illness, breakthrough, role-familiarity upgrade).

#### `advance_weeks(n: u32) → GoatGameState`
Fast-forwards `n` weeks in one call. Useful for "skip to next match" or injury recovery.
Stops at the first notable event (same semantics as repeated `advance_week`).

**Caution:** Does not stop at match rounds — call this only when no match is pending in
the skipped range, or pair it with a check of `week_fixtures`.

#### `set_routine(attr_ids: Vec<u8>, intensity: u8) → GoatGameState`

| Param | Type | Values |
|---|---|---|
| `attr_ids` | `Vec<u8>` | Up to 4 `AttrId` discriminants (0–29); see `get_attributes()` |
| `intensity` | `u8` | `0`=Low `1`=Medium `2`=High |

Sets the player's weekly training routine. The routine persists until changed. Applying
it with an empty `attr_ids` clears the focus.

---

### 4.4 Match

#### `play_round(interactive: bool) → (GoatGameState, MatchResultDto)`
Auto-plays (or skips) the current season round. `interactive` is currently ignored
(always auto-plays); for interactive beats use `start_interactive_match()` instead.

Returns the updated state snapshot plus a `MatchResultDto`. If the player has no fixture
this round (bye/no opponent), the `MatchResultDto` is empty.

#### `start_interactive_match() → Option<ActiveBeatDto>`
Starts an interactive match for the current round. Returns the first beat as
`ActiveBeatDto`, or `None` if there is no fixture this round or the season is over.

Once called, `ACTIVE_MATCH` is locked until the match completes. Do not call
`play_round` while a match is active.

#### `make_beat_choice(choice_idx: u8) → Option<BeatOutcomeDto>`
Resolves the current beat with `choice_idx` (0-based, matching `ActiveBeatDto.choices`
index). Returns `None` if no match is active.

The returned `BeatOutcomeDto` tells you:
- What happened (`success`, `outcome_text`, `output_delta`)
- Any card events (`yellow_card`, `red_card`)
- Whether the match is finished (`is_complete`)
- If `is_complete = true`: `final_result` and `game_state` are populated; the world
  state is already updated, ACTIVE_MATCH is cleared, and you can resume the weekly loop.
- If `is_complete = false`: `next_beat` is the next `ActiveBeatDto`; loop back.

**Interactive match loop:**
```
start_interactive_match() → first_beat
loop:
  render beat (setup, choices)
  wait for player choice
  make_beat_choice(idx) → outcome
  render outcome feedback
  if outcome.is_complete:
    render final_result (score, moments, cards)
    update UI from outcome.game_state
    break
  else:
    render next beat from outcome.next_beat
```

---

### 4.5 Data Queries (read-only, no mutations)

#### `get_attributes() → Vec<AttrDto>`
All 30 sub-attributes with current, potential, and family label.
Attribute index in the returned vec is stable and matches `AttrId` discriminant order.

#### `get_families() → FamilyDto`
The six derived family averages (pace / shooting / passing / dribbling / defending /
physical). Use for the compact attribute bar display.

#### `get_roles() → Vec<RoleDto>`
All 14 outfield roles sorted by rating (highest first). Each has name, rating, and
familiarity tier string.

#### `get_table() → Vec<TableRowDto>`
Current season league table, sorted by points. `is_player_club` marks the player's
club. Returns empty vec before Season 1 starts.

#### `get_legacy() → LegacyDto`
Legacy axes, school rankings, and reputation. Safe to call at any point mid-career.
See `LegacyDto`.

#### `get_peers() → Vec<PeerDto>`
The peer cohort (8 generated players). `is_rival` is true for the crystallised rival
(if one has emerged).

---

### 4.6 Season End

Call this sequence after `season_round >= rounds_per_season`:

```
1. get_season_awards()          → render awards night
2. get_transfer_offers()        → render transfer window
3. (optional) accept_transfer() or agitate_for_transfer()
4. apply_season_end()           → settles legacy/rep, batch-ticks peers,
                                   checks rival crystallisation, collects wage
5. if !is_retired:
     start_next_season()        → begin the next season, return to weekly loop
```

#### `get_season_awards() → Vec<AwardDto>`
Computes Player of the Year and Golden Boot for the season just ended. Does not mutate
state. Returns two `AwardDto`s in that order.

#### `get_transfer_offers() → Vec<TransferOfferDto>`
Generates transfer offers for the current window. Returns empty if form < 55 or
age ≥ 34. Deterministic for a given season (same offers every call).

#### `accept_transfer(club_id, wage, length) → GoatGameState`
Executes a transfer to the given club. Use `TransferOfferDto.club_id/wage/length` from
`get_transfer_offers()`.

#### `agitate_for_transfer() → GoatGameState`
Advances the power-ladder and burns character reputation. Repeat calls escalate the
ladder further.

#### `apply_season_end() → GoatGameState`
**Must be called exactly once per season end**, after rendering awards/transfers.
Settles legacy axes, updates reputation, collects wage, batch-ticks peers, and checks
rival crystallisation. Returns the updated snapshot including `has_rival`/`rival_name`
if a rival crystallised.

#### `start_next_season() → GoatGameState`
Increments `season_number`, resets per-season counters, regenerates fixtures. Returns
the initial state for Season N+1.

---

### 4.7 Career End

#### `retire() → GoatGameState`
Marks the player retired (`is_retired = true`). Call when the player chooses to retire.
After this, render the retirement/legacy screen and return to main menu.

---

## 5. Data Transfer Objects

### 5.1 GoatGameState (master snapshot)

Returned by every mutating call. All values are flat scalars / vecs — no nested
structs except `WeekFixtureDto` inside `week_fixtures`.

| Field | Type | Notes |
|---|---|---|
| `player_name` | `String` | |
| `age_years` | `u32` | |
| `age_weeks_in_year` | `u32` | 0–51 |
| `energy` | `i32` | 0–100 |
| `ovr` | `i32` | Best role rating 0–100 |
| `injury_weeks` | `u32` | 0 = healthy |
| `position` | `u8` | 0..7 — `PrimaryPosition`: 0=ST 1=W 2=WM 3=CAM 4=CM 5=DM 6=FB 7=CB |
| `club_name` | `String` | |
| `div_name` | `String` | |
| `nationality` | `String` | |
| `season_number` | `u32` | 0 before first season |
| `season_round` | `u32` | 0-indexed; ≥ `rounds_per_season` = season over |
| `rounds_per_season` | `u32` | Always 22 (current world size) |
| `form` | `i32` | 0–100 |
| `season_goals` | `u32` | |
| `season_matches` | `u32` | |
| `is_suspended` | `bool` | |
| `contract_seasons_left` | `u32` | 0 = out of contract |
| `wage_annual` | `i64` | In-game currency units |
| `savings` | `i64` | |
| `power_ladder` | `u8` | 0=none … 4=full strike |
| `yellow_cards_season` | `u32` | |
| `discipline_rep` | `i32` | 0=clean … 100=enforcer |
| `career_goals` | `u32` | |
| `career_matches` | `u32` | |
| `career_seasons` | `u32` | |
| `league_titles` | `u32` | |
| `player_of_year_wins` | `u32` | |
| `sporting_rep` | `i32` | 0–100 |
| `club_fan_rep` | `i32` | 0–100 |
| `character_rep` | `i32` | 0–100 (= 100 − discipline_rep) |
| `lifestyle` | `u8` | 0=Pro 1=Balanced 2=Flashy |
| `is_retired` | `bool` | |
| `has_rival` | `bool` | |
| `rival_name` | `String` | Empty if no rival |
| `routine_intensity` | `u8` | 0/1/2 |
| `routine_focus_attr_names` | `Vec<String>` | Display names of focused attrs |
| `last_events` | `Vec<String>` | Human-readable event strings from last advance |
| `calendar_week` | `u32` | 0-indexed week within season |
| `calendar_weeks` | `u32` | Total weeks in a season (38) |
| `calendar_week_label` | `String` | e.g. "Game Week 5 · Sep 2025" |
| `is_break_week` | `bool` | True = no matches this week |
| `week_fixtures` | `Vec<WeekFixtureDto>` | Matches in current calendar week |
| `week_fixtures_played` | `u32` | Rounds played so far this calendar week |
| `week_training_done` | `bool` | True if advance_week was called this calendar week |

---

### 5.2 WeekFixtureDto

One scheduled match within the current calendar week.

| Field | Type | Notes |
|---|---|---|
| `round` | `u32` | 0-indexed round index |
| `opponent` | `String` | |
| `opp_strength` | `u8` | 0–100 |
| `is_home` | `bool` | |
| `played` | `bool` | True if `round < season_round` |
| `date` | `String` | e.g. "Sat 16 Aug 2025" |

---

### 5.3 AttrDto

| Field | Type | Notes |
|---|---|---|
| `name` | `String` | e.g. "Finishing" |
| `current` | `i32` | 1–99 |
| `potential` | `i32` | 1–99; ceiling that training cannot exceed |
| `family` | `String` | One of: pace/shooting/passing/dribbling/defending/physical |

Attr IDs 0–29 match the `AttrId::ALL` array order in `goat-core`. Use the index from
`get_attributes()` as the `attr_ids` element when calling `set_routine()`.

---

### 5.4 FamilyDto

Six `i32` fields: `pace`, `shooting`, `passing`, `dribbling`, `defending`, `physical`.
Each is 1–99.

---

### 5.5 RoleDto

| Field | Type | Notes |
|---|---|---|
| `name` | `String` | e.g. "Complete Forward" |
| `rating` | `i32` | 0–99 |
| `familiarity` | `String` | "Natural" / "Competent" / "Awkward" / "Unfamiliar" |

---

### 5.6 TableRowDto

| Field | Type | Notes |
|---|---|---|
| `position` | `u32` | 1-based rank |
| `club_name` | `String` | |
| `played` | `u32` | |
| `won` / `drawn` / `lost` | `u32` | |
| `goals_for` / `goals_against` | `u32` | |
| `points` | `u32` | |
| `is_player_club` | `bool` | Use to highlight the player's row |

---

### 5.7 MatchResultDto

| Field | Type | Notes |
|---|---|---|
| `player_output` | `i32` | 0–100 player rating |
| `goals_for` | `u32` | Team goals scored |
| `goals_against` | `u32` | Team goals conceded |
| `rating_label` | `String` | "★★★☆☆" style |
| `moments` | `Vec<String>` | Up to 5 key moment strings (icons + text) |
| `yellow_cards` | `u32` | |
| `red_card` | `bool` | |

---

### 5.8 ActiveBeatDto (interactive match)

| Field | Type | Notes |
|---|---|---|
| `beat_id` | `u32` | Placeholder (0); situation identified by `setup` text |
| `setup` | `String` | Situation description to render |
| `choices` | `Vec<BeatChoiceDto>` | 2–4 choices |
| `minute` | `u32` | Match clock minute (0–90) |
| `player_output` | `i32` | Running output tally |
| `goals_for` / `goals_against` | `u32` | Live score |
| `stamina` | `i32` | 0–100 |
| `beat_number` | `u32` | 1-based beat index |
| `total_beats` | `u32` | Pre-calculated number of beats in the match |
| `opp_name` | `String` | |

---

### 5.9 BeatChoiceDto

| Field | Type | Notes |
|---|---|---|
| `text` | `String` | Player-facing option text |
| `primary_attr` | `String` | Attribute name that governs this choice |
| `difficulty` | `u8` | 0–100; higher = harder contest |

---

### 5.10 BeatOutcomeDto

| Field | Type | Notes |
|---|---|---|
| `success` | `bool` | Whether the player succeeded the contest |
| `outcome_text` | `String` | Narrative result line |
| `output_delta` | `i32` | Change in player_output this beat |
| `goal_for` / `goal_against` | `bool` | Scoreline changed |
| `yellow_card` / `red_card` | `bool` | Card issued |
| `player_output` | `i32` | Running total after this beat |
| `goals_for` / `goals_against` | `u32` | Running score |
| `is_complete` | `bool` | Match finished |
| `next_beat` | `Option<ActiveBeatDto>` | Non-null if `is_complete = false` |
| `final_result` | `Option<MatchResultDto>` | Non-null if `is_complete = true` |
| `game_state` | `Option<GoatGameState>` | Non-null if `is_complete = true`; world state is already updated |

---

### 5.11 LegacyDto

| Field | Type | Notes |
|---|---|---|
| `axes` | `Vec<LegacyAxisDto>` | 8 axes (see below) |
| `rankings` | `Vec<SchoolRankingDto>` | 4 school rankings |
| `reputation_label` | `String` | e.g. "Club Legend" |
| `sporting_rep` | `i32` | 0–100 |
| `character_rep` | `i32` | 0–100 |
| `club_fan_rep` | `i32` | 0–100 |

**LegacyAxisDto** — `name: String`, `value: i32` (0–100). The 8 axes in order:
Winning, Accolades, Output, Longevity, Decisive Moments, Loyalty, Icon, Rival.

**SchoolRankingDto** — `school_name`, `school_tagline`, `score` (0–100), `rank` and
`total` (rank N out of total pantheon members). Schools never agree; show all four.

---

### 5.12 ClubDto

| Field | Type | Notes |
|---|---|---|
| `club_id` | `u32` | Pass to `new_game()` |
| `name` | `String` | |
| `strength` | `u8` | 0–100 |
| `div_name` | `String` | Division label |
| `div_idx` | `u32` | Internal division index; use for grouping in the picker |

---

### 5.13 AwardDto

| Field | Type | Notes |
|---|---|---|
| `award_name` | `String` | "Player of the Year" or "Golden Boot" |
| `winner_name` | `String` | Winner's name (may be the player) |
| `runner_up` | `String` | May be empty |
| `pc_won` | `bool` | True if the player won this award |
| `pc_score` | `i32` | Player's score for this award |
| `winner_score` | `i32` | Winner's score |

---

### 5.14 TransferOfferDto

| Field | Type | Notes |
|---|---|---|
| `club_id` | `u32` | Pass to `accept_transfer()` |
| `club_name` | `String` | |
| `div_name` | `String` | |
| `wage` | `i64` | Annual in-game wage |
| `length` | `u32` | Contract length in seasons |
| `strength` | `u8` | Club quality 0–100 |

---

### 5.15 PeerDto

| Field | Type | Notes |
|---|---|---|
| `name` | `String` | |
| `nationality` | `String` | |
| `career_goals` | `u32` | |
| `career_matches` | `u32` | |
| `avg_output` | `u8` | Season average output |
| `titles` | `u32` | |
| `is_rival` | `bool` | True for the crystallised rival |

---

## 6. Calendar and Training Logic

The season uses two parallel counters:

- **`season_round`** — 0-indexed match round (0 to `rounds_per_season - 1`). Increments
  when a round is played.
- **`calendar_week`** — 0-indexed calendar week (0 to `calendar_weeks - 1` = 0–37).
  Increments implicitly via `round_to_week()` mapping.

Multiple match rounds can fall in the same calendar week. `week_fixtures` lists all
rounds in the current calendar week. `week_fixtures_played` tells you how many have
already been played.

**Training is once-per-calendar-week:** `week_training_done` becomes true after the
first `advance_week()` call in a calendar week and resets when the calendar week
advances (i.e., when a new match round begins in a new calendar week). Show the `[Train]`
button as disabled/greyed when `week_training_done = true`.

**Break weeks** (`is_break_week = true`) have no fixtures. The player can train freely
or fast-forward.

---

## 7. Integration Gotchas

### 7.1 Singleton state — no concurrent calls
The `WorldState` lives in a `Mutex`. All bridge calls are synchronous. Do not call
multiple bridge functions concurrently from Dart isolates — route through a single
isolate or a dedicated state management layer.

### 7.2 Seed selection
The seed governs the entire universe. Pass wall-clock nanoseconds as `u64` when the
player does not enter a seed manually — this provides effectively random generation
while preserving shareability when the player notes their seed.

### 7.3 Interactive match session
`start_interactive_match()` locks an internal `ACTIVE_MATCH` session. Do not call
`play_round()`, `advance_week()`, or any other mutating function while the session is
open. The session clears automatically when `BeatOutcomeDto.is_complete = true`.

### 7.4 Auto-play fallback
If the player starts an interactive match (`start_interactive_match()`) but then
navigates away, you must call `play_round(false)` (auto-play) to advance the round
and clear the session — or handle it by preventing navigation during an active match.

### 7.5 Season end order is mandatory
The call order in §4.6 is load-bearing. Calling `start_next_season()` before
`apply_season_end()` skips legacy/rep updates for that season. These cannot be undone.

### 7.6 No floats
All simulation values cross the bridge as `i32` integers (Fixed-point scaled to integer
for display). The renderer should not convert these to floats for any purpose that feeds
back into the core. Display formatting in Dart (e.g. `"${value / 100.0}"`) is safe
because it never re-enters the core.

### 7.7 `last_events` is transient
`GoatGameState.last_events` reflects events from the **most recent** mutating call only.
Do not persist or accumulate it across calls — surface it immediately after
`advance_week()` / `advance_weeks()` and then discard.

---

## 8. Attribute ID Reference

Use `get_attributes()` to get the full list at runtime. The stable indices by family:

| Idx | Name | Family |
|---|---|---|
| 0 | Acceleration | pace |
| 1 | Sprint Speed | pace |
| 2 | Finishing | shooting |
| 3 | Shot Power | shooting |
| 4 | Long Shots | shooting |
| 5 | Volleys | shooting |
| 6 | Penalties | shooting |
| 7 | Short Passing | passing |
| 8 | Long Passing | passing |
| 9 | Vision | passing |
| 10 | Crossing | passing |
| 11 | Free Kick Acc. | passing |
| 12 | Dribbling | dribbling |
| 13 | Ball Control | dribbling |
| 14 | Agility | dribbling |
| 15 | Balance | dribbling |
| 16 | Reactions | dribbling |
| 17 | Tackling | defending |
| 18 | Marking | defending |
| 19 | Standing Tackle | defending |
| 20 | Sliding Tackle | defending |
| 21 | Interceptions | defending |
| 22 | Heading | defending |
| 23 | Strength | physical |
| 24 | Stamina | physical |
| 25 | Jumping | physical |
| 26 | Aggression | physical |
| 27 | Composure | physical |
| 28 | Positioning | physical |
| 29 | Curve | physical |

**Passing these to `set_routine()`:** pass the index (0–29) as a `u8` element in
`attr_ids`.

---

## 9. Role Reference

`get_roles()` returns these 14 roles, sorted by rating at call time:

```
CentreBack     |  FullBack       |  DefensiveMid  |  CentralMid
AttackingMid   |  WingBack       |  Winger        |  InsideForward
DeepLyingFwd   |  TargetMan      |  PressForward  |  CompleteForward
BallPlayingDef |  Sweeper
```

---

## 10. Pundit Context (for future pundit screen)

`goat-meta` exposes `pundit_comment()` for generating pundit dialogue. The bridge does
not yet wrap this as a dedicated API call — if you need pundit commentary, add a
`get_pundit_comments() → Vec<String>` function to `api.rs` that mirrors the
`run_awards_and_pundits()` logic from `goat-tui/src/main.rs`.

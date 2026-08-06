# GOAT — Flutter App Implementation Guide

A screen-by-screen guide to building the Flutter renderer. Pairs with
**`docs/CLIENT-IMPL.md`** (the exhaustive bridge API + DTO reference) — this doc is the
*app*: navigation, screens, and how player actions map to bridge calls. Read both.

> **The one rule:** the Flutter app is a **dumb renderer**. It calls bridge functions,
> renders the DTOs they return, and sends player choices back as bridge calls. It contains
> **zero game logic, zero rules, zero randomness, no floats in anything that feeds back to
> core.** If you find yourself computing a game outcome in Dart, stop — it belongs in Rust.
> Deleting the Flutter app must lose no game logic.

---

## 0. ⚠️ Buildable now vs. pending the bridge refresh

The Rust core has moved well past Phase 10 (design rounds 2–9 added club economy,
managers, academies, cup/continental/national-team competitions, dual nationality, and
injury proneness), but **the FFI bridge still only exposes ~Phase 8** — and the gap grows
with every design round. So:

- **Buildable today** (bridge already exposes): create player, weekly training loop,
  matches (auto + interactive beats), league table, season end, transfers/contracts,
  legacy/pantheon, awards, basic peers/rival flag, retirement screen.
- **Pending the bridge refresh** (`tasks/TASK-BRIDGE-refresh.md`): the full-world screens
  (other leagues, seeded pantheon canon, emergent-rival detail), all of Phase 10
  **life & money** (economy panel, sponsors/marketability, relationships/scandals, media
  flashpoints, the richer retirement verdict), and eventually the design-round surface
  (cups, continental runs, national team, club economy detail).

Build the "today" surface first; the later screens come online as the bridge slices
land. This guide marks each screen **[NOW]** or **[AFTER BRIDGE]**.

> **⚠️ The bridge surface is frozen until Slice B.0 lands.** `frb_generated.rs` is
> field-locked codegen: changing *any* DTO breaks its compilation, and the
> `flutter_rust_bridge_codegen` toolchain + a minimal Dart target don't exist yet
> (see "D2" in `tasks/TASK-BRIDGE-refresh.md`). Until that prerequisite slice is done,
> no new bridge functions or DTO fields can appear — build strictly against the shipped
> surface in CLIENT-IMPL.md, and don't design screens around DTOs you wish existed.

---

## 1. Architecture & data flow

```
 Flutter widgets ──user action──▶ bridge pub fn (e.g. advance_week, sign_sponsor)
        ▲                                   │
        │                                   ▼
   render DTOs  ◀──GoatGameState/*Dto── Rust core (reduce → state)
```

- The bridge holds a **singleton** `WorldState` in Rust. Every mutating call returns a
  fresh `GoatGameState` snapshot. **Treat each returned snapshot as the new truth and
  rebuild from it** — don't cache derived game values in Dart.
- **No concurrent bridge calls** (singleton + Mutex). Serialize calls; disable buttons
  while one is in flight.
- All numbers crossing the boundary are integers (no floats in sim). Display formatting
  (e.g. "£4.2M") happens in Dart and never feeds back.

**Suggested state management:** a single `GameController` (Riverpod/Bloc/ChangeNotifier)
that owns the latest `GoatGameState`, exposes typed action methods wrapping each bridge fn,
and notifies the widget tree on every snapshot. Keep all `frb`/FFI calls behind it.

---

## 1a. Loading the beat library (offline asset + optional online update)

Match dialogue comes from **`beats.json`** — authored data the engine loads once at boot
(see "how do we load the beats" / `BeatLibrary::load`). The app is **offline-first by
rule** (the game is 100% offline at runtime); online loading is **only a content refresh**
and must never be required to play.

**Three sources, in priority order:**

1. **Bundled-in-bridge (always available, zero work).** The Rust bridge already embeds a
   copy of `beats.json` (`include_str!`) and lazy-loads it on first match. If you do
   nothing, matches still work fully offline. Use this as the guaranteed fallback.
2. **Flutter offline asset (recommended baseline).** Ship `beats.json` in the app bundle so
   *you* control the version the UI references, and pass it explicitly:
   ```yaml
   # pubspec.yaml
   flutter:
     assets:
       - assets/beats.json
   ```
   ```dart
   Future<void> loadBeats() async {
     // Prefer a cached online update if present, else the bundled asset.
     final json = await _readCachedBeats()                  // app docs dir, may be null
                  ?? await rootBundle.loadString('assets/beats.json');
     final ok = await bridge.loadBeatLibrary(json: json);   // pub fn load_beat_library
     if (!ok) {
       // JSON failed to parse → fall back to the asset, never ship a broken library.
       await bridge.loadBeatLibrary(
         json: await rootBundle.loadString('assets/beats.json'));
     }
   }
   ```
   Call this **once at boot, before any match** (part of the bootstrap in CLIENT-IMPL §2).
3. **Online update (optional, non-blocking).** Fetch a newer `beats.json` from your CDN,
   validate it, cache it locally, and load the cache *next boot* — never block startup or
   gameplay on the network:
   ```dart
   Future<void> checkForBeatUpdate() async {
     try {
       final res = await http.get(beatsUrl).timeout(const Duration(seconds: 5));
       if (res.statusCode == 200 && await bridge.loadBeatLibrary(json: res.body)) {
         await _cacheBeats(res.body);   // validated by the bridge parsing it
       }
     } catch (_) { /* offline / failure → keep current library, no-op */ }
   }
   ```
   Run it in the background after the game is already playable. On any failure, the app
   silently keeps the cached/bundled library.

**`load_beat_library(json) -> bool`** is the single entry point for all three — it returns
`false` if the JSON doesn't parse, so you can always fall back safely. It's part of the
already-shipped (non-stale) bridge surface.

### ⚠️ Determinism: version your beat content
Matches are deterministic in `(seed, beat library)`. Swapping `beats.json` changes future
matches — and because background/history are recomputed from the seed, a *replayed* career
will narrate differently against new content. So:
- **Tag the beat library with a version.** `beats.json` does not carry a version field
  yet — adding one is a core-repo to-do. Until it lands, treat the bundled asset as an
  implicit version and never hot-swap content mid-session.
- For reproducible replays, pin a save to the content version it was played on, and prefer
  loading the matching `beats.json` for that save. (A small concern for a single-player
  life-sim, but real if you support shared seeds.)
- Treat online updates as a **new content version**, applied at boot, not mid-career.

## 2. App lifecycle / navigation map

```
Splash/Boot
  └─ load_beat_library(beats.json)         (once, at startup — see CLIENT-IMPL §2)
Main Menu ── New Game ─▶ Create Player ─▶ HOME (career hub)
          └─ Load ─────▶ HOME
HOME (tabbed/hub)
  ├─ Squad/Player sheet       (attributes, roles, OVR)
  ├─ Train / Week loop        (the core day-to-day)
  ├─ Match                    (auto-play or interactive beats)
  ├─ League Table
  ├─ Season End flow          (awards → legacy → contract/transfer → wage)
  ├─ Legacy / Pantheon
  ├─ World            [AFTER BRIDGE]   (other leagues, canon, rival)
  ├─ Money & Life     [AFTER BRIDGE]   (economy, sponsors, relationships)
  └─ Retirement → Final Verdict        (the ending)
```

Drive navigation off the **lifecycle state machine** in CLIENT-IMPL §3 (e.g. whether a
season is active, whether a match is pending, whether the player has retired). Read the
flags on `GoatGameState` (`season_number`, `season_round`, `is_retired`, `is_suspended`,
`week_training_done`, `week_fixtures`, …) — never infer them in Dart.

---

## 3. Screens

For each screen: **shows** (which DTO fields), **actions** (which bridge fns). Field/fn
names are authoritative in CLIENT-IMPL §4–5.

### 3.1 Create Player  [NOW]
- **Shows:** name input, position picker (8-way — one of `PrimaryPosition` ST/W/WM/CAM/
  CM/DM/FB/CB, sent as its `u8` index 0–7), club list (`list_clubs` → `ClubDto`), seed
  (optional; Enter = random). **No nationality picker and no lifestyle picker** —
  nationality is derived from the chosen club's nation inside core, and lifestyle is no
  longer chosen at creation.
- **Actions:** `new_game(player_name, position, club_id, seed, lifestyle)` →
  `GoatGameState`. `club_id` is the stable club index from `list_clubs`. The `lifestyle`
  arg is ignored (kept for FFI binary compatibility) — pass any value, e.g. `1`.
- Offer a "re-roll seed" to preview different talent before committing. Note that
  changing the seed re-rolls the *world* as well as the player's talent — the club list
  indexes stay stable, but everything generated from the seed differs.

### 3.2 Player Sheet  [NOW]
- **Shows:** `get_attributes()` → `AttrDto[]` (current/potential bars per attribute,
  grouped by family via `get_families`), `get_roles()` → role familiarity, OVR, age, energy,
  injury weeks, form, and lifestyle as a **read-only status** (`GoatGameState.lifestyle`:
  0=Professional/1=Balanced/2=Flashy) — it emerges from training intensity, dev
  investment, and sponsor choices over the career; there is no lifestyle picker anywhere
  in the app.
- **Actions:** none (read-only). This is the player's identity screen.

### 3.3 Week Loop (Train)  [NOW] — the heartbeat
- **Shows:** current calendar week (`calendar_week_label`, `is_break_week`),
  `week_fixtures`, energy, `week_training_done`, and `last_events` (injuries, breakthroughs,
  familiarity upgrades, **calendar flashpoints** like transfer windows).
- **Actions:** `set_routine(attr_ids, intensity)`; `advance_week()` (single) or
  `advance_weeks(n)` (fast-forward, stops on first noteworthy event); render `last_events`
  as a digest after each advance. **Manage-by-exception:** quiet weeks should be one tap;
  only interrupt the player when `last_events` is non-empty.

### 3.4 Match  [NOW]
- **Auto-play:** `play_round(false)` → `(GoatGameState, MatchResultDto)`.
  Show the **output rating separate from the scoreline** — a high `player_output` in a
  defeat is the intended drama ("hat-trick, lost 3–2").
- **Interactive beats:** call `start_interactive_match()` → `Option<ActiveBeatDto>` to open
  a beat session and get the first beat; then loop: present `BeatChoiceDto[]` →
  `make_beat_choice(choice_idx)` → render `BeatOutcomeDto`, until the match completes
  (CLIENT-IMPL §4.4, §5.8). Show headspace/momentum cues from the DTOs. **The app never
  decides outcomes** — it only relays the chosen index.
  - ⚠️ **Do not** use `play_round(interactive=true)` for this — the `interactive` flag is
    currently **ignored** (always auto-plays). Interactive matches only exist through the
    `start_interactive_match` / `make_beat_choice` flow.
  - ⚠️ **Session lock:** while an interactive session is active, no other mutating bridge
    calls (`play_round`, `advance_week`, …) are allowed. If the player navigates away
    mid-match, call `play_round(false)` to auto-resolve the round and close the session
    (CLIENT-IMPL §7).

### 3.5 League Table  [NOW]
- **Shows:** `get_table()` → `TableRowDto[]`; highlight the PC's club; mark champions/
  relegation if the DTO flags them.

### 3.6 Season End flow  [NOW] — order is mandatory (CLIENT-IMPL §7.5)
1. `apply_season_end()` → snapshot (settles legacy/rep, batch-ticks peers, checks rival).
2. `get_season_awards()` → `AwardDto[]` (awards night).
3. Show legacy delta (`get_legacy`).
4. Contract/transfer step (`get_transfer_offers` → `TransferOfferDto[]`,
   `accept_transfer`, `agitate_for_transfer`).
5. `start_next_season()`. Wage collected. Then a new season begins.
Render these as a guided sequence, not a dump.

### 3.7 Legacy / Pantheon  [NOW]
- **Shows:** `get_legacy()` → `LegacyDto` (the 7+1 axes + the schools' rankings). Present
  the schools **disagreeing** — that's the point; don't average them into one number.

### 3.8 Save / Load  [NOW]
- **Actions:** `save_game(path) -> bool`; `load_game(path) -> Option<GoatGameState>`.
- Use the app documents directory (e.g. `path_provider`) for save paths — one file per
  slot. `load_game` returning `null` means missing/corrupt: report it and stay on the
  menu; don't retry-loop or patch the file.
- After a successful load, replace the controller's snapshot with the returned state and
  navigate straight to HOME.

### 3.9 World  [AFTER BRIDGE]
- Other-league standings + top scorers, the seeded **pantheon canon** (past greats), and
  the **emergent rival** (or the weak-era "you reign alone" note). Backed by the Phase-9
  read-models the bridge refresh adds (`get_pantheon_canon`, `get_rival_verdict`,
  `get_world_standings`).

### 3.10 Money & Life  [AFTER BRIDGE]
- **Economy:** savings, wage, business value, bankruptcy risk; actions
  `invest_in_business`, `set_dev_investment` (ceiling-capped — surface that it speeds
  development but can't exceed potential).
- **Sponsors:** marketability tier gauge; `sign_sponsor(tier)`; show the energy/time and
  reputation trade-off of over-commercialising.
- **Life:** 2–3 relationship threads; surface scandals on rupture; **media flashpoints**
  as a contrite-vs-defiant choice (`respond_to_media`) that visibly moves reputation facets.

### 3.11 Retirement → Final Verdict  [NOW basic / richer AFTER BRIDGE]
- Trigger on `is_retired` (player choice via `retire()` now; decline-driven
  `should_retire` after the bridge exposes it). Show the career retrospective, then
  **the schools' final, disagreeing placements** — the Icon axis reflecting the off-pitch
  career once Phase 10 is bridged. **No win screen. The debate is the ending.**

---

## 4. UX principles (from the design bible)

- **Manage-by-exception:** auto-advance quiet time; interrupt only at flashpoints. A whole
  season should be playable in a few minutes when nothing demands attention.
- **You build a case, you don't "win".** No victory screen; the schools argue forever.
- **Text is data.** All narrative strings come from the core (match beats already do).
  Don't write game prose in Dart; render what the bridge gives you. (Screen *chrome* —
  labels, buttons — is the renderer's; *content* is the core's.)
- **Tone:** dramatic but terse; the sim is deep, the UI is calm.

---

## 5. Integration gotchas (see CLIENT-IMPL §7 for the full list)

- Singleton state → no concurrent calls; one action in flight at a time.
- Interactive matches lock the session: no other mutating calls until the match completes;
  auto-resolve with `play_round(false)` if the player bails out mid-match.
- Call the season-end functions **in order**; skipping steps corrupts the flow.
- `last_events` is **transient** — read it immediately after an advance; it's overwritten.
- Re-derive nothing: potentials, fixtures, history, and background players are recomputed
  in core from the seed. The app only ever displays.
- Determinism: same seed + same actions = same career. If a save reloads to a different
  state, that's a bridge bug, not a UI workaround — report it.

---

## 6. Pointers

- **`docs/CLIENT-IMPL.md`** — every bridge fn + DTO field, exact names/types (authoritative).
- **`tasks/TASK-BRIDGE-refresh.md`** — what the bridge still needs to expose Phase 9/10+
  (gates the [AFTER BRIDGE] screens; note the Slice B.0 codegen prerequisite in §0 above).
- **`docs/DESIGN_BIBLE.md`** — the game's intent and tone.
- **`crates/goat-tui`** — a working reference renderer in Rust; mirror its flows (it proves
  the same bridge-shaped contract end-to-end).

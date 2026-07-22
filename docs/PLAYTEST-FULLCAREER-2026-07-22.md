# Full-career playtest — 2026-07-22

Follow-up to `docs/PERSONA-PLAYTEST-2026-07-22.md` (round 1, 10 personas) and
`docs/PLAYTEST-ROUND2-2026-07-22.md` (round 2, 5 personas). Both prior rounds
only ever played a **few weeks of season 1** per persona (character creation
through a handful of rounds and one match). This round does the opposite: one
persona, played **start to finish** — young academy prospect through to
retirement, matching the game's own framing in `docs/MAIN.md` ("You play one
footballer, from a teenager in an academy to retirement").

## Method

- Binary: `goat-tui`, driven via scripted/reactive stdin (no source changes —
  read-only playtest). Built twice: first the **debug** profile (`cargo build
  -p goat-tui`, the default the task asked to try first), then, after that
  build turned out to crash reliably at the end of season 2 (see Bug 1), the
  **release** profile (`cargo build --release -p goat-tui`) for the rest of
  the career.
- Persona (one, held for the whole run): **Marcus Chen**, CM, England,
  Premier League, **Aston Villa**, seed `1234567`. Routine: Short Pass /
  Vision / Interceptions / Stamina, Medium intensity, set once at creation and
  never changed.
- A season is 30 rounds (`ROUNDS_PER_SEASON = 30`). Every round was trained
  (`W`) first. Round 1 of every season was played **interactively** (`P`,
  picking through the real beat-by-beat match choices) specifically to keep
  sampling match beats/training feedback/menu behaviour across the whole
  career, not just at the start — the task explicitly asked for this, since
  it's exactly what a short playtest can't do. Round 15 of every season did a
  manual `T` (table) + `G` (Legacy, mid-season) check. All other rounds used
  `K` (auto-skip) to cover the season efficiently.
- `F` (fast-forward) was sampled 8 times, spread from age 16 to age 44, purely
  to re-verify round 1/round 2's "F never advances the season round" finding
  at much larger scale (see Note 5).
- Transfer offers: the very first offer seen in the career was accepted (one
  transfer, Aston Villa → Cruzeiro, end of season 1); every offer after that
  was declined (stay), to keep the "one club, mostly" spirit of a career while
  still exercising `ExecuteTransfer` once. Contract renewals were always
  accepted. The auto-retirement prompt (`[R] Retire now`, only offered once
  age ≥ 35 **and** form < 40) was always accepted the moment it appeared.
- This was driven by a throwaway Python harness (not committed — it's not
  game code and the task asked for read-only playtesting) that read stdout
  reactively (idle-gap detection) and answered whichever prompt appeared,
  rather than a fixed keystroke script — necessary because transfer/contract/
  retirement prompts only appear conditionally.
- The run ultimately covered **62 completed seasons** (age 16 → 77) before
  retiring at the end of season 62. That is far beyond a "long career" and
  was driven deliberately far past the point some findings below were already
  confirmed, specifically to stress-test season-end banking across as many
  transitions as possible and to see whether organic retirement was reachable
  at all.

## Headline verdict

Season-end **banking itself is solid**: goals/matches/seasons/titles/POTY and
all 4 Pantheon scores+ranks update correctly and monotonically across 61 real
season boundaries, and Legacy stays correctly frozen mid-season every single
time it was checked (61/61). Attribute growth-then-decline across a lifetime
also behaves sensibly (see the attribute section below). But three things a
short playtest structurally cannot see are broken, and one of them means the
game **cannot reach its own stated ending under an extremely common condition**:

1. **[CRITICAL] Debug builds crash reliably at the end of season 2, every time, for every save.** An integer-overflow panic in the awards RNG-seed hashing.
2. **[HIGH] Retirement is effectively unreachable during a normal high-performing career** — it took until **age 77 / season 62** in this run, long after attributes had collapsed to near-zero, and the hard retirement age defined in `goat-core` is never enforced by the TUI at all.
3. **[HIGH] Checking Legacy at the season-end menu re-runs the entire season-end pipeline**, double-banking career stats, wage, and re-rolling transfer/contract offers.

Details below, most severe first.

---

## Bugs found

### Bug 1 (CRITICAL) — debug builds panic at the end of every season ≥ 2

`crates/goat-meta/src/awards.rs:51` (`ai_competitor_score`):

```rust
let mut rng = GoatRng::new(
    world_seed ^ (season as u64 * 0x9e3779b97f4a7c15) ^ (candidate_idx as u64 * 0xdeadbeef),
);
```

`0x9e3779b97f4a7c15` (the standard 64-bit golden-ratio hash constant) equals
`11400714819323198485`. `u64::MAX` is `18446744073709551615`. So
`season * 0x9e3779b97f4a7c15` overflows `u64` for **any `season >= 2`** —
`season=1` is fine (`11400714819323198485`), `season=2` is
`22801429638646396970`, already past `u64::MAX`. Confirmed by direct
computation, not just inspection. This function is called once per AI
competitor (8 for Player of the Year, 8 for Golden Boot) at the end of
**every** season from season 2 onward, unconditionally — it is not
RNG-dependent or seed-dependent whether it fires; it always does, for every
career, the very first time a second season closes.

In a debug build (`cargo build -p goat-tui`, the default profile, and the one
the task suggested trying first if the binary needed building), Rust's
default `overflow-checks = true` turns that overflow into a hard panic. Exact
reproduction, first try, this persona, no edge cases needed — verbatim from
the captured transcript:

```
--- ROUND 30 / 30 · Game Week 35 · Apr 2027 ---
...
>>> SEND: 'K'

thread 'main' (471411) panicked at crates/goat-meta/src/awards.rs:51:22:
attempt to multiply with overflow
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

The connection died with `BrokenPipeError` a few commands later; from that
point the game state is no longer trustworthy. Every prior playtest round
only ever reached season 1, so nobody had hit this before — **this is exactly
the class of bug the task exists to catch**: no amount of "a few weeks of
season 1" playtesting can ever reach it, because it requires finishing an
entire second season, and it fires 100% of the time, deterministically, the
first time `compute_player_of_year`/`compute_golden_boot` run for season 2.

Confirmed it does **not** reproduce in a `--release` build (Cargo's release
profile defaults `overflow-checks = false`, so the multiplication silently
wraps instead of panicking) — that's how the rest of this career was played.
That's a mitigation, not a fix: the intent here is clearly a hash-mixing
multiply that's supposed to wrap, so it should use `wrapping_mul` (e.g.
`season.wrapping_mul(0x9e3779b97f4a7c15)` and similarly for
`candidate_idx as u64 * 0xdeadbeef`) rather than plain `*`. As written, anyone
building this project the normal way (`cargo build`/`cargo run`, debug)
cannot play past season 1.

### Bug 2 (HIGH) — retirement is not reachable in practice; the hard-age cap is dead code in the TUI

`crates/goat-core/src/tuning.rs` defines both a soft and hard retirement age:

```rust
pub const RETIRE_AGE_SOFT: u32 = 34;
pub const RETIRE_AGE_HARD: u32 = 40;
```

and `crates/goat-core/src/state.rs` has a `should_retire()` function that
encodes both, covered by dedicated core tests
(`spec_phase10_retire.rs`, `spec_phase10_longhorizon.rs`). **`should_retire()`
is never called anywhere in `goat-tui`** (`crates/goat-tui/src/main.rs`) — it
only exists in test code. The *only* retirement path reachable through the
TUI is this one-shot suggestion at season end
(`crates/goat-tui/src/main.rs`, around line 349):

```rust
if age_years >= 35 && state.pc_form.to_int() < 40 {
    ...
    writeln!(out, "  [R] Retire now   [C] Continue playing").unwrap();
    ...
}
```

i.e. it only ever *offers* retirement, and only when **both** age ≥ 35 **and**
current form < 40 hold simultaneously that exact season-end. There is no
other in-game way to retire — no menu "Retire" command, no hard cutoff at 40.

In this run, that combination — age ≥ 35 *and* form < 40 — never once
coincided from season 20 (age 35, form 79) all the way through **season 61**
(age 76, form 60). Form spent this entire stretch oscillating in the
high-40s to mid-90s (see the season table below) even as OVR steadily
declined from a peak of 86 down into the 50s and PACE/SHOOTING/PHYSICAL
crashed to single digits (see the attribute section). The gate only finally
fired at the end of **season 62, age 77**:

```
  At 77 years old with form 32, the end may be near.
  [R] Retire now   [C] Continue playing
```

— by which point the character's attribute sheet read `Pac:1 Sho:2 Pas:24
Dri:7 Def:16 Phy:3` (see attribute section) and he had *still* just won that
season's Golden Boot. A real player would have retired decades earlier; the
game gave no earlier way to do so short of manually declining every "Continue
playing" prompt that never appeared, because it never appeared. Nothing else
in the loop enforces `RETIRE_AGE_HARD = 40` at all — the TUI will happily run
a top-flight 77-year-old goal-scorer if form cooperates.

This is a genuinely long-career-only finding: any playtest of a handful of
seasons would see the *design* (a form/age gate exists) and reasonably assume
it works, without ever discovering that the gate's actual real-world hit rate,
for a well-performing player, is "not for 40+ simulated years."

### Bug 3 (HIGH) — pressing Legacy at the season-end menu re-runs the whole season-end pipeline

At the special end-of-season menu (`crates/goat-tui/src/main.rs`, inside the
`if state.season_round >= ROUNDS_PER_SEASON` block):

```rust
writeln!(out, "\n  [Y] Next season   [G] Legacy   [Z] Save & quit   [Q] Quit").unwrap();
...
match lines.next() {
    Some(Ok(l)) => match l.trim().to_ascii_uppercase().as_str() {
        "Y" => { state = reduce(state, Intent::StartSeason, ...); continue; }
        "G" => {
            let ev = build_legacy_evidence(&state);
            render_legacy_screen(out, &ev, &state);
            continue;   // <-- jumps to the top of the OUTER loop, not back to this menu
        }
        ...
```

That `continue` re-enters the top of `run_game_loop`'s outer `loop`, which
immediately re-evaluates `season_round >= ROUNDS_PER_SEASON` — still true,
since `G` didn't touch it — so it **reprints the season review and re-runs
the entire season-end sequence a second time**: `CollectWage` (double wage),
`run_awards_and_pundits` → `ApplySeasonEndLegacy` (double-banks that season's
goals/matches into career totals and increments `seasons_played` again),
`BatchTickPeers`, the rival-crystallisation check, a **fresh** transfer window
roll (new/different offers, since RNG is reseeded from the same
`season_number` but club/wage state has already changed), and contract
renewal — all before the player ever gets back to the `[Y]/[G]/[Z]/[Q]` menu
they actually asked to see.

Reproduced cleanly in an early exploratory session with this exact persona
(before the rest of this run started deliberately avoiding it): after
finishing season 1 (Aston Villa, 46 goals, 28 matches, 1 title), the player
accepted a transfer offer to Cruzeiro, then pressed `G` to check Legacy —

```
║  LEGACY — Respected Pro
║  CAREER EVIDENCE
║  Goals:   46   Matches:   28   Seasons:  1
...
```

— which is correct so far, but the very next thing printed, unprompted, was:

```
╔══════════════════════════════════════════════╗
║  SEASON 1 REVIEW                             ║
╠══════════════════════════════════════════════╣
║  Cruzeiro: finished 6th in Série A
║  Your season: 28 matches  46 goals  Output avg: 74
...
╔══════════════════════════════════════════════╗
║  TRANSFER WINDOW                             ║
...
```

i.e. a second, unsolicited "Season 1 Review" (now rendered against the
*post-transfer* club/division, itself a confusing side effect) and a second
transfer window, with `CollectWage`/`ApplySeasonEndLegacy` having silently run
again in between. Left unchecked, every "check my Legacy before moving on" —
an extremely natural thing to do at a season boundary, and the exact thing
this playtest's protocol does every single season — would compound: goals and
matches would roughly double per check, `seasons_played` would run ahead of
the real season counter, and transfer/contract offers could fire multiple
times inside what should be one season boundary.

Once this was identified, the rest of the 61 clean season transitions
**deliberately avoided pressing `G` at that specific menu** (sampling Legacy
safely instead, from the normal in-loop `G` available at round 1 of the next
season, which does not have this problem) specifically so the season table
below would be trustworthy. That workaround is why the table's
`career_goals`/`career_matches`/`titles` numbers are clean and monotonic
across all 61 real transitions — but an ordinary player has no way to know to
avoid this, and would hit it the first time curiosity got the better of them
at a season boundary.

**Fix suggestion:** the `"G"` arm inside the season-end block should `return`
or otherwise loop back to re-print just that small menu, not `continue` the
outer loop.

### Note 4 — zero injuries observed across 62 seasons / ~1,860 matches

Discipline (red cards/suspensions) fired constantly and believably — 111
times over the career, direct and immediate impact on suspensions and
`Character` reputation, all self-consistent. Fast-forward's calendar events
(international breaks etc.) also fired regularly. But not a single `⚠
INJURY!` event was seen in 61 completed seasons of weekly training and
~1,860 played matches for this persona (Medium intensity throughout). That
might be correct tuning (injuries deliberately rare), but it's worth a
deliberate look — this is squarely a "you'd never notice in a short playtest"
class of question, since round 1/round 2 never played long enough to expect
one either way.

### Note 5 — confirmed, at scale, findings already suspected from short playtests

- **`F` fast-forward never advances the season round or auto-resolves a
  fixture.** Sampled 8 times across the career (ages 16, 20, 24, 28, 32, 36,
  40, 44): every single time, `F` → `2` weeks advanced age by 2 weeks and left
  the round marker byte-identical (e.g. `S17 Round 2/30` before and after).
  This matches round 1's Slice 10 "could not reproduce" conclusion and round
  2's re-confirmation — now confirmed a further 8 times across a 46-year span,
  reinforcing that this is stable, expected behaviour rather than a
  regression risk, and that `F` is really only useful for burning training
  weeks between two matches of the same round, not for skipping through a
  season faster.
- **Double-fixture weeks correctly gate the second `W` of a fixture-doubled
  round.** Fired 186 times over the career (about 3 times per season, in line
  with `week_ends_after_round`'s comment about congested-calendar weeks) —
  every time, `W` on the follow-up round printed the same "already trained"
  message documented in round 1/round 2, never a crash or desync. This isn't
  actually the same bug those rounds were probing (that was about *pressing W
  twice in the same round*) — this is the calendar's *own* double-fixture
  weeks correctly leaving the flag set going into the next round, which is
  documented, intended behaviour (`calendar.rs`'s own doc comment). Recorded
  here only because the volume (186 hits) makes it worth noting that it never
  once misbehaved at scale.
- **Legacy's mid-season freeze note held 61/61 times.** Every single
  round-15 check across the whole career printed the "Career totals and
  Pantheon scores update at season end" note and showed unchanged totals —
  round 1's fix (c) holds at a scale two orders of magnitude beyond what
  round 1/round 2 tested (5-10 checks vs. 61 here).
- **Rivalry crystallises exactly once, as designed** (season 5, vs. O. Brown)
  and stayed stable (same rival named at the season-62 retirement screen,
  57 seasons later) — no duplicate/overwritten rivalry across the whole run.

### Note 6 — attribute decline is real and eventually severe, but doesn't reduce output much until very late

OVR peaked at **86** around age 26-33 (season 11-18), then declined smoothly
and monotonically all the way to **58** by age 76 (season 61) — decline is
clearly implemented and working. But the *shape* of decline is uneven and
worth flagging:

- **Physical/pace attributes crash to the floor even while still actively
  trained.** Stamina was one of the 4 routine-trained attributes the entire
  career, yet went `94/94` (fully maxed, age 27) → `1/44` (age 76). Pace
  (untrained, but inherited from creation) went `37` → `1` by around age 42
  and stayed pinned at the floor for the next 34 seasons.
- **Trained technical/mental attributes decline far more gently.**
  Short Pass (trained) went `90/93` (peak) → `71/93` (age 76); Vision
  (trained) `66/91` → `62/91`; Interceptions (trained) `68/92` → `66/92` —
  barely moved in 50 years of in-game time.
- **The practical consequence:** a 76-year-old with `Pac:1 Sho:2 Phy:3` was
  still posting `Form` in the high 50s/60s, still finishing top-5 in Série A,
  and had just won a Golden Boot at 77. Match output clearly weights
  passing/technical attributes heavily enough (for a CM in this engine) that
  physical collapse alone doesn't crater performance the way it would for a
  real footballer. This interacts directly with Bug 2: it's precisely
  *because* output/form stays this resilient into extreme old age that the
  only retirement gate (form < 40) almost never fires.

---

## Season-by-season log (all 61 completed seasons)

Snapshot taken at every season boundary (post-`ApplySeasonEndLegacy`, before
any training in the new season) — age, full category breakdown, OVR, form,
league finish, that season's/career goals+matches, career titles/POTY, and
all 4 Pantheon scores+ranks (out of 11 pros in the world). `Character rep`
(0-100, discipline/off-pitch reputation) is included as the discipline/
suspension carry-forward signal; no persistent injury flag ever appeared to
report (see Note 4). One club change happened, end of season 1 (Aston Villa →
Cruzeiro, Série A); `clubs_served = 2` for the rest of the career (omitted
from the table since it never changes again).

| S | Age | OVR | Pac | Sho | Pas | Dri | Def | Phy | Form | Finish (division) | Season G/M | Career G | Career M | Titles | POTY | Trophy Cabinet | Eye-Test Rom. | Stats Purists | Loyalty Trad. | Character rep |
|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|:--|--:|--:|--:|--:|--:|:--|:--|:--|:--|--:|
| 1 | 16 | 74 | 37 | 34 | 45 | 44 | 41 | 54 | 79 | 1st (Premier League) | 46/28 | 46 | 28 | 1 | 0 | 24/100 #11 | 23/100 #11 | 40/100 #11 | 44/100 #11 | 46 |
| 2 | 17 | 76 | 37 | 34 | 46 | 44 | 41 | 54 | 74 | 2nd (Série A) | 39/28 | 85 | 56 | 1 | 0 | 25/100 #11 | 24/100 #11 | 42/100 #11 | 47/100 #11 | 40 |
| 3 | 18 | 77 | 37 | 34 | 46 | 44 | 42 | 54 | 88 | 1st (Série A) | 44/28 | 129 | 84 | 2 | 1 | 40/100 #11 | 36/100 #11 | 56/100 #11 | 58/100 #9 | 53 |
| 4 | 19 | 79 | 37 | 34 | 47 | 44 | 42 | 55 | 87 | 1st (Série A) | 50/29 | 179 | 113 | 3 | 1 | 48/100 #8 | 40/100 #11 | 60/100 #11 | 65/100 #8 | 65 |
| 5 | 20 | 80 | 37 | 34 | 48 | 44 | 42 | 55 | 79 | 2nd (Série A) | 34/28 | 213 | 141 | 3 | 1 | 48/100 #8 | 40/100 #11 | 62/100 #10 | 66/100 #8 | 43 |
| 6 | 21 | 82 | 37 | 34 | 48 | 44 | 43 | 55 | 91 | 1st (Série A) | 65/28 | 278 | 169 | 4 | 2 | 62/100 #5 | 51/100 #9 | 73/100 #4 | 75/100 #2 | 52 |
| 7 | 22 | 83 | 37 | 34 | 49 | 44 | 43 | 55 | 77 | 1st (Série A) | 54/27 | 332 | 196 | 5 | 2 | 70/100 #4 | 54/100 #8 | 77/100 #3 | 80/100 #1 | 29 |
| 8 | 23 | 84 | 37 | 34 | 49 | 44 | 43 | 55 | 78 | 1st (Série A) | 48/28 | 380 | 224 | 6 | 2 | 72/100 #4 | 56/100 #7 | 80/100 #2 | 81/100 #1 | 40 |
| 9 | 24 | 84 | 37 | 34 | 50 | 44 | 43 | 56 | 82 | 1st (Série A) | 51/28 | 431 | 252 | 7 | 2 | 72/100 #4 | 56/100 #7 | 82/100 #2 | 83/100 #1 | 35 |
| 10 | 25 | 85 | 37 | 34 | 50 | 44 | 44 | 56 | 94 | 1st (Série A) | 66/29 | 497 | 281 | 8 | 3 | 73/100 #4 | 57/100 #7 | 85/100 #1 | 84/100 #1 | 45 |
| 11 | 26 | 86 | 37 | 34 | 51 | 44 | 44 | 56 | 82 | 1st (Série A) | 54/30 | 551 | 311 | 9 | 3 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 74 |
| 12 | 27 | 86 | 37 | 34 | 52 | 44 | 44 | 56 | 86 | 1st (Série A) | 62/28 | 613 | 339 | 10 | 3 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 67 |
| 13 | 28 | 86 | 36 | 34 | 52 | 44 | 45 | 55 | 85 | 2nd (Série A) | 62/29 | 675 | 368 | 10 | 3 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 76 |
| 14 | 29 | 86 | 35 | 34 | 53 | 44 | 45 | 55 | 83 | 1st (Série A) | 59/30 | 734 | 398 | 11 | 4 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 100 |
| 15 | 30 | 86 | 34 | 34 | 53 | 43 | 45 | 54 | 94 | 1st (Série A) | 64/30 | 798 | 428 | 12 | 4 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 100 |
| 16 | 31 | 86 | 33 | 34 | 53 | 43 | 45 | 54 | 73 | 3rd (Série A) | 41/28 | 839 | 456 | 12 | 4 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 100 |
| 17 | 32 | 86 | 31 | 34 | 53 | 42 | 45 | 52 | 90 | 1st (Série A) | 63/30 | 902 | 486 | 13 | 5 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 100 |
| 18 | 33 | 86 | 28 | 34 | 53 | 41 | 44 | 51 | 92 | 3rd (Série A) | 62/30 | 964 | 516 | 13 | 5 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 100 |
| 19 | 34 | 86 | 26 | 33 | 53 | 40 | 44 | 50 | 84 | 1st (Série A) | 51/29 | 1015 | 545 | 14 | 5 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 100 |
| 20 | 35 | 85 | 24 | 33 | 53 | 40 | 43 | 48 | 79 | 1st (Série A) | 58/29 | 1073 | 574 | 15 | 5 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 100 |
| 21 | 36 | 85 | 21 | 32 | 52 | 38 | 42 | 46 | 81 | 1st (Série A) | 60/30 | 1133 | 604 | 16 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 100 |
| 22 | 37 | 85 | 17 | 31 | 51 | 36 | 41 | 44 | 79 | 1st (Série A) | 61/28 | 1194 | 632 | 17 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 100 |
| 23 | 38 | 84 | 14 | 30 | 50 | 34 | 40 | 41 | 79 | 1st (Série A) | 54/29 | 1248 | 661 | 18 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 100 |
| 24 | 39 | 83 | 10 | 29 | 50 | 33 | 39 | 39 | 74 | 1st (Série A) | 59/30 | 1307 | 691 | 19 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 100 |
| 25 | 40 | 83 | 6 | 28 | 49 | 31 | 37 | 36 | 76 | 1st (Série A) | 53/30 | 1360 | 721 | 20 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 100 |
| 26 | 41 | 82 | 2 | 27 | 48 | 29 | 36 | 34 | 85 | 1st (Série A) | 57/28 | 1417 | 749 | 21 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 82 |
| 27 | 42 | 81 | 1 | 25 | 47 | 27 | 34 | 31 | 79 | 2nd (Série A) | 53/29 | 1470 | 778 | 21 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 93 |
| 28 | 43 | 80 | 1 | 24 | 46 | 27 | 34 | 29 | 80 | 1st (Série A) | 61/28 | 1531 | 806 | 22 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 87 |
| 29 | 44 | 80 | 1 | 23 | 45 | 26 | 33 | 27 | 75 | 6th (Série A) | 39/27 | 1570 | 833 | 22 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 81 |
| 30 | 45 | 79 | 1 | 22 | 44 | 25 | 32 | 26 | 65 | 1st (Série A) | 40/26 | 1610 | 859 | 23 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 59 |
| 31 | 46 | 79 | 1 | 21 | 43 | 25 | 32 | 24 | 83 | 1st (Série A) | 61/26 | 1671 | 885 | 24 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 53 |
| 32 | 47 | 78 | 1 | 19 | 42 | 24 | 31 | 21 | 78 | 2nd (Série A) | 49/25 | 1720 | 910 | 24 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 30 |
| 33 | 48 | 78 | 1 | 19 | 41 | 23 | 30 | 20 | 83 | 1st (Série A) | 60/27 | 1780 | 937 | 25 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 24 |
| 34 | 49 | 77 | 1 | 17 | 40 | 22 | 29 | 18 | 59 | 1st (Série A) | 45/28 | 1825 | 965 | 26 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 18 |
| 35 | 50 | 76 | 1 | 16 | 39 | 22 | 28 | 16 | 56 | 6th (Série A) | 36/28 | 1861 | 993 | 26 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 31 |
| 36 | 51 | 75 | 1 | 15 | 38 | 21 | 28 | 14 | 67 | 4th (Série A) | 48/26 | 1909 | 1019 | 26 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 18 |
| 37 | 52 | 74 | 1 | 14 | 37 | 20 | 27 | 12 | 74 | 2nd (Série A) | 38/24 | 1947 | 1043 | 26 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 5 |
| 38 | 53 | 73 | 1 | 13 | 36 | 20 | 26 | 11 | 77 | 1st (Série A) | 44/30 | 1991 | 1073 | 27 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 86/100 #1 | 32 |
| 39 | 54 | 72 | 1 | 12 | 35 | 19 | 26 | 10 | 58 | 1st (Série A) | 43/28 | 2034 | 1101 | 28 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 85/100 #1 | 27 |
| 40 | 55 | 71 | 1 | 11 | 34 | 18 | 25 | 9 | 57 | 1st (Série A) | 43/24 | 2077 | 1125 | 29 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 85/100 #1 | 1 |
| 41 | 56 | 70 | 1 | 9 | 33 | 18 | 24 | 8 | 59 | 9th (Série A) | 31/24 | 2108 | 1149 | 29 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 85/100 #1 | 2 |
| 42 | 57 | 69 | 1 | 8 | 32 | 17 | 23 | 7 | 67 | 8th (Série A) | 38/27 | 2146 | 1176 | 29 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 85/100 #1 | 10 |
| 43 | 58 | 68 | 1 | 7 | 31 | 16 | 23 | 7 | 69 | 2nd (Série A) | 47/28 | 2193 | 1204 | 29 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 85/100 #1 | 22 |
| 44 | 59 | 68 | 1 | 6 | 30 | 15 | 22 | 6 | 57 | 3rd (Série A) | 36/27 | 2229 | 1231 | 29 | 6 | 73/100 #4 | 57/100 #7 | 86/100 #1 | 85/100 #1 | 17 |
| 45 | 60 | 67 | 1 | 6 | 30 | 15 | 21 | 6 | 63 | 15th (Série A) | 30/23 | 2259 | 1254 | 29 | 6 | 73/100 #4 | 57/100 #7 | 85/100 #1 | 85/100 #1 | 43 |
| 46 | 61 | 67 | 1 | 5 | 30 | 14 | 21 | 6 | 55 | 1st (Série A) | 36/25 | 2295 | 1279 | 30 | 6 | 73/100 #4 | 57/100 #7 | 85/100 #1 | 85/100 #1 | 22 |
| 47 | 62 | 66 | 1 | 5 | 29 | 14 | 20 | 5 | 63 | 2nd (Série A) | 39/25 | 2334 | 1304 | 30 | 6 | 73/100 #4 | 57/100 #7 | 85/100 #1 | 85/100 #1 | 9 |
| 48 | 63 | 65 | 1 | 5 | 29 | 13 | 20 | 5 | 68 | 10th (Série A) | 31/26 | 2365 | 1330 | 30 | 6 | 73/100 #4 | 57/100 #7 | 85/100 #1 | 85/100 #1 | 7 |
| 49 | 64 | 65 | 1 | 5 | 29 | 13 | 20 | 5 | 47 | 13th (Série A) | 25/22 | 2390 | 1352 | 30 | 6 | 73/100 #4 | 57/100 #7 | 85/100 #1 | 85/100 #1 | 0 |
| 50 | 65 | 64 | 1 | 5 | 28 | 13 | 20 | 5 | 59 | 4th (Série A) | 38/26 | 2428 | 1378 | 30 | 6 | 73/100 #4 | 57/100 #7 | 85/100 #1 | 85/100 #1 | 14 |
| 51 | 66 | 63 | 1 | 4 | 28 | 12 | 19 | 5 | 68 | 3rd (Série A) | 39/23 | 2467 | 1401 | 30 | 6 | 73/100 #4 | 57/100 #7 | 85/100 #1 | 85/100 #1 | 4 |
| 52 | 67 | 63 | 1 | 4 | 28 | 11 | 19 | 4 | 61 | 9th (Série A) | 30/25 | 2497 | 1426 | 30 | 6 | 73/100 #4 | 57/100 #7 | 85/100 #1 | 85/100 #1 | 20 |
| 53 | 68 | 62 | 1 | 4 | 27 | 11 | 19 | 4 | 47 | 15th (Série A) | 30/27 | 2527 | 1453 | 30 | 6 | 73/100 #4 | 57/100 #7 | 85/100 #1 | 85/100 #1 | 50 |
| 54 | 69 | 62 | 1 | 4 | 27 | 10 | 18 | 4 | 60 | 10th (Série A) | 34/25 | 2561 | 1478 | 30 | 6 | 73/100 #4 | 57/100 #7 | 85/100 #1 | 85/100 #1 | 43 |
| 55 | 70 | 61 | 1 | 4 | 26 | 10 | 18 | 4 | 54 | 4th (Série A) | 34/24 | 2595 | 1502 | 30 | 6 | 73/100 #4 | 57/100 #7 | 85/100 #1 | 85/100 #1 | 19 |
| 56 | 71 | 60 | 1 | 3 | 26 | 9 | 18 | 4 | 47 | 3rd (Série A) | 24/22 | 2619 | 1524 | 30 | 6 | 73/100 #4 | 57/100 #7 | 85/100 #1 | 85/100 #1 | 3 |
| 57 | 72 | 60 | 1 | 3 | 26 | 9 | 17 | 4 | 61 | 11th (Série A) | 24/20 | 2643 | 1544 | 30 | 6 | 73/100 #4 | 57/100 #7 | 84/100 #1 | 85/100 #1 | 5 |
| 58 | 73 | 60 | 1 | 3 | 25 | 8 | 17 | 3 | 53 | 4th (Série A) | 30/25 | 2673 | 1569 | 30 | 6 | 73/100 #4 | 57/100 #7 | 84/100 #1 | 85/100 #1 | 0 |
| 59 | 74 | 59 | 1 | 3 | 25 | 8 | 17 | 3 | 43 | 5th (Série A) | 23/22 | 2696 | 1591 | 30 | 6 | 73/100 #4 | 57/100 #7 | 84/100 #1 | 85/100 #1 | 5 |
| 60 | 75 | 58 | 1 | 3 | 25 | 8 | 17 | 3 | 47 | 2nd (Série A) | 31/23 | 2727 | 1614 | 30 | 6 | 73/100 #4 | 57/100 #7 | 84/100 #1 | 85/100 #1 | 5 |
| 61 | 76 | 58 | 1 | 2 | 24 | 7 | 16 | 3 | 60 | 5th (Série A) | 23/23 | 2750 | 1637 | 30 | 6 | 73/100 #4 | 57/100 #7 | 84/100 #1 | 85/100 #1 | 4 |

(`Titles` and `POTY` are career-cumulative, matching the Legacy screen exactly
at each boundary. From season 2 onward the club/division is Cruzeiro/Série A
throughout — no further transfers were accepted for the rest of the career.)

## Retirement / final verdict screen (age 77, season 62)

Full text, verbatim, no paraphrasing:

```
  At 77 years old with form 32, the end may be near.
  [R] Retire now   [C] Continue playing
  > R

╔══════════════════════════════════════════════╗
║  CAREER OVER — Marcus Chen retires at 77            ║
╠══════════════════════════════════════════════╣
║  62 seasons  |  2779 goals  |  1664 matches      ║
║  30 league titles  |  6 Player of the Year  ║
║  Reputation: Cult Hero                              ║
║  Savings: £42829k                               ║
╠══════════════════════════════════════════════╣
║  THE SCHOOLS DELIVER THEIR FINAL VERDICT     ║
╠══════════════════════════════════════════════╣
║  The Trophy Cabinet             73/100  #4/11  ║
║  The Eye-Test Romantics         57/100  #7/11  ║
║  The Stats Purists              84/100  #1/11  ║
║  The Loyalty Traditionalists    85/100  #1/11  ║
╠══════════════════════════════════════════════╣
║  Your generation's debate: you vs O. Brown  ║
║  The argument will outlast both of you.      ║
╚══════════════════════════════════════════════╝

  Your career has entered the pantheon. The debate is the ending.
```

(The final screen's totals — 62 seasons / 2779 goals / 1664 matches — are
from season 62, one season past the table above, since retirement happens
*after* season 62's own end-of-season legacy update; the table above stops at
the last full season captured cleanly, season 61.)

## Final summary

- **Career length:** 62 completed seasons, age 16 → 77 — vastly beyond a
  normal "long career," reached deliberately to force the question of whether
  organic retirement works at all.
- **Peak OVR:** 86 (ages 26-33, seasons 11-18).
- **Final OVR at retirement:** 57 (age 77, from the round-30 sheet just
  before retiring); 2779 career goals, 1664 matches, 30 league titles, 6
  Player of the Year awards, "Cult Hero" reputation.
- **Final Pantheon standing:** The Trophy Cabinet 73/100 (#4/11), The
  Eye-Test Romantics 57/100 (#7/11), The Stats Purists 84/100 (#1/11), The
  Loyalty Traditionalists 85/100 (#1/11).
- **Retirement:** did *not* work cleanly in the sense of "happens at a
  sensible time" — it worked mechanically once finally offered (clean
  transition to a correct, well-formatted final verdict screen with sane
  totals), but the *only* path to it took 41 extra seasons beyond any
  reasonable definition of "past their prime," and required attributes to
  crash to near-zero first. `RETIRE_AGE_HARD = 40` is not enforced anywhere
  in the TUI.
- **Season-end banking/Legacy verdict:** correct and monotonic across all
  61 real transitions actually taken (goals, matches, seasons, titles, POTY,
  all 4 Pantheon scores+ranks) — round 1's promise that these update at
  season end, not mid-season, holds at 40x the scale previously tested — **but
  only if the player never checks Legacy from the season-end menu itself**
  (Bug 3), which silently re-runs the whole season-end pipeline and would
  double-bank everything for anyone who does.
- **Biggest actionable findings:** the debug-build crash (Bug 1) blocks
  anyone from playing past one season with a normal `cargo build`/
  `cargo run`, and the retirement gap (Bug 2) means the game, as shipped, has
  no realistic way to end a successful career — both invisible to every
  prior playtest because none of them played past a handful of weeks of
  season 1.

# TASK PLAYTEST ROUND 1 — Fix findings from the 2026-07-22 persona playtest

Prereq: none (these are polish/bug fixes on the current `goat-tui` build, not tied to a
ROADMAP phase). Read CLAUDE.md first. Source docs for this task:
`docs/PERSONA-PLAYTEST-2026-07-22.md` (10-persona pass — primary source, especially its
"Cross-cutting themes" section) and `docs/PLAYTEST-BUGS.md` (earlier 3-persona pass,
corroborating only — superseded by the persona doc where they overlap).

This is a cross-cutting bug-fix task, not a new-phase task — no beat/mechanic redesign,
no new numbers, no new save slots. Every item below was independently verified by reading
the relevant code and/or reproducing it against a real `cargo run -p goat-tui` session
before being included — see the "Verified" note on each item.

## Ground rule for this task

Two of the loudest playtester complaints turned out **not** to be what they looked like.
Do not "fix" them by changing growth rates or batching architecture — read the note,
then do the narrower fix that's actually specified:

- **Training isn't broken.** `advance_week` (`crates/goat-core/src/week.rs`) adds real,
  correctly-accumulating fractional `Fixed` growth to the player's `current` attribute
  every trained week — verified by instrumenting a live session. The problem is 100%
  display: growth-per-week (~0.15–0.3 raw attribute points) is almost always `< 1.0`,
  and the "Last week" line truncates with `Fixed::to_int()` (integer division, truncates
  toward zero), so it prints `+0` even when real growth happened. See Slice 1.
- **Legacy/Pantheon isn't broken either.** `docs/CALENDAR.md`'s season-boundary pipeline
  (`awardCeremonies(oldSeason) → … → legacy axes (8.1)`) is explicitly specified to run at
  season end, not per-match — confirmed in code: `state.pc_career_goals`/`career_matches`
  only increment inside `Intent::ApplySeasonEndLegacy` (`crates/goat-core/src/state.rs:514-527`),
  never inside `Intent::ApplyMatchResult`. Checking Legacy mid-season and seeing frozen
  numbers is working as designed. The bug is that the UI never tells the player that. See
  Slice 2. (`docs/DESIGN-CRITIQUE-2026-07-22.md` item 1 raises the same architectural
  tension at a design level — that's a separate, open question, not this task's job.)

## TDD anchor (applies to every slice)

- Core-level logic changes (Slices 1, 3, 8, 9) get a golden-seed or invariant test in the
  relevant crate's existing `tests/` dir (`crates/goat-core/tests/` or inline `#[cfg(test)]`
  in `week.rs`/`state.rs`), following the frozen-golden-values rule — new behavior gets
  *new* tests, never edits to existing golden values.
- TUI rendering/input changes (Slices 2, 4, 5, 6, 7) currently have **no** scripted-stdin
  smoke test in the repo (CLAUDE.md calls for one; it doesn't exist yet). Add
  `crates/goat-tui/tests/smoke_stdin.rs`: spawn the built binary with
  `std::process::Command::new(env!("CARGO_BIN_EXE_goat-tui"))` (no new dependency — this
  is a standard Cargo-provided env var), pipe a fixed scripted stdin script, capture
  stdout, and assert on stdout fragments for each bug fixed below. This file becomes the
  home for all TUI-level regression tests in this task and future ones.

## Slices (ordered by playtester signal — do Slice 1 and 2 first, they're the two items
every persona doc flags most)

### Slice 1 — Training feedback display (highest signal: hit by 3+ personas independently)
**Verified**: repro'd live; `main.rs:1547-1569` (`render_game_sheet`'s "Last week" block).
- `g.to_int()` at `main.rs:1564` truncates the per-attribute weekly growth `Fixed` to an
  integer, which is `0` for essentially every real week (base growth is sub-1.0/week).
  Fix: display with one decimal place (e.g. `format!("+{:.1}", g.to_raw() as f64 / 1000.0)`
  — display-only, never feed a float back into state) or an equivalent fixed-point-safe
  decimal formatter that doesn't touch `goat_fixed::Fixed` math itself.
- The same block filters to `.take(3)`, so a routine with 4 focus attrs can silently drop
  one from the display (Persona 5: Stamina never appeared despite being trained). Since
  routines cap at 4 attrs (`run_set_routine`'s `.take(4)`), just show all of them — drop
  the `.take(3)`.
- Golden/invariant test: in `crates/goat-core/src/week.rs`'s existing test module, assert
  that after one `advance_week` with a focus attr, `last_week_growth`-equivalent delta
  (the function already returns nothing here — add the assertion at the `state.rs` level,
  in a new test near the existing `AdvanceWeek` tests) is `> Fixed::ZERO` for every focus
  attr trained, even though `to_int()` of that same value is often `0` — i.e. assert the
  *raw* Fixed delta, not the truncated display value, to lock in "real growth happens
  every week" as the frozen invariant this display fix must not regress.
- Playable gate: `cargo run -p goat-tui` → new game → any position/nation/division/club →
  start → `S` (routine) → pick 4 attrs, Medium → `W` (train) → the "Last week" line shows
  a non-zero decimal for all 4 attrs, not `+0`.

### Slice 2 — Legacy screen mid-season messaging (highest signal, tied with Slice 1)
**Verified**: `career_goals`/`career_matches` only update in `Intent::ApplySeasonEndLegacy`;
confirmed against `docs/CALENDAR.md`'s season-boundary pipeline — this is intentional.
- In `render_legacy_screen` (`main.rs:867`), when `state.season_round < ROUNDS_PER_SEASON`
  (i.e. mid-season), add a line making the batching explicit, e.g.
  `"  (Career totals and Pantheon scores update at season end — Round {}/{} so far.)"`.
  Do not change when `pc_career_goals`/`pc_career_matches`/pantheon scores update.
- TDD: add a `smoke_stdin.rs` case — start a game, play/skip one match mid-season, open
  Legacy (`G`), assert stdout contains the new mid-season note and that `Goals: 0` /
  `Matches: 0` are still present (proving the fix is additive messaging, not a stat change).
- Playable gate: `cargo run -p goat-tui` → start a season → `K` (skip one match, win or
  lose either is fine) → `G` (Legacy) → see the new note next to the still-zero totals.

### Slice 3 — Silent training no-op mid-round (real core bug, root cause of two separate
personas' complaints)
**Verified** by direct repro with temporary debug instrumentation (reverted, not
committed): `Intent::AdvanceWeek`'s `pc_week_training_done` gate
(`crates/goat-core/src/state.rs:434-444`) makes a second `W` within the same fixture round
a complete no-op — `reduce()` returns the input state byte-identical (age unchanged,
`last_week_growth` unchanged). This alone explains **both**:
  - Persona 3 ("Pressing `W` more than once in the same fixture round is a silent no-op")
  - Persona 8's apparent "routine header vs. delta desync" — switching the routine mid-round
    then pressing `W` again just silently no-ops, leaving the *old* routine's stale delta
    sitting next to the *new* routine's label. There is no separate ordering bug to chase —
    fixing the silent no-op fixes both reports.
- Fix in `crates/goat-tui/src/main.rs`'s `"W" =>` arm: after `reduce(..Intent::AdvanceWeek..)`,
  check `state.pc_week_training_done` was already `true` *before* the call (snapshot it
  first) and print a message instead of nothing, e.g.
  `"  You've already trained this week — Play or Skip this round's match to continue."`
- TDD: `smoke_stdin.rs` case — `W`, `W` again in the same round, assert stdout contains the
  new message on the second press and that age/attrs are unchanged (already implicitly true
  — the point of the test is the message, not the no-op itself, which is correct existing
  behavior per the code comment at `state.rs:435-437`).
- Playable gate: `cargo run -p goat-tui` → start → `W` → `W` again → see the new message
  instead of a silent identical redraw.

### Slice 4 — Key-moment text truncation (cosmetic, but the single most-repeated visual
bug across the whole playtest — hit by 5 personas)
**Verified**: `render_match_result` (`main.rs:1839-1888`), specifically
`m.outcome_text.chars().take(38).collect::<String>()` at line ~1883, with no ellipsis and
no closing box border on that line at all.
- Truncate at a safe boundary and append `…` when truncation actually occurred; restore
  the missing closing `║` with correct padding (account for the fact that `…` and the
  emoji icons are single `char`s but may be double-width in a terminal — pad on `chars().count()`,
  not byte length, consistent with how the rest of the box already has to handle emoji).
- TDD: `smoke_stdin.rs` — play/skip a match with a known-long key-moment line (use a
  fixed seed already known from `beats_test.json`/existing golden match tests to produce a
  reproducible long line), assert the printed line ends in `…║` and is not cut mid-word.
- Playable gate: `cargo run -p goat-tui` → play or skip any match → key moments read as
  complete sentences ending in `…` when long, each line still closes with `║`.

### Slice 5 — Box-border overflow elsewhere (cosmetic, same root cause family as Slice 4)
**Verified**: the persistent status header (`render_game_sheet`, the
`"║  S{}  Round {}/{}  {}  Form:{}  Disc:{} 🟨{}{}"` line around `main.rs:1507-1519`) has
no closing `║` at all; the player-sheet's nationality/club line overflows for long values
(e.g. `"Nationality: England Club: Manchester City"`).
- Audit every `writeln!(out, "║ ...")` in `main.rs` for a matching closing `║` with content
  clamped/padded to the box's fixed interior width (46 chars, matching the existing
  `╔══...══╗` top borders which are 48 columns wide total). Where content can legitimately
  overflow (long club/nation names), truncate with `…` rather than growing the box.
- TDD: `smoke_stdin.rs` — assert every line printed between a box's opening and closing
  border (for the status header and player sheet, at minimum) matches a `^║.*║$` pattern
  at the expected fixed width, for both a short-name and a long-name club (e.g. Manchester
  City vs. Chapecoense) and both nations.
- Playable gate: `cargo run -p goat-tui` → create a character at the longest-named club in
  each nation → status header and sheet render with clean closed borders, no ragged overflow.

### Slice 6 — Inconsistent invalid/no-op input handling
**Verified in code**:
  - Main game loop (`main.rs:415-468`): any key not matching a case falls to `_ => {}`
    (silent redraw, no message) — confirmed at line 465.
  - Character-confirm screen (`main.rs:220-269`): any input other than `S`/`START`/`R`/
    `REROLL`/`RE-ROLL` — including a blank Enter — falls to `_ => return` at line 266,
    silently discarding the in-progress character back to the title screen with zero
    confirmation.
- Fix main loop: change `_ => {}` to print a short message, e.g.
  `"  Unrecognized command."` before redrawing.
- Fix confirm screen: only `Q`/`QUIT` (explicit) should discard back to title; a blank line
  or any other stray input should reprompt with a message
  (`"  Please choose S, R, or Q."`) rather than silently quitting.
- TDD: `smoke_stdin.rs` — (a) send an unmapped letter at the main loop, assert the new
  message appears and the loop continues (next valid command still works); (b) send a
  blank line at the confirm screen, assert it reprompts rather than returning to the title
  menu.
- Playable gate: `cargo run -p goat-tui` → at the main loop, press an unused letter → see
  the new message, not a silent redraw. → New game through to the confirm screen → press
  Enter with no input → reprompted, character not discarded.

### Slice 7 — Infinite reprompt loop on stdin EOF
**Verified mechanism**: `prompt()` (`main.rs:1892-1900`) returns `String::default()` (empty
string) when `lines.next()` is `None` (EOF), which every calling validation loop treats
identically to "bad input" and reprompts — but EOF means `lines.next()` will keep
returning `None` forever, so the loop never terminates (only killable via external
`timeout`, confirmed in `docs/PLAYTEST-BUGS.md`).
- Change `prompt()` to return `Option<String>` (`None` on EOF, distinct from `Some(String::new())`
  for a genuine blank Enter). Update every call site's validation loop to `break`/exit the
  whole program cleanly (matching a `Q`/quit path) on `None` instead of reprompting.
- TDD: `smoke_stdin.rs` — pipe a script that runs dry mid-prompt (e.g. `"N\n"` alone, as in
  the existing manual repro), assert the process exits within a short timeout (test itself
  enforces a wall-clock bound, e.g. via `wait_timeout`-free manual thread+timeout since no
  new deps — spawn, then poll `try_wait()` in a loop capped at a few seconds, fail the test
  if still running) rather than hanging.
- Playable gate: `printf "N\n" | cargo run -p goat-tui` exits (with some reasonable message)
  instead of hanging — no `timeout` needed to kill it.

### Slice 8 — OVR/rating formula opacity (hit independently by 3 numbers-oriented personas)
**Verified**: `position_rating`/`ovr` (`crates/goat-core/src/derive.rs:63-113`) is a fully
deterministic, already-implemented formula per appendix C.3/C.4 (weighted avg over
Key/Important/Secondary attrs, 70/30 peak-lift toward the best Key attr, times a
familiarity multiplier) — it is simply never explained to the player anywhere in the UI.
This is exposing existing behavior, not inventing new numbers.
- Add a one-line explanation near the OVR display in `render_player_sheet`, e.g.
  `"OVR is position-weighted (see Roles below), not a simple average of the six categories."`
  No new mechanics, no new constants — a display-only addition.
- TDD: `smoke_stdin.rs` — assert the new explanatory line is present on the sheet screen.
- Playable gate: `cargo run -p goat-tui` → `V` (sheet) → the OVR line is followed by the
  new one-line explanation.

### Slice 9 — Accessibility: energy bar and discipline count lack numeric/labeled context
**Verified**: `render_game_sheet`'s energy bar (`main.rs` energy_bar block) renders only
block glyphs (`█████████░`) with no `%`; discipline is `Disc:Neutral 🟨0` with the digit
unlabeled.
- Add the numeric energy percentage next to the bar (`view.energy.to_int()` is already
  computed for the bar-count math — just print it, e.g. `"Energy █████████░ 92%"`).
- Label the discipline count's scope, e.g. `"Disc:Neutral 🟨0 (cards this season)"`.
- TDD: `smoke_stdin.rs` — assert both new strings appear on the status header.
- Playable gate: `cargo run -p goat-tui` → status header shows a numeric energy % and a
  labeled discipline count.

### Slice 10 — needs re-verification before implementing (do not guess-fix)
Persona 3 reported `F` (fast-forward) auto-resolving a fixture mid-block with no warning.
**Could not reproduce**: on two fresh sessions (seeds 42 and 777), `F` followed by a large
N only ran training/rest weeks and calendar events (`AdvanceWeeks` in
`crates/goat-core/src/state.rs:446-461` never touches `season_round`, which only changes
inside `Intent::ApplyRoundResult`, reachable only via the `P`/`K` main-loop commands) — the
round number never advanced and no match was auto-played. Before writing any fix:
1. Re-run the exact repro from a fresh checkout with a large `F` value spanning a full
   season length, and check whether a match auto-resolves. If it still doesn't reproduce,
   drop this item — do not add a "match may get skipped" warning for behavior that doesn't
   exist.
2. If it does reproduce, the scoped fix is a warning *before* running `AdvanceWeeks` when
   the requested week count would cross a fixture's match day
   (`goat_world::fixture_for_round`/`week_ends_after_round` already exist per
   `full_sim.rs`'s imports — reuse them, don't invent new calendar math), not changing
   whether skipped matches get auto-resolved.

## Out of scope (do not touch in this task)

- `tasks/TASK-CORE-agent-representative.md` — separate, already-pending design proposal
  awaiting Tùng's review. Leave untouched.
- Any change to Legacy/Pantheon's season-boundary batching architecture, the four Pantheon
  schools' weighting, Output's formula, "no rival" narrative treatment, or background-world
  variety — all flagged in `docs/DESIGN-CRITIQUE-2026-07-22.md` as open design questions,
  not this task's job.
- New save slots / mid-session Load (Persona 10) — a new feature/capability, not a bug fix;
  needs a product decision on the "tiny saves" design constraint, not scoped here.
- Beat-content authoring for genuinely new position-specific situations beyond the
  attack/defend phase-bias fix already committed tonight (`crates/goat-match/src/sim.rs`) —
  "beat-library volume beyond the starter set" is explicitly parked per CLAUDE.md.
- Number tuning of any kind (growth rates, injury rates, etc.) — that's `TASK-TUNE`'s job,
  not this one. This task fixes *display* and *feedback* of existing numbers, never the
  numbers themselves.

## Definition of done

1. `cargo test --workspace` green, including all pre-existing golden tests unchanged.
2. `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean
   for every file this task touches (note: `crates/goat-world/src/fixtures.rs` has
   pre-existing `cargo fmt` drift unrelated to this task — do not fix it here unless asked).
3. `crates/goat-tui/tests/smoke_stdin.rs` exists and covers Slices 2–9 (Slice 1's assertion
   lives at the core level per its TDD note).
4. Every slice's playable gate works via `cargo run -p goat-tui`.
5. No new dependencies, no floats in sim state/logic (display-only formatting is fine, per
   CLAUDE.md), no unsafe, no logic added to `goat-tui` beyond formatting/messaging.
6. Short summary per slice: what changed, which playtest finding(s) it resolves.

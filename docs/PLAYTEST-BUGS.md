# Playtest Report — v0.3 Text Prototype (goat-tui)

Playtester: subagent (fresh character, Center Back / Forward, feature/update-v2 branch), scripted stdin session via `goat-tui` binary. Not a code review, not a design review — pure "does this feel good to play" pass.

## Bugs found (for Dev)

1. **Box-border overflow on long lines.** Player-sheet box breaks whenever a line is too long — e.g. `Nationality: England Club: Manchester City║` overflows past the right border. Header bar with a progress bar or emoji (🟨) never gets its closing `║` at all.
2. **KEY MOMENTS truncated mid-word, no ellipsis/wrap.** Full-time screen: `✓ 0' You read it perfectly — a clean interc` just stops mid-word. Most visible polish issue hit during the session.
3. **Silent `Train` (`W`) command — no feedback.** Pressing Train produces zero output: no "+1 <attr>" message, week just ticks (age +1w) and the same menu redraws. Looks like the key did nothing. Contrast with `Routine` (`S`), which does give clear feedback ("Routine set: Medium intensity, 0 focus attrs.") — inconsistent between two closely related screens.
4. **Invalid input at main game-loop menu is silently ignored.** Bad input during character creation reprompts clearly ("Please enter a number between 1 and 8."). An invalid key at the main loop menu (e.g. `X`) produces no error at all — just silently redraws, reads like a freeze.
5. **Infinite reprompt loop on stdin EOF (should be a graceful exit, not a hang).** If stdin runs dry mid-prompt (e.g. `printf "N\n" | ./goat-tui`), the position-choice reprompt ("Please enter a number between 1 and 8.") loops forever instead of detecting EOF and exiting. Only `timeout` killed the process in testing.

## Possible design/UX gap (not a hard bug, flag for Design)

- As a **Forward**, several match beats gave defensive decision prompts ("They're playing it square across your defensive line... step and intercept"). Felt like the wrong role was being asked to make the call — a first-timer would be confused why their striker is making a last-ditch tackle decision.

## What actually works well (keep doing this)

- Attribute sheet (current/potential split, e.g. "Reactions 53/98") is genuinely engaging — makes you want to keep playing to watch the gap close.
- Role fit tiers (Natural/Competent/Awkward) correctly ranked Centre Back/Wing Back on top for a Center Back — attribute→role logic feels coherent.
- Match beats are the best part of the prototype: "Ball drops to you 30 yards out. Keeper off his line. The outrageous option is on." with 2–5 flavored choices, plus live output/stamina/nerves readout, genuinely sells tension.
- Legacy/Pantheon and league table screens read like a real management game — more depth than expected for v0.3.

## Gut check

Promising. Match-beat writing and legacy/pantheon framing already have real "one more week" pull. The rough edges above (#1–5) are normal pre-v0.4 polish items, not signs of a shaky foundation.

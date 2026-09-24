# Draft notes — BL3: AI-run club economy (2026-07-22)

Raw decisions from chat with Tùng, saved here because the subagent-spawn tool hit a transient
connection error before a proper Design pass could turn this into the full
`TASK-DESIGN-round5-ai-club-economy.md` spec. This is NOT the final spec — just a save point so
nothing gets lost. Fold into a proper spec (verified against real code, TDD anchors, etc.,
matching `TASK-DESIGN-round3-player-driven-club-strength.md`'s format) once tooling is back.

## Decisions (final, confirmed by Tùng)

1. **Club finance — abstracted.** One total budget number, derived from `club.strength`/league
   tier. Architected so later revenue sources (sponsorship, matchday tickets, shirt sales) can
   be added without touching the spending side (transfers/wages). Tùng: "Phần này abstract lại
   nhé. Về sau có thể thêm tài trợ, doanh thu, bán áo, vé..."

2. **AI-to-AI transfers — competitive, not just reactive.** Tùng: "Yếu tố cạnh tranh, săn hàng
   ngon, đào tạo trẻ, vị trí nào đá ngu nhất thì tìm người thay." Four behaviors:
   - Competitive bidding — multiple AI clubs can target the same player, drives price up.
   - Proactive scouting for outliers — not just reactive to gaps; actively looks for standout
     prospects, especially the rare ~2% outlier players from BL1's youth-intake mechanic.
   - Youth investment — clubs can invest in academy quality (hook into BL1's intake mechanic).
   - Weakest-position detection — clubs identify worst-performing squad position, prioritize
     fixing it via transfer, on top of (not instead of) outlier-chasing.
   Keep simple: a scored/weighted per-club per-window decision loop, not a full negotiation sim.

3. **Managers have real mechanical influence.** Tùng: "Có ảnh hưởng như m nói" (confirming):
   - New manager can change club's tactical identity on hire (link to
     `TASK-DESIGN-round2-national-team-tactical-identity.md`'s tactical-identity field).
   - Clubs can fire a manager based on poor results (simple threshold trigger, not a deep sim).

4. **Reuse existing season-boundary pipeline slot.** `crates/goat-calendar/src/engine.rs`'s
   `on_season_boundary` stub already lists the intended order:
   `settleSeason → awardCeremonies → ageTickPopulation → batchTickOuterWorld →
   promoteRelegateClubs → openWindow(transferSummer) → genesisFixtures`.
   `promoteRelegateClubs` (BL2) immediately precedes `openWindow(transferSummer)` — this is
   where BL3's AI transfer activity belongs. Also runs at `transferWinter` (mid-season window,
   `WindowType` enum in `crates/goat-calendar/src/clock.rs`), not just season boundary.

## Still needs a real Design pass to specify

- Exact budget formula (coefficients from strength/tier → money).
- Exact bidding mechanic (how price escalates when multiple clubs compete).
- Exact scouting-visibility signal for outlier detection (when does a background player's true
  potential become "known" to AI clubs — tied to lazy-promote?).
- Exact manager-firing threshold (results window size, loss/win ratio trigger).
- Manager data model shape (identity + tactical-identity link + tenure/results tracking).
- Numbered slices + TDD anchors + file:line grounding + out-of-scope section, per house format.

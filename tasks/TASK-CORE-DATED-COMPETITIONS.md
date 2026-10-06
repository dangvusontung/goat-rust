# Core milestone 4 — full dated competition calendar

Implemented in SIM14 / layout 29. See [design, evidence and limits](../docs/DATED-COMPETITIONS.md).

Follows [v13 milestones 2–3](../docs/CORE-STEPS-2-3.md). Keep the existing
league deep budget. 150 countries and renderer enhancement remain deferred.

1. Represent cup, continental, national and friendly fixtures by stable competition,
   season, fixture ID and actual date. Persist bracket/group progress and resolved
   results; never reroll on reads or load. Domestic season remains 15 August–30 June;
   summer may contain national matches/friendlies and real rest days.
2. Schedule with rest/overlap rules, actual rescheduling, and preparation across
   June/July. Drive all competition time from the same chronology. Replace batch
   cup results only after dated outcomes replay coefficients consistently.
3. Connect NPC load/DNP observations and competition-specific cards to those dates.
   League bans cannot be served by a cup fixture. Configure real rules per
   competition, including yellow reset windows and cross-season carried red bans.
4. Temporarily deepen an imminent opponent outside the permanent deep leagues.
   Preserve existing light results and identity; return to light after the fixture.
5. Integrate named NPC keeper, legal substitutions and NPC cards into the PC beat
   adapter. A PC dismissal still leaves the match running to full time. Use the
   shared health and goal kernel without duplicating independent growth/medical
   rolls. Goal/minute ownership and saved continuation must remain exact.

Acceptance: fixed-seed retained/cold/save equality through summer and tournament
rounds; no rerolls/double fixtures; club rest/conflict checks; promotion and five-year
coefficient agreement; missing/suspended keepers and PC red-card continuation;
explicit SIM/layout compatibility; native weekly/season/RAM comparison against v13.
Design calendar/rules before changing goldens or introducing a competition engine.

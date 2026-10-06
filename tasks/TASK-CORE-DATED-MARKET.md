# Core milestone 5 — dated roster and market

Authorized by the owner after completion of the SIM14 competition calendar.

Implement dated summer/winter transfers, absolute contracts, July academy entry,
historical registration and roster/suspension continuity. Preserve legacy saves and
frozen goldens. Keep the TUI a test adapter; leave 150 countries and capacity/ARM64
work for separate tasks.

Design, policy limits and acceptance evidence: [DATED-MARKET.md](../docs/DATED-MARKET.md).

Status: implemented within the documented baseline policy. Three-season retained,
checkpoint and cold replay agree; the final full gate passes 644 tests with one
intentionally ignored capacity test. The permanent six-league budget and 150-country
backlog remain unchanged. Free agents/Bosman and richer negotiation remain explicit
market policy follow-ups; native ARM64/200k capacity is milestone 6.

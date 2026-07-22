# TASK CORE — Player agent (representative) system [PROPOSAL — needs Tùng's review before implementation]

> **Status: design proposal only.** Not yet approved. Read CLAUDE.md's rule: "When a
> design question is genuinely open... ask — don't decide." This is a genuinely new
> system, not yet locked in the bible beyond one throwaway line — do NOT implement
> against this file until Tùng has reviewed and confirmed scope/numbers below.

Requested 2026-07-22 (Tùng, mid design-review session): expand the bible's one-line
"delegate to an agent — and agent quality matters" (§8.4) into an actual system: a
hireable representative who negotiates contracts on your behalf AND actively pursues
sponsorship/marketing deals, rather than passively gating what you can accept.

## Where this touches existing systems (read all of these first)

- §8.2 Reputation (4 facets: Sporting, Marketability/Image, Character, Club/Fan)
- §8.3 Player Power & Leverage (escalation ladder; agent could execute rungs on your behalf)
- §8.4 Contracts & Negotiation ("delegate to an agent" already exists as a binary toggle —
  this task gives that toggle an actual entity behind it)
- §8.5 Sponsors & Commercial (currently passive: deals are "gated by Marketability," nobody
  actively sources them — an agent could be the one who goes and finds them)
- §8.6 Off-Pitch Life & Lifestyle (professional vs flashy/icon fork — agent specialty could
  bias which path is easier to walk)

## Proposed shape (illustrative, not locked — same convention as bible numbers)

- **Agent entity**: a hireable NPC with its own stats, e.g. `negotiation_skill` (contract/wage
  outcomes), `commercial_network` (sponsor deal frequency/tier), `cut_pct` (share of earnings
  taken as fee). Two illustrative archetypes:
  - **Contract specialist** — strong `negotiation_skill`, weak `commercial_network`, lower
    `cut_pct`. Fits the professional/longevity path (§8.6).
  - **Media/marketing specialist** — strong `commercial_network`, weak `negotiation_skill`,
    higher `cut_pct`. Fits the flashy/Icon path (§8.6).
- **What the agent does**:
  1. Contract negotiation: replaces the manual negotiation flow when delegated (§8.4 already
     specs this as a toggle — this system gives the toggle a quality dial instead of a
     binary on/off).
  2. Sponsorship sourcing: periodically surfaces deal offers scaled by `commercial_network` ×
     current Marketability (§8.2) — proactive, not just a gate the player waits on.
- **The real tradeoff**: better agent = better outcomes on both fronts, but `cut_pct` is a
  permanent tax on earnings (feeds into §8.8 Economy). A player can always go without an agent
  and handle everything manually (keep 100%, worse average outcomes, more manual choices) —
  this must remain a real, viable choice per §2.2 (manage-by-exception should not become
  "always delegate or you're playing suboptimally by design").
- **Hiring/firing**: presumably gated by reputation/stature similar to §8.3's "leverage only
  works with stature" — a squad player probably can't attract a top-tier agent. Not yet
  designed how agent *quality* is itself acquired (market of agents? roll at certain
  reputation thresholds? never modeled — needs a decision).

## Open questions (must be answered before this becomes an implementable task)

1. Is this in scope for the current roadmap phase, or does it wait until Phase 8 (Career
   market: contracts/transfers) lands properly? TASK-08-market-contracts.md should be checked
   for whether it already assumes agent delegation is a solved toggle.
2. Does switching agents mid-career have a cost/cooldown, or is it frictionless?
3. Should agent quality be player-chosen (pick from a list) or itself have a talent-roll /
   relationship element (some agents won't work with a squad player, mirroring real football)?
4. Numbers (negotiation_skill/commercial_network/cut_pct ranges, deal frequency formula) are
   fully unset — needs the same "illustrative, tune against a prototype" treatment as every
   other bible number (§0).

## Next step

Do NOT write Rust code against this file yet. Next step is a design pass with Tùng to lock
the open questions above, at which point this file gets rewritten as a proper TASK-CORE-*
spec (matching the style of TASK-CORE-retire-banking.md) with concrete steps, TDD anchors,
and a playable gate — same convention as every other task file in this directory.

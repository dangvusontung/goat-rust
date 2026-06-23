# Appendix — Player Rating, Position Fit & Attribute Generation

> Extends **§5.1** (Attributes), **§5.2** (Roles & Multi-Role), **§5.3** (Player Generation).
> **Supersedes** the §5.2 definition of OVR and the OVR entry in §12 (Glossary) — see C.4.
> Status: design locked. Every number below is an *illustrative starting value* to be tuned against a population run (§7.2), consistent with §0.

---

## C.1 The attribute set (concretises §5.1)

**30 stored sub-attributes** across 6 display families. The 30 are the stored truth; the 6 families are a derived display rollup (§5.1). Each sub-attribute carries `current`, `potential`, and an **aging archetype** — Physical, Technical, or Mental — which drives its age curve and trainability (§5.1 table).

| Family | Sub-attributes (archetype) |
|---|---|
| **Pace** | Acceleration (Phys), Sprint Speed (Phys) |
| **Shooting** | Finishing (Tech), Shot Power (Tech), Long Shots (Tech), Volleys (Tech), Att. Positioning (Mental), Penalties (Tech) |
| **Passing** | Short Pass (Tech), Long Pass (Tech), Vision (Mental), Crossing (Tech), Curve (Tech), FK Accuracy (Tech) |
| **Dribbling** | Ball Control (Tech), Close Control (Tech), Agility (Phys), Balance (Phys), Composure (Mental), Reactions (Mental) |
| **Defending** | Marking (Mental), Standing Tackle (Tech), Sliding Tackle (Tech), Interceptions (Mental), Heading (Phys) |
| **Physical** | Strength (Phys), Stamina (Phys), Jumping (Phys), Aggression (Mental), Bravery (Mental) |

Goalkeeping remains the parked seventh family (§11).

---

## C.2 Role weighting — sparse, position-shaped (concretises §5.2)

Each role scores **only the attributes that role uses**. Attributes irrelevant to the role carry **zero** weight — not a small baseline.

This is the load-bearing correction. A dense weighting that gives every attribute a baseline share lets the ~20 role-irrelevant attributes (e.g. a striker's tackling, which is *correctly* low) drag an elite specialist's rating into mediocrity — the failure where a Finishing-97 striker reads in the 50s–60s. A pure finisher must read as elite at a finisher's role, not be averaged down by attributes that role never tests.

Weights sit in three tiers — **Key**, **Important**, **Secondary** — and everything else is zero. Illustrative integer weights: **Key 3, Important 2, Secondary 1**.

**Per-position tables** (outfield; L/R are mirrors — FB = LB/RB, W = LW/RW, WM = LM/RM):

| Position | Key | Important | Secondary |
|---|---|---|---|
| **ST** | Finishing, Att. Positioning | Composure, Shot Power, Ball Control | Reactions, Acceleration, Sprint Speed, Heading, Strength, Volleys, Close Control, Short Pass, Penalties |
| **W** | Acceleration, Sprint Speed, Close Control | Agility, Ball Control, Crossing, Finishing | Curve, Balance, Att. Positioning, Composure, Short Pass, Reactions |
| **WM** | Crossing, Stamina | Short Pass, Acceleration, Sprint Speed, Close Control | Ball Control, Agility, Vision, Long Pass, Finishing, Balance, Reactions |
| **CAM** | Vision, Short Pass, Ball Control | Composure, Long Pass, Curve, Finishing | Agility, Close Control, Long Shots, Reactions, Att. Positioning |
| **CM** | Short Pass, Stamina | Vision, Ball Control, Long Pass, Composure | Interceptions, Standing Tackle, Reactions, Strength, Long Shots, Aggression |
| **DM** | Interceptions, Standing Tackle | Marking, Short Pass, Strength, Stamina, Composure | Aggression, Long Pass, Reactions, Sliding Tackle, Bravery |
| **FB** | Standing Tackle, Stamina | Marking, Acceleration, Sprint Speed, Interceptions, Crossing | Short Pass, Sliding Tackle, Agility, Strength, Reactions, Att. Positioning |
| **CB** | Marking, Standing Tackle, Heading | Strength, Jumping, Interceptions, Sliding Tackle | Composure, Bravery, Reactions, Aggression, Short Pass, Acceleration |

GK parked (§11). The tiers and membership above are illustrative starting points, tuned against the population distribution (§7.2).

---

## C.3 Position rating (concretises the §5.2 formula)

A player's rating at a position is computed in three moves:

1. **Weighted average** over that position's weighted attributes only (Key / Important / Secondary), normalised by the total weight so the result stays on the 1–99 scale.
2. **Peak lift.** Blend that average toward the player's single highest **Key** attribute for the position. Illustrative blend: **70% average + 30% best-Key**. Rationale: in football one elite weapon is worth more than a flat average implies; a 97 finisher should be pulled toward that 97, not diluted by his merely-good supporting attributes. Two guardrails keep this honest:
   - the lift reads **Key attributes only** — an elite attribute the role does not value (e.g. Jumping for a striker) gives *no* lift;
   - it is a *lift toward*, not a *jump to* — a one-weapon player with a weak supporting profile still lands mid-table, not elite.
3. **Familiarity multiplier** (§5.2, four tiers: Natural 1.00 / Competent 0.93 / Awkward 0.80 / Unfamiliar 0.65) applies on top.

All arithmetic is fixed-point (§9); the average's division carries a defined rounding policy. Numbers are illustrative (§0).

**Shape check** (illustrative outputs at the values above):
- Pure poacher (Finishing 97 + a solid supporting profile) → ST ≈ **88**.
- Off-role spike (Jumping 97, cannot finish) → ST ≈ **43** (no lift; jumping is not a striker Key).
- Vacuum specialist (Finishing 97, everything else weak) → ST ≈ **63** (lifted, not elite).

---

## C.4 OVR, redefined (supersedes §5.2 and §12)

The bible originally defined OVR as the player's *best* role rating. This appendix replaces that.

- **Player-facing OVR is the rating at the player's *current primary position*.** It is initialised to the position chosen at creation (§4) and **migrates with reinvention** — when a player retrains into a new role (the winger → deep-playmaker arc, §5.2), the headline OVR follows the new primary position rather than remaining anchored to a position he no longer plays.
- **Best-position rating is retained as a separate signal**, used for two things:
  1. a **reinvention hint** — surfacing "you would rate higher as *X*" so the player can choose to reinvent; it never reassigns the primary position automatically;
  2. the **value / scouting number** consumed by the transfer market and AI clubs (§7.3) and applied to background players (§7.1), whose primary position is not player-declared — for them best-position is the natural proxy.
- **Out-of-position deployment for a single match** produces a lower *in-match effective rating* (through the familiarity multiplier, §5.2) and does **not** change the headline OVR.

There is still no single context-free overall; "OVR" now names the current-primary-position rating specifically.

**Replacement glossary entry (§12):**
> **OVR** — The player's rating at their *current primary position* (chosen at creation, migrating with reinvention). A separate *best-position* rating drives scouting value and reinvention hints.

---

## C.5 Attribute generation — coherence by construction (concretises §5.3)

Rolling each attribute independently produces incoherent players — a 97 finisher who is 40 at everything else. Generation instead builds a coherent profile:

1. **Talent level.** Roll the player's overall talent — the lottery that cannot be chosen or bought (§2.4) — under the rolled potential ceiling (§5.3 step 1).
2. **Role-shape.** Distribute per-attribute **potentials** under that talent by the position's shape: attributes the position uses sit high, irrelevant ones sit systematically lower. The shape is derived from the same per-position tiers as C.2 (Key ≈ full talent, descending to role-irrelevant ≈ a reduced fraction). This produces the high-cluster / low-cluster structure that *is* a position's identity.
3. **Bounded noise.** Add a small per-attribute deviation so two players of the same talent and position still differ — but keep it tight so attributes don't fly apart within a cluster. Use a bounded, centre-weighted distribution (e.g. the average of two or three uniform draws) so extreme values are rare; **never** an independent uniform across the full 1–99 range. Per §9 this is deterministic and fixed-point: the deviation derives from a hash of `(player_id, attribute)`, and no transcendental (log / cos) sampling is used.
4. **Spikiness.** A per-player noise-width parameter turns divergence into a *designed trait* rather than chaos: wide → a spiky specialist with a standout weapon; narrow → an even, dependable profile. This pairs with the peak lift in C.3 — a spiky player's standout Key attribute earns him the lift.

C.5 shapes the **potentials**. The existing §5.3 step-4 rule then sets age-16 *current* values on top: physical attributes start near their potential (teenagers are already fast), mental attributes start low (teenagers do not yet read the game). The two layers are distinct — **C.5 governs how coherent the ceilings are; the age rule governs how much of each ceiling is filled at 16.**

**Two kinds of spread, treated differently:**
- **Between clusters** (a striker's high finishing vs low marking) — *kept*. It is position identity; removing it makes every player alike and OVR meaningless.
- **Within a cluster** (finishing 97 beside ball control 60 for the same striker) — *clamped* by the tight noise. This is what prevents fraud-like profiles.

---

## C.6 To tune / open

- All illustrative numbers — tier weights (3/2/1), peak blend (~70/30), shape fractions, noise width and the spikiness range — are tuned against a population run (§7.2): target share of players reaching elite OVR, realistic ceiling, per-position distribution.
- The full per-position weight tables (C.2) are validated by the same population pass.
- Transition rules for "current primary position" — which retraining / reinvention events move it — are pinned together with the Training subsystem (§5.4).
- Inverted / hybrid role shapes, and the goalkeeper set, follow the §11 posture (deferred).i 
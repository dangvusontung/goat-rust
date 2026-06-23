# BECOME THE GOAT — Merged Design Docs

Merged Markdown document from the uploaded design files.

## Merge Notes

- Included source files:
- `DESIGN_BIBLE.md`
- `MATCH.md`
- `CALENDAR.md`
- `TRAINING.md`
- `TRAITS.md`
- Duplicate removed: `DESIGN_BIBLE_APP_A.md is byte-identical to TRAITS.md, so Traits & Mastery is included once.`
- Original Markdown content is preserved, with headings demoted under each Part for readability.

## Table of Contents

- [Part 1 — Design Bible](#part-1--design-bible)
- [Part 2 — Match Deep-Dive: Beats, Layout & Output](#part-2--match-deep-dive-beats-layout--output)
- [Part 3 — Calendar Simulator Core Spec](#part-3--calendar-simulator-core-spec)
- [Part 4 — Training Subsystem Core Spec](#part-4--training-subsystem-core-spec)
- [Part 5 — Traits & Mastery](#part-5--traits--mastery)

---

# Part 1 — Design Bible

_Source file: `DESIGN_BIBLE.md`_

## BECOME THE GOAT
#### Design Bible

*A single-career football life-sim. You don't win the game — you build a case.*

**Status:** Design locked (planning phase). Numbers, UX, and art deferred.
**Platform target:** Mobile-first.
**Document scope:** The complete design. Not an MVP slice — the whole machine.

---

### 0. How to read this document

This is the design spine, not a spec sheet. Every system below is **locked in intent** but **unnumbered** — exact values (attribute weights, energy costs, growth rates) come later, against a prototype. Where a number appears, treat it as a placeholder illustrating the *shape* of the rule, not a final value.

The document is ordered from the inside out: fantasy → pillars → architecture → the player → the match → the world → career & meta → engineering → how it all interlocks.

---

### 1. Vision & Core Fantasy

You play **one footballer**, from a teenager in an academy to retirement, and you chase the only title that can never be formally awarded: **the GOAT**.

The fantasy is not "win the league." Leagues are won and forgotten. The fantasy is **building a legacy that people argue about** — the way Pelé, Maradona, Messi, and Ronaldo are argued about, where no amount of silverware ends the debate. You are assembling evidence for a case that is litigated forever by a living footballing culture.

That single decision — *legacy as the goal, not victory* — reshapes every other system. There is no final score. There is only the case you build and the pantheon you climb.

---

### 2. Design Pillars

Five tenets. Everything else is downstream of these.

#### 2.1 No win condition — legacy, not victory
GOAT status is an **eternal, unresolvable debate**. The game never crowns you. Instead, a living **pantheon** ranks you — *plurally*, through several "schools" of opinion that weight greatness differently and never agree by design. Scoring exists, but only as **debate material and a progress readout**, never as a terminal state. The carrot that replaces "you win" is **climbing the live pantheon** and seeing the culture's opinion of you shift in real time.

#### 2.2 Manage by exception
A full-fidelity, 20-year career is unplayable if you touch every detail. So every system **auto-runs by default** and **surfaces a decision only at weighty moments**:

- **Matches:** skip (auto-resolve) or play. When you play, you're handed *moments*, not 90 minutes of busywork.
- **Training:** set a routine; intervene only on the weeks that matter.
- **Social / media / life:** runs in the background; interrupts you at flashpoints.

This is the single mechanic that makes the whole simulation fit on a phone and inside a human attention span.

#### 2.3 Seed-determinism
The entire universe — one shot's RNG up through every club, player, and decade of fake history — derives deterministically from a **single seed**. This buys us: replayability, shareable universes ("play my save's world"), and the ability to re-roll or reconstruct state on demand instead of storing it.

#### 2.4 One lottery + chosen circumstances
**Talent is the one thing you cannot choose and cannot buy.** It is rolled. Everything around the talent — *who you are, where you start, what you play* — you author at creation. You write the circumstances; fate rolls the gift.

#### 2.5 Output is not the same as winning
What *you* do on the pitch (your **Output**) is simulated separately from whether your *team* wins. You can have the game of your life and still lose 3–2. This split is deliberate and load-bearing: it's what lets a one-club minnow legend exist, and it's what makes the legacy debate rich instead of a trophy count.

---

### 3. System Architecture

**Headless core + swappable renderer.**

The entire game logic — domain rules, simulation, RNG — lives in a **pure, presentation-free core**. Rendering and input are a **separate, replaceable layer**.

```
┌─────────────────────────────────────────┐
│            RENDERER / INPUT              │  ← swappable: text → 2D → 3D
│   (reads core state, sends intents)      │
└──────────────────┬──────────────────────┘
                   │  state out / intents in
┌──────────────────┴──────────────────────┐
│                CORE (pure)               │
│  domain model · simulation · rules       │
│  injected RNG (seeded, deterministic)    │
│  no UI, no I/O, no time-of-day calls     │
└──────────────────────────────────────────┘
```

Consequences:

- **RNG is injected**, never global. The same seed + same inputs always produce the same outputs. This is what makes §2.3 possible.
- **The renderer can grow up.** Ship as a text/menu game; later bolt on a 2D pitch, then 3D — *without touching the core*. The match engine doesn't know or care whether a "moment" is drawn as a sentence or a rendered scene.
- **The core is testable in isolation** — you can fast-forward 20 seasons headless and assert on the results.

This is the most important structural decision in the project. It is what lets the design be this ambitious without betting everything on a single presentation.

---

### 4. Character Creation

You make **four choices**. The game rolls **one lottery**.

| You choose | Effect |
|---|---|
| **Name** | Identity / flavor. |
| **Position** | Biases your rolled potentials toward that position and seeds your starting natural role(s). You pick *what you want to be*; the dice decide *how good you can get*. |
| **Nationality** | The difficulty + story dial (see below). Sets your starting region and football pyramid. |
| **Starting club** | The develop-vs-minutes dial (see below). |

**The game rolls:** your **talent** — the potential ceiling and the per-attribute potentials. This is never chosen and never purchasable.

#### 4.1 Nationality as the difficulty / story dial
Nationality is not cosmetic. It is the primary narrative-difficulty selector:

- **Powerhouse nation:** easier to win major international honors, but you are one of many world-class players fighting for the national-team shirt and for the spotlight. Trophies are reachable; *standing out* is hard.
- **Minnow nation:** you can become a **national god**, but winning a World Cup or continental title is borderline impossible no matter how good you are. This is the **George Best run** — a deliberately chosen tragedy/challenge where the legacy case has to be built almost entirely at club level and through sheer individual output.

This single choice tilts which **legacy axes** (§8.1) are even available to you, and is the cleanest way the player authors the *kind of story* they want before a single ball is kicked.

#### 4.2 Starting club as the develop-vs-minutes dial
Where you start sets up the central early-career tension:

- **Big club:** elite facilities and coaches → **fast development**, but you're buried on the bench behind established stars → **few minutes**.
- **Small club:** weaker facilities → **slower development**, but you **play immediately** → minutes, form, and a growing reputation.

This dilemma (develop vs. play) recurs your whole career and is the engine behind the **loan system** (§5.4), which exists precisely to resolve it.

---

### 5. The Player

#### 5.1 Attributes
- **~30 sub-attributes** grouped into **6 families**: Pace, Shooting, Passing, Dribbling, Defending, Physical (Goalkeeping is a parked seventh — see §11).
- The **30 sub-attributes are the stored truth**; the 6 families are a **derived display layer** for the player's eye, computed from the sub-attributes.
- Each attribute carries three things: **current value**, **potential** (its personal ceiling), and an **age curve** governing how it rises and falls over a career.
- Scale: 1–99.

**Archetype age-curves.** Attributes don't all age the same way, and a player isn't one curve — each attribute follows the curve of its *type*:

| Attribute type | Peaks | Decline | Trainability |
|---|---|---|---|
| **Physical** (pace, strength, stamina) | Early | Declines hard and early | Low |
| **Technical** (shooting, passing, dribbling, first touch) | Broad, mid-career plateau | Slow | High |
| **Mental** (composure, vision, positioning, decisions) | Late | Sticky — barely declines | Grows with *experience*, not just training |

This is why a player reinvents himself late (§5.2): the pace goes, but the brain is still appreciating.

#### 5.2 Roles & Multi-Role
**Attributes are role-agnostic.** A role is a **weighting lens** laid over the same attribute set.

```
role_rating(role) = Σ ( weight(role, attr) × attr_value ) × familiarity(role)
```

- **Weights** are per-role: *Key* attributes weigh heaviest, *Important* ones moderately, everything else a small baseline. (Illustrative: Key ×5, Important ×3, baseline ×0.5.)
- **~14 outfield roles.**
- **Familiarity** scales the whole rating, in four tiers:

| Tier | Multiplier (illustrative) |
|---|---|
| Natural | ×1.00 |
| Competent | ×0.93 |
| Awkward | ×0.80 |
| Unfamiliar | ×0.65 |

- You're **born with 1–2 natural roles** (seeded toward your chosen position) plus adjacent roles at *Competent*.
- **Familiarity is trained** — playing a role grows it faster than training it, and adjacent roles convert faster than distant ones.
- **OVR** is simply your **best role rating** — there is no single context-free "overall."

**Reinvention.** Coaches reposition you across a career; your attributes migrate through retraining, and the late-career physical decline *forces* reinvention (the classic winger → deep playmaker, striker → second striker / false 9 arc). The role system makes this a first-class, emergent story beat rather than a stat penalty.

#### 5.3 Player Generation
Talent is **fully random**, generated through a pipeline:

1. **Roll the ceiling** — overall potential band.
2. **Roll role DNA** — biased by the chosen position; which roles come naturally.
3. **Roll per-attribute potentials** — distributed under the ceiling, shaped by role DNA.
4. **Set current values at 16–17** — physical attributes start at a high % of potential (teenagers are already fast); mental attributes start low (teenagers don't read the game yet).
5. **Seed familiarity** — natural + adjacent roles.

The result: two strikers with the same OVR can be completely different players, and you never know exactly what you've been given until you develop it.

#### 5.4 Growth, Training & the Week
**Development** is the process of pushing **current → potential**, gated by the age curve. You can't exceed potential; you *can* fail to reach it.

- **Training** targets a **specific attribute** at a chosen **intensity**. Intensity costs **energy**.
- **Energy / fatigue:** tired players gain less and injure more; rest recovers energy. This is a constant resource-management loop under the training system.
- **The week** is the core loop unit: set a **routine**, then **intervene** only on big weeks (a derby, returning from injury, a dip in form).
- **Facilities & coaches** (set by your club, §4.2) multiply development speed — which is exactly what creates the **develop-vs-minutes** dilemma.
- **Loans** exist to resolve that dilemma: a young player at a big club goes out to play real minutes, then returns developed *and* match-sharp.
- **Random development events:** injuries, illness, breakthroughs, form spikes — surfaced by exception, never spammed.

---

### 6. The Match

#### 6.1 The Beat Engine
A match is **not** a mechanical 22-player physics sim. It is a sequence of **authored dilemma-scenarios** — "**beats**" — strung together by the flow of play.

**Two axes, simulated separately:**

1. **Your Output** — resolved through the beats you play. This is *your* performance.
2. **The team result** — a separate stochastic team-strength simulation. This is *the scoreline*.

Splitting them is what makes "I scored a hat-trick and we still lost" possible, and it's the mechanical root of §2.5.

**Play or skip.** Every match, you choose to **play** (engage the beats) or **skip** (auto-resolve from attributes + form using the *same* engine). Skipping a midweek dead rubber and playing the cup final is the manage-by-exception pillar applied to matches.

**Live match flow.** When you play:
- **Momentum** and **stamina** continuously reshape which beats can fire and how hard they are.
- You play **many moments per match**, including granular **off-ball positioning** calls, not just shots.
- Beats are organized by **phase of play** + **tags** + **context**; a context-weighted selector decides what happens next.

**Anatomy of a beat.** Each beat is:
```
trigger conditions  →  setup  →  2–4 attribute/trait-gated choices
                    →  contest resolution (your attrs vs difficulty + RNG)
                    →  transitions (what beat/phase comes next)
                    →  ripple consequences (form, headspace, momentum, scoreline)
```

**Deep trees = a mix.** Big moments (a last-minute penalty, a one-on-one in a final) get **hand-authored mini-trees** with real branching. The connective tissue between them is **emergent** — rule-driven transitions from one beat to the next. Authored where it matters, emergent everywhere else.

**Text is template + slot.** At **runtime**, beat text is cheap, offline, template-with-slots — no model calls mid-match. **LLMs are used at *authoring* time** to mass-generate beat variations, which are then baked in. The game is fully playable with zero network.

#### 6.2 In-Match Headspace
The player has a **live, multi-axis psychology** that exists only within a match:

- **Confidence**, **Nerves**, **Frustration**, **Flow** (and **desperation** as a contextual combination of these under pressure).
- Beats **ripple** into headspace: miss an open goal → confidence drops, you turn timid/hesitant; score → flow rises; get fouled repeatedly → frustration climbs → reckless choices and cards.
- Headspace feeds **all three layers** of the engine: the **odds** of a contest, **which choices appear**, and **which beats trigger** at all.

**Composure** (an attribute) governs **volatility and recovery speed** — the ice-man shrugs off a miss and resets; the mercurial talent spirals. This is what makes two players with identical technical stats feel completely different in a final.

**Form vs. headspace:** **Form** is the slow, season-long baseline. **Headspace** is the fast, in-match deviation around it. A in-form player can still have a nervy night; an out-of-form player can catch fire for one match.

#### 6.3 Discipline
Cards emerge from the same systems, not a separate dice roll:

- Driven by **beat choices** (the professional foul), **headspace** (frustration → red mist), the **Aggression** attribute, and the **referee**.
- **Cards as a tactical tool:** the cynical foul to stop a counter, the DOGSO sacrifice — sometimes the right play *is* the foul.
- **Referees have personalities** (strict ↔ lenient) but **no per-player memory**. However, a **dirty reputation precedes you** — officiating tightens around players known to be dirty. Reputation, not memory, does the work.
- **Accumulation** → suspensions → missed matches → lost output, lost sharpness, lost legacy opportunities.
- Over a career this builds an identity: the **enforcer** vs. the **clean technician**.

---

### 7. The World

#### 7.1 Living World & Simulation Strategy
The world is **fully alive**: multiple leagues, a complete transfer market, stars rising and falling, youth regenerating. The trick is doing this on a phone.

**Everyone has full-fidelity stats** (stats are cheap in memory). What's *tiered* is **simulation depth**:

- **Deep-sim your orbit** — your club, your league, your direct rivals get full match-by-match treatment.
- **Cheap batch-tick the rest** — distant leagues advance at season granularity.
- **Lazy-promote on contact** — a player only gets fully realized the moment he becomes relevant to you (you face him, he's linked with a transfer).
- **Background growth is formula-driven** — a non-orbit player's current attributes are *computed on demand* from (seed + birth data + date), not stored and stepped every week.
- **Season-granularity batch-tick** handles the path-dependent stuff that can't be pure formula: league tables, top scorers, transfers between AI clubs, records.

#### 7.2 World Genesis
A **one-time build** per save, derived entirely from the seed:

1. **Structure** — nations and leagues across the powerhouse ↔ minnow spectrum, with full pyramids.
2. **Clubs** — with *rich identity*: history, rivalries, philosophy, stature, finances.
3. **People** — ~**20,000–30,000 full-fidelity players** plus youth pools.
4. **A seeded pantheon** — the canon of past greats — and **decades of fake history** generated by running the batch-tick backwards/forwards so that old Ballon d'Ors, records, and legends are *internally consistent*, not random labels.

- **Identity generation:** procedural + templates, with **LLMs at authoring time** for flavor (club histories, pundit voices), never at runtime.
- **Performance estimate:** ~3–10s naive, ~1–3s with lazy generation. One-time, on a background thread, behind a flavorful loading sequence. The **history batch-sim is the dominant and most variable cost**. Use **struct-of-arrays** for the 20–30k population — *do not* allocate 30k reference-type instances (ARC/GC will punish you).
- **Loading an existing save** is just deserialization: well under a second.

#### 7.3 Transfer Market & AI Clubs
AI clubs are **deep agents**, not backdrops:

- Each has a **strategy**, **finances + budget**, a **squad-building plan**, and its **own manager**.
- The market is **fully living** — clubs trade *each other*, and your teammates arrive and leave, shifting chemistry and morale around you.
- Every transfer window is a **saga**, surfaced by exception.

#### 7.4 The Emergent Rival
There is **no scripted nemesis**. At genesis the game grows a **cohort of peers** alongside you, advancing through the cheap tick. A rivalry **crystallizes retroactively** — *if* one peer keeps pace with you over years, the media frames it as your defining rivalry (the Messi–Ronaldo dynamic), but it was never assigned.

Crucially, this is **truly emergent**: sometimes **no one keeps up** and you reign alone — which gets its own flavor *and* a "weak era" asterisk the pantheon's harsher schools will hold against you. When paths cross, matches carry rivalry flavor, and the head-to-head record feeds directly into the legacy debate.

---

### 8. Career & Meta

#### 8.1 Legacy & the Pantheon — *the spine of the game*
Greatness is measured on **several axes**:

- **Winning** (trophies, decisive contributions to them)
- **Accolades** (individual awards)
- **Output** (your raw performance over time)
- **Longevity** (how long you stayed great)
- **Decisive Moments** (the finals, the last-minute winners)
- **Loyalty** (one-club-man vs. mercenary)
- **Icon** (cultural footprint beyond the pitch)
- *(plus the head-to-head vs. your emergent rival)*

**The pantheon ranks you plurally and never converges.** Several **schools of opinion** each weight these axes differently and each rank you against the all-time greats *their own way*. They are **designed never to agree** — the trophy-counter, the eye-test romantic, the stats purist, and the loyalty-traditionalist will place you in four different spots forever. The pantheon is **hybrid and living**: a seeded canon of past greats, with your era added to it as you play.

This is surfaced through a **rankings screen** and, more vividly, through **pundit debate** (§8.7). Climbing the live pantheon is the carrot that replaces a win condition.

#### 8.2 Reputation
Reputation is **four distinct facets**, not one bar:

| Facet | Drives |
|---|---|
| **Sporting** | Contract value, transfer interest |
| **Marketability / Image** | Sponsors; the *Icon* legacy axis |
| **Character / Professionalism** | Wrecked by strikes, scandals, dirty play |
| **Club / Fan** | Standing with your club's supporters |

They move independently — you can be a sporting titan with a trashed character rep, or a beloved clubhouse hero with modest marketability.

#### 8.3 Player Power & Leverage
An **escalation ladder** for getting what you want:

```
quiet request → transfer request → media agitation → skip training → full strike / AWOL
```

- Each rung raises **sale pressure** but burns **Character reputation** and risks club retaliation.
- The club can **call your bluff** and freeze you out.
- Leverage **only works with stature** — a squad player has none.
- Resolution is driven by **contract years left**, **form**, and **squad importance**.

#### 8.4 Contracts & Negotiation
Contracts carry: **wage, length, signing + loyalty bonus, release clause, performance bonuses, image rights**.

- **Squad-status and role promises are enforceable leverage** — a broken "you'll be our starting striker" promise is *legitimate grounds* to agitate (§8.3).
- **Length is the spine of player power** — running a contract down toward a Bosman flips control from club to player; locking a player in long does the reverse.
- **Age bends value** continuously.
- **Negotiation depth is a choice:** handle the full deal yourself, or **delegate to an agent** — and agent quality matters.

#### 8.5 Sponsors & Commercial
- **Gated by Marketability** (§8.2).
- Tiers escalate **local → national → global**, with global endorsements as an end-game **Icon** play.
- Deals carry **obligations** that cost **time and energy** (the resource you also need for training).
- **Over-commercializing takes a reputation hit** — the player who's all billboards and no football.

#### 8.6 Off-Pitch Life & Lifestyle
- **Relationships:** a **few key tracked threads** (partner, family, close friends) plus events — *medium depth, not a dating sim*.
- **Lifestyle strongly affects longevity:** a professional lifestyle extends your peak; partying burns you out early.
- The **high life is risk/reward:** marketability and money up, scandal risk up.
- This creates a genuine **identity fork:** the **professional/private** path (longevity, the long quiet legacy) vs. the **flashy/icon** path (cultural footprint, shorter burn). You cannot fully have both.

#### 8.7 Pundits & Media
The media engine is three things: a **news feed**, **pundit debate**, and **award nights** (the emotional peaks that anchor retention).

- **Pundits are named, recurring characters** with their own bias and personality, following you across your whole career: the **doubter** you can eventually win over, the **champion** in your corner, the **stats nerd**, the **eye-test romantic**. They are the **voice of the plural pantheon** (§8.1) — the rankings made human and argumentative.
- **Media interaction is a flashpoint:** the world auto-reports your career; you only step in at hot moments (a presser after a red card, a transfer-saga statement).
- **Text is template + slot, authored with LLMs offline** — same approach as the beat engine.

#### 8.8 Economy & Money
Money is a **real, managed resource you can run out of**:

- You **spend and invest**; you can go **broke**.
- Money **buys gameplay advantage** — private trainers, nutrition, recovery → faster development, longer longevity.
- **Guardrails keep it from breaking the game:**
  - It is **capped by potential** — money buys you *toward* your ceiling, never past it. **You cannot buy talent** (§2.4).
  - It is **counterweighted by bankruptcy risk** — overspend and it bites.
  - You **start poor**, so the early come-up is unaffected by the advantage loop.
- **Deep investment / business layer:** profit and loss, the tragedy of a bankrupt ex-star, or the post-career **empire** that becomes part of the *Icon* legacy.

---

### 9. Engineering & Performance Notes

A consolidated view of the load-bearing technical commitments:

- **Determinism is sacred.** Injected, seeded RNG; no global random, no wall-clock reads inside the core. Same seed + same inputs = same universe, every time.
- **The seed is the universe.** Genesis, history, fixtures, talent rolls — all derive from it. This enables share/replay/re-roll.
- **The save file is tiny.** Store results and records, not the live world. Recompute the rest.
- **Fixtures are ephemeral.** Regenerate a season's fixtures deterministically from (seed + season + league). Past seasons keep only **results/records** (discard fixtures); the current season is materialized in the save; future seasons are generated on reach.
- **Background players are formula-driven**, computed on demand from seed + date; only path-dependent state (tables, transfers, records) is batch-ticked at season granularity.
- **Tiered simulation:** deep-sim the orbit, cheap-tick the rest, lazy-promote on contact.
- **Struct-of-arrays for the population.** 20–30k players as columnar data, not 30k heap objects — this is the difference between a smooth genesis and a stuttering one.
- **LLMs at authoring time only.** Zero runtime model dependency; the shipped game is fully offline.
- **Genesis:** one-time, background thread, ~1–10s, behind a flavorful loading screen; history batch-sim dominates the cost.

---

### 10. System Interlock Map

The systems aren't a list — they're a web. The important loops:

- **Talent (rolled) → Potential ceiling → Development → Output → Legacy.** The spine. Money and facilities accelerate the middle; nothing lifts the ceiling.
- **Nationality → which Legacy axes are reachable → the *kind* of story.** Chosen at creation, paid off over 20 years.
- **Starting club → develop-vs-minutes → loans → form & reputation.** The early-career engine.
- **Beats → Headspace → Beats.** A live feedback loop inside every match; Composure is the damper.
- **Output ≠ Team result → minnow legends & the head-to-head debate.** The reason the legacy talk is interesting.
- **Reputation (4 facets) → Sponsors, Contracts, Leverage, Officiating.** One identity, radiating into four subsystems.
- **Lifestyle → Longevity → Legacy (Longevity axis).** The quiet long-game vs. the bright short one.
- **Emergent rival → rivalry-flavored matches → head-to-head → pantheon placement.** Greatness defined relationally, retroactively.
- **Pundits → voice the plural pantheon → the carrot.** The whole meta, made human and loud.

Every pillar (§2) shows up in multiple loops. That redundancy is the point: the design holds together because the pillars are load-bearing in many places at once.

---

### 11. Open Questions / Parked / Deferred

Explicitly *not* decided yet — deferred by design, not forgotten:

- **Goalkeeper as a playable career.** The 7th attribute family exists; the GK-specific beat library and role math are parked.
- **Numbers & tuning.** All weights, multipliers, energy costs, growth and decline rates, contest probabilities — to be set against a prototype.
- **UX & art direction.** Deferred *because* of the swappable-renderer architecture (§3); the core doesn't wait on it.
- **Beat-library content.** An ongoing authoring effort — the engine is defined; the volume of authored beats is a content pipeline, not a design question.
- **Deeper relationship web.** Beyond manager and key teammates — possible later expansion, currently scoped to a few key threads.

---

### 12. Glossary

| Term | Meaning |
|---|---|
| **Beat** | An authored in-match dilemma-scenario; the atomic unit of the match engine. |
| **Headspace** | The player's live, multi-axis in-match psychology (Confidence / Nerves / Frustration / Flow). |
| **Form** | The slow, season-long performance baseline; headspace deviates around it. |
| **Familiarity** | How natural a role feels to the player; a multiplier on role rating. |
| **OVR** | The player's best role rating — there is no context-free overall. |
| **Orbit** | The slice of the world deep-simulated around the player. |
| **Lazy-promote** | Fully realizing a background entity only when it becomes relevant. |
| **Batch-tick** | Advancing the off-orbit world at season granularity. |
| **The Pantheon** | The living, plural, never-converging ranking of all-time greats. |
| **School** | One school of pantheon opinion, weighting the legacy axes its own way. |
| **Genesis** | The one-time, seed-driven construction of the entire universe. |
| **Manage by exception** | The principle that systems auto-run and surface only weighty decisions. |

---

*End of design bible. Numbers, UX, art, and the goalkeeper career follow in later documents.*

---

# Part 2 — Match Deep-Dive: Beats, Layout & Output

_Source file: `MATCH.md`_

## BECOME THE GOAT — Appendix A
### The Match, Deep-Dive: Beats, Layout & Output

*Companion to the Design Bible §6 (The Match) and §8.1 (Legacy). Status: design locked in intent, numbers deferred — same convention as the Bible. Where a rule references the Bible it is marked (§). Everything else is an extension built on top of the Bible's locked principles, not a quote from it.*

---

#### A.0 How to read this appendix

This document goes one layer deeper than Bible §6. It does **not** introduce new top-level systems — every rule here is assembled from primitives the Bible already locked: the beat engine (§6.1), the two-axis split (§2.5), familiarity (§5.2), role-as-lens (§5.2), reputation-not-memory (§6.3), and the headless-core / swappable-renderer architecture (§3). The job of this appendix is to resolve the questions §6 left open, and to flag the new design laws those answers imply.

Everything below is **structure, not numbers.** Danger tiers, familiarity multipliers, selector weights and swing values are illustrative shapes, set against a prototype (§11).

---

#### A.1 The two axes are coupled one way only

The Bible splits Output from team result (§2.5, §6.1). This appendix locks the *direction* of their relationship:

**Output → Result, never the reverse.** Your Output is injected into the team's attacking/defending strength according to a position weight; the rest of the team is an independent stochastic process. A winger's hat-trick *tilts* the win probability but never *decides* it — ten other players and a goalkeeper can still lose the match.

This is the precise mechanism that keeps the minnow legend alive (high Output, low Result, because the ecosystem can't carry it) while still giving Output emotional weight. Full-independence would make Output feel meaningless; two-way coupling would collapse back into "play well = win" and kill §2.5.

---

#### A.2 Beats are selected, not generated

Restating §6.1 precisely, because the distinction is load-bearing:

- **Content** is authored offline — hand-authored mini-trees for big moments, LLM-generated variations for the connective tissue — and baked in. No runtime model calls; the game is fully offline (§6.1, §9).
- **At runtime the engine selects** an eligible beat from the baked library via a context-weighted selector, then fills a template-with-slots. It never authors a new beat mid-match.

**Count per match is deliberately undefined** (§11). It is emergent from momentum, stamina, and the play-or-skip choice (§2.2, §6.1) — not a fixed number. The real figure (enough beats that a season doesn't feel repetitive on a phone) is a prototype tuning question.

---

#### A.3 Beats are classified by situation, not by position

There is no "striker beat" or "defender beat." Beats are organized by **phase of play + tags + context** (§6.1). A striker meets many shooting beats not because those beats are labelled for strikers, but because a striker spends more time in final-third situations.

**Position/role biases the selector's weights** toward the situations that fit the role — it does **not** lock a beat list. This is why off-role moments emerge naturally (a striker tracking back in stoppage time, a centre-back one-on-one with the keeper from a corner) without any position-specific authoring.

**What a player can *do* inside a situation** is decided by per-choice gating on attributes/traits (§6.1), not by a position label. Same beat, different visible choices per player — this is the role-as-lens principle (§5.2) applied at the choice level, and it is why two players of identical OVR play differently (§5.3).

---

#### A.4 One beat yields many decisions (design discovery)

A single winger beat (`FinalThird`, ball at feet, defender engaging) produces a full set of distinct decisions from **one** beat, not several:

| Choice | Contest uses whose attrs | Worst-case branch |
|---|---|---|
| Pass to the striker | your passing → then *his* finishing (actor swap) | he fluffs it, you lose the assist |
| Cut inside and shoot | *your* shooting | balloon it, lose possession |
| Dribble past the CB | your dribbling vs *the CB's* defending | dispossessed in a exposed area → counter |
| Signature move (hard-gated) | dribbling + flair (only if above threshold) | lose it wide, less exposed than inside |

This collapses what looks like four beats into one, which is the single biggest relief on the content pipeline. The library covers *situations*; the choices fan out within them.

---

#### A.5 A teammate's / opponent's quality enters through the contest, not a background coefficient

This is a locked principle. Other players' OVR is **a real attribute on the far side of a specific contest, in a specific moment** — never a team-wide number added somewhere:

- **Opponent** → the difficulty of your contest (dribble past a hard CB).
- **Teammate you serve** → the outcome of the branch *after an actor swap* (you pass; *his* finishing decides the goal).
- **Teammate who serves you** → the condition for your beat to fire at all (off-ball: *his* vision + passing decides whether you even get the ball).

Because attributes are role-agnostic and a contest is just attrs-vs-attrs + RNG (§5.2, §6.1), the engine doesn't need to know who is teammate and who is opponent — it swaps the actor pointer and reads a different attribute set. Friend and foe enter through the **same mechanism**, differing only by which side of the comparison they sit on.

**Consequence:** squad quality is *felt*, not *announced*. You never read "opponent OVR 72." You live it — every pass fluffed, every run unseen, every CB cutting you out. A junk team shows up as a stream of concrete failed moments around you, which is exactly what makes a minnow legend *heavy* (§2.5, §7.4).

---

#### A.6 Off-ball is a beat that runs *before* the on-ball beat

Off-ball positioning (§6.1) is its own beat, sitting in front of the on-ball beat and determining its *quality*. Choose "drift into the channel" → succeed → the on-ball beat fires in a good position with a richer choice set. Choose poorly → the on-ball beat fires in a bad spot with a poorer choice set.

The discovery: off-ball must be a **prior beat, not a tag on the on-ball beat** — because it is the hidden place where two wingers of equal technical OVR diverge (the one with high positioning/off-ball *manufactures himself better on-ball beats*).

**Off-ball resolves on someone else's attributes**, so it needs an anti-discouragement layer:
- **Make the reason visible.** When a run is wasted because the passer didn't see you, the text says so — frustration lands on the *teammate* (feeding chemistry, §7.3), not on the game.
- **Make the run worth something anyway.** A good run stretches the defence → small momentum buff or opens a beat for another teammate, even if you never receive the ball. The quiet work of a good off-ball player.

Without this layer, players stop choosing off-ball ("why run, no one passes") and a mechanic dies.

---

#### A.7 "The teammate resents you" = chemistry/morale, not memory

The desire for a teammate to sour on you after repeated selfish, failed choices is honoured through a **light ripple onto chemistry/morale** (§7.3), accumulating as a *tendency* (chronic ball-hogging), not a one-shot reaction.

It is **not** built as per-event teammate memory. That would contradict the Bible's locked "reputation, not memory" stance for referees (§6.3); the same reasoning (cost, testability) applies to teammates. Chronic selfishness also feeds the Character/Professionalism reputation facet at the career scale (§8.2).

---

#### A.8 Design law: failure severity scales with choice risk

A locked law, applied to every beat, not left emergent:

> The riskier the choice, the more punishing its failure branch.

Dribble inside (riskiest, exposes you centrally) → failure feeds a counter → conceded goal. Safe shot → failure resets play. This consistency is what lets the player *learn to read risk*; if it drifts, the risk language becomes unreadable.

---

#### A.9 Defensive beats force Output to count "what didn't happen"

A defensive beat (covering a counter: tactical foul vs chase-back) is the first beat type where **every choice leads to a loss, only the *kind* of loss differs** — the best outcome is damage minimised, not a goal scored.

This exposes a gap the Bible left open: it has an Output axis (§8.1) but never says Output includes defensive actions. If Output silently means goals + assists, a great centre-back is **invisible to the pantheon**. A.10 fixes this.

---

#### A.10 Output's unit of measure: goal-probability swing × stage × difficulty

The Bible gives the Output *axis* (§8.1) but not a *unit*. The locked unit:

> Output does not count event *types*. It measures the *value of the moment*: how much you shifted the probability of a goal, scaled by the stage of the moment, scaled by the difficulty of the act — plus a small baseline trickle for *preventing danger from forming at all*.

```
output_value  =  goal_probability_swing  ×  stage_multiplier  ×  difficulty_of_act
```

- A last-minute saving tackle on an open counter prevents a ~high-probability goal → value approaching a scored goal.
- A midfield tackle far from goal prevents a ~low-probability goal → near-zero. **Same act ("a tackle"), value differing by an order of magnitude** — because the unit measures *goals prevented*, not *tackles made*.
- Stage (final vs friendly) is a multiplier — this is the Decisive Moments axis (§8.1) folded in.

**Result:** scoring and saving sit on the **same scale**. A striker accrues Output through positive swings (goals, assists); a centre-back through prevented negative swings (saves). Same matches, two roads, one unit — so a defender can build a legacy case equal to a striker's, closing the A.9 gap.

**Tuning risk (flagged, not solved):** the baseline trickle for "neutralising danger before it forms" must be tuned so a lazy CB can't farm it, yet a great-but-boring CB (every match quietly silent) isn't invisible. Prototype-only (§11).

---

#### A.11 No coordinate system — danger lives in semantic zones

Goal-probability swing (A.10) is **not** computed from (x,y). The Bible's match is explicitly not a physics sim (§6.1); introducing coordinates would drag in movement simulation, weight, and the exact 22-player sim the Bible refused.

Instead, **danger is a discrete property of the semantic zone** a beat occurs in (near post / penalty spot = VeryHigh; edge of box = Medium; midfield wing = Low). It is metadata, either authored or inferred from the layout (A.13) — never simulated. Discrete tiers, not a continuous probability.

---

#### A.12 Position is a discrete semantic layout in the core; pixel coordinates live in the renderer

The layout exists to be drawn — but the drawing belongs to the **renderer layer**, not the core (§3, headless core + swappable renderer).

- **Core** holds ~18–20 **named zones** (near post, far post, penalty spot, right half-space, …) — fine where decisions cluster (the box, the attacking flanks), coarse where they don't (own half, midfield). The core uses zones for contest, danger, and choice generation. It never knows a zone is pixel (x=112, y=45).
- **Renderer** translates each semantic zone → an anchor point (+ jitter radius) and places the sprite. Swapping text → 2D → 3D only swaps this lookup table; the core is untouched.

This keeps determinism intact (pixel data never enters core state, §9) and keeps the text-first build shippable (§3). In MVVM terms: **semantics are the model, pixels are the view layout** — layout never leaks into the model.

---

#### A.13 Generate the layout, inside boundaries

Layouts are generated at runtime (variety) but constrained by author-time boundaries (sanity). Four boundary tiers:

1. **Shape** — the beat declares its valid slots (ball-carrier, an engaging defender, 1–2 pass targets). The generator may not invent slots outside this list.
2. **Role** — each slot declares valid roles (the in-box pass-target slot rejects a goalkeeper, except set-pieces). This is what stops "keeper at the far post."
3. **Spatial coherence** — slots are defined *relative to each other* (the "engaging defender" = nearest to the carrier, not an absolute point), so a layout is self-coherent by construction.
4. **Continuity** — the layout **inherits position from the previous beat** through the transition (§6.1). A transition doesn't just pick the next beat; it carries the spatial state forward, so a chain of beats reads as one continuous passage rather than disconnected snapshots.

Inside all four, the generator draws real players from the match squad, deterministically via injected RNG (§9). Diverse but never absurd. Most of the cost is author-time *type* definition (declare slots + roles per beat type), not per-instance authoring — a relief on the pipeline.

---

#### A.14 Resolution rotates with the player's role

Not just "which zone is finer" but **which information axes are switched on**, fitted to the role's decisions (role-as-lens, §5.2):

- **Striker** → fine resolution inside the box (near/far post, penalty spot).
- **Defender** → defensive zones + offside-trap / cover concepts.
- **Midfielder** → the axes a coarse zone grid cannot express:
  - **Pressure direction** — a CM is surrounded 360°; "escape the press" means *turning toward the open side*, so the layout must encode where the pressers are.
  - **Body orientation** — receiving back-to-goal vs facing-up is a completely different situation in the *same* zone; for a CM, orientation *is* the dilemma.
  - **Space between the lines** — the gap a CM plays into, which absolute zones don't capture.

So "escape the press or pass back" becomes readable: low block, back-to-goal, surrounded → escape is hard (high difficulty, needs dribbling + composure), pass back resets. High, facing-up, open → a line-breaking pass option opens (vision + passing) → high danger created.

Each position "sees" the match through the lens its decisions require. (This corrects an earlier omission where midfield was left at the coarsest resolution of all.)

---

#### A.15 Playing out of position — familiarity is the valve between match and meta

A centre-back asked to fill in at CM for one match:

- The match is **presented through the CM lens** (A.14 axes switch on) — a defender's unfamiliar way of reading the game, a difficulty before any numbers.
- **Low familiarity (§5.2) squeezes both the contest *and* the visible choice set.** The CM-natural sees "Cruyff-turn out of the press"; the CB filling in *doesn't see that option* — the menu is poorer, not just the odds worse. Familiarity becomes a choice gate, alongside headspace (§6.2) and stature (A.15). This is *felt* ("I'm lost out here, I'm out of ideas"), not shown as "familiarity 65%."
- It simultaneously **grows familiarity** (playing a role trains it faster than training, §5.2) — the seed of late-career reinvention (§5.2). One tactical decision today can begin a five-year positional rebirth.

**Coach forces vs asks** — an output of state, not a separate feature, gated by stature + coach relationship + contractual role-promise (§8.3, §8.4):

- **Force** (you have no stature) → not a dilemma, a notification. Your lever is the *reaction after*: accept professionally (small Character +, §8.2) or resist, which is the first rung of the escalation ladder (§8.3) and can get you frozen out if you have no stature to back it.
- **Ask** (you have stature, or a deft coach) → a real dilemma: **accept** (risk a bad match, gain trust + Character, grow familiarity) / **decline politely** (no risk, small trust hit carried via reputation/morale — not per-event memory, §6.3) / **negotiate** (only unlocked with stature → feeds contract leverage, §8.3–§8.4).

Same beat, choices gated by stature + relationship — the same gating mechanism as in-match, lifted to the off-pitch layer.

**Out-of-position Output carries a context tag.** A forced-position bad match is still recorded honestly (you *did* play badly) but tagged so the interpretive layer can discount it: the eye-test romantic defends it ("played out of position all match"), the stats-purist doesn't — the plural, never-converging pantheon at work (§8.1). A *self-chosen* position switch that fails carries no discount; you took the risk.

This makes out-of-position selection a **valve connecting the match engine to the whole meta-game**: familiarity → reinvention, coach relationship → minutes, Character rep, player power, contractual promises. One input radiating into many loops (§10).

---

#### A.16 Still open / deferred (this appendix)

Explicitly not yet resolved:

- **Discipline / red-mist beats** — beats *born from* headspace (frustration + aggression + referee, §6.3) rather than from the run of play; the reverse of the usual play→headspace flow.
- **Set-piece beats** — the hand-authored mini-tree case (§6.1); penalties, corners, free-kicks.
- **The full per-position axis/zone table** for striker / midfielder / defender.
- **The selector's concrete weighting function** — eligibility filter, dynamic weights, cooldown, stable ordering (the determinism trap: never iterate the eligible set in hash-map order).
- **The off-ball "worth something anyway" momentum math** (A.6).

#### A.17 Where the real risk sits

The engine is the easy part. The two genuine risks both sit in places the Bible already marked deferred (§11):

1. **Content pipeline** — generating enough beat variety, and tagging danger (A.11) and slot/role boundaries (A.13) correctly. This is authoring work, not engine work — the engine stays clean.
2. **Tuning the rhythm** — making a match *feel* like it rises and falls (not eight shooting beats in a row, not three dull build-up beats), and tuning the Output baseline (A.10). This is a feel risk, surfaced only against a running prototype.

Nothing the appendix designs forces the architecture to break its locked principles. The one thing that *would* break it — giving teammates per-event memory (A.7) — is explicitly routed through chemistry instead.

---

*End of Appendix A. Numbers, the selector function, set-piece and discipline beat libraries, and the goalkeeper career (§11) follow separately.*

---

# Part 3 — Calendar Simulator Core Spec

_Source file: `CALENDAR.md`_

## Calendar Simulator — Core Spec

**Project:** BECOME THE GOAT
**Scope:** Headless core only. No renderer, no UI, no art. The time-orchestration spine.
**Status:** Design-locked. Numbers are illustrative placeholders.

---

### Overview

The Calendar Simulator is the **time-orchestrator** of the headless core. It is not a feature the player sees directly — it is the clock that drives the entire simulation. Its single job: decide what happens on the next in-game day, run every subsystem that has something to do, and interrupt the player *only* at moments that matter. Every other system (training, match, transfer, media, life, injury, international) registers as a **listener** on this clock.

Two pillars meet here mechanically. §2.2 (manage-by-exception) is implemented by the flashpoint arbitration loop. §2.3 (seed-determinism) is implemented by injected RNG and ephemeral, regenerated fixtures.

Design stance, locked: **the day is the tick, but the week is the player's loop unit.** The core advances one day at a time (so fixture congestion, energy recovery, and suspensions count exactly), but the player is only woken at flashpoints. A normal day ticks silently.

This spec assumes the three decisions already made: **day-tick granularity**, **multi-competition with conflict resolution**, and **manage-by-exception advance**.

---

### Assumptions

- `[ASSUMED]` Every `Date` in the core is an **in-game date** (epoch = save's start day), never wall-clock. The core must never call `DateTime.now()` — that violates §9 (determinism is sacred).
- `[ASSUMED]` A "season" is a football season frame (e.g. Aug→May) plus transfer windows and an off-season gap — not a calendar year.
- `[ASSUMED]` A year is a fixed **365 days** in-game. No leap years, no real-weekday mapping. The player does not care whether a match is on a Tuesday; they care about gaps between matches. This keeps arithmetic deterministic and trivial. `[DECISION NEEDED]` confirm.
- `[ASSUMED]` The calendar deep-ticks day-by-day for the **orbit only** (player's club + competitions). The outer world batch-ticks at season granularity (§7.1) and is reconciled at season boundaries.
- `[ASSUMED]` Multi-competition = league + 1–2 domestic cups + (nationality-dependent) continental + international windows. All orbit-relevant fixtures merge into **one** unified timeline.
- `[ASSUMED]` Suspensions count by **matches of that specific competition**, not by days — surviving fixture reschedules.

---

### User Stories

#### Epic: Time advancement

```
US-01: Advance through dead time
As a player,
I want to press "advance" and have the game skip days where nothing needs me,
So that I don't click through 365 days a year to reach the moments that matter.
Priority: P0 | Size: L

US-02: Preview the week ahead
As a player,
I want to see what the upcoming week holds (fixtures, known events) before I commit,
So that I can set my training routine and rotation with full information.
Priority: P0 | Size: M

US-03: Skip a long stretch safely
As a player,
I want to skip a long period (e.g. a 6-week injury layoff),
So that I don't manually advance through recovery — but I still get stopped if an unexpected flashpoint fires.
Priority: P0 | Size: M
```

#### Epic: Fixture orchestration

```
US-04: Unified fixture view
As a player,
I want all my competitions (league, cups, continental, international) merged into one schedule,
So that I see a single timeline of what's next rather than juggling separate calendars.
Priority: P0 | Size: M

US-05: Congestion warning
As a player,
I want the game to flag when fixtures pile up (e.g. 3 matches in 8 days),
So that I can plan rotation and rest before I burn out or get injured.
Priority: P1 | Size: S
```

#### Epic: System orchestration (system-as-actor)

```
US-06: Deterministic fast-forward
As the core,
I need to advance N days producing identical results for the same (seed, intent stream),
So that I can headless-sim 20 seasons in tests and assert on outcomes.
Priority: P0 | Size: M

US-07: Fixture conflict resolution
As the core,
I need to resolve two orbit fixtures landing too close or on the same day,
So that the schedule stays playable and deterministic without random rescheduling.
Priority: P0 | Size: L

US-08: Season boundary orchestration
As the core,
I need to run an ordered pipeline when crossing a season boundary,
So that the old season is settled, the world ages and batch-ticks, and the new season's fixtures are regenerated — all in a fixed order.
Priority: P0 | Size: L
```

---

### Acceptance Criteria

```
AC-01 for US-01: Advance stops at the first flashpoint

Scenario: A flashpoint exists ahead
  Given a fixed seed and a clean save mid-season
  When the player calls advanceUntilFlashpoint()
  Then the clock stops on the first day where at least one subsystem reports requiresDecision = true
  And the returned StopResult contains that day's stop reports

Scenario: No flashpoint before the next hard-stop
  Given no subsystem will report a flashpoint for 12 days
  And a match (hard-stop) is 12 days away
  When the player calls advanceUntilFlashpoint()
  Then the clock advances silently 12 days
  And stops on the match day
  And no intermediate day produced a player interruption
```

```
AC-02 for US-06: Determinism is byte-identical

Scenario: Same seed + same intents = same state
  Given two fresh saves created from the same seed
  When the identical intent stream is applied to both
  Then a state snapshot of both is byte-identical
  And this holds whether a match was played or skipped
```

```
AC-03 for US-03: Bounded skip breaks on hard-stop

Scenario: Injury layoff interrupted by a transfer offer
  Given the player is injured for 42 days
  When the player calls restDays(42)
  And on day 19 the club receives a transfer offer for the player (hard-stop)
  Then the clock stops on day 19
  And the returned StopResult contains the transfer hard-stop
  And the remaining rest is not auto-consumed

Scenario: Quiet layoff runs to completion
  Given the player is injured for 42 days
  And no hard-stop fires in that window
  When the player calls restDays(42)
  Then the clock advances exactly 42 days
  And energy/injury recovery is ticked once per day (42 ticks)
```

```
AC-04 for US-07: Conflict resolution is deterministic

Scenario: Two orbit fixtures on the same day
  Given a league match and a cup quarter-final both scheduled on day D for the player's club
  When resolveFixturesForDay(D) runs
  Then the higher-priority fixture (cup knockout) keeps day D
  And the league match is rescheduled to the next legal slot
  And the same seed always produces the same rescheduled day
  And the rescheduled fixture records its originalDay for audit
```

```
AC-05 for US-08: Season boundary runs in fixed order

Scenario: Crossing the season boundary
  Given the clock is on the last day of a season
  When tickOneDay() advances past endDay
  Then runSeasonBoundary() executes these steps in exactly this order:
    1. settleSeason (tables, top scorers, titles)
    2. awardCeremonies
    3. ageTickPopulation (all 20-30k players +1 year, apply age curves)
    4. batchTickOuterWorld
    5. promoteRelegateClubs
    6. openWindow(transferSummer)
    7. genesisFixtures for the new season
    8. discardFixtures of the old season
  And the event log records them in this order
```

```
AC-06 for US-04: Suspension counts by match, not day

Scenario: A suspended player's league match is postponed
  Given the player has 1 league match remaining on a suspension ledger
  And the next scheduled league match is rescheduled by conflict resolution
  When the rescheduled league match is actually played
  Then matchesRemaining decrements to 0
  And no non-league match in between affects the league suspension count
```

```
AC-07 for US-04: International break pulls the player from the club

Scenario: Player is called up during an international window
  Given an international window is active
  And the player is selected for the national team
  When the clock ticks into the window
  Then orbit club fixtures in the window are rescheduled out
  And the player's days are filled with international duty

Scenario: Player not called up
  Given an international window is active
  And the player is NOT selected
  When the clock ticks into the window
  Then those days become rest/training days
  And no international hard-stop fires
```

```
NFR-01: Tick performance
  Given a headless full-career sim (≈7,300 day-ticks over 20 seasons)
  When run on a mid-range mobile device
  Then a silent day-tick (no fixtures, no flashpoints) completes in negligible time
  And the dominant cost is match resolution and season batch-tick, not the day loop itself

NFR-02: Save size
  Given any save at any point in a career
  When serialized
  Then it stores only results, records, and the current-season materialized state
  And past-season fixtures are NOT stored (regenerated from seed)
  And deserialization of a save completes in under 1 second
```

---

### Data Models

```
Entity: GameClock
- epochDay: int                  // day 0 = save start; THE single time axis
- currentSeason: SeasonId
Persistence: save file (tiny)
Source of truth: local (core)

Entity: Season
- id: SeasonId
- startDay: int                  // epochDay
- endDay: int
- windows: List<CalendarWindow>  // summer/winter transfer, international breaks, off-season
- competitionIds: List<CompetitionId>   // orbit competitions active this season
Persistence: current season materialized; past seasons keep results only
Source of truth: regenerated from seed (ephemeral), except current

Entity: CalendarWindow
- type: WindowType
- startDay: int
- endDay: int

Entity: Competition
- id: CompetitionId
- kind: CompetitionKind          // league | domesticCup | continental | international
- priority: int                  // for conflict resolution; higher wins
- isOrbit: bool                   // relevant to the player's club / nation
Persistence: regenerated from seed

Entity: Fixture
- id: FixtureId                   // deterministic = hash(seed, competition, season, round, slot)
- competitionId: CompetitionId
- scheduledDay: int              // epochDay; may be reschedule-shifted
- originalDay: int               // audit trail for conflict resolution
- homeClub: ClubId
- awayClub: ClubId
- importance: FixtureImportance
- legForId: FixtureId?           // for 2-legged ties; must reschedule together
- isOrbit: bool
Persistence: current-season only; past = discard fixtures, keep results
Source of truth: regenerated from seed

Entity: DayContext               // published read-only to subsystems each tick
- epochDay: int
- season: SeasonId
- todaysFixtures: List<Fixture>  // already conflict-resolved
- activeWindows: List<WindowType>
- daysUntilNextFixture: int
- congestionScore: double        // fixtures in a rolling 10-day window
- rngStream: RngStream           // injected sub-stream "calendar"

Entity: DayReport                // each subsystem returns this after handling a day
- source: SubsystemId
- stopClass: StopClass           // silent | softFlashpoint | hardStop
- payload: EventPayload?         // event data for the renderer
- mutations: List<StateMutation> // applied deterministically in registration order

Entity: StopResult               // returned to the renderer when advance halts
- day: int
- stops: List<DayReport>         // the reports that caused the halt
- pending: List<DayReport>       // buffered soft flashpoints flushed alongside

Entity: SuspensionLedger
- playerId: PlayerId
- competitionId: CompetitionId   // suspension is scoped per competition
- matchesRemaining: int          // decrements only when a match of THIS comp is played
Persistence: save

Relations:
- Season has many Fixture
- Season has many CalendarWindow
- GameClock belongs to one currentSeason
- Fixture belongs to one Competition
- SuspensionLedger belongs to Player, scoped per Competition

Enums:
enum WindowType        { transferSummer, transferWinter, internationalBreak, offSeason }
enum CompetitionKind   { league, domesticCup, continental, international }
enum FixtureImportance { deadRubber, league, derby, cupKnockout, continental, final }
enum StopClass         { silent, softFlashpoint, hardStop }
enum AdvanceMode       { tickOne, untilFlashpoint, restDays, advanceToDate, simSeasonHeadless }
enum SubsystemId       { match, training, transfer, media, life, injury, international, contract }
```

---

### Core Loop — Pseudo-code

#### Intent entry point (from renderer)

```
// The renderer only sends intents. The core decides everything.
function handleIntent(intent):
    switch intent.type:
        ADVANCE        -> return advanceUntilFlashpoint()
        REST(n)        -> return advanceBounded(n, allowBreak = true)
        SKIP_TO(date)  -> return advanceBounded(date - clock.epochDay, allowBreak = true)
        PLAY_MATCH(id) -> return delegateToMatchEngine(id)   // not the calendar's job
```

#### Atomic tick — the ONLY function allowed to mutate the clock

```
function tickOneDay() -> List<DayReport>:
    day = clock.epochDay

    // 1. Resolve fixtures for today (conflict resolution applied)
    fixtures = resolveFixturesForDay(day)
    ctx = buildDayContext(day, fixtures)

    // 2. Poll subsystems in FIXED registration order (determinism-critical)
    reports = []
    for sys in REGISTERED_SUBSYSTEMS:        // ordered list, never a hash-map iteration
        report = sys.onDay(ctx)              // subsystem runs its own rules
        applyMutations(report.mutations)     // mutate immediately, in order
        reports.append(report)

    // 3. Decrement match-scoped counters for matches actually played today
    decrementCountersForPlayedMatches(ctx, reports)

    // 4. Advance the clock
    clock.epochDay += 1

    // 5. Season boundary?
    if clock.epochDay > clock.currentSeason.endDay:
        runSeasonBoundary(clock.currentSeason)

    return reports
```

#### advanceUntilFlashpoint — the heart of manage-by-exception

```
function advanceUntilFlashpoint() -> StopResult:
    softBuffer = []                          // accumulate soft flashpoints to batch
    loop:
        reports = tickOneDay()

        hard = reports.filter(r => r.stopClass == hardStop)
        soft = reports.filter(r => r.stopClass == softFlashpoint)
        softBuffer.extend(soft)

        if hard.notEmpty():
            // must stop NOW (match day, window open, call-up, serious injury, offer)
            return StopResult(day = clock.epochDay, stops = hard, pending = softBuffer)

        if shouldFlushSoft(softBuffer, clock):
            // enough soft events / heavy enough / hit a week boundary -> surface them
            return StopResult(day = clock.epochDay, stops = softBuffer, pending = [])

        // nothing -> day passed silently, loop continues
```

#### advanceBounded — rest / injury / skip with a cap

```
function advanceBounded(maxDays, allowBreak) -> StopResult:
    target = clock.epochDay + maxDays
    while clock.epochDay < target:
        reports = tickOneDay()
        if allowBreak and reports.any(r => r.stopClass == hardStop):
            return StopResult(day = clock.epochDay, stops = hardStops(reports), pending = [])
    return StopResult(day = clock.epochDay, stops = [], pending = [])
```

#### simSeasonHeadless — for tests + outer-world (NEVER stops for flashpoints)

```
function simSeasonHeadless(seasonId):
    // Same code path as live; only difference is autoResolve instead of stopping.
    while not seasonEnded(seasonId):
        reports = tickOneDay()
        for r in reports where r.stopClass != silent:
            autoResolve(r)        // default policy: skip match, keep routine, decline offers
```

#### resolveFixturesForDay — deterministic conflict resolution, NO random

```
function resolveFixturesForDay(day) -> List<Fixture>:
    raw = mergeAllOrbitFixtures(day)         // gather from every orbit competition
    if raw.length <= 1:
        return raw

    // Two or more orbit fixtures on the same day -> conflict.
    // Fixed-priority policy. No dice.
    sort raw by (competition.priority DESC, importance DESC, fixtureId ASC)

    keep   = raw[0]                          // highest-priority fixture holds the day
    bumped = raw[1..]

    for f in bumped:
        // international windows always win club fixtures (FIFA-style):
        // if `keep` is international, club fixtures here were already bumped upstream.
        newDay = nextLegalSlot(f,
                               after = day,
                               avoid = [keep.scheduledDay] + windowDays + restMinGap)
        if f.legForId != null:
            rescheduleTieTogether(f, f.legForId, newDay)   // keep 2-leg ties paired
        else:
            reschedule(f, newDay)            // records originalDay

    return [keep]
```

#### runSeasonBoundary — fixed-order pipeline (order is load-bearing)

```
function runSeasonBoundary(oldSeason):
    // DO NOT REORDER. Each step depends on the previous.
    settleSeason(oldSeason)                  // 1. final tables, top scorers, titles
    awardCeremonies(oldSeason)               // 2. -> media engine (8.7), legacy axes (8.1)
    ageTickPopulation()                      // 3. all 20-30k players +1yr, apply age curves (5.1)
    batchTickOuterWorld(oldSeason)           // 4. non-orbit leagues advance, season granularity (7.1)
    promoteRelegateClubs()                   // 5. update the pyramid
    openWindow(transferSummer)               // 6. open summer window + transfer sagas (7.3)
    newSeason = genesisFixtures(seed, oldSeason.id + 1)   // 7. regen new season schedule (ephemeral)
    discardFixtures(oldSeason)               // 8. drop old fixtures, keep results (9: tiny save)
    clock.currentSeason = newSeason
```

#### Determinism plumbing (§9 — sacred)

```
// RNG is split into independent streams per domain.
// The calendar must NOT draw from the same stream as the match engine,
// or "play vs skip a match" would shift the RNG of transfers/injuries.

rootRng     = seededRng(saveSeed)
calendarRng = rootRng.fork("calendar")
matchRng    = rootRng.fork("match")
transferRng = rootRng.fork("transfer")
injuryRng   = rootRng.fork("injury")

// Fixture ids and reschedule slots derive from calendarRng (or pure hashing),
// so the same seed always yields the same schedule, even after regeneration.
```

**Hard rules for the code reviewer:**
- Banned in the core: `DateTime.now()`, global `Random()`, `System.currentTimeMillis()`, any wall-clock read.
- All randomness flows through the `RngStream` injected via `DayContext`.
- Subsystem poll order is a **fixed ordered list** — never iterate an unordered `Map` or `Set`.

---

### Feature Breakdown

#### Phase 1 — MVP (single competition, no conflict)

| # | Task | Layer | Notes |
|---|------|-------|-------|
| 1 | `GameClock` + epochDay arithmetic (365-day year) | Domain | no wall-clock; pure int math |
| 2 | `Season` + `CalendarWindow` models | Domain | windows as day-ranges |
| 3 | Deterministic `RngStream` with `fork(name)` | Infra | foundation for everything |
| 4 | `Fixture` model + deterministic id from seed | Domain | id = hash(seed, comp, season, round, slot) |
| 5 | `genesisFixtures(seed, season)` — single league | Domain | ephemeral; regen on reach |
| 6 | `DayContext` builder | Domain | computes daysUntilNextFixture, congestion |
| 7 | Subsystem interface + fixed registry | Domain | `onDay(ctx) -> DayReport` |
| 8 | `tickOneDay()` | Domain | the only clock-mutator |
| 9 | `advanceUntilFlashpoint()` + `shouldFlushSoft()` | Domain | core player loop |
| 10 | `advanceBounded()` (rest/skip) | Domain | injury layoffs |
| 11 | `simSeasonHeadless()` + `autoResolve()` | Domain | test harness |
| 12 | Save/load: serialize clock + current season only | Data | regen the rest |

> ⚠️ Tasks 8–11 depend on 6–7. Task 5 depends on 3–4. Task 3 first — everything leans on it.

#### Phase 2 — Multi-competition + conflict

| # | Task | Layer | Notes |
|---|------|-------|-------|
| 13 | `Competition` model + priority field | Domain | league/cup/continental/intl |
| 14 | `mergeAllOrbitFixtures(day)` | Domain | unified timeline |
| 15 | `resolveFixturesForDay()` conflict resolution | Domain | fixed-priority, deterministic |
| 16 | `nextLegalSlot()` reschedule with avoid-set | Domain | respects windows + min rest gap |
| 17 | 2-leg tie pairing (`rescheduleTieTogether`) | Domain | legForId stays coupled |
| 18 | `SuspensionLedger` per competition | Domain | count by match, not day |
| 19 | `decrementCountersForPlayedMatches()` | Domain | survives reschedules |
| 20 | International window subsystem | Domain | hard-stop; pulls player from club |
| 21 | Congestion score (rolling 10-day) | Domain | feeds soft-flashpoint warning |

#### Phase 3 — Season boundary + world reconciliation

| # | Task | Layer | Notes |
|---|------|-------|-------|
| 22 | `runSeasonBoundary()` ordered pipeline | Domain | order is load-bearing |
| 23 | `ageTickPopulation()` — struct-of-arrays | Infra | 20-30k players, columnar (9) |
| 24 | `batchTickOuterWorld()` hook | Domain | season-granularity tick (7.1) |
| 25 | `promoteRelegateClubs()` | Domain | pyramid update |
| 26 | `discardFixtures()` + results retention | Data | keep records, drop fixtures |
| 27 | Season-review flashpoint (optional stop) | Domain | player toggle |

> ⚠️ Phase 3 depends on Phase 2. Task 23 is the perf-critical one — see Tech Notes.

---

### Tech Notes & Gotchas

#### Time & determinism
- **Day-tick is cheaper than it sounds.** 365 ticks/year × 20 years = ~7,300 ticks per career. Most days are silent and early-return. The real cost is match resolution and the season batch-tick — not the day loop. Do not micro-optimize the loop before profiling.
- **Banned in the core:** `DateTime.now()`, global `Random()`, wall-clock reads. Inject `RngStream` through `DayContext` only. This is the single most important rule — one stray `now()` and §2.3 is dead.
- **Subsystem registration order is an ABI.** Once shipped, reordering the subsystem list breaks determinism for every existing save. Lock it as a versioned enum. If you must add a subsystem, append it and bump a `simVersion`.
- **Split RNG streams per domain.** If the calendar and match engine share a stream, then playing vs skipping a match shifts every downstream roll (transfers, injuries). Fork per domain.

#### Fixtures & conflict
- **Don't store fixtures.** Regenerate from `(seed, season, league)` every time. The save holds results + records + current-season materialized state only (§9). This is what keeps load under 1s.
- **Suspension counts by match, not day** (AC-06). If you count by day, a postponed match wrongly serves the ban. The counter only decrements when a match of *that exact competition* is actually played.
- **2-leg ties reschedule together.** When bumping a first leg, the `legForId` second leg moves with it — don't shift one and orphan the other.
- **International windows are an external interrupt.** They both pull the player out (national duty) and bump club fixtures. Treat `international` as a first-class subsystem with hard-stop authority, not a special case inside the league logic.

#### Performance
- **Struct-of-arrays for `ageTickPopulation`** (§9). Aging 30k players each season boundary with 30k heap objects → ARC/GC spike → frame stutter exactly when crossing seasons. Store the population columnar (parallel typed arrays), not as 30k reference objects.
- **Headless sim must be the same code path.** `simSeasonHeadless` differs from live only in calling `autoResolve` instead of returning a `StopResult`. If you write two code paths (one for "play", one for "test"), they will diverge and your tests stop meaning anything.

#### UX-adjacent (lives in core but shapes feel)
- **`shouldFlushSoft()` is where the game feels good or annoying.** Stop on every minor event → the player rages. Buffer soft flashpoints and flush as a cluster (by count threshold, by cumulative weight, or at a week boundary). This is the single most-tuned function in the system — expect to iterate against a prototype.
- `[DECISION NEEDED]` Soft-flush policy: count-threshold, weight-threshold, or fixed cadence (every Monday)? Affects the entire feel of manage-by-exception.

#### Decisions still open
- `[DECISION NEEDED]` Fixed 365-day year vs real calendar mapping. Recommendation: fixed 365 — simpler, deterministic, players don't care about weekdays.
- `[DECISION NEEDED]` Competition priority table for conflicts. Proposed ladder: `final > continental > cupKnockout > derby > league > deadRubber`. Confirm whether priority can shift by nationality/era.
- `[DECISION NEEDED]` Do reschedules propagate to the outer (batch-ticked) world, or are orbit reschedules local-only and invisible to AI clubs? This affects league-table consistency.

---

### ⚠️ Risks & Open Questions

- **International break stacked on club congestion = double-whammy.** The player is simultaneously fatigued and pulled away. The energy/injury model must receive the right signal from the calendar, or you get an absurd "inexplicably exhausted player" bug. Wire the congestion + intl-duty signals explicitly.
- **`shouldFlushSoft` tuning is unbounded scope.** It's a feel problem, not a correctness problem — it cannot be "finished" on paper. Budget prototype iteration time, don't try to nail it in the spec.
- **Reschedule ripple into the outer world** (the open decision above) can quietly break league-table consistency if orbit reschedules aren't reflected where they should be. Decide the boundary before coding Phase 3.
- **GK career is parked (§11)** but the calendar must stay player-type-agnostic from day one. Don't hard-code outfield assumptions into `DayContext` or fixture handling — adding GK later should touch beats/flashpoints, not the calendar.
- **Save migration vs subsystem ABI.** The moment you ship and later add/reorder a subsystem, old saves desync. You need a `simVersion` and a migration story before the first public build, not after.

---
---

## APPENDIX — `TASK-0X-goat-calendar.md` (paste-ready for Claude Code)

> This is the Claude Code task file derived from the spec above. It follows the same
> convention as `TASK-01-goat-core`: read source-of-truth first, work in reviewable
> steps with pauses, frozen golden values, determinism non-negotiable.
>
> **Prereqs before pasting this:**
> - `TASK-01-goat-core` is merged and `cargo test --workspace` is green.
> - `goat-core` exposes the player/attribute/role model the calendar will poll against.
> - `CLAUDE.md` is in the repo root and the tech doc is current.
>
> **Phasing note:** This task covers Phase 1 of the calendar (single-competition,
> deterministic tick loop). Multi-competition + conflict resolution and the season
> boundary pipeline become `TASK-0X+1` and `TASK-0X+2` — do not pull them forward.

---

Read CLAUDE.md, then docs/BecomeTheGOAT-RustCore-TechDoc.md (the module map, RNG design,
and build order), then the design-bible sections §2.2, §2.3, §5.4, §6.1, §7.1, §9, and
finally the public APIs of crates/goat-rng, crates/goat-fixed, and crates/goat-core.
Do not write any code until you've read all of them.

If anything in this task contradicts the tech doc, STOP and flag it — the tech doc wins.

Then build the `goat-calendar` crate, Phase 1 (single competition, deterministic tick),
in these steps — pause for my review after each step:

### Step 1 — Time core + season model
- New workspace member `crates/goat-calendar` (`#![forbid(unsafe_code)]`, deps limited
  to goat-rng, goat-fixed, goat-core).
- `GameClock { epoch_day: u32, current_season: SeasonId }`. The in-game year is a fixed
  365 days — no leap years, no wall-clock. epoch_day is the single time axis.
- `Season { id, start_day, end_day, windows: Vec<CalendarWindow>, competition_ids }`.
- `CalendarWindow { kind: WindowKind, start_day, end_day }` with WindowKind =
  { TransferSummer, TransferWinter, InternationalBreak, OffSeason }.
- All day arithmetic is pure integer math. No `std::time`, no `chrono`, no `SystemTime`.
- Property test: epoch_day → (season-relative day) round-trips; windows never overlap
  illegally within a season.

### Step 2 — Subsystem registry + DayContext + tick_one_day
- Define the `Subsystem` trait: `fn on_day(&mut self, ctx: &DayContext) -> DayReport`.
- `DayReport { source: SubsystemId, stop_class: StopClass, payload, mutations }` with
  StopClass = { Silent, SoftFlashpoint, HardStop }.
- The registry is a FIXED-ORDER `Vec`, never a HashMap iteration. Document that this
  order is an ABI: reordering breaks save determinism. Gate it behind a `SIM_VERSION`
  const.
- `DayContext` carries epoch_day, season id, today's fixtures, active windows,
  days_until_next_fixture, congestion_score, and an injected RNG sub-stream
  (`rng.fork("calendar")` — never the global or the match stream).
- `tick_one_day(&mut self) -> Vec<DayReport>`: the ONLY function permitted to mutate
  `epoch_day`. Build context → poll subsystems in order, applying each report's
  mutations immediately → decrement match-scoped counters for matches played today →
  `epoch_day += 1` → if past season end_day, call the (stubbed for now) season-boundary
  hook.
- For Phase 1, provide 1–2 trivial stub subsystems (e.g. a training stub and a match
  stub) so the loop is exercisable. Real subsystems land in their own crates later.

### Step 3 — advance loop + golden-seed test (calendar test #1)
- `advance_until_flashpoint(&mut self) -> StopResult`: loop tick_one_day; stop on the
  first HardStop, or when `should_flush_soft(buffer, clock)` says the buffered
  SoftFlashpoints are worth surfacing; silent days loop. `StopResult { day, stops,
  pending }`.
- `advance_bounded(&mut self, max_days, allow_break) -> StopResult`: tick up to max_days,
  breaking early on a HardStop only if allow_break. This is rest/injury/skip.
- `should_flush_soft` for Phase 1: simplest defensible rule (e.g. flush when buffer
  length >= N, N a named constant in `tuning`). Mark it clearly as TUNABLE — the real
  policy is deferred (bible-style open question), so leave a doc comment, do not invent
  a clever final rule.
- Golden-seed test: fixed seed + fixed intent sequence (a mix of ADVANCE and REST over,
  say, 60 days with the stub subsystems firing scripted reports) → assert the exact
  sequence of stop days and the exact final clock state. These expected values become
  FROZEN once I approve.
- Determinism test: run the same 60-day sequence twice, assert byte-identical state
  snapshots (use `insta` if it's already in the workspace, otherwise a manual
  serialize-and-compare).

### Rules reminders (from CLAUDE.md — these override convenience)
- No floats in sim. No std HashMap iteration feeding results. RNG only via injection,
  forked per domain — the calendar stream must be independent of the match stream.
- Do NOT touch goat-rng, goat-fixed, or goat-core. If their API seems insufficient,
  stop and ask — do not refactor them as a side effect.
- All pre-existing golden tests must stay green with their ORIGINAL expected values.
  Never "fix" a failing test by editing the expected value.
- `epoch_day` mutation lives in exactly one place (`tick_one_day`). If you find yourself
  incrementing it anywhere else, stop — that's a design smell this task forbids.
- Season-boundary pipeline is OUT OF SCOPE for this task (stub the hook only). Do not
  implement settle/age-tick/batch-tick here.
- Multi-competition and conflict resolution are OUT OF SCOPE. Phase 1 is single-comp.
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` clean before each
  pause.

At each pause: show me the file tree of what you added, the key type definitions
(GameClock, Season, DayContext, DayReport, the Subsystem trait), and the test output.

### Definition of done (this task)
1. `cargo test --workspace` green — including all pre-existing golden tests at their
   original expected values (goat-rng 9, goat-fixed 6, plus goat-core's).
2. `cargo fmt --check` and `cargo clippy -D warnings` clean.
3. The tick loop's deterministic behavior is covered by the golden-seed test AND the
   byte-identical determinism test.
4. No new heavy deps (insta is fine if already present), no floats in sim, no unsafe,
   no I/O, no wall-clock reads anywhere in the crate.
5. `grep -rn "now()\|SystemTime\|Instant\|chrono\|f32\|f64" crates/goat-calendar/src`
   returns nothing.
6. Short summary of what changed and which bible/tech-doc sections it implements
   (expected: §2.2, §2.3, §5.4, §9).

---

# Part 4 — Training Subsystem Core Spec

_Source file: `TRAINING.md`_

## Training Subsystem — Core Spec + Claude Code Task

**Project:** BECOME THE GOAT
**Scope:** Headless core. The first content-bearing subsystem — gives the calendar tick something to *do*.
**Status:** Design-locked. Numbers are illustrative placeholders (final tuning deferred per §11).
**Depends on:** `goat-core` (attributes, age curves, potential), `goat-calendar` Phase 1 (the tick loop + Subsystem trait).

---

### Overview

Training is the first subsystem that registers on the calendar's tick loop and produces real state change. Its job, per bible §5.4: push a player's **current → potential**, gated by age curves (§5.1) and paid for in **energy**. It is the mechanical core of the week-as-loop-unit pillar — the player sets a routine once, then the calendar auto-runs it day by day, surfacing a decision only on "big weeks".

This is deliberately the first content subsystem because it's the simplest way to validate the `Subsystem` trait the calendar defines: it reads `DayContext`, mutates player attributes deterministically, costs a resource, and occasionally raises a soft-flashpoint — without touching the beat engine's complexity. If the trait is wrong, training reveals it cheaply.

Training is NOT the match engine and NOT player generation. It only moves existing attributes toward their existing ceilings.

---

### Assumptions

- `[ASSUMED]` Training runs as a per-day tick but the player only *interacts* at the week granularity (set a routine; intervene on big weeks). The subsystem reads the same day-tick as everything else.
- `[ASSUMED]` A "routine" is a standing instruction: which attribute(s) to target, at what intensity. It persists across days until changed. Default routine exists so a never-intervening player still develops.
- `[ASSUMED]` Energy is a per-player resource on a 0–100 fixed-point scale: training spends it, rest recovers it. Low energy reduces gains and raises injury risk (§5.4). `[DECISION NEEDED]` exact curve.
- `[ASSUMED]` Growth is gated by the attribute's age-curve archetype (Physical / Technical / Mental, §5.1): trainability differs (Physical low, Technical high, Mental grows with experience not just training). Current can never exceed potential (§2.4 ceiling clamp — already enforced in goat-core).
- `[ASSUMED]` Facilities/coach multipliers (§4.2) feed in as a development-speed multiplier, but the values come from the club model. For this task, accept a multiplier input and default it to 1.0 — do not build the club model here.
- `[ASSUMED]` Match days are not training days. On a fixture day, training yields no growth (the match handles load); the subsystem either rests or applies match-fatigue, but match logic itself is out of scope.

---

### User Stories

```
US-01: Set a training routine
As a player,
I want to set a standing training routine (target attribute + intensity),
So that my player develops automatically without me touching every day.
Priority: P0 | Size: M

US-02: Develop toward potential
As a player,
I want my trained attributes to rise over time toward their personal ceiling,
So that the talent I was dealt actually turns into ability.
Priority: P0 | Size: M

US-03: Feel the age curve
As a player,
I want physical, technical, and mental attributes to grow at different rates by age,
So that development feels like a real career arc, not a uniform stat-pump.
Priority: P0 | Size: M

US-04: Manage energy / fatigue
As a player,
I want training to cost energy and rest to recover it,
So that overtraining has a real cost (worse gains, higher injury risk).
Priority: P0 | Size: M

US-05: Get interrupted only on big weeks
As a player,
I want the game to surface a training decision only at meaningful moments
(a breakthrough, a dip, returning from injury, a derby week),
So that routine weeks stay silent (manage-by-exception, §2.2).
Priority: P1 | Size: M

US-06 (system): Deterministic development
As the core,
I need development for the same (seed, routine, intensity, days) to be identical,
So that headless career sims are reproducible.
Priority: P0 | Size: S
```

---

### Acceptance Criteria

```
AC-01 for US-02: Growth pushes current toward potential, never past

Scenario: Sustained training on a trainable attribute
  Given a 17-year-old with a technical attribute at 60 current / 85 potential
  And a routine targeting that attribute at moderate intensity
  When the calendar advances one season of non-match days
  Then the attribute's current value increases
  And it never exceeds 85 (potential)
  And on reaching 85 further training yields zero growth

AC-02 for US-03: Age-curve archetypes grow differently

Scenario: Same intensity, three archetypes, young player
  Given a 17-year-old training a Physical, a Technical, and a Mental attribute
    at identical intensity for identical days
  When development is applied
  Then the Technical attribute gains the most from training
  And the Physical attribute gains less (low trainability)
  And the Mental attribute's gain leans on experience/age, not just training input

AC-03 for US-04: Energy gates gains and recovers on rest

Scenario: Training drains energy
  Given a player at 100 energy
  When they train at high intensity for several consecutive non-rest days
  Then energy decreases each training day
  And once energy is low, per-day attribute gain is reduced versus the same training at full energy

Scenario: Rest recovers energy
  Given a player at low energy
  When the calendar advances rest days (no training, no match)
  Then energy increases each rest day toward the cap
  And no attribute growth occurs on pure rest days

AC-04 for US-05: Only big weeks raise a flashpoint

Scenario: Routine week is silent
  Given a standing routine and an ordinary training week with no events
  When the calendar ticks through it
  Then the training subsystem returns Silent for each day
  And the player is not interrupted

Scenario: Breakthrough raises a soft flashpoint
  Given a young player whose attribute crosses a notable threshold (a "breakthrough")
  When that day is ticked
  Then the training subsystem returns a SoftFlashpoint with a payload describing the breakthrough
  And the player is surfaced this per the calendar's flush policy

AC-05 for US-06: Development is deterministic

Scenario: Reproducible season of training
  Given two players from the same seed with the same routine and intensity
  When each is advanced the same number of identical days
  Then their attribute values and energy are identical at the end

NFR-01: No floats
  Given any development or energy calculation
  When inspected
  Then it uses goat-fixed math only — no f32/f64 anywhere in the crate
```

---

### Data Models

```
Entity: TrainingRoutine
- target: AttrTarget            // which attribute(s) the routine pushes
- intensity: Intensity          // Light | Moderate | Hard (illustrative tiers)
Persistence: save (small, per-player-of-interest)
Note: a default routine exists so a non-intervening player still develops

Entity: EnergyState
- value: Fixed                  // 0..100 fixed-point
Persistence: save (orbit players); background players computed on demand (7.1)

Entity: DevelopmentInput        // assembled per training day, fed to the growth fn
- attr_archetype: AgeArchetype  // Physical | Technical | Mental (from goat-core, 5.1)
- age_days: u16
- current: Fixed
- potential: Fixed
- intensity: Intensity
- energy: Fixed
- facility_mult: Fixed          // default 1.0; real value from club model (out of scope)
- rng: RngStream                // injected fork("training") — NEVER the calendar/match stream

Entity: TrainingDayResult       // what the subsystem emits per day (wrapped into DayReport)
- attr_deltas: List<(AttrId, Fixed)>   // growth applied this day
- energy_delta: Fixed
- event: TrainingEvent?         // None on a routine day
Note: maps onto the calendar's DayReport { stop_class, payload, mutations }

Enums:
enum Intensity     { Light, Moderate, Hard }
enum AgeArchetype  { Physical, Technical, Mental }   // mirrors goat-core's curve types
enum TrainingEvent { Breakthrough, FormDip, ReturnFromInjury, Overtrained }
enum AttrTarget    { Single(AttrId), Family(FamilyId) }  // start with Single; Family later
```

---

### Core Loop — Pseudo-code

```
// Registered on the calendar as a Subsystem. Called once per day with DayContext.
impl Subsystem for Training:
  fn on_day(ctx) -> DayReport:
      player = ctx.orbit_player()

      // Match day? Training yields nothing; match handles load (match engine out of scope).
      if ctx.todays_fixtures.involve(player):
          return DayReport.silent()        // match-fatigue applied by match subsystem later

      // Pure rest day (no routine active, or routine = rest)?
      if routine.is_rest() or player.is_injured():
          new_energy = recover_energy(player.energy, ctx.rng)
          return DayReport.silent_with(mutations = [set_energy(new_energy)])

      // Training day.
      input = build_development_input(player, routine, ctx.rng)
      delta = compute_growth(input)          // the heart of it; see below
      energy_delta = spend_energy(input.intensity, input.energy)

      event = detect_event(player, delta)    // breakthrough / overtrained / dip
      stop_class = match event:
          Some(Breakthrough | ReturnFromInjury) => SoftFlashpoint
          Some(Overtrained)                      => SoftFlashpoint
          _                                      => Silent

      return DayReport {
          source: Training,
          stop_class,
          payload: event.map(describe),
          mutations: [apply_attr_delta(delta), apply_energy(energy_delta)],
      }


// The growth function — gated by age archetype, intensity, energy, ceiling.
fn compute_growth(input) -> Fixed:
    if input.current >= input.potential:
        return 0                            // ceiling clamp (2.4). Never exceed potential.

    headroom   = input.potential - input.current
    base       = intensity_factor(input.intensity)         // tuning constant
    trainable  = trainability(input.attr_archetype, input.age_days)   // 5.1 curve
    energy_mod = energy_factor(input.energy)               // low energy -> smaller gains
    noise      = small_seeded_jitter(input.rng)            // deterministic variance

    raw = base * trainable * energy_mod * headroom_scaled(headroom) * noise
    return clamp_to_headroom(raw, headroom)


// Trainability per archetype (bible 5.1):
//   Physical  -> low, declines early with age
//   Technical -> high, broad mid-career plateau
//   Mental    -> grows with EXPERIENCE/age, not just training input
fn trainability(archetype, age_days) -> Fixed:
    match archetype:
        Physical  => physical_curve(age_days)    // peaks early, low ceiling on gains
        Technical => technical_curve(age_days)   // high, slow to decline
        Mental    => mental_curve(age_days)      // appreciates with age
```

All curves and factors are named constants in a `tuning` module — illustrative placeholders, flagged TUNABLE, final values deferred per §11.

---

### Feature Breakdown

#### Phase 1 — MVP (single-attribute routine, energy, deterministic growth)

| # | Task | Layer | Notes |
|---|------|-------|-------|
| 1 | `goat-training` crate skeleton (`forbid(unsafe)`, deps: rng/fixed/core/calendar) | Infra | registers as a Subsystem |
| 2 | `TrainingRoutine`, `EnergyState`, `Intensity` models | Domain | Single-attr target first |
| 3 | `tuning` module: intensity factors, energy curve, archetype curves | Domain | all placeholder consts, TUNABLE |
| 4 | `compute_growth()` with ceiling clamp | Domain | fixed-point; never exceed potential |
| 5 | `trainability()` archetype curves (Physical/Technical/Mental) | Domain | bible §5.1 |
| 6 | Energy spend/recover | Domain | drains on train, recovers on rest |
| 7 | `detect_event()` (breakthrough/overtrained) | Domain | raises SoftFlashpoint |
| 8 | `impl Subsystem for Training` (`on_day`) | Domain | the integration point |
| 9 | Golden-seed test: one season of training | Domain | frozen expected values |
| 10 | Determinism test: same seed → identical state | Domain | byte-identical snapshot |

> ⚠️ Task 8 depends on the calendar's `Subsystem` trait being merged. Tasks 4–6 depend on 3. Do task 3 (tuning) right after the skeleton — everything reads from it.

#### Phase 2 — Post-MVP (deferred)

| # | Task | Layer | Notes |
|---|------|-------|-------|
| 11 | Family-target routines (train a whole family) | Domain | `AttrTarget::Family` |
| 12 | Facility/coach multiplier wired from club model | Domain | needs club model first |
| 13 | Injury risk from overtraining feeds injury subsystem | Domain | cross-subsystem |
| 14 | Form-dip detection + interaction with season-long form | Domain | needs form model |

---

### Tech Notes & Gotchas

#### Determinism
- RNG via `ctx.rng.fork("training")` ONLY. Never the calendar or match stream — sharing would couple training variance to unrelated rolls and break §2.3.
- `grep` gate in DoD: no `f32`/`f64`, no `now()`/`SystemTime`/`chrono` in the crate.
- All growth/energy math in goat-fixed. A single float sneaking into a curve constant will silently desync saves across platforms.

#### Correctness
- **Ceiling clamp is sacred (§2.4):** current can NEVER exceed potential. This is already enforced in goat-core — call its clamp, do not re-implement a looser one here. Money/facilities accelerate approach to the ceiling, never lift it.
- **Match days yield no training growth.** Don't double-count load: on a fixture day, training is silent and the (future) match subsystem owns fatigue. For this task, just detect fixture days from `DayContext` and skip growth.
- **Mental attributes are special:** their trainability leans on age/experience, not raw training input. Don't model all three archetypes with one scaled curve — that flattens the late-career reinvention arc (§5.2) the whole design hangs on.

#### Scope discipline
- Do NOT build the club model, the injury subsystem, the form model, or the match engine here. Accept their inputs as parameters with sane defaults (facility_mult = 1.0, not injured, no match) and leave hooks.
- Do NOT touch goat-rng, goat-fixed, goat-core, or goat-calendar. If a trait or API seems insufficient, STOP and ask — do not refactor a dependency as a side effect.

#### Decisions open
- `[DECISION NEEDED]` Energy curve shape: linear drain/recovery, or diminishing? Affects how punishing overtraining feels.
- `[DECISION NEEDED]` Breakthrough threshold: fixed attribute milestones, or relative-to-potential jumps? Determines how often US-05 flashpoints fire.
- `[DECISION NEEDED]` Does intensity affect injury risk directly, or only via energy? (Cross-subsystem; can defer to Phase 2.)

---

### ⚠️ Risks & Open Questions

- **This task validates the `Subsystem` trait.** If `on_day(ctx) -> DayReport` turns out too thin (e.g. training needs multi-day lookahead, or to read another subsystem's state mid-tick), that's a finding about the *calendar's* design, not training. Surface it loudly rather than hacking around it — it's cheaper to fix the trait now than after three subsystems depend on it.
- **Tuning is unbounded and deferred (§11).** Don't try to make development "feel right" in this task — that needs a prototype and playtesting. Ship defensible placeholder constants, clearly marked TUNABLE, and move on.
- **Cross-subsystem ordering.** Training reads energy; a future injury subsystem also reads/writes energy. The calendar's fixed subsystem order decides who sees what first. Note any ordering assumption training makes so it's explicit when injury lands.
- **Background vs orbit players.** Orbit players store energy/current; background players are formula-driven (§7.1). This task only handles the orbit player. Don't accidentally allocate per-day energy state for the 20-30k population — that's the SoA/perf trap from §9.

---
---

## APPENDIX — `TASK-0X-goat-training.md` (paste-ready for Claude Code)

> Same convention as TASK-01-goat-core: read source-of-truth first, reviewable steps
> with pauses, frozen golden values, determinism non-negotiable.
>
> **Prereqs before pasting:**
> - `goat-core` and `goat-calendar` Phase 1 are merged; `cargo test --workspace` green.
> - The calendar exposes the `Subsystem` trait and `DayContext`.
> - `CLAUDE.md` in repo root; tech doc current.
>
> **Scope:** Phase 1 only (single-attribute routine, energy, deterministic growth).
> Family targets, club multipliers, injury coupling, and form are OUT OF SCOPE.

---

Read CLAUDE.md, then docs/BecomeTheGOAT-RustCore-TechDoc.md (module map + build order),
then design-bible §2.2, §2.4, §5.1, §5.2, §5.4, §9, then the public APIs of
crates/goat-rng, crates/goat-fixed, crates/goat-core, and crates/goat-calendar —
especially goat-core's attribute/age-curve model and goat-calendar's Subsystem trait
and DayContext. Do not write any code until you've read all of them.

If anything here contradicts the tech doc, STOP and flag it — the tech doc wins.

Then build the `goat-training` crate, Phase 1, in these steps — pause after each:

### Step 1 — Crate skeleton + models + tuning
- New workspace member `crates/goat-training` (`#![forbid(unsafe_code)]`, deps limited
  to goat-rng, goat-fixed, goat-core, goat-calendar).
- Models: `TrainingRoutine { target, intensity }`, `EnergyState { value: Fixed }`,
  `Intensity { Light, Moderate, Hard }`. Start with single-attribute targets only.
- A `tuning` module holding ALL magic numbers as named consts: intensity factors,
  energy spend/recover rates, and the three age-archetype trainability curves
  (Physical / Technical / Mental). Every constant documented as TUNABLE placeholder
  per bible §11. No final numbers — defensible placeholders only.
- A default routine so a non-intervening player still develops.

### Step 2 — Growth + energy (the math)
- `compute_growth(input) -> Fixed` per bible §5.4: gated by age-archetype trainability
  (§5.1), intensity, energy, and ceiling. Entirely in goat-fixed.
- The ceiling clamp: current can NEVER exceed potential (§2.4). Call goat-core's
  existing clamp — do not re-implement a looser one.
- `trainability(archetype, age_days)`: three DISTINCT curves. Physical = low, declines
  early. Technical = high, broad plateau. Mental = appreciates with age/experience,
  not just training input (this preserves the late-career reinvention arc §5.2 — do not
  collapse all three into one scaled curve).
- Energy: spend on training (scaled by intensity), recover on rest. Low energy reduces
  per-day gain. No growth on pure rest days.
- Property tests: growth is 0 at the ceiling; growth monotonic in intensity at fixed
  energy/age; energy stays in [0,100]; Technical out-gains Physical at young age,
  same intensity.

### Step 3 — Subsystem impl + golden-seed test
- `impl Subsystem for Training`: `on_day(ctx) -> DayReport`. Match day (fixture in
  ctx involving the orbit player) => Silent, no growth. Rest/injured => recover energy,
  Silent. Training day => compute growth + energy delta, detect event, emit DayReport
  with mutations.
- `detect_event`: at minimum Breakthrough (attribute crosses a notable threshold) and
  Overtrained (trained at low energy) => SoftFlashpoint. Ordinary day => Silent.
  Threshold is a TUNABLE const.
- Golden-seed test (training test #1): a fixed seed + fixed routine + a scripted ~one-
  season sequence of training/rest/match days via the calendar tick → assert the exact
  final attribute values, exact final energy, and the exact set of days that produced a
  SoftFlashpoint. These expected values become FROZEN once I approve.
- Determinism test: run the same sequence twice → byte-identical snapshot.

### Rules reminders (from CLAUDE.md — override convenience)
- No floats in sim. RNG only via `ctx.rng.fork("training")` — never the calendar or
  match stream. No std HashMap iteration feeding results.
- Do NOT touch goat-rng, goat-fixed, goat-core, or goat-calendar. If their API seems
  insufficient, STOP and ask — do not refactor them.
- All pre-existing golden tests stay green at ORIGINAL expected values. Never "fix" a
  failing test by editing the expected value.
- Ceiling clamp uses goat-core's existing function — do not write a looser one.
- Out of scope, do not build: club/facility model, injury subsystem, form model, match
  engine, family-target routines. Accept their inputs as defaulted parameters + hooks.
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` clean before each pause.

At each pause: show me the file tree added, the key type definitions (TrainingRoutine,
EnergyState, DevelopmentInput, the Subsystem impl signature), the tuning constants, and
the test output.

### Definition of done (this task)
1. `cargo test --workspace` green — all pre-existing golden tests at original expected
   values (goat-rng 9, goat-fixed 6, plus goat-core's and goat-calendar's).
2. `cargo fmt --check` and `cargo clippy -D warnings` clean.
3. Deterministic behavior covered by the golden-seed test AND the byte-identical
   determinism test.
4. No new heavy deps, no floats in sim, no unsafe, no I/O, no wall-clock reads.
5. `grep -rn "now()\|SystemTime\|Instant\|chrono\|f32\|f64" crates/goat-training/src`
   returns nothing.
6. Short summary of what changed and which bible/tech-doc sections it implements
   (expected: §2.2, §2.4, §5.1, §5.2, §5.4, §9).

---

# Part 5 — Traits & Mastery

_Source file: `TRAITS.md`_

## BECOME THE GOAT — Appendix A

### Traits & Mastery

*Companion to §5 (The Player), §6 (The Match), and §11 (Open Questions). Status: design-locked in intent; the full trait catalogue is a content pipeline, not a design question — same posture as the beat library (§11). Numbers, bands, and per-trait tuning deferred against a prototype.*

---

#### A.0 Why this appendix exists

The core design (§5–§6) defines attributes and how they feed beats, but it leaves a gap between the two: attributes resolve a contest *once a beat is already running*, yet nothing decides **which beats appear** or **how a player plays them** based on his individual signature. Two strikers with identical Shooting can post the same OVR (§5.3) and still feel nothing alike — one bends finesse shots into the far corner, the other ghosts in behind the line for tap-ins. Attributes alone don't carry that difference.

Traits are the missing layer. A trait is a **special skill** that bends the match engine in a player-specific way. Where an attribute is a number that scales a contest, a trait changes the *shape* of play: it summons beats, unlocks choices, rewrites how a finish resolves, or colours the player's in-match psychology. Traits are what make the legacy debate (§8.1) argue about *players*, not stat lines.

---

#### A.1 What a trait is

A trait is not an attribute. The two are deliberately different in kind:

| | Attribute | Trait |
|---|---|---|
| **Value** | A number, 1–99 | A discrete mastery tier — **no 1–99 scale** |
| **Effect** | Scales a contest quantitatively | Bends a beat *qualitatively* |
| **Display** | Stored truth (30 sub-attrs) / derived families | A named, recognisable "special skill" |

Traits reuse the engine's existing pattern for discrete tiers — the four-tier role familiarity ladder of §5.2 — so the core gets no new machinery, only a new layer running on the old rails.

**Mastery tiers.** Every trait a player has sits at one of four tiers. The point of tiers is not "more %"; each step changes play *in kind*, opening choices and beats that lower tiers don't have. Illustrative, using *Finesse Merchant* (the curled finish):

| Tier | What it does in a beat |
|---|---|
| **None** | No finesse branch exists; an edge-of-box chance resolves as a normal power strike |
| **Raw** | The curl branch *appears*, but at a contest penalty — it misfires often |
| **Proficient** | Contest is balanced; curling is now a real, viable choice |
| **Mastery** | Strong buff, plus new sub-choices unlock (the impossible far-corner bend from a tight angle) |

The lesson generalises: a high-mastery trait doesn't only make the player *better* at something — it gives him *options nobody else has*.

---

#### A.2 The five ways a trait bends the engine

Every trait is classified by where it plugs into the match engine (§6). This keeps the catalogue honest — a trait that can't name its hook is not yet designed.

1. **Signature finish** — opens or rewrites how an existing contest resolves. *(Finesse Merchant.)*
2. **Beat-summoner** — makes a class of beat *fire more often* via the context-weighted selector (§6.1). This is the hook that finally lets off-ball and reading play *create* beats, not just gate choices inside them. *(Ghost, Reader.)*
3. **Choice-unlocker** — adds a branch inside a beat that wouldn't otherwise be on the menu. *(Through-ball Maestro.)*
4. **Headspace modifier** — feeds the in-match psychology of §6.2 (Confidence / Nerves / Frustration / Flow). *(Big-Game Player.)*
5. **Stat-shaper** — bends a contest without creating a beat. *(Clinical, Engine.)*

---

#### A.3 How traits are acquired

A trait is rolled with **two** values at character creation, mirroring the attribute split of current-value vs potential (§5.3):

- **Aptitude (mastery ceiling)** — the highest tier this player can ever reach for that trait. This is the *gift*: some are born with a Mastery ceiling for curling, some cap at Proficient no matter how they train, some have a ceiling of zero and will never learn it at all. This is the one thing fate decides, consistent with §2.4.
- **Starting tier** — where he sits at 16–17. Default low (None / Raw), because teenagers haven't put in the reps. A high starting tier is the "raised on it from childhood" roll — e.g. a prodigy who arrives already Proficient at curling because his father drilled it into him. A player arriving already at *Mastery* is a rare jackpot, not the norm: rarity is what makes it a story.

The current tier then climbs toward the ceiling over a career, **never past it** — you can fail to reach a ceiling; you can never exceed it. There are three acquisition paths, and not every trait uses every path:

| Path | How it climbs | Trait family |
|---|---|---|
| **Innate** | Rolled in at creation (aptitude + possibly a high starting tier) | Any |
| **Trainable** | Spend training focus to grind a tier up, gated by aptitude | Technical / positional |
| **Experiential** | Only emerges from *playing* — never from the training ground | Mental / reading |

This split is not invented; it applies the age-curve law already in §5.1 — technical attributes have *high trainability*, mental ones *grow with experience, not just training*. Traits inherit that law:

- **Trainable traits** (Finesse Merchant, Poacher, Press-Resistant…) can be ground out on the training pitch, capped by aptitude.
- **Experiential traits** (Big-Game Player, Reader, Leader…) cannot be drilled. You can't practise "ice in a final" on a Tuesday. They ripen only through living the relevant situations in real matches.

---

#### A.4 Hidden ceilings — develop to discover

A player's trait ceilings are **never shown as numbers**. This keeps faith with §5.3: *"you never know exactly what you've been given until you develop it."* A player with a zero ceiling for a trait isn't flagged; he simply trains and trains and never breaks through — and the player works it out the honest way.

To keep this from being blind dice-rolling, three layers of signal sit above the hidden ceiling, from opaque to legible:

| Layer | What the player perceives |
|---|---|
| **The ceiling itself** | Fully hidden — no number, ever |
| **Rate of progress** | *Felt* — gains slow as the ceiling nears (diminishing returns). Steady climb then a stall reads as "probably near the top" |
| **Coach opinion** | A soft, possibly-biased steer ("his foot has no gift for the curl — put him on finishing instead"), surfaced in the §8.7 voice, not as fact |

Crucially, a low ceiling is **not a dead end**. A capped player still reaches Raw or Proficient and still *uses* the trait at an ordinary level — he just never becomes its artist. Training "the wrong trait" costs some time, not a ruined career: a suboptimal choice, never a catastrophe. The penalty is gentle on purpose.

---

#### A.5 Trait catalogue (illustrative)

The full library is content-pipeline work. The set below shows the *shape* of the system across the pitch; it is not exhaustive and not balanced. Each trait names its engine hook (§A.2) and acquisition family (§A.3).

**Mental / Temperament — apply to every position.** These live in the headspace layer (§6.2), so they are role-agnostic; a centre-back sweats a final the same as a striker.

- **Big-Game Player** — *headspace, experiential.* On finals / derbies / knockouts: resists Nerves, lifts the Flow baseline for the whole match. Ripens by *playing* big matches well; it cannot bloom at a club that never reaches them — the George Best trap of §4.1, paid off mechanically.
- **Flat-Track Bully** — *headspace, experiential.* The dark mirror: dominant against weak sides, freezes on the big stage. Emergent antagonist to Big-Game.
- **Hothead** — *headspace.* Frustration climbs fast → reckless choices and cards. Feeds directly into discipline (§6.3).
- **Leader** — *headspace, experiential.* Buffs the headspace of *teammates* around him, not himself — the first hook for collective influence.
- **Slow Starter** — *headspace.* Weak early in a match / early in a season; strengthens as it wears on.

*Note on Big-Game vs Composure.* Composure is an **attribute** (a number, §5.1) — the static temperament that damps volatility in *any* pressured moment, and it climbs slowly with minutes played while scarring only under rare catastrophes (a missed decisive penalty, a long injury). Big-Game is a **trait** — earned, context-specific knowledge of the *big stage*. A cool-headed 17-year-old (high Composure, no Big-Game) does *not* bottle his first final — he wobbles but holds — and because he performs, he *accrues* Big-Game faster than a low-Composure peer who freezes. The two interlock without overlapping: Composure governs survival in the moment; Big-Game is the reward for having survived enough of them.

**Striker.**
- **Finesse Merchant** — *signature finish, trainable.* Unlocks the curled far-corner branch on edge-of-box chances; contest leans on technique + composure over power.
- **Poacher** — *beat-summoner + finish, trainable.* Loose-ball-in-the-six-yard-box beats fire more often; close-range finishing buffed.
- **Ghost (Raumdeuter)** — *beat-summoner, experiential.* The Müller archetype. "Drift into space, arrive unmarked in the box" beats fire more often. Reading space is knowledge, not a drill — hence experiential. This is the clearest proof of the §A.2 beat-summoner hook.
- **Target Man** — *beat-summoner, trainable + physical.* Unlocks "receive back-to-goal, hold up, lay off" beats — generating beats for *teammates* as well as the player.
- **Clinical** — *stat-shaper, trainable.* Reduces headspace dependence when finishing — fewer chances spurned to nerves.

**Midfield.**
- **Playmaker** — *beat-summoner.* "Receive in midfield and open a chance" beats fire more; creative passing branches unlock.
- **Through-Ball Maestro** — *choice-unlocker.* Adds the line-splitting pass branch ordinary players don't get.
- **Engine** — *stat-shaper, physical.* Stamina drains slowly; holds tempo across the full 90.
- **Press-Resistant** — *signature contest, trainable.* Under pressing: unlocks escape branches and buffs ball-retention contests in tight space.
- **Box-to-Box** — *beat-summoner.* Participates in both attacking and defensive beats, never locked to one phase.

**Defence.**
- **Ball-Winner** — *beat-summoner.* Tackle / interception beats fire more; challenge contests buffed.
- **Reader** — *beat-summoner, experiential.* The defensive twin of Ghost. "Read the attack, cut the passing lane, cover" beats fire more often. Anticipation is earned by playing, not drilled.
- **Ball-Playing Defender** — *choice-unlocker.* Unlocks attack-initiation branches from the back line.
- **Aerial Dominator** — *beat-summoner, physical.* Aerial-duel beats (both boxes) fire and buff.
- **Last-Ditch** — *signature contest.* Unlocks the desperate goal-saving tackle — high risk: it rescues the game or concedes a penalty / card.

**Goalkeeper.** Deferred, consistent with §11 — the GK beat library and role math are parked, and so is the GK trait set.

---

#### A.6 Where this slots into the interlock map (§10)

Traits add edges to the existing web rather than a new island:

- **Aptitude (rolled) → trait ceiling → mastery climb → beat behaviour.** A trait-level mirror of the talent → potential → development spine; money and facilities (§8.8, §4.2) accelerate the climb for *trainable* traits, but lift no ceiling — you still cannot buy talent (§2.4).
- **Beat-summoner traits → the §6.1 selector.** This is the edge that was missing in the base design: off-ball reading and positioning finally *create* beats, closing the gap §A.0 named.
- **Experiential traits → big matches → trait unlock.** Ties trait growth back to Nationality and club choice (§4): the stages you reach decide which traits can ripen at all.
- **Mastery tiers → new choices/beats → richer Output.** Higher mastery widens what a player can attempt on the pitch, feeding the Output axis of the legacy case (§8.1) — not just by doing the same things better, but by doing things others can't.

*End of Appendix A. Catalogue expansion, mastery-tier tuning, and the goalkeeper trait set follow as content / later documents.*

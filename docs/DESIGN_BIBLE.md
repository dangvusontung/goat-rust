# BECOME THE GOAT
### Design Bible

*A single-career football life-sim. You don't win the game — you build a case.*

**Status:** Design locked (planning phase). Numbers, UX, and art deferred.
**Platform target:** Mobile-first.
**Document scope:** The complete design. Not an MVP slice — the whole machine.

---

## 0. How to read this document

This is the design spine, not a spec sheet. Every system below is **locked in intent** but **unnumbered** — exact values (attribute weights, energy costs, growth rates) come later, against a prototype. Where a number appears, treat it as a placeholder illustrating the *shape* of the rule, not a final value.

The document is ordered from the inside out: fantasy → pillars → architecture → the player → the match → the world → career & meta → engineering → how it all interlocks.

---

## 1. Vision & Core Fantasy

You play **one footballer**, from a teenager in an academy to retirement, and you chase the only title that can never be formally awarded: **the GOAT**.

The fantasy is not "win the league." Leagues are won and forgotten. The fantasy is **building a legacy that people argue about** — the way Pelé, Maradona, Messi, and Ronaldo are argued about, where no amount of silverware ends the debate. You are assembling evidence for a case that is litigated forever by a living footballing culture.

That single decision — *legacy as the goal, not victory* — reshapes every other system. There is no final score. There is only the case you build and the pantheon you climb.

---

## 2. Design Pillars

Five tenets. Everything else is downstream of these.

### 2.1 No win condition — legacy, not victory
GOAT status is an **eternal, unresolvable debate**. The game never crowns you. Instead, a living **pantheon** ranks you — *plurally*, through several "schools" of opinion that weight greatness differently and never agree by design. Scoring exists, but only as **debate material and a progress readout**, never as a terminal state. The carrot that replaces "you win" is **climbing the live pantheon** and seeing the culture's opinion of you shift in real time.

### 2.2 Manage by exception
A full-fidelity, 20-year career is unplayable if you touch every detail. So every system **auto-runs by default** and **surfaces a decision only at weighty moments**:

- **Matches:** skip (auto-resolve) or play. When you play, you're handed *moments*, not 90 minutes of busywork.
- **Training:** set a routine; intervene only on the weeks that matter.
- **Social / media / life:** runs in the background; interrupts you at flashpoints.

This is the single mechanic that makes the whole simulation fit on a phone and inside a human attention span.

### 2.3 Seed-determinism
The entire universe — one shot's RNG up through every club, player, and decade of fake history — derives deterministically from a **single seed**. This buys us: replayability, shareable universes ("play my save's world"), and the ability to re-roll or reconstruct state on demand instead of storing it.

### 2.4 One lottery + chosen circumstances
**Talent is the one thing you cannot choose and cannot buy.** It is rolled. Everything around the talent — *who you are, where you start, what you play* — you author at creation. You write the circumstances; fate rolls the gift.

### 2.5 Output is not the same as winning
What *you* do on the pitch (your **Output**) is simulated separately from whether your *team* wins. You can have the game of your life and still lose 3–2. This split is deliberate and load-bearing: it's what lets a one-club minnow legend exist, and it's what makes the legacy debate rich instead of a trophy count.

---

## 3. System Architecture

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

## 4. Character Creation

You make **four choices**. The game rolls **one lottery**.

| You choose | Effect |
|---|---|
| **Name** | Identity / flavor. |
| **Position** | Biases your rolled potentials toward that position and seeds your starting natural role(s). You pick *what you want to be*; the dice decide *how good you can get*. |
| **Nationality** | The difficulty + story dial (see below). Sets your starting region and football pyramid. |
| **Starting club** | The develop-vs-minutes dial (see below). |

**The game rolls:** your **talent** — the potential ceiling and the per-attribute potentials. This is never chosen and never purchasable.

### 4.1 Nationality as the difficulty / story dial
Nationality is not cosmetic. It is the primary narrative-difficulty selector:

- **Powerhouse nation:** easier to win major international honors, but you are one of many world-class players fighting for the national-team shirt and for the spotlight. Trophies are reachable; *standing out* is hard.
- **Minnow nation:** you can become a **national god**, but winning a World Cup or continental title is borderline impossible no matter how good you are. This is the **George Best run** — a deliberately chosen tragedy/challenge where the legacy case has to be built almost entirely at club level and through sheer individual output.

This single choice tilts which **legacy axes** (§8.1) are even available to you, and is the cleanest way the player authors the *kind of story* they want before a single ball is kicked.

### 4.2 Starting club as the develop-vs-minutes dial
Where you start sets up the central early-career tension:

- **Big club:** elite facilities and coaches → **fast development**, but you're buried on the bench behind established stars → **few minutes**.
- **Small club:** weaker facilities → **slower development**, but you **play immediately** → minutes, form, and a growing reputation.

This dilemma (develop vs. play) recurs your whole career and is the engine behind the **loan system** (§5.4), which exists precisely to resolve it.

---

## 5. The Player

### 5.1 Attributes
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

### 5.2 Roles & Multi-Role
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

### 5.3 Player Generation
Talent is **fully random**, generated through a pipeline:

1. **Roll the ceiling** — overall potential band.
2. **Roll role DNA** — biased by the chosen position; which roles come naturally.
3. **Roll per-attribute potentials** — distributed under the ceiling, shaped by role DNA.
4. **Set current values at 16–17** — physical attributes start at a high % of potential (teenagers are already fast); mental attributes start low (teenagers don't read the game yet).
5. **Seed familiarity** — natural + adjacent roles.

The result: two strikers with the same OVR can be completely different players, and you never know exactly what you've been given until you develop it.

### 5.4 Growth, Training & the Week
**Development** is the process of pushing **current → potential**, gated by the age curve. You can't exceed potential; you *can* fail to reach it.

- **Training** targets a **specific attribute** at a chosen **intensity**. Intensity costs **energy**.
- **Energy / fatigue:** tired players gain less and injure more; rest recovers energy. This is a constant resource-management loop under the training system.
- **The week** is the core loop unit: set a **routine**, then **intervene** only on big weeks (a derby, returning from injury, a dip in form).
- **Facilities & coaches** (set by your club, §4.2) multiply development speed — which is exactly what creates the **develop-vs-minutes** dilemma.
- **Loans** exist to resolve that dilemma: a young player at a big club goes out to play real minutes, then returns developed *and* match-sharp.
- **Random development events:** injuries, illness, breakthroughs, form spikes — surfaced by exception, never spammed.

---

## 6. The Match

### 6.1 The Beat Engine
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

### 6.2 In-Match Headspace
The player has a **live, multi-axis psychology** that exists only within a match:

- **Confidence**, **Nerves**, **Frustration**, **Flow** (and **desperation** as a contextual combination of these under pressure).
- Beats **ripple** into headspace: miss an open goal → confidence drops, you turn timid/hesitant; score → flow rises; get fouled repeatedly → frustration climbs → reckless choices and cards.
- Headspace feeds **all three layers** of the engine: the **odds** of a contest, **which choices appear**, and **which beats trigger** at all.

**Composure** (an attribute) governs **volatility and recovery speed** — the ice-man shrugs off a miss and resets; the mercurial talent spirals. This is what makes two players with identical technical stats feel completely different in a final.

**Form vs. headspace:** **Form** is the slow, season-long baseline. **Headspace** is the fast, in-match deviation around it. A in-form player can still have a nervy night; an out-of-form player can catch fire for one match.

### 6.3 Discipline
Cards emerge from the same systems, not a separate dice roll:

- Driven by **beat choices** (the professional foul), **headspace** (frustration → red mist), the **Aggression** attribute, and the **referee**.
- **Cards as a tactical tool:** the cynical foul to stop a counter, the DOGSO sacrifice — sometimes the right play *is* the foul.
- **Referees have personalities** (strict ↔ lenient) but **no per-player memory**. However, a **dirty reputation precedes you** — officiating tightens around players known to be dirty. Reputation, not memory, does the work.
- **Accumulation** → suspensions → missed matches → lost output, lost sharpness, lost legacy opportunities.
- Over a career this builds an identity: the **enforcer** vs. the **clean technician**.

---

## 7. The World

### 7.1 Living World & Simulation Strategy
The world is **fully alive**: multiple leagues, a complete transfer market, stars rising and falling, youth regenerating. The trick is doing this on a phone.

**Everyone has full-fidelity stats** (stats are cheap in memory). What's *tiered* is **simulation depth**:

- **Deep-sim your orbit** — your club, your league, your direct rivals get full match-by-match treatment.
- **Cheap batch-tick the rest** — distant leagues advance at season granularity.
- **Lazy-promote on contact** — a player only gets fully realized the moment he becomes relevant to you (you face him, he's linked with a transfer).
- **Background growth is formula-driven** — a non-orbit player's current attributes are *computed on demand* from (seed + birth data + date), not stored and stepped every week.
- **Season-granularity batch-tick** handles the path-dependent stuff that can't be pure formula: league tables, top scorers, transfers between AI clubs, records.

### 7.2 World Genesis
A **one-time build** per save, derived entirely from the seed:

1. **Structure** — nations and leagues across the powerhouse ↔ minnow spectrum, with full pyramids.
2. **Clubs** — with *rich identity*: history, rivalries, philosophy, stature, finances.
3. **People** — ~**20,000–30,000 full-fidelity players** plus youth pools.
4. **A seeded pantheon** — the canon of past greats — and **decades of fake history** generated by running the batch-tick backwards/forwards so that old Ballon d'Ors, records, and legends are *internally consistent*, not random labels.

- **Identity generation:** procedural + templates, with **LLMs at authoring time** for flavor (club histories, pundit voices), never at runtime.
- **Performance estimate:** ~3–10s naive, ~1–3s with lazy generation. One-time, on a background thread, behind a flavorful loading sequence. The **history batch-sim is the dominant and most variable cost**. Use **struct-of-arrays** for the 20–30k population — *do not* allocate 30k reference-type instances (ARC/GC will punish you).
- **Loading an existing save** is just deserialization: well under a second.

### 7.3 Transfer Market & AI Clubs
AI clubs are **deep agents**, not backdrops:

- Each has a **strategy**, **finances + budget**, a **squad-building plan**, and its **own manager**.
- The market is **fully living** — clubs trade *each other*, and your teammates arrive and leave, shifting chemistry and morale around you.
- Every transfer window is a **saga**, surfaced by exception.

### 7.4 The Emergent Rival
There is **no scripted nemesis**. At genesis the game grows a **cohort of peers** alongside you, advancing through the cheap tick. A rivalry **crystallizes retroactively** — *if* one peer keeps pace with you over years, the media frames it as your defining rivalry (the Messi–Ronaldo dynamic), but it was never assigned.

Crucially, this is **truly emergent**: sometimes **no one keeps up** and you reign alone — which gets its own flavor *and* a "weak era" asterisk the pantheon's harsher schools will hold against you. When paths cross, matches carry rivalry flavor, and the head-to-head record feeds directly into the legacy debate.

---

## 8. Career & Meta

### 8.1 Legacy & the Pantheon — *the spine of the game*
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

### 8.2 Reputation
Reputation is **four distinct facets**, not one bar:

| Facet | Drives |
|---|---|
| **Sporting** | Contract value, transfer interest |
| **Marketability / Image** | Sponsors; the *Icon* legacy axis |
| **Character / Professionalism** | Wrecked by strikes, scandals, dirty play |
| **Club / Fan** | Standing with your club's supporters |

They move independently — you can be a sporting titan with a trashed character rep, or a beloved clubhouse hero with modest marketability.

### 8.3 Player Power & Leverage
An **escalation ladder** for getting what you want:

```
quiet request → transfer request → media agitation → skip training → full strike / AWOL
```

- Each rung raises **sale pressure** but burns **Character reputation** and risks club retaliation.
- The club can **call your bluff** and freeze you out.
- Leverage **only works with stature** — a squad player has none.
- Resolution is driven by **contract years left**, **form**, and **squad importance**.

### 8.4 Contracts & Negotiation
Contracts carry: **wage, length, signing + loyalty bonus, release clause, performance bonuses, image rights**.

- **Squad-status and role promises are enforceable leverage** — a broken "you'll be our starting striker" promise is *legitimate grounds* to agitate (§8.3).
- **Length is the spine of player power** — running a contract down toward a Bosman flips control from club to player; locking a player in long does the reverse.
- **Age bends value** continuously.
- **Negotiation depth is a choice:** handle the full deal yourself, or **delegate to an agent** — and agent quality matters.

### 8.5 Sponsors & Commercial
- **Gated by Marketability** (§8.2).
- Tiers escalate **local → national → global**, with global endorsements as an end-game **Icon** play.
- Deals carry **obligations** that cost **time and energy** (the resource you also need for training).
- **Over-commercializing takes a reputation hit** — the player who's all billboards and no football.

### 8.6 Off-Pitch Life & Lifestyle
- **Relationships:** a **few key tracked threads** (partner, family, close friends) plus events — *medium depth, not a dating sim*.
- **Lifestyle strongly affects longevity:** a professional lifestyle extends your peak; partying burns you out early.
- The **high life is risk/reward:** marketability and money up, scandal risk up.
- This creates a genuine **identity fork:** the **professional/private** path (longevity, the long quiet legacy) vs. the **flashy/icon** path (cultural footprint, shorter burn). You cannot fully have both.

### 8.7 Pundits & Media
The media engine is three things: a **news feed**, **pundit debate**, and **award nights** (the emotional peaks that anchor retention).

- **Pundits are named, recurring characters** with their own bias and personality, following you across your whole career: the **doubter** you can eventually win over, the **champion** in your corner, the **stats nerd**, the **eye-test romantic**. They are the **voice of the plural pantheon** (§8.1) — the rankings made human and argumentative.
- **Media interaction is a flashpoint:** the world auto-reports your career; you only step in at hot moments (a presser after a red card, a transfer-saga statement).
- **Text is template + slot, authored with LLMs offline** — same approach as the beat engine.

### 8.8 Economy & Money
Money is a **real, managed resource you can run out of**:

- You **spend and invest**; you can go **broke**.
- Money **buys gameplay advantage** — private trainers, nutrition, recovery → faster development, longer longevity.
- **Guardrails keep it from breaking the game:**
  - It is **capped by potential** — money buys you *toward* your ceiling, never past it. **You cannot buy talent** (§2.4).
  - It is **counterweighted by bankruptcy risk** — overspend and it bites.
  - You **start poor**, so the early come-up is unaffected by the advantage loop.
- **Deep investment / business layer:** profit and loss, the tragedy of a bankrupt ex-star, or the post-career **empire** that becomes part of the *Icon* legacy.

---

## 9. Engineering & Performance Notes

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

## 10. System Interlock Map

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

## 11. Open Questions / Parked / Deferred

Explicitly *not* decided yet — deferred by design, not forgotten:

- **Goalkeeper as a playable career.** The 7th attribute family exists; the GK-specific beat library and role math are parked.
- **Numbers & tuning.** All weights, multipliers, energy costs, growth and decline rates, contest probabilities — to be set against a prototype.
- **UX & art direction.** Deferred *because* of the swappable-renderer architecture (§3); the core doesn't wait on it.
- **Beat-library content.** An ongoing authoring effort — the engine is defined; the volume of authored beats is a content pipeline, not a design question.
- **Deeper relationship web.** Beyond manager and key teammates — possible later expansion, currently scoped to a few key threads.

---

## 12. Glossary

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
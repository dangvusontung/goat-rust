# BECOME THE GOAT — Appendix A
## The Match, Deep-Dive: Beats, Layout & Output

*Companion to the Design Bible §6 (The Match) and §8.1 (Legacy). Status: design locked in intent, numbers deferred — same convention as the Bible. Where a rule references the Bible it is marked (§). Everything else is an extension built on top of the Bible's locked principles, not a quote from it.*

---

### A.0 How to read this appendix

This document goes one layer deeper than Bible §6. It does **not** introduce new top-level systems — every rule here is assembled from primitives the Bible already locked: the beat engine (§6.1), the two-axis split (§2.5), familiarity (§5.2), role-as-lens (§5.2), reputation-not-memory (§6.3), and the headless-core / swappable-renderer architecture (§3). The job of this appendix is to resolve the questions §6 left open, and to flag the new design laws those answers imply.

Everything below is **structure, not numbers.** Danger tiers, familiarity multipliers, selector weights and swing values are illustrative shapes, set against a prototype (§11).

---

### A.1 The two axes are coupled one way only

The Bible splits Output from team result (§2.5, §6.1). This appendix locks the *direction* of their relationship:

**Output → Result, never the reverse.** Your Output is injected into the team's attacking/defending strength according to a position weight; the rest of the team is an independent stochastic process. A winger's hat-trick *tilts* the win probability but never *decides* it — ten other players and a goalkeeper can still lose the match.

This is the precise mechanism that keeps the minnow legend alive (high Output, low Result, because the ecosystem can't carry it) while still giving Output emotional weight. Full-independence would make Output feel meaningless; two-way coupling would collapse back into "play well = win" and kill §2.5.

---

### A.2 Beats are selected, not generated

Restating §6.1 precisely, because the distinction is load-bearing:

- **Content** is authored offline — hand-authored mini-trees for big moments, LLM-generated variations for the connective tissue — and baked in. No runtime model calls; the game is fully offline (§6.1, §9).
- **At runtime the engine selects** an eligible beat from the baked library via a context-weighted selector, then fills a template-with-slots. It never authors a new beat mid-match.

**Count per match is deliberately undefined** (§11). It is emergent from momentum, stamina, and the play-or-skip choice (§2.2, §6.1) — not a fixed number. The real figure (enough beats that a season doesn't feel repetitive on a phone) is a prototype tuning question.

---

### A.3 Beats are classified by situation, not by position

There is no "striker beat" or "defender beat." Beats are organized by **phase of play + tags + context** (§6.1). A striker meets many shooting beats not because those beats are labelled for strikers, but because a striker spends more time in final-third situations.

**Position/role biases the selector's weights** toward the situations that fit the role — it does **not** lock a beat list. This is why off-role moments emerge naturally (a striker tracking back in stoppage time, a centre-back one-on-one with the keeper from a corner) without any position-specific authoring.

**What a player can *do* inside a situation** is decided by per-choice gating on attributes/traits (§6.1), not by a position label. Same beat, different visible choices per player — this is the role-as-lens principle (§5.2) applied at the choice level, and it is why two players of identical OVR play differently (§5.3).

---

### A.4 One beat yields many decisions (design discovery)

A single winger beat (`FinalThird`, ball at feet, defender engaging) produces a full set of distinct decisions from **one** beat, not several:

| Choice | Contest uses whose attrs | Worst-case branch |
|---|---|---|
| Pass to the striker | your passing → then *his* finishing (actor swap) | he fluffs it, you lose the assist |
| Cut inside and shoot | *your* shooting | balloon it, lose possession |
| Dribble past the CB | your dribbling vs *the CB's* defending | dispossessed in a exposed area → counter |
| Signature move (hard-gated) | dribbling + flair (only if above threshold) | lose it wide, less exposed than inside |

This collapses what looks like four beats into one, which is the single biggest relief on the content pipeline. The library covers *situations*; the choices fan out within them.

---

### A.5 A teammate's / opponent's quality enters through the contest, not a background coefficient

This is a locked principle. Other players' OVR is **a real attribute on the far side of a specific contest, in a specific moment** — never a team-wide number added somewhere:

- **Opponent** → the difficulty of your contest (dribble past a hard CB).
- **Teammate you serve** → the outcome of the branch *after an actor swap* (you pass; *his* finishing decides the goal).
- **Teammate who serves you** → the condition for your beat to fire at all (off-ball: *his* vision + passing decides whether you even get the ball).

Because attributes are role-agnostic and a contest is just attrs-vs-attrs + RNG (§5.2, §6.1), the engine doesn't need to know who is teammate and who is opponent — it swaps the actor pointer and reads a different attribute set. Friend and foe enter through the **same mechanism**, differing only by which side of the comparison they sit on.

**Consequence:** squad quality is *felt*, not *announced*. You never read "opponent OVR 72." You live it — every pass fluffed, every run unseen, every CB cutting you out. A junk team shows up as a stream of concrete failed moments around you, which is exactly what makes a minnow legend *heavy* (§2.5, §7.4).

---

### A.6 Off-ball is a beat that runs *before* the on-ball beat

Off-ball positioning (§6.1) is its own beat, sitting in front of the on-ball beat and determining its *quality*. Choose "drift into the channel" → succeed → the on-ball beat fires in a good position with a richer choice set. Choose poorly → the on-ball beat fires in a bad spot with a poorer choice set.

The discovery: off-ball must be a **prior beat, not a tag on the on-ball beat** — because it is the hidden place where two wingers of equal technical OVR diverge (the one with high positioning/off-ball *manufactures himself better on-ball beats*).

**Off-ball resolves on someone else's attributes**, so it needs an anti-discouragement layer:
- **Make the reason visible.** When a run is wasted because the passer didn't see you, the text says so — frustration lands on the *teammate* (feeding chemistry, §7.3), not on the game.
- **Make the run worth something anyway.** A good run stretches the defence → small momentum buff or opens a beat for another teammate, even if you never receive the ball. The quiet work of a good off-ball player.

Without this layer, players stop choosing off-ball ("why run, no one passes") and a mechanic dies.

---

### A.7 "The teammate resents you" = chemistry/morale, not memory

The desire for a teammate to sour on you after repeated selfish, failed choices is honoured through a **light ripple onto chemistry/morale** (§7.3), accumulating as a *tendency* (chronic ball-hogging), not a one-shot reaction.

It is **not** built as per-event teammate memory. That would contradict the Bible's locked "reputation, not memory" stance for referees (§6.3); the same reasoning (cost, testability) applies to teammates. Chronic selfishness also feeds the Character/Professionalism reputation facet at the career scale (§8.2).

---

### A.8 Design law: failure severity scales with choice risk

A locked law, applied to every beat, not left emergent:

> The riskier the choice, the more punishing its failure branch.

Dribble inside (riskiest, exposes you centrally) → failure feeds a counter → conceded goal. Safe shot → failure resets play. This consistency is what lets the player *learn to read risk*; if it drifts, the risk language becomes unreadable.

---

### A.9 Defensive beats force Output to count "what didn't happen"

A defensive beat (covering a counter: tactical foul vs chase-back) is the first beat type where **every choice leads to a loss, only the *kind* of loss differs** — the best outcome is damage minimised, not a goal scored.

This exposes a gap the Bible left open: it has an Output axis (§8.1) but never says Output includes defensive actions. If Output silently means goals + assists, a great centre-back is **invisible to the pantheon**. A.10 fixes this.

---

### A.10 Output's unit of measure: goal-probability swing × stage × difficulty

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

### A.11 No coordinate system — danger lives in semantic zones

Goal-probability swing (A.10) is **not** computed from (x,y). The Bible's match is explicitly not a physics sim (§6.1); introducing coordinates would drag in movement simulation, weight, and the exact 22-player sim the Bible refused.

Instead, **danger is a discrete property of the semantic zone** a beat occurs in (near post / penalty spot = VeryHigh; edge of box = Medium; midfield wing = Low). It is metadata, either authored or inferred from the layout (A.13) — never simulated. Discrete tiers, not a continuous probability.

---

### A.12 Position is a discrete semantic layout in the core; pixel coordinates live in the renderer

The layout exists to be drawn — but the drawing belongs to the **renderer layer**, not the core (§3, headless core + swappable renderer).

- **Core** holds ~18–20 **named zones** (near post, far post, penalty spot, right half-space, …) — fine where decisions cluster (the box, the attacking flanks), coarse where they don't (own half, midfield). The core uses zones for contest, danger, and choice generation. It never knows a zone is pixel (x=112, y=45).
- **Renderer** translates each semantic zone → an anchor point (+ jitter radius) and places the sprite. Swapping text → 2D → 3D only swaps this lookup table; the core is untouched.

This keeps determinism intact (pixel data never enters core state, §9) and keeps the text-first build shippable (§3). In MVVM terms: **semantics are the model, pixels are the view layout** — layout never leaks into the model.

---

### A.13 Generate the layout, inside boundaries

Layouts are generated at runtime (variety) but constrained by author-time boundaries (sanity). Four boundary tiers:

1. **Shape** — the beat declares its valid slots (ball-carrier, an engaging defender, 1–2 pass targets). The generator may not invent slots outside this list.
2. **Role** — each slot declares valid roles (the in-box pass-target slot rejects a goalkeeper, except set-pieces). This is what stops "keeper at the far post."
3. **Spatial coherence** — slots are defined *relative to each other* (the "engaging defender" = nearest to the carrier, not an absolute point), so a layout is self-coherent by construction.
4. **Continuity** — the layout **inherits position from the previous beat** through the transition (§6.1). A transition doesn't just pick the next beat; it carries the spatial state forward, so a chain of beats reads as one continuous passage rather than disconnected snapshots.

Inside all four, the generator draws real players from the match squad, deterministically via injected RNG (§9). Diverse but never absurd. Most of the cost is author-time *type* definition (declare slots + roles per beat type), not per-instance authoring — a relief on the pipeline.

---

### A.14 Resolution rotates with the player's role

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

### A.15 Playing out of position — familiarity is the valve between match and meta

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

### A.16 Still open / deferred (this appendix)

Explicitly not yet resolved:

- **Discipline / red-mist beats** — beats *born from* headspace (frustration + aggression + referee, §6.3) rather than from the run of play; the reverse of the usual play→headspace flow.
- **Set-piece beats** — the hand-authored mini-tree case (§6.1); penalties, corners, free-kicks.
- **The full per-position axis/zone table** for striker / midfielder / defender.
- **The selector's concrete weighting function** — eligibility filter, dynamic weights, cooldown, stable ordering (the determinism trap: never iterate the eligible set in hash-map order).
- **The off-ball "worth something anyway" momentum math** (A.6).

### A.17 Where the real risk sits

The engine is the easy part. The two genuine risks both sit in places the Bible already marked deferred (§11):

1. **Content pipeline** — generating enough beat variety, and tagging danger (A.11) and slot/role boundaries (A.13) correctly. This is authoring work, not engine work — the engine stays clean.
2. **Tuning the rhythm** — making a match *feel* like it rises and falls (not eight shooting beats in a row, not three dull build-up beats), and tuning the Output baseline (A.10). This is a feel risk, surfaced only against a running prototype.

Nothing the appendix designs forces the architecture to break its locked principles. The one thing that *would* break it — giving teammates per-event memory (A.7) — is explicitly routed through chemistry instead.

---

*End of Appendix A. Numbers, the selector function, set-piece and discipline beat libraries, and the goalkeeper career (§11) follow separately.*
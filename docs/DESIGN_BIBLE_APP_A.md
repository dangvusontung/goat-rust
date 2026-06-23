# BECOME THE GOAT — Appendix A

## Traits & Mastery

*Companion to §5 (The Player), §6 (The Match), and §11 (Open Questions). Status: design-locked in intent; the full trait catalogue is a content pipeline, not a design question — same posture as the beat library (§11). Numbers, bands, and per-trait tuning deferred against a prototype.*

---

### A.0 Why this appendix exists

The core design (§5–§6) defines attributes and how they feed beats, but it leaves a gap between the two: attributes resolve a contest *once a beat is already running*, yet nothing decides **which beats appear** or **how a player plays them** based on his individual signature. Two strikers with identical Shooting can post the same OVR (§5.3) and still feel nothing alike — one bends finesse shots into the far corner, the other ghosts in behind the line for tap-ins. Attributes alone don't carry that difference.

Traits are the missing layer. A trait is a **special skill** that bends the match engine in a player-specific way. Where an attribute is a number that scales a contest, a trait changes the *shape* of play: it summons beats, unlocks choices, rewrites how a finish resolves, or colours the player's in-match psychology. Traits are what make the legacy debate (§8.1) argue about *players*, not stat lines.

---

### A.1 What a trait is

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

### A.2 The five ways a trait bends the engine

Every trait is classified by where it plugs into the match engine (§6). This keeps the catalogue honest — a trait that can't name its hook is not yet designed.

1. **Signature finish** — opens or rewrites how an existing contest resolves. *(Finesse Merchant.)*
2. **Beat-summoner** — makes a class of beat *fire more often* via the context-weighted selector (§6.1). This is the hook that finally lets off-ball and reading play *create* beats, not just gate choices inside them. *(Ghost, Reader.)*
3. **Choice-unlocker** — adds a branch inside a beat that wouldn't otherwise be on the menu. *(Through-ball Maestro.)*
4. **Headspace modifier** — feeds the in-match psychology of §6.2 (Confidence / Nerves / Frustration / Flow). *(Big-Game Player.)*
5. **Stat-shaper** — bends a contest without creating a beat. *(Clinical, Engine.)*

---

### A.3 How traits are acquired

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

### A.4 Hidden ceilings — develop to discover

A player's trait ceilings are **never shown as numbers**. This keeps faith with §5.3: *"you never know exactly what you've been given until you develop it."* A player with a zero ceiling for a trait isn't flagged; he simply trains and trains and never breaks through — and the player works it out the honest way.

To keep this from being blind dice-rolling, three layers of signal sit above the hidden ceiling, from opaque to legible:

| Layer | What the player perceives |
|---|---|
| **The ceiling itself** | Fully hidden — no number, ever |
| **Rate of progress** | *Felt* — gains slow as the ceiling nears (diminishing returns). Steady climb then a stall reads as "probably near the top" |
| **Coach opinion** | A soft, possibly-biased steer ("his foot has no gift for the curl — put him on finishing instead"), surfaced in the §8.7 voice, not as fact |

Crucially, a low ceiling is **not a dead end**. A capped player still reaches Raw or Proficient and still *uses* the trait at an ordinary level — he just never becomes its artist. Training "the wrong trait" costs some time, not a ruined career: a suboptimal choice, never a catastrophe. The penalty is gentle on purpose.

---

### A.5 Trait catalogue (illustrative)

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

### A.6 Where this slots into the interlock map (§10)

Traits add edges to the existing web rather than a new island:

- **Aptitude (rolled) → trait ceiling → mastery climb → beat behaviour.** A trait-level mirror of the talent → potential → development spine; money and facilities (§8.8, §4.2) accelerate the climb for *trainable* traits, but lift no ceiling — you still cannot buy talent (§2.4).
- **Beat-summoner traits → the §6.1 selector.** This is the edge that was missing in the base design: off-ball reading and positioning finally *create* beats, closing the gap §A.0 named.
- **Experiential traits → big matches → trait unlock.** Ties trait growth back to Nationality and club choice (§4): the stages you reach decide which traits can ripen at all.
- **Mastery tiers → new choices/beats → richer Output.** Higher mastery widens what a player can attempt on the pitch, feeding the Output axis of the legacy case (§8.1) — not just by doing the same things better, but by doing things others can't.

*End of Appendix A. Catalogue expansion, mastery-tier tuning, and the goalkeeper trait set follow as content / later documents.*
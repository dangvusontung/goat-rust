# Beat Library Authoring Guide (for LLM generation of `beats.json`)

How to generate valid, good `beats.json` content with an LLM. This is **authoring-time
only** — the LLM produces static JSON data; at runtime the Rust engine selects and renders
it deterministically with **no model dependency** (CLAUDE.md: "LLMs at authoring time
only"). Your output is *data*, never code or runtime calls.

The engine entry point that consumes your file is `BeatLibrary::load(json)` (parses with
serde). If it doesn't parse against the schema below, it's rejected.

---

## 1. File schema (exact)

Top-level object with **three arrays**:

```json
{
  "situations": [ /* RawSituation */ ],
  "choices":    [ /* RawChoice */ ],
  "outcomes":   [ /* RawOutcome */ ]
}
```

### 1.1 `situations[]` — the moment shown to the player
| field | type | required | meaning |
|---|---|---|---|
| `id` | string | yes | unique, snake_case |
| `phase` | string | yes | one of: `attack` `defend` `setpiece` `positioning` `key` |
| `bias` | `[u8,u8,u8]` | yes | selection weight per match third `[early, mid, late]`; higher = more likely then |
| `setup` | string | yes | the situation text (2nd person, present tense) |
| `tags` | `[string]` | yes | used to pick eligible **choices** (see §2) |

### 1.2 `choices[]` — the options auto-resolved against the player
| field | type | required | meaning |
|---|---|---|---|
| `id` | string | yes | unique, snake_case |
| `text` | string | yes | the option label (what the player "tries") |
| `attr` | string | yes | the contested attribute — **must be a valid name (§3)** |
| `difficulty` | `u8` 1–99 | yes | contest target; higher = harder to succeed |
| `tags` | `[string]` | yes | a choice is **eligible for a situation when they share ≥1 tag** |

### 1.3 `outcomes[]` — the resolution text (a SHARED pool)
| field | type | default | meaning |
|---|---|---|---|
| `id` | string | — | unique, snake_case |
| `text` | string | — | what happened (rendered after the choice) |
| `output_delta` | `i16` | — | change to the player's match rating (±) |
| `confidence` | `i8` | 0 | headspace: confidence delta |
| `frustration` | `i8` | 0 | headspace: frustration delta |
| `flow` | `i8` | 0 | headspace: flow/momentum delta |
| `score_event` | string\|null | null | `null` \| `"goal_for"` \| `"assist_for"` \| `"goal_against"` |
| `stamina_cost` | `u8` | 0 | energy drained this beat |
| `next_situation` | string\|null | null | optional: force the next situation by `id` |
| `polarity` | string | `"any"` | `"success"` \| `"failure"` \| `"any"` |

> **⚠️ Outcomes are a GLOBAL pool, selected by `polarity` only — NOT linked to a specific
> choice.** When a choice succeeds, the engine picks any `success`/`any` outcome at random;
> on failure, any `failure`/`any` outcome. So **write outcome text generically enough to
> read plausibly after many different choices.** (Per-choice outcome text would need an
> engine change — out of scope; don't author as if outcomes are choice-specific.)

---

## 2. How the pieces connect (the rules you must satisfy)

- **Situation → choices:** at match time the engine takes a situation and gathers every
  choice that shares **at least one tag** with it. **Each situation needs ≥2 eligible
  choices** (ideally 3–5) or it can't render. Plan tags so every situation has enough.
- **Choice → outcome:** the contest (`attr` vs `difficulty`, modified by form/headspace +
  RNG) yields success/failure; the engine then pulls a matching-polarity outcome from the
  shared pool. Keep a healthy pool of both `success` and `failure` outcomes.
- **Match shape:** a match is 15 beats — 5 early, 5 mid, 4 late, +1 **climax**. The climax
  is chosen from `phase: "key"` situations, so **include several `key` situations** with
  dramatic, decisive setups.
- **Periods:** `bias` skews when a situation appears. e.g. `[3,2,1]` = early-game flavour;
  `[1,2,3]` = late drama; `[2,2,2]` = anytime.

---

## 3. Valid `attr` names (use these exactly)

Canonical (camelCase, no spaces):

```
Acceleration SprintSpeed Finishing LongShots ShotPower Volleys Penalties
ShortPassing LongPassing Vision Crossing FreeKickAcc CloseControl BallControl
Agility Balance Reactions StandingTackle Marking Interceptions Heading Curve
AttPositioning Strength Stamina Aggression Jumping Composure Bravery SlidingTackle
```

Pick the attribute the choice genuinely tests (a through-ball → `Vision` or `ShortPassing`;
a 30-yarder → `LongShots`/`ShotPower`; a last-ditch block → `SlidingTackle`/`Reactions`).
An unknown `attr` makes the choice unusable, so spell them exactly.

## 4. Tag vocabulary (for choice↔situation eligibility)

In use today (extend sparingly, keep them meaningful):

```
attack defend setpiece positioning key            ← phase-ish anchors
wide dribble passing shooting delivery aerial      ← attacking flavours
tackle pressing reading                            ← defending flavours
discipline physical aggressive safe                ← modifiers
```

Give each situation 2–3 tags and each choice 2–4, so the **intersection** yields a sensible
choice set. (A "cross_opportunity" situation tagged `["attack","wide","delivery"]` should
match cross/cutback choices tagged with `wide` or `delivery`.)

---

## 5. Calibration (so the sim feels right)

- **difficulty:** routine action ~30–45; contested ~55–70; spectacular ~80–95.
- **output_delta:** small for routine (±2–6); medium for chances/errors (±8–15); large for
  goals and howlers (±18–30). A goal outcome should carry a big positive + `confidence`/
  `flow`; a glaring miss a notable negative + `frustration`.
- **score_event:** put `goal_for` only on **success** shooting/finishing outcomes;
  `goal_against` only on **failure** defensive outcomes. Use `assist_for` on a
  **success** outcome where the PC makes the final pass and a teammate scores — it
  still adds to the team's score, but is counted as a PC assist, not a PC goal
  (BL5.1). Most outcomes have `null`.
- **headspace:** successes nudge `confidence`/`flow` up; failures push `frustration` up.
  Keep magnitudes small (±3–10) — these accumulate over 15 beats.
- **Tone:** second person, present tense, terse and dramatic. Setups ≤ ~130 chars. No
  player/club names (the engine has no context for them). British football register.

---

## 6. Coverage targets for a good batch

- All five phases represented; **several `key` climax situations**.
- Every situation has ≥3 eligible choices via tags (verify the intersections).
- Outcome pool: roughly balanced `success`/`failure`, plus a few `any`; enough variety that
  the same text doesn't repeat within a match (aim for ≥15 success + ≥15 failure).
- A spread of attributes contested (don't make everything `Finishing`).

---

## 7. LLM prompt template (paste-ready)

> You are authoring static content for a football match "beat" engine. Output **only** a
> single JSON object, no prose, matching this schema:
> `{ "situations": [...], "choices": [...], "outcomes": [...] }`.
>
> **situation** = `{id, phase, bias:[early,mid,late], setup, tags}`; `phase` ∈
> {attack,defend,setpiece,positioning,key}; `bias` are 0–3 weights.
> **choice** = `{id, text, attr, difficulty(1–99), tags}`; `attr` MUST be one of:
> [Acceleration, SprintSpeed, Finishing, LongShots, ShotPower, Volleys, Penalties,
> ShortPassing, LongPassing, Vision, Crossing, FreeKickAcc, CloseControl, BallControl,
> Agility, Balance, Reactions, StandingTackle, Marking, Interceptions, Heading, Curve,
> AttPositioning, Strength, Stamina, Aggression, Jumping, Composure, Bravery, SlidingTackle].
> **outcome** = `{id, text, output_delta, confidence, frustration, flow, score_event,
> stamina_cost, polarity}`; `score_event` ∈ {null,"goal_for","assist_for","goal_against"};
> `polarity` ∈ {success,failure,any}.
>
> RULES: a choice is eligible for a situation when they SHARE ≥1 tag — so give every
> situation ≥3 eligible choices. Outcomes are a SHARED pool picked by polarity only, so
> write outcome text generically (it must read after many choices; never name the specific
> action). Include several `phase:"key"` climax situations. Calibrate: routine difficulty
> ~40 / spectacular ~85; output_delta small for routine, ±18–30 for goals/howlers;
> `goal_for` only on success shooting outcomes, `goal_against` only on failure defending
> outcomes. Second person, present tense, terse, British register, no names.
>
> Generate: N situations (mix of phases incl. ≥3 `key`), M choices covering every
> situation's tags and a spread of attributes, and a balanced pool of success + failure
> outcomes.

---

## 8. Validate before shipping

1. **It parses:** `BeatLibrary::load` must accept it (serde, strict to the schema).
2. **It plays:** eyeball real matches —
   `cargo run -p goat-tui --bin career-sim -- --match-beats <seed> <opp_strength>` prints a
   full beat-by-beat match; run a few seeds and check setups get sensible choices/outcomes.
3. **Determinism is preserved:** content is data only; never embed logic. Same seed + same
   `beats.json` = same match. If you ship a new library, treat it as a new content version
   (see `docs/FLUTTER-APP-GUIDE.md` §1a) — replays will narrate differently.
4. **Small fixture:** `beats_test.json` is a minimal valid library; mirror its shape if you
   need a tiny set for tests.

---

## 9. Minimal worked example

```json
{
  "situations": [
    { "id": "edge_of_box_loose", "phase": "attack", "bias": [2,3,2],
      "setup": "The ball spills loose to you 20 yards out. A defender is closing.",
      "tags": ["attack", "shooting"] },
    { "id": "last_minute_chance", "phase": "key", "bias": [0,1,3],
      "setup": "Injury time, level scores. The ball drops to you six yards out.",
      "tags": ["attack", "shooting", "key"] }
  ],
  "choices": [
    { "id": "first_time_curler", "text": "First-time curler into the far corner",
      "attr": "Curve", "difficulty": 78, "tags": ["attack", "shooting"] },
    { "id": "take_a_touch", "text": "Take a touch, pick your spot",
      "attr": "Composure", "difficulty": 52, "tags": ["attack", "shooting"] }
  ],
  "outcomes": [
    { "id": "screamer", "text": "Unstoppable — it flies into the top corner.",
      "output_delta": 24, "confidence": 8, "flow": 6, "score_event": "goal_for",
      "polarity": "success" },
    { "id": "dragged_wide", "text": "You snatch at it and drag it wide.",
      "output_delta": -10, "frustration": 7, "score_event": null,
      "polarity": "failure" }
  ]
}
```

# TASK — Player rating & attribute generation rebuild

**Source of truth:** `CLAUDE.md` (project rules) + the appendix **"Player Rating, Position Fit & Attribute Generation"** (C.1–C.6). This task implements that appendix. Where this task's prose and the appendix disagree, **the appendix wins**.

**Working style (per `CLAUDE.md`):**
- Read the source of truth and the existing code **first**; report what exists before changing anything.
- Work in **reviewable steps** and **PAUSE** after each step for approval.
- Golden test values are **frozen on purpose** — re-freeze only when behaviour intentionally changes (this task does).
- **Determinism is non-negotiable**: seeded, injected RNG; same seed ⇒ identical output.
- **No floats in the simulation path — `goat-fixed` only.** Any fractional constant is expressed in fixed-point.

---

## 0. Pre-flight (read, do not write)

- Read `CLAUDE.md`.
- Read the appendix above (C.1–C.6).
- Read the current implementations of: role rating, OVR, `generate_player`, and the weight/tuning constants — likely `goat-core/src/roles.rs`, `goat-core/src/tuning.rs`, the player-generation module, and the rating function.
- **Report** the current shape of these (weight tiers, how OVR is computed, how attributes are rolled) before touching anything.

---

## Reconciliation with `sim-fixes.md` (read before implementing)

This task **replaces** the OVR-related items in `sim-fixes.md`. Do **not** apply them:

- **Fix 4 (raise `W_KEY` / add a Poacher role) — SUPERSEDED.** The whole weighting scheme is being replaced by sparse per-position weights (C.2) + peak lift (C.3). Do **not** raise `W_KEY`; do **not** add a one-off Poacher role — the per-position tables already cover specialists.
- **Fix 8 (OVR / Finishing disconnect) — SUPERSEDED.** Addressed by C.2–C.3.
- **Fix 7 (`PHYSICAL_START_PCT`) — OUT OF SCOPE / on hold.** It concerns age-16 *fill*, not rating; it conflicts with §5.1/§5.3. Leave it.

Out of scope here (a **separate batch** — do not touch in this task): **Fix 1** (training rotation), **Fix 2** (form floor), **Fix 3** (goal variance), **Fix 5** (age snapshot). **Fix 6** (OVR → team-strength modifier) is a **pending design decision** — do not implement.

---

## 1. Per-position weight tables  *(PAUSE after)*

Replace the dense `W_KEY / W_IMP / W_BAS`-over-all-attributes scheme with **sparse per-position tables**.

- For each outfield position — **ST, W (LW/RW), WM (LM/RM), CAM, CM, DM, FB (LB/RB), CB** — store only the attributes that position uses, each tagged **Key / Important / Secondary**; every other attribute is implicitly weight **0**.
- Use the membership and illustrative weights (**Key 3 / Important 2 / Secondary 1**) from appendix **C.2**.
- Represent as **data** (a table keyed by position → attribute → tier), not hard-coded per-role functions.
- Keep weights as **integers** (port cleanly to `Fixed`).
- GK is parked — no table.

---

## 2. Position rating function  *(PAUSE after)*

Rewrite the per-position rating per appendix **C.3**, three moves, **all fixed-point**:

1. **Weighted average** over the position's weighted attributes only, normalised by total weight. Define and document the **rounding policy** for the division.
2. **Peak lift**: blend the average toward the player's highest **Key** attribute for that position (illustrative **70/30**). The lift reads **Key attributes only**.
3. **Familiarity multiplier** (existing four tiers).

- No floats anywhere. The 70/30 blend (and any other fraction) lives in `goat-fixed`.

---

## 3. OVR redefinition  *(PAUSE after)*

Per appendix **C.4**:

- Add a `primary_position` to the player; **initialise from the chosen position** at creation.
- **Player-facing OVR = rating at `primary_position`.**
- Add a separate `best_position_rating()` (max over positions), used for value/scouting and for background players, plus a way to expose the best position for **reinvention hints**. Do **not** auto-reassign `primary_position`.
- An out-of-position match uses the rating at the **deployed** position (familiarity-penalised) as that match's effective rating, **without** mutating `primary_position` or the headline OVR.
- Update any callers that assumed `OVR = max over positions`.

---

## 4. Attribute generation rewrite  *(PAUSE after)*

Per appendix **C.5**:

- Roll a **talent level** under the ceiling.
- Set per-attribute **potentials** = talent shaped by the position's role-shape (high for used attributes, reduced fraction for irrelevant), derived from the **C.2** tiers.
- Add **bounded per-attribute noise**: a centre-weighted bounded deviation (average of 2–3 uniform draws), **derived deterministically from a hash of `(player_id, attribute)`** via `goat-rng`. No transcendental sampling.
- Add a **per-player spikiness** parameter controlling noise width.
- **Keep** the existing §5.3 step-4 current-from-potential rule (physical near potential at 16, mental low). C.5 shapes **potentials only**.
- Fully **deterministic** and **fixed-point**.

---

## 5. Tests & golden re-freeze  *(PAUSE after)*

- This breaks existing goldens. Per `CLAUDE.md` they are **new-behaviour goldens**, not fixes to broken tests — re-freeze with the new expected values.
- Add **invariant / property tests** (organised by bible section, per the test-suite convention):
  - elite Key attribute at a position ⇒ high rating at that position (e.g. Finishing 97 striker ⇒ OVR ≥ 85, illustrative threshold);
  - off-role spike does **not** inflate (Jumping 97 non-finisher ⇒ low OVR);
  - vacuum specialist (one Key at 97, weak supporting profile) ⇒ **mid** OVR, not elite;
  - generation coherence: within-cluster spread bounded; between-cluster structure preserved;
  - flat player (all attributes equal) ⇒ rating equals that value;
  - OVR diversity across seeds; determinism (same seed ⇒ identical players).

---

## Done criteria

- OVR = current-primary-position rating; best-position available separately for value + hints.
- Sparse per-position weights + peak lift + familiarity — **all fixed-point**, deterministic.
- Coherent generation with a spikiness knob; §5.3 step-4 fill preserved.
- Goldens re-frozen; invariant tests green.
- **No floats in the simulation path.**
- `sim-fixes.md` Fixes 4/7/8 not applied; Fixes 1/2/3/5/6 untouched.
//! Struct-of-arrays background population + deterministic genesis (TASK-09A Slice 9A.1).
//!
//! The outer world is a columnar population (parallel `Vec`s keyed by an index), never
//! per-player heap objects (bible §9 / SoA discipline). Genesis stores only the cheap
//! *identity* columns plus a per-player seed; a background player's full attributes are
//! recomputed on demand from `(seed + birth data + date)` in Slice 9A.2 — they are never
//! stored or stepped weekly (the §9 SoA/perf trap). Same `world_seed` ⇒ bit-for-bit the
//! same universe on every platform: that is the Phase 9 determinism spine, pinned by the
//! `fingerprint` golden.

use crate::world::{nation_name, NUM_CLUBS};
use crate::worldgen::{generate_world, GeneratedWorld};
use goat_core::attrs::NUM_ATTRS;
use goat_core::generation::{generate_player, CreationChoices, Position};
use goat_core::player::PlayerView;
use goat_fixed::Fixed;
use goat_rng::{GoatRng, RngSource};

/// Age (years) at which a background player retires; past it, lazy-promote refuses so a
/// retired identity can never re-enter the live world as an active player.
pub const RETIRE_AGE_YEARS: u32 = 38;

/// Players generated per club at genesis. Headcount = `NUM_CLUBS * SQUAD_SIZE`; it scales
/// directly with the club/nation count when the world expands to the full 20–30k pyramid.
pub const SQUAD_SIZE: usize = 25;

/// Total background population at genesis.
pub const POP_SIZE: usize = NUM_CLUBS * SQUAD_SIZE;

/// Background population as parallel columns. Index `i` identifies one player across all
/// columns — there is no per-player struct.
#[derive(Debug, Clone, Default)]
pub struct Population {
    /// Per-player deterministic seed; everything derivable is recomputed from this.
    pub seed: Vec<u64>,
    /// Club index into `CLUBS`.
    pub club: Vec<u16>,
    /// Nationality (Nation as u8).
    pub nation: Vec<u8>,
    /// Primary position: 0 = Defender, 1 = Midfielder, 2 = Forward.
    pub position: Vec<u8>,
    /// Age in weeks at genesis (birth data is the stored residue; age advances by date).
    /// Signed: youth-intake players born AFTER genesis get negative values.
    pub birth_age_weeks: Vec<i64>,
    /// Headline potential OVR (1–99). Cached identity column; the per-attribute potential
    /// is re-derivable from `seed`.
    pub potential_ovr: Vec<u8>,
    // ── Path-dependent accumulators (batch-tick residue; bible §247) ──────────
    /// Career goals accumulated by season batch-tick. Not derivable — persisted.
    pub career_goals: Vec<u32>,
    /// Career league appearances accumulated by batch-tick.
    pub career_apps: Vec<u32>,
    /// League titles won (player's club finished top of its division that season).
    pub career_titles: Vec<u32>,
    /// Slow-moving individual form 0–100 (PA2 M3), fed ONLY by deep-simmed PC
    /// matches via the orbit overlay (EMA of synthetic match ratings). Players
    /// never touched by the orbit keep the neutral 50. Path-dependent — but not
    /// stored itself: it is replayed from `WorldState::orbit_records` on rebuild.
    pub form: Vec<i16>,
}

impl Population {
    pub fn len(&self) -> usize {
        self.seed.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seed.is_empty()
    }

    /// Deterministic FNV-1a fingerprint over every identity column, in the fixed genesis
    /// order (insertion order is itself deterministic, so no sort is needed). Same seed ⇒
    /// same fingerprint on every platform — the spine golden for Phase 9 determinism.
    pub fn fingerprint(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for i in 0..self.len() {
            for x in [
                self.seed[i],
                self.club[i] as u64,
                self.nation[i] as u64,
                self.position[i] as u64,
                self.birth_age_weeks[i] as u64,
                self.potential_ovr[i] as u64,
            ] {
                h ^= x;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        h
    }

    /// Fingerprint over the path-dependent career accumulators (goals/apps/titles).
    /// Stable for a fixed seed + batch-tick sequence — the golden anchor for 9A.3.
    pub fn career_fingerprint(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for i in 0..self.len() {
            for x in [
                self.career_goals[i] as u64,
                self.career_apps[i] as u64,
                self.career_titles[i] as u64,
            ] {
                h ^= x;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        h
    }
}

/// Squad position spread across a 25-man squad: roughly a third each of D / M / F.
fn squad_position(slot: usize) -> u8 {
    (slot % 3) as u8
}

/// Combine the world seed with club + slot into a stable per-player seed. NOTE:
/// `GoatRng::new` does NOT whiten — xorshift's first outputs on power-of-two
/// ranges sample only the seed's low bits, and this formula leaves those equal
/// to `world_seed`'s for every player (the rotate/multiply terms only stir the
/// high bits). Callers making such draws must whiten first (see
/// `history::name_from_seed`); non-power-of-two ranges divide the full u64 and
/// are unaffected. Whitening HERE instead would re-roll every player's
/// attributes — a worldgen change, not a name fix.
fn player_seed(world_seed: u64, club_id: u64, slot: u64) -> u64 {
    world_seed
        ^ club_id.rotate_left(21).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ slot.rotate_left(43).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
}

/// Seed for a youth-intake player: a NEW identity replacing a retiree, so it
/// must differ from every genesis seed — the season mixes in as a domain tag.
pub(crate) fn player_seed_for_intake(world_seed: u64, season: u32, club_id: u64, slot: u64) -> u64 {
    player_seed(world_seed ^ (season as u64).rotate_left(17), club_id, slot)
        .wrapping_add(0xAC4D_E11A_55C2_9001)
}

/// Generate the background population deterministically from `world_seed`. Every club
/// gets a `SQUAD_SIZE` squad; potential is anchored to club stature (stronger clubs draw
/// stronger players) with per-player variance. Pure and order-stable.
pub fn genesis(world_seed: u64) -> Population {
    let world = generate_world(world_seed);
    let mut pop = Population::default();
    pop.seed.reserve(POP_SIZE);

    for club in &world.clubs {
        for slot in 0..SQUAD_SIZE {
            let pseed = player_seed(world_seed, club.id as u64, slot as u64);
            let mut rng = GoatRng::new(pseed);

            let position = squad_position(slot);
            let age_years = rng.next_range_u32(16, 33);
            let birth_age_weeks = age_years * 52;

            // Potential anchored to club strength ± variance, clamped to a sane band.
            let base = club.strength as i32;
            let variance = rng.next_range_u32(0, 30) as i32 - 15;
            let potential_ovr = (base + variance).clamp(30, 99) as u8;

            pop.seed.push(pseed);
            pop.club.push(club.id as u16);
            pop.nation.push(club.nation);
            pop.position.push(position);
            pop.birth_age_weeks.push(birth_age_weeks as i64);
            pop.potential_ovr.push(potential_ovr);
            pop.career_goals.push(0);
            pop.career_apps.push(0);
            pop.career_titles.push(0);
            pop.form.push(50);
        }
    }

    pop
}

// ── Formula-driven background growth + lazy-promote (bible §245–246) ───────────

/// Closed-form development curve: the fraction of potential a player has realised at a
/// given age (×1000 fixed-point, i.e. raw 1000 = 1.0). Rises from 0.60 at 16 to a 25–31
/// peak (1.00), then declines. Pure — background growth is *computed on demand* from age,
/// never stored or stepped weekly (the §9 SoA/perf trap).
fn development_fraction(age_years: u32) -> Fixed {
    let pct: u32 = match age_years {
        0..=16 => 600,
        17..=25 => 600 + (age_years - 16) * 45, // 17→645 … 25→1005 (capped to 1.0)
        26..=31 => 1000,                        // peak plateau
        32..=37 => 1000 - (age_years - 31) * 35, // 32→965 … 37→790
        _ => 770,
    };
    Fixed::raw(pct.min(1000) as i32)
}

fn position_from_u8(p: u8) -> Position {
    match p {
        0 => Position::Defender,
        1 => Position::Midfielder,
        _ => Position::Forward,
    }
}

impl Population {
    /// Age in years of background player `idx` at `elapsed_weeks` after genesis.
    fn age_years_at(&self, idx: usize, elapsed_weeks: u32) -> u32 {
        ((self.birth_age_weeks[idx] + elapsed_weeks as i64).max(0) / 52) as u32
    }

    /// Cheap O(1) current OVR of a background player at a date (epoch weeks since genesis),
    /// derived on demand from `(potential, age)`. Never exceeds the stored potential
    /// (§2.4). Used for outer-world ranking without realising the full player.
    pub fn current_ovr(&self, idx: usize, elapsed_weeks: u32) -> u8 {
        let frac = development_fraction(self.age_years_at(idx, elapsed_weeks));
        let cur = (Fixed::from_int(self.potential_ovr[idx] as i32) * frac).to_int();
        cur.clamp(0, self.potential_ovr[idx] as i32) as u8
    }

    /// True once the player has reached the retirement age at the given date.
    pub fn is_retired(&self, idx: usize, elapsed_weeks: u32) -> bool {
        self.age_years_at(idx, elapsed_weeks) >= RETIRE_AGE_YEARS
    }

    /// Lazy-promote a background player into a full-fidelity `PlayerView` "on contact"
    /// (bible §245) — the moment he becomes relevant (you face him, a transfer links him).
    /// Returns `None` if he has retired. Pure & deterministic: same `(idx, date)` ⇒ the
    /// same player on every run/platform. The caller pushes the view into a `PlayerStore`.
    pub fn promote(
        &self,
        idx: usize,
        elapsed_weeks: u32,
        name: impl Into<String>,
        world: &GeneratedWorld,
    ) -> Option<PlayerView> {
        if self.is_retired(idx, elapsed_weeks) {
            return None;
        }
        let choices = CreationChoices {
            name: name.into(),
            position: position_from_u8(self.position[idx]),
            nationality: nation_name(self.nation[idx]),
            club: world.clubs[self.club[idx] as usize].name.clone(),
        };
        // generate_player gives the realistic per-attribute potential + shape + roles; we
        // overwrite current to the age-appropriate fraction of that potential.
        let mut view = generate_player(self.seed[idx], &choices);
        let frac = development_fraction(self.age_years_at(idx, elapsed_weeks));
        for a in 0..NUM_ATTRS {
            view.current[a] = (view.potential[a] * frac).clamp(Fixed::MIN_ATTR, view.potential[a]);
        }
        view.age_weeks = (self.birth_age_weeks[idx] + elapsed_weeks as i64).max(0) as u32;
        Some(view)
    }

    /// Indices of a club's match-day lineup (PA2 M1): the top `count` available
    /// squad members by current OVR at `elapsed_weeks`, skipping the retired.
    /// Deterministic — ties broken by population index (insertion order).
    pub fn lineup_indices(&self, club_id: usize, elapsed_weeks: u32, count: usize) -> Vec<usize> {
        let mut squad: Vec<usize> = (0..self.len())
            .filter(|&i| self.club[i] as usize == club_id && !self.is_retired(i, elapsed_weeks))
            .collect();
        squad.sort_by_key(|&i| std::cmp::Reverse(self.current_ovr(i, elapsed_weeks)));
        squad.truncate(count);
        squad
    }

    /// Mean current attributes of a club's lineup (PA2 M1), optionally plus one
    /// external player (the PC occupies a starting slot, so his real attrs lift
    /// the team profile). Returns `None` if the club cannot field a full side.
    pub fn squad_avg_attrs(
        &self,
        club_id: usize,
        elapsed_weeks: u32,
        world: &GeneratedWorld,
        extra: Option<&[Fixed; NUM_ATTRS]>,
    ) -> Option<[Fixed; NUM_ATTRS]> {
        let n_npc = 11 - usize::from(extra.is_some());
        let lineup = self.lineup_indices(club_id, elapsed_weeks, n_npc);
        if lineup.len() < n_npc {
            return None;
        }
        let mut sums = [0i64; NUM_ATTRS];
        let mut n = 0i64;
        for idx in lineup {
            let view = self.promote(
                idx,
                elapsed_weeks,
                crate::history::name_from_seed(self.seed[idx]),
                world,
            )?;
            for (a, s) in sums.iter_mut().enumerate() {
                *s += view.current[a].to_raw() as i64;
            }
            n += 1;
        }
        if let Some(attrs) = extra {
            for (a, s) in sums.iter_mut().enumerate() {
                *s += attrs[a].to_raw() as i64;
            }
            n += 1;
        }
        let mut avg = [Fixed::ZERO; NUM_ATTRS];
        for (a, v) in avg.iter_mut().enumerate() {
            *v = Fixed::raw((sums[a] / n) as i32);
        }
        Some(avg)
    }

    // ── PA2 M1.5: formation-aware lineup + PC selection ──────────────────────

    /// Blended selection score for an NPC (PA2 M3): current OVR + the real
    /// form pull — the same `(form − 50) × 3/10` weight the PC's own selection
    /// formula uses. Players untouched by the orbit sit at form 50 and score
    /// exactly their OVR (pre-M3 behaviour).
    fn npc_selection_score(&self, idx: usize, elapsed_weeks: u32) -> i32 {
        self.current_ovr(idx, elapsed_weeks) as i32 + (self.form[idx] as i32 - 50) * 3 / 10
    }

    /// Top `slots.{0,1,2}` available players within each position group (D/M/F),
    /// skipping the retired, ranked by the form-blended selection score (PA2 M3:
    /// a teammate in a hot streak holds his shirt; form 50 = pure OVR order).
    /// Deterministic and noise-free — this is the *profile* lineup (team
    /// strength), not the selection drama.
    ///
    /// `slots` counts OUTFIELD players only (real football convention: the
    /// formation's numbers sum to 10). The population has no goalkeeper entity,
    /// so the 11th man is an implicit abstract GK — see
    /// `TacticalProfile::formation_slots`.
    pub fn lineup_indices_formation(
        &self,
        club_id: usize,
        elapsed_weeks: u32,
        slots: (usize, usize, usize),
    ) -> Vec<usize> {
        let mut out = Vec::with_capacity(slots.0 + slots.1 + slots.2);
        for (pos, &n) in [slots.0, slots.1, slots.2].iter().enumerate() {
            let mut group: Vec<usize> = (0..self.len())
                .filter(|&i| {
                    self.club[i] as usize == club_id
                        && self.position[i] == pos as u8
                        && !self.is_retired(i, elapsed_weeks)
                })
                .collect();
            group.sort_by_key(|&i| std::cmp::Reverse(self.npc_selection_score(i, elapsed_weeks)));
            group.truncate(n);
            out.extend(group);
        }
        out
    }

    /// Formation-aware mean attributes of a club's lineup. When the PC starts,
    /// `pc` is `Some((position, attrs))` and he takes one slot in his position
    /// group (one fewer NPC is averaged there); when he is benched, `None`.
    /// Returns `None` if the club cannot field the full split.
    ///
    /// The mean is taken over the 10 outfield slots (plus the PC when he
    /// starts) — the abstract GK contributes nothing, and a mean stays
    /// comparable to the opponent's top-11-OVR mean regardless of n.
    pub fn squad_avg_attrs_formation(
        &self,
        club_id: usize,
        elapsed_weeks: u32,
        world: &GeneratedWorld,
        slots: (usize, usize, usize),
        pc: Option<(u8, &[Fixed; NUM_ATTRS])>,
    ) -> Option<[Fixed; NUM_ATTRS]> {
        let npc_slots = match pc {
            Some((pos, _)) => {
                let mut s = slots;
                match pos {
                    0 => s.0 = s.0.saturating_sub(1),
                    1 => s.1 = s.1.saturating_sub(1),
                    _ => s.2 = s.2.saturating_sub(1),
                }
                s
            }
            None => slots,
        };
        let lineup = self.lineup_indices_formation(club_id, elapsed_weeks, npc_slots);
        let want = npc_slots.0 + npc_slots.1 + npc_slots.2;
        if lineup.len() < want {
            return None;
        }
        let mut sums = [0i64; NUM_ATTRS];
        let mut n = 0i64;
        for idx in lineup {
            let view = self.promote(
                idx,
                elapsed_weeks,
                crate::history::name_from_seed(self.seed[idx]),
                world,
            )?;
            for (a, s) in sums.iter_mut().enumerate() {
                *s += view.current[a].to_raw() as i64;
            }
            n += 1;
        }
        if let Some((_, attrs)) = pc {
            for (a, s) in sums.iter_mut().enumerate() {
                *s += attrs[a].to_raw() as i64;
            }
            n += 1;
        }
        let mut avg = [Fixed::ZERO; NUM_ATTRS];
        for (a, v) in avg.iter_mut().enumerate() {
            *v = Fixed::raw((sums[a] / n) as i32);
        }
        Some(avg)
    }

    /// Decide whether the PC starts or is benched this week (PA2 M1.5). The PC
    /// competes inside his position group for `group_slots` places; NPCs carry
    /// their REAL accumulated form (PA2 M3 — orbit overlay EMA, neutral 50 when
    /// untouched) plus a small seeded weekly noise (±5) and a ~3% availability
    /// exclusion, both ephemeral (nothing is stored). A PC just below the
    /// cutoff gets a seeded borderline roll where manager favor nudges the odds.
    pub fn select_pc(
        &self,
        club_id: usize,
        elapsed_weeks: u32,
        group_slots: usize,
        pc: &PcSelectionInput,
        week_seed: u64,
    ) -> SelectionOutcome {
        if pc.unavailable || group_slots == 0 {
            return SelectionOutcome::Benched;
        }
        let pc_score = pc_selection_score(pc);

        let mut cand: Vec<(i32, bool)> = Vec::new(); // (score, is_pc)
        for i in 0..self.len() {
            if self.club[i] as usize != club_id
                || self.position[i] != pc.position
                || self.is_retired(i, elapsed_weeks)
            {
                continue;
            }
            let mut rng = GoatRng::new(self.seed[i] ^ week_seed);
            if rng.next_range_u32(0, NPC_UNAVAILABLE_DIV) == 0 {
                continue; // knocked/suspended this week — ephemeral abstraction
            }
            let noise = rng.next_range_u32(0, 2 * NPC_FORM_NOISE) as i32 - NPC_FORM_NOISE as i32;
            cand.push((self.npc_selection_score(i, elapsed_weeks) + noise, false));
        }
        cand.push((pc_score, true));
        // Stable sort, score descending: on ties the PC (pushed last) loses to NPCs.
        cand.sort_by_key(|&(score, _)| std::cmp::Reverse(score));

        let rank = cand.iter().position(|&(_, is_pc)| is_pc).unwrap();
        if rank < group_slots {
            return SelectionOutcome::Starts;
        }
        let cutoff = cand[group_slots - 1].0; // lowest selected score
        let diff = pc_score - cutoff;
        if diff < -(BORDERLINE_BAND as i32) {
            return SelectionOutcome::Benched;
        }
        // Borderline: the manager hesitates — favor nudges the odds (±10).
        let pct = (50 + diff * 10 + (pc.favor - 50) / 5).clamp(1, 99);
        let mut roll = GoatRng::new(week_seed ^ BORDERLINE_SALT);
        if roll.next_range_u32(1, 100) as i32 <= pct {
            SelectionOutcome::Starts
        } else {
            SelectionOutcome::Benched
        }
    }
}

/// NPC unavailability divisor: `next_range(0, NPC_UNAVAILABLE_DIV) == 0` ⇒ out
/// this week (~3%; knock/suspension abstraction, seeded per player-week, never stored).
pub const NPC_UNAVAILABLE_DIV: u32 = 33;
/// Weekly NPC form noise (±points on selection score). Halved at M3: the real
/// accumulated form now carries the signal, noise only keeps weeks alive.
pub const NPC_FORM_NOISE: u32 = 5;
/// Score band below the selection cutoff in which the borderline roll applies.
pub const BORDERLINE_BAND: u32 = 5;
/// Salt for the borderline roll stream (independent of availability/noise draws).
const BORDERLINE_SALT: u64 = 0xB0DE_21A4_7C3F_55E1;

/// Everything the selection formula needs from the PC — all sourced from existing
/// `WorldState`/`PlayerView` fields (no new systems). See the task doc
/// (`tasks/TASK-PA2-world-into-match.md`, M1.5) for the locked formula.
#[derive(Debug, Clone, Copy)]
pub struct PcSelectionInput {
    /// Primary position: 0 = Defender, 1 = Midfielder, 2 = Forward.
    pub position: u8,
    /// Role rating at the PC's best role (familiarity multiplier already inside).
    pub role_rating: i32,
    /// Familiarity tier at that role (0=Awkward … 3=Natural) — tactical fit term.
    pub familiarity_tier: u8,
    pub form: i32,
    pub trust: i32,
    pub favor: i32,
    /// Academy hype — only counts in the PC's first season at the club.
    pub academy_hype: i32,
    pub first_season_at_club: bool,
    /// Annual wage (thousands) — top-earner proxy vs `club_strength * 3`.
    pub wage_annual: i64,
    pub club_strength: u8,
    pub fan_rep: i32,
    pub marketability: i32,
    pub power_ladder: u8,
    /// Energy 0–100.
    pub energy: i32,
    /// Hard exclusion: injured or suspended.
    pub unavailable: bool,
}

/// The PC's multi-factor selection score (locked formula, M1.5):
/// role rating + form/trust pulls + tactical fit + first-season hype +
/// top-earner status + fan/market appeal − power-ladder rebellion − fatigue.
pub fn pc_selection_score(pc: &PcSelectionInput) -> i32 {
    let fit = [-6, -2, 0, 4][pc.familiarity_tier.min(3) as usize];
    let hype = if pc.first_season_at_club {
        (pc.academy_hype / 10).clamp(-5, 5)
    } else {
        0
    };
    let wage = if pc.wage_annual >= pc.club_strength as i64 * 3 {
        5
    } else {
        0
    };
    let energy = if pc.energy < 40 {
        -10
    } else if pc.energy < 60 {
        -4
    } else {
        0
    };
    pc.role_rating
        + (pc.form - 50) * 3 / 10
        + (pc.trust - 50) * 2 / 5
        + fit
        + hype
        + wage
        + (pc.fan_rep - 50) / 10
        + if pc.marketability >= 70 { 3 } else { 0 }
        - 8 * pc.power_ladder as i32
        + energy
}

/// Whether the PC starts or watches from the bench this week.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionOutcome {
    Starts,
    Benched,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_is_full_and_columnar() {
        let pop = genesis(7);
        assert_eq!(pop.len(), POP_SIZE);
        // All columns are the same length (true SoA — no ragged rows).
        assert_eq!(pop.club.len(), POP_SIZE);
        assert_eq!(pop.potential_ovr.len(), POP_SIZE);
        // Invariants on derived columns.
        assert!(pop.position.iter().all(|&p| p <= 2));
        assert!(pop.potential_ovr.iter().all(|&o| (30..=99).contains(&o)));
        assert!(pop
            .birth_age_weeks
            .iter()
            .all(|&w| (16 * 52..=33 * 52).contains(&w)));
    }

    #[test]
    fn genesis_is_deterministic() {
        assert_eq!(genesis(42).fingerprint(), genesis(42).fingerprint());
        assert_ne!(genesis(1).fingerprint(), genesis(2).fingerprint());
    }

    #[test]
    fn background_current_never_exceeds_potential() {
        let pop = genesis(7);
        // Sweep every player across a 24-year span of dates.
        for idx in 0..pop.len() {
            for wk in (0..24 * 52).step_by(26) {
                assert!(
                    pop.current_ovr(idx, wk) <= pop.potential_ovr[idx],
                    "idx {idx}: current > potential at week {wk}"
                );
            }
        }
    }

    #[test]
    fn background_rederive_is_deterministic() {
        let pop = genesis(3);
        let world = generate_world(3);
        assert_eq!(pop.current_ovr(100, 260), pop.current_ovr(100, 260));
        let a = pop.promote(50, 6 * 52, "X", &world).unwrap();
        let b = pop.promote(50, 6 * 52, "X", &world).unwrap();
        assert_eq!(a.current, b.current, "promote must be deterministic");
    }

    #[test]
    fn promoted_player_respects_talent_ceiling() {
        let pop = genesis(9);
        let world = generate_world(9);
        let view = pop.promote(50, 8 * 52, "Prospect", &world).unwrap();
        for i in 0..NUM_ATTRS {
            assert!(
                view.current[i] <= view.potential[i],
                "attr {i} exceeds potential"
            );
        }
    }

    #[test]
    fn lazy_promote_never_resurrects_retired() {
        let pop = genesis(11);
        let world = generate_world(11);
        let idx = 0;
        // Elapsed time that puts this player exactly at the retirement age.
        let elapsed = (RETIRE_AGE_YEARS * 52) as i64 - pop.birth_age_weeks[idx];
        let elapsed = elapsed.max(0) as u32;
        assert!(pop.is_retired(idx, elapsed));
        assert!(
            pop.promote(idx, elapsed, "Veteran", &world).is_none(),
            "a retired player must never promote to an active view"
        );
    }

    #[test]
    fn lineup_picks_highest_ovr_and_skips_retired() {
        let pop = genesis(7);
        let lineup = pop.lineup_indices(3, 260, 11);
        assert_eq!(lineup.len(), 11);
        // Every picked player outranks every unpicked squad mate.
        let min_picked = lineup
            .iter()
            .map(|&i| pop.current_ovr(i, 260))
            .min()
            .unwrap();
        for i in 0..pop.len() {
            if pop.club[i] as usize == 3 && !lineup.contains(&i) && !pop.is_retired(i, 260) {
                assert!(pop.current_ovr(i, 260) <= min_picked);
            }
        }
        // Late enough that the oldest squad members are retired → they can't be picked.
        let late = pop.lineup_indices(3, 22 * 52, 11);
        for &i in &late {
            assert!(!pop.is_retired(i, 22 * 52));
        }
    }

    #[test]
    fn squad_avg_attrs_deterministic_and_pc_lifts_profile() {
        let pop = genesis(5);
        let world = generate_world(5);
        let a = pop.squad_avg_attrs(2, 260, &world, None).unwrap();
        let b = pop.squad_avg_attrs(2, 260, &world, None).unwrap();
        assert_eq!(a, b, "lineup attrs must be deterministic");
        // A superstar PC in the side raises every group mean.
        let star = [Fixed::from_int(95); NUM_ATTRS];
        let with_pc = pop.squad_avg_attrs(2, 260, &world, Some(&star)).unwrap();
        assert!(with_pc[2] > a[2], "PC attrs must lift the squad mean");
    }

    fn pc_input(position: u8, role_rating: i32) -> PcSelectionInput {
        PcSelectionInput {
            position,
            role_rating,
            familiarity_tier: 3, // Natural
            form: 50,
            trust: 50,
            favor: 50,
            academy_hype: 0,
            first_season_at_club: false,
            wage_annual: 0,
            club_strength: 50,
            fan_rep: 50,
            marketability: 50,
            power_ladder: 0,
            energy: 100,
            unavailable: false,
        }
    }

    #[test]
    fn formation_lineup_respects_slots() {
        let pop = genesis(7);
        let slots = (5, 4, 1);
        let lineup = pop.lineup_indices_formation(3, 260, slots);
        assert_eq!(lineup.len(), 10, "10 outfield slots; the GK is abstract");
        for pos in 0..3u8 {
            let want = [slots.0, slots.1, slots.2][pos as usize];
            let got = lineup.iter().filter(|&&i| pop.position[i] == pos).count();
            assert_eq!(got, want, "position {pos} must fill exactly its slots");
        }
        // Deterministic.
        assert_eq!(lineup, pop.lineup_indices_formation(3, 260, slots));
    }

    #[test]
    fn formation_avg_pc_occupies_his_group_slot() {
        let pop = genesis(5);
        let world = generate_world(5);
        let slots = (4, 3, 3);
        let without = pop
            .squad_avg_attrs_formation(2, 260, &world, slots, None)
            .unwrap();
        let star = [Fixed::from_int(95); NUM_ATTRS];
        let with_pc = pop
            .squad_avg_attrs_formation(2, 260, &world, slots, Some((2, &star)))
            .unwrap();
        assert!(
            with_pc[2] > without[2],
            "a starting PC still lifts the squad mean under formation lineups"
        );
    }

    #[test]
    fn star_pc_always_starts_at_weak_club() {
        let pop = genesis(7);
        // Club 0's players cap near its strength; a 95-rated PC clears them all.
        let pc = pc_input(2, 95);
        for week in 0..40u64 {
            let outcome = pop.select_pc(0, week as u32 * 7, 3, &pc, 0xA11CE ^ week);
            assert_eq!(outcome, SelectionOutcome::Starts, "week {week}");
        }
    }

    #[test]
    fn hopeless_pc_is_benched_at_strong_club() {
        let pop = genesis(7);
        // A 20-rated PC at the strongest club should essentially never start.
        let strong_club = {
            let world = generate_world(7);
            (0..world.clubs.len())
                .max_by_key(|&c| world.clubs[c].strength)
                .unwrap()
        };
        let pc = pc_input(1, 20);
        let mut starts = 0;
        for week in 0..60u64 {
            if pop.select_pc(strong_club, week as u32 * 7, 4, &pc, 0xA11CE ^ week)
                == SelectionOutcome::Starts
            {
                starts += 1;
            }
        }
        assert!(
            starts <= 6,
            "20-rated PC started {starts}/60 at an elite club"
        );
    }

    #[test]
    fn selection_is_deterministic_and_unavailable_is_hard() {
        let pop = genesis(3);
        let pc = pc_input(0, 60);
        let a = pop.select_pc(4, 260, 4, &pc, 0xBEEF);
        let b = pop.select_pc(4, 260, 4, &pc, 0xBEEF);
        assert_eq!(a, b, "same week ⇒ same decision");
        let mut injured = pc;
        injured.unavailable = true;
        assert_eq!(
            pop.select_pc(4, 260, 4, &injured, 0xBEEF),
            SelectionOutcome::Benched,
            "injury/suspension is a hard exclusion"
        );
    }

    #[test]
    fn npc_weekly_unavailability_is_rare() {
        // ~1-in-33 per player-week: over 25 players × 200 weeks the outage count
        // must land in a sane band around 150 (3%).
        let pop = genesis(7);
        let mut out = 0u32;
        let mut total = 0u32;
        for week in 0..200u64 {
            let week_seed = 0xA11CE ^ week;
            for i in 0..pop.len() {
                if pop.club[i] as usize != 3 {
                    continue;
                }
                total += 1;
                let mut rng = GoatRng::new(pop.seed[i] ^ week_seed);
                if rng.next_range_u32(0, NPC_UNAVAILABLE_DIV) == 0 {
                    out += 1;
                }
            }
        }
        let pct = out * 100 / total;
        assert!((1..=6).contains(&pct), "NPC outage {pct}% (target ~3%)");
    }
}

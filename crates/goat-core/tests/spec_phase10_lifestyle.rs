//! Phase 10 SPEC — lifestyle has real, lasting consequences (TASK-10A, bible §8.6).
//!
//! Written test-FIRST: these started red and drove the lifestyle↔injury,
//! lifestyle↔ceiling, and lifestyle↔longevity coupling in `goat-core` (`week.rs`,
//! `tuning.rs`). They are the executable spec — a break here means the model changed.
//!
//! The bug they fixed (career-sim, Forward, seeds 0–19, 20 seasons, BEFORE):
//!   PRO  (Professional/High): Peak OVR 93, Injured weeks 113
//!   TOXIC (Flashy/High)     : Peak OVR 93, Injured weeks 113   ← identical to PRO
//! Lifestyle changed nothing. AFTER: Pro 90 / Balanced 113 / Flashy 174 injured
//! weeks, Flashy peaks 89 vs 93 — toxic finally costs ceiling + durability.

use goat_core::derive::ovr;
use goat_core::generation::CreationChoices;
use goat_core::positions::PrimaryPosition;
use goat_core::state::{reduce, Intent, WorldState};
use goat_core::week::{Intensity, Routine};
use goat_fixed::Fixed;
use goat_rng::GoatRng;

const PROFESSIONAL: u8 = 0;
const FLASHY: u8 = 2;

/// Maps a forced test lifestyle tier to the extreme lifestyle score that produces it
/// (bible §8.5/§8.6 — lifestyle is now emergent, not a settable intent). Pinned to the
/// extremes of the [-1.000, 1.000] band so a single week's nudge can never cross a
/// tier threshold on its own.
fn score_for_lifestyle(lifestyle: u8) -> Fixed {
    match lifestyle {
        0 => Fixed::raw(-1_000),
        2 => Fixed::raw(1_000),
        _ => Fixed::ZERO,
    }
}

/// Build a Forward at a fixed seed, pin the given lifestyle tier + a High-intensity
/// striker routine, then run `weeks` weeks. Returns (peak OVR seen, weeks injured).
///
/// The High-intensity routine nudges the lifestyle score toward Professional every
/// week (bible §8.5); this harness re-pins the score before each week so the forced
/// tier holds for the whole run — reproducing the old "lifestyle set once, never
/// changes" behaviour byte-for-bit, which is what the frozen goldens below assume.
fn run_career(seed: u64, lifestyle: u8, weeks: u32) -> (i32, u32) {
    let choices = CreationChoices {
        name: "Spec".into(),
        primary_position: PrimaryPosition::ST,
        primary_role: None,
        nationality: "Brazilian".to_string(),
        club: "Riverside Town".to_string(),
    };
    let mut s = WorldState::new();
    s = reduce(
        s,
        Intent::CreatePlayer { seed, choices },
        &mut GoatRng::new(0),
    );
    let forced_score = score_for_lifestyle(lifestyle);
    s.pc_lifestyle_score = forced_score;
    s.pc_lifestyle = lifestyle;
    let routine = Routine {
        focus_attrs: vec![
            goat_core::attrs::AttrId::Finishing,
            goat_core::attrs::AttrId::AttPositioning,
            goat_core::attrs::AttrId::BallControl,
        ],
        intensity: Intensity::High,
    };
    s = reduce(s, Intent::SetRoutine { routine }, &mut GoatRng::new(0));

    let pc = s.pc_player_id.unwrap();
    let mut rng = GoatRng::new(seed ^ 0xA11CE);
    let mut peak = 0i32;
    let mut injured = 0u32;
    for _ in 0..weeks {
        s.pc_lifestyle_score = forced_score;
        s.pc_lifestyle = lifestyle;
        s = reduce(s, Intent::AdvanceWeek, &mut rng);
        s.pc_week_training_done = false; // week boundary — harness has no round loop
        if s.players.get_injury_weeks(pc) > 0 {
            injured += 1;
        }
        let v = s.players.snapshot(pc);
        let o = ovr(&v.current, v.primary_position).to_int();
        if o > peak {
            peak = o;
        }
    }
    (peak, injured)
}

/// A toxic (Flashy) lifestyle must raise injury risk versus a Professional one,
/// holding seed and training intensity constant. Today lifestyle is not an input
/// to `injury_prob`, so the two are identical → RED.
#[test]
fn flashy_lifestyle_increases_injuries() {
    let mut pro_total = 0u32;
    let mut flashy_total = 0u32;
    for seed in 0u64..8 {
        let (_, pro_inj) = run_career(seed, PROFESSIONAL, 300);
        let (_, flashy_inj) = run_career(seed, FLASHY, 300);
        pro_total += pro_inj;
        flashy_total += flashy_inj;
    }
    assert!(
        flashy_total > pro_total,
        "Flashy lifestyle should cause more injured weeks than Professional \
         (flashy={flashy_total}, pro={pro_total}); lifestyle must feed injury risk"
    );
}

/// The "lifestyle ↔ longevity fork": a toxic lifestyle must lower the ceiling the
/// player actually reaches (or pull peak forward into earlier decay), not merely
/// slow growth that fully catches up. Today both converge to the same peak → RED.
#[test]
fn professional_lifestyle_reaches_higher_peak() {
    // ~15 seasons of weeks — long enough that a pure growth-SPEED penalty washes out.
    let (pro_peak, _) = run_career(3, PROFESSIONAL, 900);
    let (flashy_peak, _) = run_career(3, FLASHY, 900);
    assert!(
        pro_peak > flashy_peak,
        "Professional should peak higher than Flashy over a full career \
         (pro={pro_peak}, flashy={flashy_peak}); lifestyle must change the \
         destination, not just the speed of arrival"
    );
}

/// Sanity guard that must hold even once coupling lands: lifestyle must never push
/// any attribute above its potential (talent ceiling is law). This one is GREEN now
/// and must STAY green after implementing the fork — a regression tripwire.
#[test]
fn lifestyle_never_breaks_talent_ceiling() {
    let choices = CreationChoices {
        name: "Ceil".into(),
        primary_position: PrimaryPosition::ST,
        primary_role: None,
        nationality: "Brazilian".to_string(),
        club: "Riverside Town".to_string(),
    };
    let mut s = WorldState::new();
    s = reduce(
        s,
        Intent::CreatePlayer { seed: 7, choices },
        &mut GoatRng::new(0),
    );
    s.pc_lifestyle_score = score_for_lifestyle(PROFESSIONAL);
    s.pc_lifestyle = PROFESSIONAL;
    let routine = Routine {
        focus_attrs: vec![goat_core::attrs::AttrId::Finishing],
        intensity: Intensity::High,
    };
    s = reduce(s, Intent::SetRoutine { routine }, &mut GoatRng::new(0));
    let pc = s.pc_player_id.unwrap();
    let mut rng = GoatRng::new(7);
    for _ in 0..600 {
        s.pc_lifestyle_score = score_for_lifestyle(PROFESSIONAL);
        s.pc_lifestyle = PROFESSIONAL;
        s = reduce(s, Intent::AdvanceWeek, &mut rng);
        s.pc_week_training_done = false; // week boundary — harness has no round loop
        for a in 0..goat_core::attrs::NUM_ATTRS {
            assert!(
                s.players.get_current(pc, a) <= s.players.get_potential(pc, a).max(Fixed::MIN_ATTR),
                "attr {a} exceeded potential under Professional lifestyle"
            );
        }
    }
}

/// Property: across a seed sweep, a Professional career is never worse than a Flashy
/// one on either axis — it peaks at least as high AND is injured no more — and is
/// strictly better somewhere. (Adjacent tiers Balanced↔Flashy saturate on injured-
/// weeks because injuries force energy-restoring rest, so we contrast the extremes.)
#[test]
fn lifestyle_pro_dominates_flashy_across_seeds() {
    let mut strict_peak = 0;
    let mut strict_inj = 0;
    for seed in 0u64..8 {
        let (pro_peak, pro_inj) = run_career(seed, PROFESSIONAL, 900);
        let (fl_peak, fl_inj) = run_career(seed, FLASHY, 900);
        assert!(
            pro_peak >= fl_peak,
            "seed {seed}: Pro peak {pro_peak} < Flashy peak {fl_peak}"
        );
        assert!(
            pro_inj <= fl_inj,
            "seed {seed}: Pro injured {pro_inj} > Flashy injured {fl_inj}"
        );
        if pro_peak > fl_peak {
            strict_peak += 1;
        }
        if pro_inj < fl_inj {
            strict_inj += 1;
        }
    }
    assert!(
        strict_peak >= 6 && strict_inj >= 6,
        "lifestyle should make a strict difference on most seeds \
         (strict peak={strict_peak}/8, strict injury={strict_inj}/8)"
    );
}

/// Golden: exact frozen outputs for a fixed seed over a full career, one row per
/// lifestyle. Balanced is the neutral baseline; Pro lowers injuries + sustains the
/// peak; Flashy burns the ceiling. Values frozen from first green run — NEVER edit
/// to fix a failing test; a break here means the lifestyle model changed.
#[test]
fn golden_lifestyle_seed3() {
    // (lifestyle, expected_peak_ovr, expected_injured_weeks)
    // Re-frozen after BL7 durability/injury-proneness trait (Design round 9): seed 3's
    // rolled `durability_x10` isn't the neutral midpoint, so injured-week counts shifted
    // for this seed across all three lifestyle tiers. Expected — this coefficient adds
    // per-seed injury variance by design (see `week.rs::durability_neutral_value_
    // reproduces_pre_existing_injury_numbers` for proof the *formula* is unchanged at
    // neutral durability). Pro-dominates-Flashy ordering (peak and injuries) still
    // holds, same as before.
    let golden = [
        (PROFESSIONAL, 88, 66),
        (1u8, 88, 113), // Balanced — must match pre-lifestyle behavior
        (FLASHY, 85, 158),
    ];
    for (lifestyle, exp_peak, exp_inj) in golden {
        let (peak, inj) = run_career(3, lifestyle, 900);
        assert_eq!(
            (peak, inj),
            (exp_peak, exp_inj),
            "lifestyle {lifestyle}: got peak={peak} inj={inj}, frozen ({exp_peak}, {exp_inj})"
        );
    }
}

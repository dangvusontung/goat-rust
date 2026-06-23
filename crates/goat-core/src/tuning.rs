//! All tunable numbers for the GOAT simulation.
//!
//! Nothing in the sim may hard-code a numeric literal; every constant lives here.
//! Bible §5 numbers are placeholders illustrating shape — these are the starting values.

use goat_fixed::Fixed;

// ── Role-weight tiers (roles.rs match engine + familiarity XP) ───────────────

/// Key attribute weight: 1.000
pub const W_KEY: Fixed = Fixed::raw(1_000);
/// Important attribute weight: 0.500
pub const W_IMP: Fixed = Fixed::raw(500);
/// Baseline attribute weight: 0.250
pub const W_BAS: Fixed = Fixed::raw(250);
/// Zero weight — attribute is irrelevant for this role.
pub const W_ZERO: Fixed = Fixed::raw(0);

// ── Position rating blend (appendix C.3) ─────────────────────────────────────

/// Weighted-average share of the position rating blend.
/// 30/70 split (C.10): raises the achievable GOAT ceiling without eliminating the
/// all-round quality requirement. At 30/70 a fully trained elite player (Key≈99,
/// Imp≈91, Sec≈90) rates 97; the absolute GOAT (all attrs at 99) reaches 99.
/// Vacuum specialists (Fin=97, everything else at 40) rate 82 — above average but
/// clearly not elite vs top-of-table 97-99s. Previously 40/60 (C.7) which capped the
/// achievable GOAT at ~96 even with perfect seeds, because weighted_avg of 93 only
/// contributed 37.2 pts, leaving no room above 96 regardless of training.
pub const POS_RATING_AVG_PCT: Fixed = Fixed::raw(300);
/// Peak-lift share: pulls toward the player's best Key attribute.
pub const POS_RATING_PEAK_PCT: Fixed = Fixed::raw(700);

// ── Attribute-generation noise (appendix C.5) ────────────────────────────────

/// Multiplicative prime used to derive per-attribute noise seeds: hash(seed, attr).
pub const NOISE_SALT: u64 = 0x9e3779b97f4a7c15;
/// Noise half-width per spikiness point (pts of potential). Spikiness ∈ {1,2,3}.
/// Spikiness 1 → ±3, 2 → ±6, 3 → ±9.
pub const NOISE_WIDTH_PER_SPIKE: i32 = 3;

// ── Position-tier potential base percentages (appendix C.5) ──────────────────

/// Key attrs: potential centres at this % of talent ceiling.
/// Raised from 90→95 (C.9 OVR ceiling pass): at 90% Key attrs centered at ~89, blocking
/// the theoretical max OVR from exceeding ~94 even with perfect noise. At 95% Key attrs
/// center at ~94 with spiky noise reaching the full 99 ceiling, enabling the GOAT arc.
pub const KEY_BASE_PCT: u8 = 95;
/// Important attrs: potential centres at this % of ceiling.
/// Raised from 82→92 (C.9 OVR ceiling pass): at 82% Imp attrs averaged ~81, keeping
/// weighted_avg below 90 even for the best seeds. At 92% Imp ceilings reach ~91, allowing
/// elite players (all Key+Imp trained to ceiling) to push weighted_avg above 95.
pub const IMP_BASE_PCT: u8 = 92;
/// Secondary attrs: potential centres at this % of ceiling.
/// Raised from 88→91 (C.9b): at 88% Sec center=87, max+noise9=96 — Sec can never hit 99,
/// making OVR=99 mathematically impossible. At 91% Sec center=90, max+noise9=99, so
/// OVR=99 becomes theoretically achievable (all attrs must align at max noise — very rare
/// but possible with the right seed). Tier Key(95)>Imp(92)>Sec(91) preserved.
pub const SEC_BASE_PCT: u8 = 91;

// ── Familiarity-tier multipliers ──────────────────────────────────────────────

/// Natural: 1.000 — played the role all career; no penalty.
pub const FAM_NATURAL: Fixed = Fixed::raw(1_000);
/// Competent: 0.900 — comfortable, minor inefficiency.
pub const FAM_COMPETENT: Fixed = Fixed::raw(900);
/// Unconvincing: 0.700 — noticeable gaps.
pub const FAM_UNCONVINCING: Fixed = Fixed::raw(700);
/// Awkward: 0.500 — significant penalty; clearly out of position.
pub const FAM_AWKWARD: Fixed = Fixed::raw(500);

// ── Familiarity XP thresholds (to advance one tier) ──────────────────────────

/// Awkward → Unconvincing.
pub const FAM_XP_AWKWARD: Fixed = Fixed::raw(10_000);
/// Unconvincing → Competent.
pub const FAM_XP_UNCONVINCING: Fixed = Fixed::raw(25_000);
/// Competent → Natural (long road — requires sustained role-specific training).
pub const FAM_XP_COMPETENT: Fixed = Fixed::raw(60_000);

/// XP gained per week when a Key attribute for the role is in the focus list.
pub const FAM_XP_KEY_PER_WEEK: Fixed = Fixed::raw(30);
/// XP gained per week when only Important attributes are focused (no Key).
pub const FAM_XP_IMP_PER_WEEK: Fixed = Fixed::raw(10);

// ── Generation: talent ceiling ────────────────────────────────────────────────

/// Minimum overall talent ceiling (inclusive). Set to 99 so the PC always has max potential.
pub const CEILING_MIN: u8 = 99;
/// Maximum overall talent ceiling (inclusive).
pub const CEILING_MAX: u8 = 99;

// ── Generation: per-attribute potential ranges (as % of ceiling) ──────────────

pub const KEY_POT_LOW_PCT: u8 = 80;

pub const IMP_POT_LOW_PCT: u8 = 60;
pub const IMP_POT_HIGH_PCT: u8 = 90;

pub const BAS_POT_LOW_PCT: u8 = 40;
pub const BAS_POT_HIGH_PCT: u8 = 70;

/// Minimum potential for an attr with zero role weight (absolute, not %).
/// 40 = "functional pro" floor — no attribute is completely absent at professional level.
pub const NONE_POT_ABS_LOW: u8 = 40;
/// Maximum potential for an attr with zero role weight (as % of ceiling).
pub const NONE_POT_HIGH_PCT: u8 = 50;

// ── Generation: starting current values (age ~16, as Fixed multiplier) ────────

/// Physical archetype: starts at 85 % of potential.
pub const PHYSICAL_START_PCT: Fixed = Fixed::raw(850);
/// Technical archetype: starts at 70 % of potential.
pub const TECHNICAL_START_PCT: Fixed = Fixed::raw(700);
/// Mental archetype: starts at 55 % of potential.
pub const MENTAL_START_PCT: Fixed = Fixed::raw(550);

// ── Generation: familiarity seeding ──────────────────────────────────────────

/// Number of adjacent (Competent) roles seeded per position family.
pub const FAM_ADJACENT_COUNT: usize = 2;

// ── Week loop: age ────────────────────────────────────────────────────────────

/// Starting age in weeks (16 years × 52 weeks/year).
pub const START_AGE_WEEKS: u32 = 16 * 52;

// ── Week loop: energy ─────────────────────────────────────────────────────────

/// Starting energy level (out of 100).
pub const ENERGY_START: Fixed = Fixed::raw(90_000); // 90.0
/// Maximum energy.
pub const ENERGY_MAX: Fixed = Fixed::raw(100_000); // 100.0
/// Passive energy recovery applied every week regardless of intensity.
/// Set equal to the Medium cost so Medium training is indefinitely sustainable.
pub const ENERGY_PASSIVE_RECOVERY: Fixed = Fixed::raw(8_000); // 8.0
/// Extra energy recovery per week while injured (forced rest).
pub const ENERGY_RECOVERY_INJURED: Fixed = Fixed::raw(18_000); // 18.0
/// Below this energy level, intensity is auto-downgraded to Low.
pub const ENERGY_AUTO_DOWNGRADE: Fixed = Fixed::raw(20_000); // 20.0
/// Energy cost per week: Low intensity — net positive (+5) with passive recovery.
pub const ENERGY_COST_LOW: Fixed = Fixed::raw(3_000); // 3.0
/// Energy cost per week: Medium intensity — net zero with passive recovery.
pub const ENERGY_COST_MED: Fixed = Fixed::raw(8_000); // 8.0
/// Energy cost per week: High intensity — drains ~10/wk; depletes in ~7 weeks from full.
pub const ENERGY_COST_HIGH: Fixed = Fixed::raw(18_000); // 18.0

// ── Week loop: growth ─────────────────────────────────────────────────────────

/// Base attribute gain per week per focused attribute before any multipliers.
pub const BASE_GROWTH_PER_WEEK: Fixed = Fixed::raw(200); // 0.200
/// Growth multiplier at Low intensity.
pub const GROWTH_MULT_LOW: Fixed = Fixed::raw(600); // 0.600
/// Growth multiplier at Medium intensity.
pub const GROWTH_MULT_MED: Fixed = Fixed::raw(1_000); // 1.000
/// Growth multiplier at High intensity.
pub const GROWTH_MULT_HIGH: Fixed = Fixed::raw(1_500); // 1.500
/// Random variance band: ± this many thousandths each week.
pub const GROWTH_VARIANCE_RAW: i32 = 50; // ±0.050
/// Single-week gain cap — prevents extreme lucky streaks.
pub const GROWTH_SINGLE_WEEK_CAP: Fixed = Fixed::raw(800); // 0.800

// ── Week loop: decay ──────────────────────────────────────────────────────────

/// Base attribute decay per week per unfocused attr when decay_rate > 0.
pub const BASE_DECAY_PER_WEEK: Fixed = Fixed::raw(100); // 0.100

// ── Week loop: injury ─────────────────────────────────────────────────────────

/// Base injury probability (per 1 000 rolls) at full energy, medium intensity, age 21.
pub const BASE_INJURY_PER_1000: u32 = 15; // 1.5 %
/// Min injury duration (weeks).
pub const INJURY_WEEKS_MIN: u8 = 1;
/// Max injury duration (weeks).
pub const INJURY_WEEKS_MAX: u8 = 8;

// ── Lifestyle ↔ consequence (bible §8.6) ──────────────────────────────────────
// The identity fork must be a real trade-off: a professional lifestyle extends the
// peak and lowers injury risk; a flashy one burns out early. Balanced is the neutral
// baseline (×1.0 everywhere) so all pre-existing golden values stay frozen.
// Placeholders illustrating shape — frozen only on user approval.

/// Injury-risk multiplier (×10) by lifestyle: Professional 0.7×, Balanced 1.0×, Flashy 1.5×.
/// Tuned (seed sweep 0–19): keeps Pro robustly the safest tier and the injury spread
/// well-proportioned (Flashy ≈ 2.1× Pro injured weeks) without an extra duration lever.
pub const INJURY_LIFESTYLE_X10_PRO: u32 = 7;
pub const INJURY_LIFESTYLE_X10_BALANCED: u32 = 10;
pub const INJURY_LIFESTYLE_X10_FLASHY: u32 = 15;

/// Effective ceiling: fraction of *potential* a lifestyle lets the player actually
/// reach. Flashy burns a little of the ceiling (0.96); Professional/Balanced reach
/// full potential. Never raises a current attr above its potential (pillar §2.4).
pub const LIFESTYLE_CEILING_PRO: Fixed = Fixed::raw(1_000); // 1.000 — full potential
pub const LIFESTYLE_CEILING_BALANCED: Fixed = Fixed::raw(1_000); // 1.000
pub const LIFESTYLE_CEILING_FLASHY: Fixed = Fixed::raw(960); // 0.960 — burned potential

/// Age-decline multiplier by lifestyle: Professional 0.7× (gentler, later burnout),
/// Balanced 1.0×, Flashy 1.4× (steeper, earlier decline). Multiplies the base decay.
pub const DECLINE_LIFESTYLE_PRO: Fixed = Fixed::raw(700); // 0.700
pub const DECLINE_LIFESTYLE_BALANCED: Fixed = Fixed::raw(1_000); // 1.000
pub const DECLINE_LIFESTYLE_FLASHY: Fixed = Fixed::raw(1_400); // 1.400

// ── Week loop: breakthrough ───────────────────────────────────────────────────

/// Chance of a training breakthrough event (per 1 000 rolls per week, when training).
pub const BREAKTHROUGH_PER_1000: u32 = 8; // 0.8 %
/// Extra attribute gain from a breakthrough.
pub const BREAKTHROUGH_BONUS: Fixed = Fixed::raw(1_000); // +1.000

// ── Facilities multipliers (indexed by stub club, matching STUB_CLUBS order) ──

/// Per-club development multiplier for the 5 stub clubs. Phase 5 will derive
/// from real club data; for now these are placeholders.
pub const FACILITIES_MULTS: [Fixed; 5] = [
    Fixed::raw(800),   // Local FC          0.800
    Fixed::raw(900),   // Academy United    0.900
    Fixed::raw(1_000), // Riverside Town    1.000
    Fixed::raw(1_100), // Hillside Athletic 1.100
    Fixed::raw(1_200), // Valley Rangers    1.200
];

// ── Phase 10 — economy (TASK-10B.1) ────────────────────────────────────────────
// Money in thousands. Advantages are ceiling-capped (§2.4): a dev multiplier speeds
// growth but never lifts a current attribute above its potential.

/// Annual lifestyle upkeep by lifestyle (0=Professional,1=Balanced,2=Flashy), in £k.
/// A flashy lifestyle burns cash; a professional one is frugal.
pub const UPKEEP_BY_LIFESTYLE: [i64; 3] = [40, 80, 200];
/// Annual business/investment return, in tenths of a percent of business value.
pub const INVEST_RETURN_PER_1000: i64 = 70; // ~7%/yr baseline
/// Business return variance band (± this many tenths-of-percent), rolled each season.
pub const INVEST_VARIANCE_PER_1000: i64 = 120; // swings -5%..+19% around baseline
/// Savings (thousands) below which the player is bankrupt.
pub const BANKRUPTCY_FLOOR: i64 = -500;
/// Ceiling-capped development multiplier by invest level 0–3. Level 0 is neutral (×1.0)
/// so the no-spend path stays byte-identical to existing goldens.
pub const DEV_INVEST_MULT: [Fixed; 4] = [
    Fixed::raw(1_000), // 0 — none (neutral)
    Fixed::raw(1_060), // 1 — nutrition
    Fixed::raw(1_120), // 2 — + private trainer
    Fixed::raw(1_180), // 3 — + full performance team
];
/// Annual cost of each development-investment level, in £k.
pub const DEV_INVEST_COST: [i64; 4] = [0, 60, 160, 320];

// ── Phase 10 — sponsors + marketability (TASK-10B.2) ───────────────────────────

/// Marketability thresholds: index 0 = min for Local tier, 1 = National, 2 = Global.
pub const SPONSOR_TIER_THRESHOLDS: [i32; 3] = [25, 50, 75];
/// Annual sponsor income by tier (0=none,1=local,2=national,3=global), in £k.
pub const SPONSOR_INCOME: [i64; 4] = [0, 150, 600, 2_000];
/// Weekly energy cost of sponsor obligations by tier (the same resource training needs).
/// Tier 0 is 0.0 so the no-sponsor path is byte-identical to existing goldens.
pub const SPONSOR_ENERGY_COST: [Fixed; 4] = [
    Fixed::raw(0),     // none
    Fixed::raw(1_000), // local      1.0/wk
    Fixed::raw(2_500), // national   2.5/wk
    Fixed::raw(5_000), // global     5.0/wk
];
/// Reputation hit for over-commercialising: signing a tier your sporting merit doesn't
/// justify (tier outranks sporting-rep band) dents Sporting rep by this much.
pub const OVERCOMMERCIAL_REP_PENALTY: i32 = 8;

// ── Phase 10 — relationships, scandals, media (TASK-10B.3) ─────────────────────

/// A relationship thread below this stability ruptures (triggers a scandal).
pub const RELATIONSHIP_RUPTURE_THRESHOLD: i32 = 25;
/// Character-rep hit when a relationship ruptures into a scandal.
pub const SCANDAL_CHARACTER_HIT: i32 = 15;
/// Scandals also dent marketability (sponsors get nervous).
pub const SCANDAL_MARKETABILITY_HIT: i32 = 10;
/// Media flashpoint — CONTRITE response: (Character delta, Sporting delta). Looks humble:
/// rebuilds Character, costs a little Sporting standing.
pub const MEDIA_CONTRITE: (i32, i32) = (12, -5);
/// Media flashpoint — DEFIANT response: (Character delta, Sporting delta). Fans love the
/// fire (Sporting up) but it reads as arrogant (Character down).
pub const MEDIA_DEFIANT: (i32, i32) = (-8, 10);

// ── Phase 10 — retirement trigger (TASK-10B.4) ─────────────────────────────────

/// Soft retirement age: past it, a player with no contract is expected to retire.
pub const RETIRE_AGE_SOFT: u32 = 34;
/// Hard retirement age: nobody plays on past it.
pub const RETIRE_AGE_HARD: u32 = 40;

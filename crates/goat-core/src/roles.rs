//! Outfield roles, familiarity tiers, and the role-weight table.
//!
//! The weight table encodes how much each sub-attribute contributes to each role
//! rating. Weights are Key / Important / Baseline / Zero (see `tuning`).
//! Goalkeeping role is parked (bible §11).

use crate::attrs::NUM_ATTRS;
use crate::tuning::{
    FAM_AWKWARD, FAM_COMPETENT, FAM_NATURAL, FAM_UNCONVINCING, W_BAS, W_IMP, W_KEY, W_ZERO,
};
use goat_fixed::Fixed;

/// Total number of implemented outfield roles.
pub const NUM_ROLES: usize = 14;

/// Outfield role identifiers.
///
/// The `usize` discriminant is the row index in `ROLE_WEIGHT_TABLE` and in
/// familiarity columns. Order is load-bearing — do not reorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum RoleId {
    CentreBack = 0,
    Sweeper = 1,
    FullBack = 2,
    WingBack = 3,
    DefensiveMid = 4,
    CentralMid = 5,
    BoxToBox = 6,
    AttackingMid = 7,
    Winger = 8,
    InsideForward = 9,
    TargetForward = 10,
    PressingForward = 11,
    CompleteForward = 12,
    Trequartista = 13,
}

impl RoleId {
    pub const ALL: [RoleId; NUM_ROLES] = [
        RoleId::CentreBack,
        RoleId::Sweeper,
        RoleId::FullBack,
        RoleId::WingBack,
        RoleId::DefensiveMid,
        RoleId::CentralMid,
        RoleId::BoxToBox,
        RoleId::AttackingMid,
        RoleId::Winger,
        RoleId::InsideForward,
        RoleId::TargetForward,
        RoleId::PressingForward,
        RoleId::CompleteForward,
        RoleId::Trequartista,
    ];

    pub fn name(self) -> &'static str {
        ROLE_NAMES[self as usize]
    }
}

/// Human-readable name for each role.
pub const ROLE_NAMES: [&str; NUM_ROLES] = [
    "Centre Back",
    "Sweeper",
    "Full Back",
    "Wing Back",
    "Defensive Mid",
    "Central Mid",
    "Box-to-Box",
    "Attacking Mid",
    "Winger",
    "Inside Forward",
    "Target Forward",
    "Pressing Forward",
    "Complete Forward",
    "Trequartista",
];

/// Which position family a role belongs to (for generation bias + familiarity seeding).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionFamily {
    Defender,
    Midfielder,
    Forward,
}

/// Position family for each role, indexed by `RoleId as usize`.
pub const ROLE_POSITION_FAMILY: [PositionFamily; NUM_ROLES] = [
    PositionFamily::Defender,   // CentreBack
    PositionFamily::Defender,   // Sweeper
    PositionFamily::Defender,   // FullBack
    PositionFamily::Defender,   // WingBack
    PositionFamily::Midfielder, // DefensiveMid
    PositionFamily::Midfielder, // CentralMid
    PositionFamily::Midfielder, // BoxToBox
    PositionFamily::Midfielder, // AttackingMid
    PositionFamily::Midfielder, // Winger
    PositionFamily::Forward,    // InsideForward
    PositionFamily::Forward,    // TargetForward
    PositionFamily::Forward,    // PressingForward
    PositionFamily::Forward,    // CompleteForward
    PositionFamily::Forward,    // Trequartista
];

/// The four familiarity tiers (bible §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FamiliarityTier {
    /// 0.500 × rating — clearly out of position.
    Awkward = 0,
    /// 0.700 × rating — noticeable gaps.
    Unconvincing = 1,
    /// 0.900 × rating — comfortable, minor inefficiency.
    Competent = 2,
    /// 1.000 × rating — no penalty.
    Natural = 3,
}

impl FamiliarityTier {
    /// Fixed-point multiplier applied to the raw role rating.
    pub fn multiplier(self) -> Fixed {
        match self {
            FamiliarityTier::Natural => FAM_NATURAL,
            FamiliarityTier::Competent => FAM_COMPETENT,
            FamiliarityTier::Unconvincing => FAM_UNCONVINCING,
            FamiliarityTier::Awkward => FAM_AWKWARD,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            FamiliarityTier::Natural => "Natural",
            FamiliarityTier::Competent => "Competent",
            FamiliarityTier::Unconvincing => "Unconvincing",
            FamiliarityTier::Awkward => "Awkward",
        }
    }

    /// Advance one tier upward; returns `None` if already Natural.
    pub fn upgrade(self) -> Option<Self> {
        match self {
            FamiliarityTier::Awkward => Some(FamiliarityTier::Unconvincing),
            FamiliarityTier::Unconvincing => Some(FamiliarityTier::Competent),
            FamiliarityTier::Competent => Some(FamiliarityTier::Natural),
            FamiliarityTier::Natural => None,
        }
    }
}

// ── Role-weight table ─────────────────────────────────────────────────────────
//
// Columns correspond to AttrId ordinals 0..30:
// [Acc, Spd, Fin, LSh, ShP, Vol, Pen, SPa, LPa, Vis, Cro, FKA,
//  Dri, BCo, Agi, Bal, Rea, Tck, Mak, Int, Hed, DAw, Pos,
//  Str, Sta, Agg, Jmp, Com, WkR, Con]

/// Role-weight table: `ROLE_WEIGHT_TABLE[role][attr]` = weight of attr in role rating.
///
/// role_rating = weighted_average(attrs, weights) × familiarity.
/// Normalization by Σweights keeps the output in [0, 99] for valid attributes.
pub const ROLE_WEIGHT_TABLE: [[Fixed; NUM_ATTRS]; NUM_ROLES] = [
    // CentreBack [0]
    // K: Tck[17] Mak[18] Int[19] Hed[20] Str[23] Con[29]
    // I: DAw[21] Pos[22] Jmp[26] Com[27]
    // B: SPa[7] Bal[15] Rea[16] Sta[24] WkR[28]
    [
        W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_BAS, W_ZERO, W_ZERO, W_ZERO,
        W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_BAS, W_BAS, W_KEY, W_KEY, W_KEY, W_KEY, W_IMP, W_IMP,
        W_KEY, W_BAS, W_ZERO, W_IMP, W_IMP, W_BAS, W_KEY,
    ],
    // Sweeper [1]
    // K: Vis[9] Rea[16] DAw[21] Pos[22] Com[27]
    // I: SPa[7] LPa[8] Tck[17] Int[19] Con[29]
    // B: Mak[18] Hed[20] Bal[15] Sta[24]
    [
        W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_IMP, W_IMP, W_KEY, W_ZERO,
        W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_BAS, W_KEY, W_IMP, W_BAS, W_IMP, W_BAS, W_KEY, W_KEY,
        W_ZERO, W_BAS, W_ZERO, W_ZERO, W_KEY, W_ZERO, W_IMP,
    ],
    // FullBack [2]
    // K: Spd[1] Cro[10] Tck[17] Mak[18] Sta[24]
    // I: SPa[7] Agi[14] Rea[16] DAw[21] WkR[28]
    // B: Acc[0] LPa[8] Bal[15] Com[27] Con[29]
    [
        W_BAS, W_KEY, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_IMP, W_BAS, W_ZERO, W_KEY, W_ZERO,
        W_ZERO, W_ZERO, W_IMP, W_BAS, W_IMP, W_KEY, W_KEY, W_ZERO, W_ZERO, W_IMP, W_ZERO, W_ZERO,
        W_KEY, W_ZERO, W_ZERO, W_BAS, W_IMP, W_BAS,
    ],
    // WingBack [3]
    // K: Spd[1] Cro[10] Dri[12] Agi[14] Sta[24]
    // I: SPa[7] Bal[15] Rea[16] Tck[17] WkR[28]
    // B: Acc[0] Fin[2] BCo[13] Mak[18] Com[27]
    [
        W_BAS, W_KEY, W_BAS, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_IMP, W_ZERO, W_ZERO, W_KEY, W_ZERO,
        W_KEY, W_BAS, W_KEY, W_IMP, W_IMP, W_IMP, W_BAS, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO,
        W_KEY, W_ZERO, W_ZERO, W_BAS, W_IMP, W_ZERO,
    ],
    // DefensiveMid [4]
    // K: SPa[7] Tck[17] Int[19] DAw[21] Pos[22]
    // I: Vis[9] Rea[16] Str[23] Com[27] WkR[28]  Con[29]
    // B: LPa[8] Mak[18] Bal[15] Sta[24]
    [
        W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_KEY, W_BAS, W_IMP, W_ZERO,
        W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_BAS, W_IMP, W_KEY, W_BAS, W_KEY, W_ZERO, W_KEY, W_KEY,
        W_IMP, W_BAS, W_ZERO, W_ZERO, W_IMP, W_IMP, W_IMP,
    ],
    // CentralMid [5]
    // K: SPa[7] Vis[9] Sta[24] Com[27] Con[29]
    // I: LPa[8] BCo[13] Rea[16] Pos[22] WkR[28]
    // B: Dri[12] Agi[14] Tck[17] Int[19]
    [
        W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_KEY, W_IMP, W_KEY, W_ZERO,
        W_ZERO, W_BAS, W_IMP, W_BAS, W_ZERO, W_IMP, W_BAS, W_ZERO, W_BAS, W_ZERO, W_ZERO, W_IMP,
        W_ZERO, W_KEY, W_ZERO, W_ZERO, W_KEY, W_IMP, W_KEY,
    ],
    // BoxToBox [6]
    // K: SPa[7] Tck[17] Sta[24] WkR[28]  Fin[2]
    // I: Spd[1] ShP[4] LPa[8] Rea[16] Int[19] Pos[22] Str[23] Agg[25]
    // B: Vis[9] Dri[12] BCo[13] Hed[20]
    [
        W_ZERO, W_IMP, W_KEY, W_ZERO, W_IMP, W_ZERO, W_ZERO, W_KEY, W_IMP, W_BAS, W_ZERO, W_ZERO,
        W_BAS, W_BAS, W_ZERO, W_ZERO, W_IMP, W_KEY, W_ZERO, W_IMP, W_BAS, W_ZERO, W_IMP, W_IMP,
        W_KEY, W_IMP, W_ZERO, W_ZERO, W_KEY, W_ZERO,
    ],
    // AttackingMid [7]
    // K: SPa[7] Vis[9] Dri[12] BCo[13] Com[27]
    // I: Fin[2] LSh[3] Agi[14] Bal[15] Rea[16]
    // B: Acc[0] Spd[1] FKA[11] Pos[22] WkR[28]  Con[29]
    [
        W_BAS, W_BAS, W_IMP, W_IMP, W_ZERO, W_ZERO, W_ZERO, W_KEY, W_ZERO, W_KEY, W_ZERO, W_BAS,
        W_KEY, W_KEY, W_IMP, W_IMP, W_IMP, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_BAS, W_ZERO,
        W_ZERO, W_ZERO, W_ZERO, W_KEY, W_BAS, W_BAS,
    ],
    // Winger [8]
    // K: Acc[0] Spd[1] Cro[10] Dri[12] Agi[14]
    // I: Fin[2] LSh[3] SPa[7] BCo[13] Bal[15] Rea[16]
    // B: Vol[5] Vis[9] Sta[24] WkR[28]
    [
        W_KEY, W_KEY, W_IMP, W_IMP, W_ZERO, W_BAS, W_ZERO, W_IMP, W_ZERO, W_BAS, W_KEY, W_ZERO,
        W_KEY, W_IMP, W_KEY, W_IMP, W_IMP, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO,
        W_BAS, W_ZERO, W_ZERO, W_ZERO, W_BAS, W_ZERO,
    ],
    // InsideForward [9]
    // K: Acc[0] Fin[2] Dri[12] BCo[13] Agi[14]
    // I: LSh[3] Vol[5] SPa[7] Bal[15] Rea[16] Com[27]
    // B: Spd[1] Cro[10] WkR[28]
    [
        W_KEY, W_BAS, W_KEY, W_IMP, W_ZERO, W_IMP, W_ZERO, W_IMP, W_ZERO, W_ZERO, W_BAS, W_ZERO,
        W_KEY, W_KEY, W_KEY, W_IMP, W_IMP, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO,
        W_ZERO, W_ZERO, W_ZERO, W_IMP, W_BAS, W_ZERO,
    ],
    // TargetForward [10]
    // K: Fin[2] Hed[20] Str[23] Jmp[26] Pos[22]
    // I: ShP[4] Vol[5] BCo[13] Rea[16] Agg[25] Com[27] Con[29]
    // B: LSh[3] Pen[6] SPa[7] Bal[15] WkR[28]
    [
        W_ZERO, W_ZERO, W_KEY, W_BAS, W_IMP, W_IMP, W_BAS, W_BAS, W_ZERO, W_ZERO, W_ZERO, W_ZERO,
        W_ZERO, W_IMP, W_ZERO, W_BAS, W_IMP, W_ZERO, W_ZERO, W_ZERO, W_KEY, W_ZERO, W_IMP, W_KEY,
        W_ZERO, W_IMP, W_KEY, W_IMP, W_BAS, W_IMP,
    ],
    // PressingForward [11]
    // K: Acc[0] Spd[1] Sta[24] Agg[25] WkR[28]
    // I: Fin[2] BCo[13] Rea[16] DAw[21] Pos[22]
    // B: ShP[4] SPa[7] Hed[20] Str[23]
    [
        W_KEY, W_KEY, W_IMP, W_ZERO, W_BAS, W_ZERO, W_ZERO, W_BAS, W_ZERO, W_ZERO, W_ZERO, W_ZERO,
        W_ZERO, W_IMP, W_ZERO, W_ZERO, W_IMP, W_ZERO, W_ZERO, W_ZERO, W_BAS, W_IMP, W_IMP, W_BAS,
        W_KEY, W_KEY, W_ZERO, W_ZERO, W_KEY, W_ZERO,
    ],
    // CompleteForward [12]
    // K: Fin[2] ShP[4] Dri[12] BCo[13] Com[27]
    // I: Acc[0] Spd[1] LSh[3] Pen[6] SPa[7] Vis[9] Agi[14] Bal[15] Rea[16] Hed[20] Str[23]
    // B: Vol[5] Pos[22] Sta[24]
    [
        W_IMP, W_IMP, W_KEY, W_IMP, W_KEY, W_BAS, W_IMP, W_IMP, W_ZERO, W_IMP, W_ZERO, W_ZERO,
        W_KEY, W_KEY, W_IMP, W_IMP, W_IMP, W_ZERO, W_ZERO, W_ZERO, W_IMP, W_ZERO, W_BAS, W_IMP,
        W_BAS, W_ZERO, W_ZERO, W_KEY, W_ZERO, W_ZERO,
    ],
    // Trequartista [13]
    // K: SPa[7] Vis[9] Dri[12] BCo[13] Com[27]
    // I: LPa[8] Fin[2] Agi[14] Bal[15] Rea[16]
    // B: Acc[0] Spd[1] LSh[3] Vol[5] FKA[11] Pos[22]
    [
        W_BAS, W_BAS, W_IMP, W_BAS, W_ZERO, W_BAS, W_ZERO, W_KEY, W_IMP, W_KEY, W_ZERO, W_BAS,
        W_KEY, W_KEY, W_IMP, W_IMP, W_IMP, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_ZERO, W_BAS, W_ZERO,
        W_ZERO, W_ZERO, W_ZERO, W_KEY, W_ZERO, W_ZERO,
    ],
];

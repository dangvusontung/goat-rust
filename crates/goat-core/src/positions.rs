//! Per-position weight tables and position rating (appendix C.1–C.4).
//!
//! Replaces the dense W_KEY/W_IMP/W_BAS role-weight scheme for OVR purposes.
//! Role weights in `roles.rs` are kept for the match engine and familiarity XP.

use crate::attrs::NUM_ATTRS;
use goat_fixed::Fixed;

/// Number of outfield positions. GK is parked (§11).
pub const NUM_POSITIONS: usize = 8;

/// The eight outfield primary positions (appendix C.2).
///
/// Stored as a `u8` column in `PlayerStore`; variant index = column value.
/// Initialised from the career-creation choice and migrates with reinvention (§5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum PrimaryPosition {
    #[default]
    ST = 0, // Striker
    W = 1,   // Winger (LW/RW)
    WM = 2,  // Wide Midfielder (LM/RM)
    CAM = 3, // Attacking Midfielder
    CM = 4,  // Central Midfielder
    DM = 5,  // Defensive Midfielder
    FB = 6,  // Full Back (LB/RB)
    CB = 7,  // Centre Back
}

impl PrimaryPosition {
    pub const ALL: [PrimaryPosition; NUM_POSITIONS] = [
        PrimaryPosition::ST,
        PrimaryPosition::W,
        PrimaryPosition::WM,
        PrimaryPosition::CAM,
        PrimaryPosition::CM,
        PrimaryPosition::DM,
        PrimaryPosition::FB,
        PrimaryPosition::CB,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PrimaryPosition::ST => "ST",
            PrimaryPosition::W => "W",
            PrimaryPosition::WM => "WM",
            PrimaryPosition::CAM => "CAM",
            PrimaryPosition::CM => "CM",
            PrimaryPosition::DM => "DM",
            PrimaryPosition::FB => "FB",
            PrimaryPosition::CB => "CB",
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(PrimaryPosition::ST),
            1 => Some(PrimaryPosition::W),
            2 => Some(PrimaryPosition::WM),
            3 => Some(PrimaryPosition::CAM),
            4 => Some(PrimaryPosition::CM),
            5 => Some(PrimaryPosition::DM),
            6 => Some(PrimaryPosition::FB),
            7 => Some(PrimaryPosition::CB),
            _ => None,
        }
    }

    /// Which broad position family this specific position belongs to (bible §4/§5.3):
    /// used for generation role-DNA bias and familiarity seeding. Mirrors the convention
    /// in `roles::ROLE_POSITION_FAMILY` (e.g. `RoleId::Winger` is Midfielder family) — no
    /// 4th family is introduced.
    pub fn family(self) -> crate::roles::PositionFamily {
        use crate::roles::PositionFamily;
        match self {
            PrimaryPosition::CB | PrimaryPosition::FB => PositionFamily::Defender,
            PrimaryPosition::DM
            | PrimaryPosition::CM
            | PrimaryPosition::WM
            | PrimaryPosition::CAM
            | PrimaryPosition::W => PositionFamily::Midfielder,
            PrimaryPosition::ST => PositionFamily::Forward,
        }
    }
}

/// Position familiarity tiers from appendix C.3 (distinct from role familiarity in §5.2).
///
/// These apply to the *position* the player is deployed in, not their role profile.
/// Primary position = Natural; out-of-position deployment uses the lower tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionFamiliarity {
    Natural,    // 1.000 — own position
    Competent,  // 0.930 — adjacent position
    Awkward,    // 0.800 — same broad area
    Unfamiliar, // 0.650 — completely different zone
}

impl PositionFamiliarity {
    pub fn multiplier(self) -> Fixed {
        match self {
            PositionFamiliarity::Natural => Fixed::raw(1_000),
            PositionFamiliarity::Competent => Fixed::raw(930),
            PositionFamiliarity::Awkward => Fixed::raw(800),
            PositionFamiliarity::Unfamiliar => Fixed::raw(650),
        }
    }
}

/// Sparse per-position weight table (appendix C.2).
///
/// `POSITION_WEIGHT_TABLE[pos][attr]`:
/// - 3 = Key attribute for this position
/// - 2 = Important
/// - 1 = Secondary
/// - 0 = irrelevant (zero weight — does not contribute to position rating)
///
/// Attr index layout matches `AttrId as usize` (see attrs.rs):
/// [0]=Acc, [1]=Spd, [2]=Fin, [3]=LSh, [4]=ShP, [5]=Vol, [6]=Pen,
/// [7]=SPa, [8]=LPa, [9]=Vis, [10]=Cro, [11]=FKA, [12]=CC, [13]=BCo,
/// [14]=Agi, [15]=Bal, [16]=Rea, [17]=StTck, [18]=Mak, [19]=Int,
/// [20]=Hed, [21]=Crv, [22]=AttPos, [23]=Str, [24]=Sta, [25]=Agg,
/// [26]=Jmp, [27]=Com, [28]=Bra, [29]=SlTck
pub const POSITION_WEIGHT_TABLE: [[u8; NUM_ATTRS]; NUM_POSITIONS] = [
    // ST — Striker
    // Key: Finishing[2], AttPositioning[22]
    // Imp: Composure[27], ShotPower[4], BallControl[13]
    // Sec: Reactions[16], Acceleration[0], SprintSpeed[1], Heading[20],
    //      Strength[23], Volleys[5], CloseControl[12], ShortPassing[7], Penalties[6]
    [
        1, 1, 3, 0, 2, 1, 1, 1, 0, 0, 0, 0, 1, 2, 0, 0, 1, 0, 0, 0, 1, 0, 3, 1, 0, 0, 0, 2, 0, 0,
    ],
    // W — Winger (LW/RW)
    // Key: Acceleration[0], SprintSpeed[1], CloseControl[12]
    // Imp: Agility[14], BallControl[13], Crossing[10], Finishing[2]
    // Sec: Curve[21], Balance[15], AttPositioning[22], Composure[27],
    //      ShortPassing[7], Reactions[16]
    [
        3, 3, 2, 0, 0, 0, 0, 1, 0, 0, 2, 0, 3, 2, 2, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0,
    ],
    // WM — Wide Midfielder (LM/RM)
    // Key: Crossing[10], Stamina[24]
    // Imp: ShortPassing[7], Acceleration[0], SprintSpeed[1], CloseControl[12]
    // Sec: BallControl[13], Agility[14], Vision[9], LongPassing[8],
    //      Finishing[2], Balance[15], Reactions[16]
    [
        2, 2, 1, 0, 0, 0, 0, 2, 1, 1, 3, 0, 2, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0,
    ],
    // CAM — Attacking Midfielder
    // Key: Vision[9], ShortPassing[7], BallControl[13]
    // Imp: Composure[27], LongPassing[8], Curve[21], Finishing[2]
    // Sec: Agility[14], CloseControl[12], LongShots[3], Reactions[16], AttPositioning[22]
    [
        0, 0, 2, 1, 0, 0, 0, 3, 2, 3, 0, 0, 1, 3, 1, 0, 1, 0, 0, 0, 0, 2, 1, 0, 0, 0, 0, 2, 0, 0,
    ],
    // CM — Central Midfielder
    // Key: ShortPassing[7], Stamina[24]
    // Imp: Vision[9], BallControl[13], LongPassing[8], Composure[27]
    // Sec: Interceptions[19], StandingTackle[17], Reactions[16], Strength[23],
    //      LongShots[3], Aggression[25]
    [
        0, 0, 0, 1, 0, 0, 0, 3, 2, 2, 0, 0, 0, 2, 0, 0, 1, 1, 0, 1, 0, 0, 0, 1, 3, 1, 0, 2, 0, 0,
    ],
    // DM — Defensive Midfielder
    // Key: Interceptions[19], StandingTackle[17]
    // Imp: Marking[18], ShortPassing[7], Strength[23], Stamina[24], Composure[27]
    // Sec: Aggression[25], LongPassing[8], Reactions[16], SlidingTackle[29], Bravery[28]
    [
        0, 0, 0, 0, 0, 0, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 1, 3, 2, 3, 0, 0, 0, 2, 2, 1, 0, 2, 1, 1,
    ],
    // FB — Full Back (LB/RB)
    // Key: StandingTackle[17], Stamina[24]
    // Imp: Marking[18], Acceleration[0], SprintSpeed[1], Interceptions[19], Crossing[10]
    // Sec: ShortPassing[7], SlidingTackle[29], Agility[14], Strength[23],
    //      Reactions[16], AttPositioning[22]
    [
        2, 2, 0, 0, 0, 0, 0, 1, 0, 0, 2, 0, 0, 0, 1, 0, 1, 3, 2, 2, 0, 0, 1, 1, 3, 0, 0, 0, 0, 1,
    ],
    // CB — Centre Back
    // Key: Marking[18], StandingTackle[17], Heading[20]
    // Imp: Strength[23], Jumping[26], Interceptions[19], SlidingTackle[29]
    // Sec: Composure[27], Bravery[28], Reactions[16], Aggression[25],
    //      ShortPassing[7], Acceleration[0]
    [
        1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 3, 3, 2, 3, 0, 0, 2, 0, 1, 2, 1, 1, 2,
    ],
];

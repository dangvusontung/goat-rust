//! Dated factual PC records. No random draws and no invented past.
use goat_fixed::Fixed;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingWeek {
    pub epoch_day: u32,
    pub age_weeks: u32,
    /// Bitset of selected attributes; zero for recovery-only weeks.
    pub focus_mask: u32,
    /// Requested intensity (0/1/2), and actual intensity (3 = injured/rest).
    pub requested_intensity: u8,
    pub effective_intensity: u8,
    /// Includes lifestyle, investment and staff modifiers.
    pub facilities: Fixed,
    pub energy_before: Fixed,
    pub energy_after: Fixed,
    pub injury_before: u32,
    pub injury_after: u32,
    pub total_attribute_delta: Fixed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthEvent {
    pub epoch_day: u32,
    /// 0 = training injury, 1 = match injury, 2 = recovered.
    pub kind: u8,
    pub remaining_weeks: u32,
}

/// Actual match workload received by core; minutes/medical diagnoses are not inferred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchWorkload {
    pub epoch_day: u32,
    pub energy_before: Fixed,
    pub energy_after: Fixed,
    pub energy_cost: Fixed,
    pub injury_before: u32,
    pub injury_after: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevelopmentHistory {
    pub weeks: Vec<TrainingWeek>,
    pub health: Vec<HealthEvent>,
    pub matches: Vec<MatchWorkload>,
}

/// Path-dependent minutes from an engine-resolved fixture; zero records an actual DNP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcMatchLoad {
    pub competition_id: u32,
    pub pop_idx: u32,
    pub fixture_id: u64,
    pub epoch_day: u32,
    pub minutes: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcMinutes {
    pub pop_idx: u32,
    pub minutes: u16,
}

//! Opt-in stress world. Never changes the standard country's/league/club registry.
use crate::world::{WorldGenesis, NUM_CLUBS};
const CHECKPOINT_TAG: &[u8; 4] = b"CAPB";
#[derive(Clone, Copy)]
pub(crate) struct CapacityProfile {
    pub players: u32,
    pub deep_budget: u32,
}
impl CapacityProfile {
    pub fn new(players: u32, deep_budget: u32) -> Result<Self, &'static str> {
        if !(1..=12).contains(&deep_budget) {
            return Err("deep budget must be 1..=12");
        }
        if players != 0
            && !(NUM_CLUBS as u32 * 18..=NUM_CLUBS as u32 * u8::MAX as u32).contains(&players)
        {
            return Err("stress population must fit 18..=255 NPCs per existing club");
        }
        Ok(Self {
            players,
            deep_budget,
        })
    }
    pub fn world(self, mut world: WorldGenesis) -> WorldGenesis {
        if self.players != 0 {
            let clubs = world.clubs.len() as u32;
            for club in &mut world.clubs {
                club.squad_size = (self.players / clubs
                    + u32::from((club.id as u32) < self.players % clubs))
                    as u8;
            }
        }
        world
    }
    pub fn wrap_checkpoint(self, body: Vec<u8>) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(body.len() + 12);
        bytes.extend_from_slice(CHECKPOINT_TAG);
        bytes.extend_from_slice(&self.players.to_le_bytes());
        bytes.extend_from_slice(&self.deep_budget.to_le_bytes());
        bytes.extend_from_slice(&body);
        bytes
    }
    pub fn checkpoint_body(self, bytes: &[u8]) -> Option<&[u8]> {
        if bytes.get(..4)? != CHECKPOINT_TAG
            || u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?) != self.players
            || u32::from_le_bytes(bytes.get(8..12)?.try_into().ok()?) != self.deep_budget
        {
            return None;
        }
        bytes.get(12..)
    }
}
/// The capacity factory preserves all structural IDs; this is roster pressure,
/// not a benchmark of future 150-country fixture volume.
pub fn world(players: u32, deep_budget: u32, seed: u64) -> Result<WorldGenesis, &'static str> {
    Ok(CapacityProfile::new(players, deep_budget)?.world(WorldGenesis::generate(seed)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_200k_population_and_stable_structural_ids() {
        let world = world(200_000, 6, 42).unwrap();
        assert_eq!(
            world
                .clubs
                .iter()
                .map(|c| c.squad_size as usize)
                .sum::<usize>(),
            200_000
        );
        assert_eq!(world.clubs.len(), 1200);
        assert_eq!(world.leagues.len(), 60);
        assert_eq!(world.nations.len(), 20);
        let pop = crate::population::genesis_dated(42, &world, 2023);
        assert_eq!(pop.len(), 200_000);
        assert!(pop.club.iter().all(|&c| c < 1200));
    }
    #[test]
    fn diagnostic_checkpoint_rejects_normal_or_wrong_profile_session() {
        use goat_core::state::{reduce, Intent, WorldState};
        let mut state = WorldState::new();
        state.world_seed = 42;
        state.dated_calendar = true;
        state.career_base_year = 2023;
        state.season_number = 1;
        state = reduce(
            state,
            Intent::EnableDatedCompetitions,
            &mut goat_rng::GoatRng::new(0),
        );
        let mut session =
            crate::session::SimulationSession::capacity_benchmark(200_000, 1).unwrap();
        session.population_deep(&state);
        let bytes = session.resume_checkpoint(&state).unwrap();
        assert!(!crate::session::SimulationSession::new().restore_checkpoint(&state, &bytes));
        assert!(
            !crate::session::SimulationSession::capacity_benchmark(200_000, 6)
                .unwrap()
                .restore_checkpoint(&state, &bytes)
        );
        let mut correct =
            crate::session::SimulationSession::capacity_benchmark(200_000, 1).unwrap();
        assert!(correct.restore_checkpoint(&state, &bytes));
        assert_eq!(correct.population_deep(&state).len(), 200_000);
    }
    #[test]
    fn default_profile_preserves_world_and_invalid_targets_are_rejected() {
        assert_eq!(world(0, 6, 42).unwrap(), WorldGenesis::generate(42));
        for (n, b) in [(1, 6), (400_000, 6), (200_000, 0), (200_000, 13)] {
            assert!(world(n, b, 42).is_err());
        }
    }
}

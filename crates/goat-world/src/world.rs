//! World structure: 50 nations, 3–4 divisions each, 16 clubs per division.
//!
//! The world's STRUCTURE (nations, division layout, club ids) is fixed — it
//! derives from the static `NATIONS` table, so club/division counts are
//! compile-time constants again (asserted against `NATIONS` in tests). The
//! world's CONTENT (club names, strengths) is generated from `world_seed` —
//! see `worldgen.rs`.

use crate::nations::NATIONS;
use goat_fixed::Fixed;
use std::sync::LazyLock;

/// Index into a `GeneratedWorld`'s club vector. Club ids are structural: the
/// club at (nation, level, rank) has the same id in every world.
pub type ClubId = usize;
/// Index into `NATIONS`.
pub type NationId = u8;

pub const NUM_NATIONS: usize = 50;
pub const CLUBS_PER_DIV: usize = 16;
/// 9 nations × 4 divisions + 41 nations × 3 divisions.
pub const NUM_DIVISIONS: usize = 159;
pub const NUM_CLUBS: usize = NUM_DIVISIONS * CLUBS_PER_DIV; // 2544

/// England's index in `NATIONS` — handy for tests and harnesses.
pub const NATION_ENGLAND: NationId = 0;
/// Brazil's index in `NATIONS`.
pub const NATION_BRAZIL: NationId = 5;

/// Division rosters, built once from `NATIONS` (seed-independent).
static ROSTERS: LazyLock<Vec<[ClubId; CLUBS_PER_DIV]>> = LazyLock::new(|| {
    let mut rosters = Vec::with_capacity(NUM_DIVISIONS);
    let mut next = 0usize;
    for nation in NATIONS {
        for _ in 0..nation.divisions {
            let mut ids = [0usize; CLUBS_PER_DIV];
            for (i, id) in ids.iter_mut().enumerate() {
                *id = next + i;
            }
            rosters.push(ids);
            next += CLUBS_PER_DIV;
        }
    }
    rosters
});

/// Reverse map: club id → division index.
static CLUB_DIV: LazyLock<Vec<u16>> = LazyLock::new(|| {
    let mut map = vec![0u16; NUM_CLUBS];
    for (div, ids) in ROSTERS.iter().enumerate() {
        for &id in ids {
            map[id] = div as u16;
        }
    }
    map
});

/// Club ids of one division, in rank order (roster[0] = strongest slot).
pub fn div_clubs(div_idx: usize) -> &'static [ClubId; CLUBS_PER_DIV] {
    &ROSTERS[div_idx]
}

/// Global division index of (nation, level) — divisions are laid out
/// nation-major in `NATIONS` order.
pub fn div_index(nation: NationId, level: u8) -> usize {
    let before: usize = NATIONS[..nation as usize]
        .iter()
        .map(|n| n.divisions as usize)
        .sum();
    before + level as usize
}

/// Which division a club belongs to (O(1)).
pub fn club_division(club_id: ClubId) -> usize {
    CLUB_DIV[club_id] as usize
}

/// Index of a club within its division (0-based).
pub fn club_div_pos(club_id: ClubId) -> usize {
    div_clubs(club_division(club_id))
        .iter()
        .position(|&c| c == club_id)
        .unwrap()
}

/// Display name of a nation.
pub fn nation_name(id: NationId) -> &'static str {
    NATIONS[id as usize].name
}

/// Facilities development multiplier from club strength (stronger clubs invest
/// more in youth). Same formula the legacy `Club::facilities_mult` used.
pub fn facilities_mult(strength: u8) -> Fixed {
    let pct = 700 + (strength as i32 - 50) * 10;
    Fixed::raw(pct.clamp(500, 1_300))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_consts_match_nations_table() {
        assert_eq!(NUM_NATIONS, NATIONS.len());
        let divs: usize = NATIONS.iter().map(|n| n.divisions as usize).sum();
        assert_eq!(NUM_DIVISIONS, divs);
        assert_eq!(NUM_CLUBS, divs * CLUBS_PER_DIV);
    }

    #[test]
    fn rosters_cover_every_club_once() {
        let mut seen = vec![false; NUM_CLUBS];
        for div in 0..NUM_DIVISIONS {
            for &id in div_clubs(div) {
                assert!(!seen[id], "club {id} in two divisions");
                seen[id] = true;
                assert_eq!(club_division(id), div);
            }
        }
        assert!(seen.into_iter().all(|s| s));
    }

    #[test]
    fn div_index_round_trips() {
        for (n_idx, nation) in NATIONS.iter().enumerate() {
            let mut base = 0;
            for prev in &NATIONS[..n_idx] {
                base += prev.divisions as usize;
            }
            for level in 0..nation.divisions {
                assert_eq!(div_index(n_idx as NationId, level), base + level as usize);
            }
        }
    }
}

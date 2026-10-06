//! Lossless fixture dictionary; row order, zero-minute DNPs and overrides survive.

use crate::history::NpcMatchLoad;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeError(pub &'static str);
fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn take<const N: usize>(bytes: &[u8], cur: &mut usize) -> Result<[u8; N], DecodeError> {
    let end = cur
        .checked_add(N)
        .ok_or(DecodeError("journal offset overflow"))?;
    let data = bytes
        .get(*cur..end)
        .ok_or(DecodeError("truncated journal"))?;
    *cur = end;
    Ok(data.try_into().unwrap())
}
fn read_u8(bytes: &[u8], cur: &mut usize) -> Result<u8, DecodeError> {
    Ok(take::<1>(bytes, cur)?[0])
}
fn read_u32(bytes: &[u8], cur: &mut usize) -> Result<u32, DecodeError> {
    Ok(u32::from_le_bytes(take(bytes, cur)?))
}
fn read_u64(bytes: &[u8], cur: &mut usize) -> Result<u64, DecodeError> {
    Ok(u64::from_le_bytes(take(bytes, cur)?))
}

fn varint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 128 {
        out.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    out.push(value as u8);
}
fn read_varint(bytes: &[u8], cur: &mut usize) -> Result<u32, DecodeError> {
    let mut value = 0;
    for shift in (0..=28).step_by(7) {
        let byte = read_u8(bytes, cur)?;
        if shift == 28 && byte > 15 {
            return Err(DecodeError("NPC workload integer overflow"));
        }
        value |= ((byte & 127) as u32) << shift;
        if byte < 128 {
            if shift > 0 && byte == 0 {
                return Err(DecodeError("noncanonical NPC workload integer"));
            }
            return Ok(value);
        }
    }
    Err(DecodeError("invalid NPC workload integer"))
}

fn write_player(out: &mut Vec<u8>, current: u32, previous: &mut u32) {
    let delta = current.wrapping_sub(*previous) as i32;
    varint(out, ((delta as u32) << 1) ^ ((delta >> 31) as u32));
    *previous = current;
}
fn read_player(bytes: &[u8], cur: &mut usize, previous: &mut u32) -> Result<u32, DecodeError> {
    let delta = read_varint(bytes, cur)?;
    let signed = (delta >> 1) as i32 ^ -((delta & 1) as i32);
    *previous = previous.wrapping_add(signed as u32);
    Ok(*previous)
}

pub fn write_credits(out: &mut Vec<u8>, credits: &[crate::state::NpcMatchCredit]) {
    push_u32(out, credits.len() as u32);
    let mut previous = 0;
    for credit in credits {
        write_player(out, credit.pop_idx, &mut previous);
        out.extend_from_slice(&[credit.goals, credit.assists, credit.result as u8]);
    }
}
pub fn read_credits(
    bytes: &[u8],
    cur: &mut usize,
    count: u32,
) -> Result<Vec<crate::state::NpcMatchCredit>, DecodeError> {
    let count = count as usize;
    if count > bytes.len().saturating_sub(*cur) / 4 {
        return Err(DecodeError("truncated compact NPC credits"));
    }
    let mut previous = 0;
    let mut credits = Vec::with_capacity(count);
    for _ in 0..count {
        credits.push(crate::state::NpcMatchCredit {
            pop_idx: read_player(bytes, cur, &mut previous)?,
            goals: read_u8(bytes, cur)?,
            assists: read_u8(bytes, cur)?,
            result: read_u8(bytes, cur)? as i8,
        });
    }
    Ok(credits)
}

pub fn write(out: &mut Vec<u8>, loads: &[NpcMatchLoad]) {
    write_iter(out, loads.iter());
}
pub fn write_iter<'a>(out: &mut Vec<u8>, loads: impl Iterator<Item = &'a NpcMatchLoad> + Clone) {
    let mut index = BTreeMap::new();
    let mut fixtures = Vec::new();
    let mut count = 0u32;
    for load in loads.clone() {
        count += 1;
        let key = (load.competition_id, load.fixture_id, load.epoch_day);
        index.entry(key).or_insert_with(|| {
            let id = fixtures.len() as u32;
            fixtures.push(key);
            id
        });
    }
    push_u32(out, fixtures.len() as u32);
    push_u32(out, count);
    for (competition, fixture, day) in fixtures {
        push_u32(out, competition);
        push_u64(out, fixture);
        push_u32(out, day);
    }
    let mut previous = 0;
    for load in loads {
        varint(
            out,
            index[&(load.competition_id, load.fixture_id, load.epoch_day)],
        );
        write_player(out, load.pop_idx, &mut previous);
        // Encode the full u16 input; the reader still validates the minute limit.
        varint(out, load.minutes as u32);
    }
}

fn visit(
    bytes: &[u8],
    cur: &mut usize,
    mut emit: impl FnMut(NpcMatchLoad, usize),
) -> Result<(), DecodeError> {
    let fixture_count = read_u32(bytes, cur)? as usize;
    let row_count = read_u32(bytes, cur)? as usize;
    if fixture_count > bytes.len().saturating_sub(*cur) / 16 || fixture_count > row_count {
        return Err(DecodeError("truncated NPC fixture dictionary"));
    }
    let mut fixtures = Vec::with_capacity(fixture_count);
    for _ in 0..fixture_count {
        let competition = read_u32(bytes, cur)?;
        let fixture = read_u64(bytes, cur)?;
        let day = read_u32(bytes, cur)?;
        if competition == 0 {
            return Err(DecodeError("invalid NPC competition"));
        }
        fixtures.push((competition, fixture, day));
    }
    if row_count > bytes.len().saturating_sub(*cur) / 3 {
        return Err(DecodeError("truncated compact NPC workload"));
    }
    let mut previous = 0;
    for _ in 0..row_count {
        let index = read_varint(bytes, cur)? as usize;
        let pop_idx = read_player(bytes, cur, &mut previous)?;
        let minutes = read_varint(bytes, cur)?;
        let &(competition_id, fixture_id, epoch_day) = fixtures
            .get(index)
            .ok_or(DecodeError("invalid NPC fixture reference"))?;
        if minutes > 120 {
            return Err(DecodeError("invalid NPC minutes"));
        }
        emit(
            NpcMatchLoad {
                competition_id,
                fixture_id,
                epoch_day,
                pop_idx,
                minutes: minutes as u16,
            },
            row_count,
        );
    }
    Ok(())
}

pub fn read(bytes: &[u8], cur: &mut usize) -> Result<Vec<NpcMatchLoad>, DecodeError> {
    let mut loads = Vec::new();
    visit(bytes, cur, |load, count| {
        if loads.capacity() == 0 {
            loads.reserve_exact(count);
        }
        loads.push(load);
    })?;
    Ok(loads)
}
pub fn matches_loads<'a>(
    bytes: &[u8],
    mut expected: impl Iterator<Item = &'a NpcMatchLoad>,
) -> bool {
    if bytes.is_empty() {
        return expected.next().is_none();
    }
    let mut cur = 0;
    let mut equal = true;
    let valid = visit(bytes, &mut cur, |load, _| {
        equal &= expected.next() == Some(&load);
    });
    valid.is_ok() && equal && cur == bytes.len() && expected.next().is_none()
}
pub fn pack_records<'a>(
    records: impl Iterator<Item = &'a crate::state::OrbitMatchRecord> + Clone,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, records.clone().count() as u32);
    for record in records {
        push_u32(&mut bytes, record.season);
        push_u32(&mut bytes, record.round);
        bytes.push(record.div);
        write_credits(&mut bytes, &record.credits);
    }
    bytes
}
pub fn matches_records<'a>(
    bytes: &[u8],
    mut records: impl Iterator<Item = &'a crate::state::OrbitMatchRecord>,
) -> bool {
    if bytes.is_empty() {
        return records.next().is_none();
    }
    let mut cur = 0;
    let check = || -> Result<bool, DecodeError> {
        let count = read_u32(bytes, &mut cur)?;
        if count as usize > bytes.len().saturating_sub(cur) / 13 {
            return Ok(false);
        }
        for _ in 0..count {
            let season = read_u32(bytes, &mut cur)?;
            let round = read_u32(bytes, &mut cur)?;
            let div = read_u8(bytes, &mut cur)?;
            let count = read_u32(bytes, &mut cur)?;
            let credits = read_credits(bytes, &mut cur, count)?;
            let Some(record) = records.next() else {
                return Ok(false);
            };
            if (season, round, div) != (record.season, record.round, record.div)
                || credits != record.credits
            {
                return Ok(false);
            }
        }
        Ok(cur == bytes.len() && records.next().is_none())
    };
    let mut check = check;
    check().unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credit_deltas_preserve_nonmonotonic_indices_results_and_full_u32_range() {
        let credits: Vec<_> = [0, u32::MAX, 128, 0x8000_0000, 7]
            .into_iter()
            .enumerate()
            .map(|(i, pop_idx)| crate::state::NpcMatchCredit {
                pop_idx,
                goals: i as u8,
                assists: 255,
                result: i as i8 % 3 - 1,
            })
            .collect();
        let mut bytes = Vec::new();
        write_credits(&mut bytes, &credits);
        let mut cur = 0;
        let count = read_u32(&bytes, &mut cur).unwrap();
        assert_eq!(read_credits(&bytes, &mut cur, count).unwrap(), credits);
        assert_eq!(cur, bytes.len());
        assert!(read_credits(&bytes, &mut 4, u32::MAX).is_err());
        for end in 4..bytes.len() {
            assert!(read_credits(&bytes[..end], &mut 4, count).is_err());
        }
    }

    #[test]
    fn retains_dnp_order_large_ids_multiple_competitions_and_reschedules() {
        let loads = [
            NpcMatchLoad {
                competition_id: 1,
                fixture_id: u64::MAX,
                epoch_day: 300,
                pop_idx: u32::MAX,
                minutes: 0,
            },
            NpcMatchLoad {
                competition_id: 3,
                fixture_id: 2,
                epoch_day: 301,
                pop_idx: 128,
                minutes: 120,
            },
            NpcMatchLoad {
                competition_id: 1,
                fixture_id: u64::MAX,
                epoch_day: 300,
                pop_idx: 7,
                minutes: 90,
            },
            NpcMatchLoad {
                competition_id: 1,
                fixture_id: u64::MAX,
                epoch_day: 302,
                pop_idx: 8,
                minutes: 30,
            },
        ];
        let mut bytes = Vec::new();
        write(&mut bytes, &loads);
        let mut cur = 0;
        assert_eq!(read(&bytes, &mut cur).unwrap(), loads);
        assert_eq!(cur, bytes.len());
        for end in 0..bytes.len() {
            assert!(read(&bytes[..end], &mut 0).is_err());
        }
    }
    #[test]
    fn rejects_bad_references_counts_minutes_and_overflow() {
        let load = NpcMatchLoad {
            competition_id: 1,
            fixture_id: 7,
            epoch_day: 1,
            pop_idx: 2,
            minutes: 90,
        };
        let mut bytes = Vec::new();
        write(&mut bytes, &[load]);
        bytes[24] = 1;
        assert!(read(&bytes, &mut 0).is_err());
        bytes[24] = 0;
        bytes[26] = 121;
        assert!(read(&bytes, &mut 0).is_err());
        assert!(read_varint(&[255, 255, 255, 255, 16], &mut 0).is_err());
        assert!(read_varint(&[128, 0], &mut 0).is_err());
        bytes[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read(&bytes, &mut 0).is_err());
    }
}

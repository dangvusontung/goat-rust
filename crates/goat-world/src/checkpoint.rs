//! Optional local resume cache. Journals stay canonical; no simulation rules change.
use goat_fixed::Fixed;
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, VecDeque},
};

/// Cache format, independent of the save container and frozen football model.
pub const FORMAT: u32 = 3;
/// Football/medical behavior version captured by this derived cache.
pub const MODEL: u32 = 15;
/// Reject oversized/unbounded optional cache payloads before allocating.
pub const MAX_BYTES: usize = 256 * 1024 * 1024;
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pub(crate) pos: usize,
    allocation_budget: usize,
}
impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            allocation_budget: MAX_BYTES * 4,
        }
    }
    pub(crate) fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let part = self.bytes.get(self.pos..end)?;
        self.pos = end;
        Some(part)
    }
    // Account for collection storage, including conservative Vec growth overhead.
    fn charge(&mut self, count: usize, bytes_per_item: usize) -> Option<()> {
        let bytes = count.checked_mul(bytes_per_item)?;
        self.allocation_budget = self.allocation_budget.checked_sub(bytes)?;
        Some(())
    }
    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }
}
pub(crate) trait Snapshot: Sized {
    const MIN: usize;
    fn write(&self, out: &mut Vec<u8>);
    fn read(r: &mut Reader<'_>) -> Option<Self>;
}
macro_rules! fields {
    ($ty:ty { $($field:ident),+ $(,)? }) => {
        impl crate::checkpoint::Snapshot for $ty {
            const MIN: usize=1;
            fn write(&self, out: &mut Vec<u8>) {$(crate::checkpoint::Snapshot::write(&self.$field,out);)+}
            fn read(r: &mut crate::checkpoint::Reader<'_>) -> Option<Self> {
                Some(Self { $($field:crate::checkpoint::Snapshot::read(r)?,)+ })
            }
        }
    };
}
pub(crate) use fields;
macro_rules! number {
    ($($ty:ty),+) => {$(impl Snapshot for $ty {
        const MIN: usize=std::mem::size_of::<Self>();
        fn write(&self,out:&mut Vec<u8>){out.extend_from_slice(&self.to_le_bytes());}
        fn read(r:&mut Reader<'_>)->Option<Self>{Some(Self::from_le_bytes(r.take(<Self as Snapshot>::MIN)?.try_into().ok()?))}
    })+};
}
number!(u8, u16, u32, u64, i16, i32, i64);
impl Snapshot for usize {
    const MIN: usize = 4;
    fn write(&self, out: &mut Vec<u8>) {
        u32::try_from(*self)
            .expect("checkpoint index exceeds u32")
            .write(out);
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(u32::read(r)? as usize)
    }
}
impl Snapshot for bool {
    const MIN: usize = 1;
    fn write(&self, out: &mut Vec<u8>) {
        u8::from(*self).write(out);
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        match u8::read(r)? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
}
impl Snapshot for Fixed {
    const MIN: usize = 4;
    fn write(&self, out: &mut Vec<u8>) {
        self.to_raw().write(out);
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Self::raw(i32::read(r)?))
    }
}
impl<T: Snapshot> Snapshot for Vec<T> {
    const MIN: usize = 4;
    fn write(&self, out: &mut Vec<u8>) {
        self.len().write(out);
        for v in self {
            v.write(out);
        }
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        let n = usize::read(r)?;
        if n > r.remaining() / T::MIN.max(1) {
            return None;
        }
        // Grow only after decoding each item: a forged count cannot reserve huge objects.
        r.charge(n, std::mem::size_of::<T>().checked_mul(2)?)?;
        let mut values = Vec::new();
        for _ in 0..n {
            values.push(T::read(r)?);
        }
        Some(values)
    }
}
impl<T: Snapshot> Snapshot for Option<T> {
    const MIN: usize = 1;
    fn write(&self, out: &mut Vec<u8>) {
        self.is_some().write(out);
        if let Some(v) = self {
            v.write(out);
        }
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        if bool::read(r)? {
            Some(Some(T::read(r)?))
        } else {
            Some(None)
        }
    }
}
impl<T: Snapshot, const N: usize> Snapshot for [T; N] {
    const MIN: usize = T::MIN * N;
    fn write(&self, out: &mut Vec<u8>) {
        for v in self {
            v.write(out);
        }
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        let mut v = Vec::new();
        for _ in 0..N {
            v.push(T::read(r)?);
        }
        v.try_into().ok()
    }
}
macro_rules! tuple {
    ($($ty:ident:$idx:tt),+) => {impl<$($ty:Snapshot),+> Snapshot for ($($ty,)+) {
        const MIN:usize=0 $(+$ty::MIN)+;
        fn write(&self,out:&mut Vec<u8>){$(self.$idx.write(out);)+}
        fn read(r:&mut Reader<'_>)->Option<Self>{Some(($($ty::read(r)?,)+))}
    }};
}
tuple!(A:0,B:1);
tuple!(A:0,B:1,C:2);
tuple!(A:0,B:1,C:2,D:3);
tuple!(A:0,B:1,C:2,D:3,E:4,F:5);
impl<T: Snapshot> Snapshot for RefCell<T> {
    const MIN: usize = T::MIN;
    fn write(&self, out: &mut Vec<u8>) {
        self.borrow().write(out);
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Self::new(T::read(r)?))
    }
}
impl<T: Snapshot + Copy> Snapshot for Cell<T> {
    const MIN: usize = T::MIN;
    fn write(&self, out: &mut Vec<u8>) {
        self.get().write(out);
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Self::new(T::read(r)?))
    }
}
impl<K: Snapshot + Ord, V: Snapshot> Snapshot for BTreeMap<K, V> {
    const MIN: usize = 4;
    fn write(&self, out: &mut Vec<u8>) {
        self.len().write(out);
        for (k, v) in self {
            k.write(out);
            v.write(out);
        }
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        let n = usize::read(r)?;
        if n > r.remaining() / (K::MIN + V::MIN).max(1) {
            return None;
        }
        r.charge(n, std::mem::size_of::<(K, V)>().checked_add(64)?)?;
        let mut out = Self::new();
        for _ in 0..n {
            let k = K::read(r)?;
            if out.last_key_value().is_some_and(|(last, _)| k <= *last) {
                return None;
            }
            out.insert(k, V::read(r)?);
        }
        Some(out)
    }
}
impl<T: Snapshot> Snapshot for VecDeque<T> {
    const MIN: usize = 4;
    fn write(&self, out: &mut Vec<u8>) {
        self.len().write(out);
        for v in self {
            v.write(out);
        }
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Vec::<T>::read(r)?.into())
    }
}
impl Snapshot for String {
    const MIN: usize = 4;
    fn write(&self, out: &mut Vec<u8>) {
        self.len().write(out);
        out.extend_from_slice(self.as_bytes());
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        let n = usize::read(r)?;
        r.charge(n, 2)?;
        String::from_utf8(r.take(n)?.to_vec()).ok()
    }
}
impl Snapshot for goat_core::chronology::Chronology {
    const MIN: usize = 4;
    fn write(&self, out: &mut Vec<u8>) {
        self.base_year.write(out);
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        let year = u32::read(r)?;
        (1900..=9999).contains(&year).then(|| Self::new(year))
    }
}
fields!(goat_core::tactical_identity::TacticalIdentity { role_weight });
fields!(goat_core::deep::DeepFixtureResult {
    season,
    round,
    league,
    epoch_day,
    home,
    away,
    home_goals,
    away_goals
});
fields!(goat_core::deep::DeepScope {
    season,
    epoch_day,
    pc_league,
    pc_club,
    leagues
});
fields!(goat_core::discipline::NpcCardEvent {
    season,
    competition_id,
    pop_idx,
    fixture_id,
    epoch_day,
    minute,
    kind
});
impl Snapshot for goat_core::history::NpcMatchLoad {
    const MIN: usize = 22;
    fn write(&self, out: &mut Vec<u8>) {
        self.competition_id.write(out);
        self.pop_idx.write(out);
        self.fixture_id.write(out);
        self.epoch_day.write(out);
        self.minutes.write(out);
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Self {
            competition_id: u32::read(r)?,
            pop_idx: u32::read(r)?,
            fixture_id: u64::read(r)?,
            epoch_day: u32::read(r)?,
            minutes: u16::read(r)?,
        })
    }
}
impl Snapshot for goat_core::state::OrbitMatchRecord {
    const MIN: usize = 13;
    fn write(&self, out: &mut Vec<u8>) {
        self.season.write(out);
        self.round.write(out);
        self.div.write(out);
        goat_core::journal::write_credits(out, &self.credits);
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        let season = u32::read(r)?;
        let round = u32::read(r)?;
        let div = u8::read(r)?;
        let count = u32::read(r)?;
        let credits = goat_core::journal::read_credits(r.bytes, &mut r.pos, count).ok()?;
        Some(Self {
            season,
            round,
            div,
            credits,
        })
    }
}
impl Snapshot for crate::transfers::TransferLane {
    const MIN: usize = 1;
    fn write(&self, out: &mut Vec<u8>) {
        (*self as u8).write(out);
    }
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        match u8::read(r)? {
            0 => Some(Self::WeakestPosition),
            1 => Some(Self::GemHunt),
            _ => None,
        }
    }
}
/// Accidental corruption detection only; exact journal binding is checked separately.
pub(crate) fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_lengths_tags_maps_and_utf8_are_rejected() {
        let forged = u32::MAX.to_le_bytes();
        assert!(Vec::<Option<[Fixed; 32]>>::read(&mut Reader::new(&forged)).is_none());
        assert!(bool::read(&mut Reader::new(&[2])).is_none());
        assert!(String::read(&mut Reader::new(&[1, 0, 0, 0, 255])).is_none());
        // Two identical keys are invalid even when the framing is complete.
        assert!(BTreeMap::<u8, u8>::read(&mut Reader::new(&[2, 0, 0, 0, 1, 3, 1, 4])).is_none());
        let mut reader = Reader::new(&[0]);
        assert!(reader.take(usize::MAX).is_none());
        assert_eq!(reader.pos, 0);
        assert!(reader.charge(usize::MAX, 2).is_none());
        assert!(reader.charge(MAX_BYTES * 4 + 1, 1).is_none());
        // A compact sequence of None tags can otherwise expand far beyond file size.
        let count = 1_048_576u32;
        let mut compact = count.to_le_bytes().to_vec();
        compact.resize(count as usize + 4, 0);
        let mut reader = Reader::new(&compact);
        assert!(Vec::<Option<[Fixed; 256]>>::read(&mut reader).is_none());
        assert_eq!(reader.pos, 4);
    }

    #[test]
    fn cache_primitives_have_stable_encoding_and_corruption_checksum() {
        let mut bytes = Vec::new();
        (0x1234u16, -2i32, Some(true)).write(&mut bytes);
        assert_eq!(bytes, [0x34, 0x12, 0xfe, 0xff, 0xff, 0xff, 1, 1]);
        assert_eq!(checksum(b"hello"), 0xa430_d846_80aa_bd0b);
    }
}

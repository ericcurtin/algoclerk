// SPDX-License-Identifier: Apache-2.0
//! Deterministic test-case generation for levels.
//!
//! Generators are described in level files (see `docs/LEVEL_FORMAT.md`) and driven by a small
//! built-in PRNG, so a given seed produces the same test cases on every platform and version.

use serde::{Deserialize, Serialize};

/// SplitMix64 pseudo random generator (stable, tiny, no dependencies).
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// Create from a seed.
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    /// Next raw value.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (`n == 0` gives 0).
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }

    /// Uniform in `lo..=hi` (swapped bounds are tolerated).
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        let span = (i64::from(hi) - i64::from(lo) + 1) as u64;
        (i64::from(lo) + self.below(span) as i64) as i32
    }

    /// True with the given percent probability.
    pub fn chance(&mut self, percent: u32) -> bool {
        self.below(100) < u64::from(percent)
    }

    /// Fisher-Yates shuffle.
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i as u64 + 1) as usize;
            v.swap(i, j);
        }
    }
}

/// Kinds of generated segments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SegKind {
    /// Random list of integers.
    #[default]
    List,
    /// One random integer.
    Int,
    /// Fixed values.
    Const,
    /// An element picked from an earlier segment (or random, with some probability).
    Pick,
    /// Shuffled consecutive integers starting at `range[0]`, optionally dropping some.
    Perm,
    /// A sequence of `+1` / `-1` values, balanced with probability `chance`.
    Balanced,
    /// A list that is a palindrome with probability `chance` (otherwise a near miss).
    Palindrome,
    /// A linked list laid out in floor slots: see `docs/LEVEL_FORMAT.md`.
    Linked,
    /// A shuffled list in which one value occurs more than half of the time.
    Majority,
    /// The auxiliary value of an earlier segment (for example the head of a linked list).
    Aux,
}

/// Sort order of a list segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    /// Non-decreasing.
    Asc,
    /// Non-increasing.
    Desc,
}

fn default_len() -> [u32; 2] {
    [1, 8]
}
fn default_range() -> [i32; 2] {
    [-9, 9]
}
fn default_chance() -> u32 {
    50
}

/// Description of one generated segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Seg {
    /// What to generate.
    #[serde(default)]
    pub kind: SegKind,
    /// Inclusive length range.
    #[serde(default = "default_len")]
    pub len: [u32; 2],
    /// Inclusive value range.
    #[serde(default = "default_range")]
    pub range: [i32; 2],
    /// Round the length down to a multiple of this (0 or 1 means no rounding).
    #[serde(default)]
    pub len_multiple: u32,
    /// Sort the values.
    #[serde(default)]
    pub sort: Option<SortOrder>,
    /// No repeated values.
    #[serde(default)]
    pub distinct: bool,
    /// No zero values.
    #[serde(default)]
    pub nonzero: bool,
    /// Fixed values for `const`.
    #[serde(default)]
    pub values: Vec<i32>,
    /// Emit the length before the values.
    #[serde(default)]
    pub prefix_len: bool,
    /// Value appended after the list.
    #[serde(default)]
    pub terminator: Option<i32>,
    /// Index of an earlier segment (inbox segments first, then floor segments) for `pick` / `aux`.
    #[serde(default)]
    pub of: usize,
    /// Probability in percent for `pick`, `balanced`, `palindrome`.
    #[serde(default = "default_chance")]
    pub chance: u32,
    /// Number of elements to drop from a `perm`.
    #[serde(default)]
    pub drop: u32,
    /// Slots per node for `linked` (value slot then next slot; default 2).
    #[serde(default)]
    pub stride: u32,
}

impl Default for Seg {
    fn default() -> Self {
        Seg {
            kind: SegKind::List,
            len: default_len(),
            range: default_range(),
            len_multiple: 0,
            sort: None,
            distinct: false,
            nonzero: false,
            values: Vec::new(),
            prefix_len: false,
            terminator: None,
            of: 0,
            chance: default_chance(),
            drop: 0,
            stride: 0,
        }
    }
}

/// Floor placement of a generated segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorSeg {
    /// First slot to fill.
    pub at: usize,
    /// What to put there.
    pub seg: Seg,
}

/// A family of generated test cases.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generator {
    /// How many cases to make.
    pub count: u32,
    /// Inbox segments, concatenated.
    #[serde(default)]
    pub inbox: Vec<Seg>,
    /// Floor segments.
    #[serde(default)]
    pub floor: Vec<FloorSeg>,
}

/// One generated input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedInput {
    /// Inbox values.
    pub inbox: Vec<i32>,
    /// `(first slot, values)` floor fills.
    pub floor: Vec<(usize, Vec<i32>)>,
}

struct SegOut {
    raw: Vec<i32>,
    emitted: Vec<i32>,
    aux: Option<i32>,
}

fn pick_len_multiple(rng: &mut Rng, len: [u32; 2], multiple: u32) -> usize {
    let (lo, hi) = if len[0] <= len[1] {
        (len[0], len[1])
    } else {
        (len[1], len[0])
    };
    let n = lo + rng.below(u64::from(hi - lo) + 1) as u32;
    let n = if multiple > 1 { n - n % multiple } else { n };
    n as usize
}

fn pick_len(rng: &mut Rng, len: [u32; 2]) -> usize {
    pick_len_multiple(rng, len, 0)
}

fn random_list(rng: &mut Rng, seg: &Seg, len: usize) -> Vec<i32> {
    let [lo, hi] = seg.range;
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    if seg.distinct {
        let mut pool: Vec<i32> = (lo..=hi).filter(|v| !(seg.nonzero && *v == 0)).collect();
        rng.shuffle(&mut pool);
        pool.truncate(len);
        return pool;
    }
    let mut v = Vec::with_capacity(len);
    while v.len() < len {
        let x = rng.range(lo, hi);
        if seg.nonzero && x == 0 && (lo != 0 || hi != 0) {
            continue;
        }
        v.push(x);
    }
    v
}

fn finish(seg: &Seg, mut raw: Vec<i32>, aux: Option<i32>) -> SegOut {
    match seg.sort {
        Some(SortOrder::Asc) => raw.sort_unstable(),
        Some(SortOrder::Desc) => {
            raw.sort_unstable();
            raw.reverse();
        }
        None => {}
    }
    let mut emitted = Vec::with_capacity(raw.len() + 2);
    if seg.prefix_len {
        emitted.push(raw.len() as i32);
    }
    emitted.extend_from_slice(&raw);
    if let Some(t) = seg.terminator {
        emitted.push(t);
    }
    SegOut { raw, emitted, aux }
}

fn gen_seg(rng: &mut Rng, seg: &Seg, pool: &[SegOut]) -> Result<SegOut, String> {
    match seg.kind {
        SegKind::List => {
            let n = pick_len_multiple(rng, seg.len, seg.len_multiple);
            let raw = random_list(rng, seg, n);
            Ok(finish(seg, raw, None))
        }
        SegKind::Int => {
            let raw = random_list(rng, seg, 1);
            Ok(finish(seg, raw, None))
        }
        SegKind::Const => Ok(finish(seg, seg.values.clone(), None)),
        SegKind::Pick => {
            let src = pool
                .get(seg.of)
                .ok_or_else(|| format!("pick refers to segment {} which does not exist yet", seg.of))?;
            let v = if !src.raw.is_empty() && rng.chance(seg.chance) {
                src.raw[rng.below(src.raw.len() as u64) as usize]
            } else {
                rng.range(seg.range[0], seg.range[1])
            };
            Ok(finish(seg, vec![v], None))
        }
        SegKind::Perm => {
            let n = pick_len(rng, seg.len);
            let mut v: Vec<i32> = (0..n as i32).map(|i| seg.range[0] + i).collect();
            rng.shuffle(&mut v);
            let drop = (seg.drop as usize).min(v.len());
            for _ in 0..drop {
                let i = rng.below(v.len() as u64) as usize;
                v.remove(i);
            }
            Ok(finish(seg, v, None))
        }
        SegKind::Balanced => {
            let pairs = (pick_len(rng, seg.len) / 2).max(1);
            let mut seq = Vec::with_capacity(pairs * 2);
            if rng.chance(seg.chance) {
                let (mut opens, mut depth) = (pairs, 0usize);
                let mut closes = pairs;
                while opens + closes > 0 {
                    let open = opens > 0 && (depth == 0 || rng.chance(50));
                    if open {
                        seq.push(1);
                        opens -= 1;
                        depth += 1;
                    } else {
                        seq.push(-1);
                        closes -= 1;
                        depth -= 1;
                    }
                }
            } else {
                for _ in 0..pairs * 2 {
                    seq.push(if rng.chance(50) { 1 } else { -1 });
                }
                if rng.chance(30) {
                    seq.pop();
                }
            }
            Ok(finish(seg, seq, None))
        }
        SegKind::Palindrome => {
            let n = pick_len(rng, seg.len);
            let half = random_list(rng, seg, n.div_ceil(2));
            let mut v = half.clone();
            let rest = n / 2;
            for i in (0..rest).rev() {
                v.push(half[i]);
            }
            if !rng.chance(seg.chance) && v.len() > 1 {
                let i = rng.below(v.len() as u64) as usize;
                v[i] = rng.range(seg.range[0], seg.range[1]);
            }
            Ok(finish(seg, v, None))
        }
        SegKind::Majority => {
            let n = pick_len_multiple(rng, seg.len, seg.len_multiple).max(1);
            let major = random_list(rng, &Seg { distinct: false, ..seg.clone() }, 1)[0];
            let k = n / 2 + 1;
            let mut v = vec![major; k];
            v.extend(random_list(rng, &Seg { distinct: false, ..seg.clone() }, n - k));
            rng.shuffle(&mut v);
            Ok(finish(seg, v, Some(major)))
        }
        SegKind::Linked => {
            let stride = if seg.stride == 0 { 2 } else { seg.stride as usize };
            let nodes = pick_len(rng, seg.len).max(1);
            // Node order along the list, and where each node lives.
            let mut places: Vec<usize> = (0..nodes).collect();
            rng.shuffle(&mut places);
            let mut slots = vec![0i32; nodes * stride];
            for (i, &place) in places.iter().enumerate() {
                let next = if i + 1 < nodes {
                    (places[i + 1] * stride) as i32
                } else {
                    -1
                };
                slots[place * stride] = rng.range(seg.range[0], seg.range[1]);
                slots[place * stride + 1] = next;
            }
            let head = (places[0] * stride) as i32;
            Ok(SegOut {
                raw: slots.clone(),
                emitted: slots,
                aux: Some(head),
            })
        }
        SegKind::Aux => {
            let src = pool
                .get(seg.of)
                .ok_or_else(|| format!("aux refers to segment {} which does not exist yet", seg.of))?;
            let a = src
                .aux
                .ok_or_else(|| format!("segment {} has no auxiliary value", seg.of))?;
            Ok(finish(seg, vec![a], None))
        }
    }
}

/// Generate the `index`-th input of a generator (each case advances the shared `rng`).
pub fn generate_one(rng: &mut Rng, g: &Generator) -> Result<GeneratedInput, String> {
    let mut pool: Vec<SegOut> = Vec::new();
    let mut inbox = Vec::new();
    for seg in &g.inbox {
        let out = gen_seg(rng, seg, &pool)?;
        inbox.extend_from_slice(&out.emitted);
        pool.push(out);
    }
    let mut floor = Vec::new();
    for fs in &g.floor {
        let out = gen_seg(rng, &fs.seg, &pool)?;
        floor.push((fs.at, out.emitted.clone()));
        pool.push(out);
    }
    Ok(GeneratedInput { inbox, floor })
}

/// Generate all inputs of a generator with a seed.
pub fn generate_all(seed: u64, g: &Generator) -> Result<Vec<GeneratedInput>, String> {
    let mut rng = Rng::new(seed);
    (0..g.count).map(|_| generate_one(&mut rng, g)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_and_known() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(1);
        assert_eq!(a.next_u64(), b.next_u64());
        // Known SplitMix64 output for seed 1 guards against accidental algorithm changes.
        assert_eq!(Rng::new(1).next_u64(), 0x910a_2dec_8902_5cc1);
    }

    #[test]
    fn range_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            let v = r.range(-3, 4);
            assert!((-3..=4).contains(&v));
        }
        assert_eq!(r.range(5, 5), 5);
        let _ = r.range(i32::MIN, i32::MAX);
    }

    #[test]
    fn list_options() {
        let g = Generator {
            count: 30,
            inbox: vec![Seg {
                len: [3, 6],
                range: [1, 20],
                distinct: true,
                sort: Some(SortOrder::Asc),
                prefix_len: true,
                terminator: Some(0),
                ..Seg::default()
            }],
            floor: vec![],
        };
        for c in generate_all(5, &g).unwrap() {
            let n = c.inbox[0] as usize;
            assert!((3..=6).contains(&n));
            assert_eq!(c.inbox.len(), n + 2);
            assert_eq!(*c.inbox.last().unwrap(), 0);
            let body = &c.inbox[1..=n];
            assert!(body.windows(2).all(|w| w[0] < w[1]));
        }
    }

    #[test]
    fn perm_with_drop() {
        let g = Generator {
            count: 10,
            inbox: vec![Seg {
                kind: SegKind::Perm,
                len: [5, 5],
                range: [1, 1],
                drop: 1,
                ..Seg::default()
            }],
            floor: vec![],
        };
        for c in generate_all(9, &g).unwrap() {
            assert_eq!(c.inbox.len(), 4);
            let mut s = c.inbox.clone();
            s.sort_unstable();
            s.dedup();
            assert_eq!(s.len(), 4);
        }
    }

    #[test]
    fn linked_list_is_a_chain() {
        let g = Generator {
            count: 20,
            inbox: vec![],
            floor: vec![FloorSeg {
                at: 0,
                seg: Seg {
                    kind: SegKind::Linked,
                    len: [1, 6],
                    ..Seg::default()
                },
            }],
        };
        let mut rng = Rng::new(3);
        for _ in 0..g.count {
            let mut one = g.clone();
            one.inbox = vec![];
            let pool_out = generate_one(&mut rng, &one).unwrap();
            let (_, slots) = &pool_out.floor[0];
            // Exactly one node points to -1 and following from some node visits all nodes once.
            let nodes = slots.len() / 2;
            let ends = (0..nodes).filter(|i| slots[i * 2 + 1] == -1).count();
            assert_eq!(ends, 1);
        }
    }

    #[test]
    fn pick_and_aux_need_earlier_segments() {
        let g = Generator {
            count: 1,
            inbox: vec![Seg {
                kind: SegKind::Pick,
                of: 3,
                ..Seg::default()
            }],
            floor: vec![],
        };
        assert!(generate_all(1, &g).is_err());
    }

    #[test]
    fn balanced_shape() {
        let g = Generator {
            count: 40,
            inbox: vec![Seg {
                kind: SegKind::Balanced,
                len: [2, 10],
                chance: 100,
                ..Seg::default()
            }],
            floor: vec![],
        };
        for c in generate_all(11, &g).unwrap() {
            let mut depth = 0i32;
            for v in &c.inbox {
                depth += v;
                assert!(depth >= 0);
            }
            assert_eq!(depth, 0);
        }
    }

    #[test]
    fn palindrome_shape() {
        let g = Generator {
            count: 40,
            inbox: vec![Seg {
                kind: SegKind::Palindrome,
                len: [1, 9],
                chance: 100,
                ..Seg::default()
            }],
            floor: vec![],
        };
        for c in generate_all(12, &g).unwrap() {
            let rev: Vec<i32> = c.inbox.iter().rev().copied().collect();
            assert_eq!(c.inbox, rev);
        }
    }
}

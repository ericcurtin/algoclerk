// SPDX-License-Identifier: Apache-2.0
//! A deterministic choice source driven by a byte vector. proptest generates and shrinks the vector; when
//! the bytes run out every choice is 0, so shrinking (shorter and smaller vectors) gives smaller programs.

#[derive(Clone, Debug)]
pub struct Src {
    bytes: Vec<u8>,
    pos: usize,
}

impl Src {
    pub fn new(bytes: Vec<u8>) -> Src {
        Src { bytes, pos: 0 }
    }

    pub fn byte(&mut self) -> u8 {
        let b = self.bytes.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        b
    }

    /// A number in `0..n` (0 when `n` is 0 or 1).
    pub fn below(&mut self, n: usize) -> usize {
        if n <= 1 {
            return 0;
        }
        if n <= 256 {
            usize::from(self.byte()) % n
        } else {
            (usize::from(self.byte()) << 8 | usize::from(self.byte())) % n
        }
    }

    /// A number in `lo..=hi`.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as usize) as i64
    }

    /// True with probability `num / den`.
    pub fn chance(&mut self, num: usize, den: usize) -> bool {
        self.below(den) < num
    }

    /// Pick an index by weights.
    pub fn weighted(&mut self, weights: &[usize]) -> usize {
        let total: usize = weights.iter().sum();
        let mut x = self.below(total.max(1));
        for (i, w) in weights.iter().enumerate() {
            if x < *w {
                return i;
            }
            x -= *w;
        }
        0
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

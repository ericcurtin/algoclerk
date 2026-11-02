// SPDX-License-Identifier: Apache-2.0
//! Deterministic test generation from the level's generator specs.

use crate::level::GenSpec;
use std::collections::BTreeSet;

/// SplitMix64: tiny, fast and identical on every platform.
#[derive(Clone, Debug)]
pub struct Rng(std::cell::Cell<u64>);

impl Rng {
    /// Seeded generator.
    pub fn new(seed: u64) -> Rng {
        Rng(std::cell::Cell::new(
            seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03,
        ))
    }
    /// Next 64 random bits.
    pub fn next(&self) -> u64 {
        self.0.set(self.0.get().wrapping_add(0x9E37_79B9_7F4A_7C15));
        let mut z = self.0.get();
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in `lo..=hi`.
    pub fn range(&self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        let span = (hi as i128 - lo as i128 + 1) as u128;
        (lo as i128 + (self.next() as u128 % span) as i128) as i64
    }
    /// Shuffle in place.
    pub fn shuffle<T>(&self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.range(0, i as i64) as usize;
            v.swap(i, j);
        }
    }
    /// True with probability `percent`/100.
    pub fn chance(&self, percent: i64) -> bool {
        self.range(0, 99) < percent
    }
}

/// One generated argument: its literal text and the integers inside it (for dependent arguments).
struct Arg {
    text: String,
    ints: Vec<i64>,
}

fn eval_expr(s: &str, n: i64) -> Result<i64, String> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(inner) = s.strip_prefix("sqrt(").and_then(|r| r.strip_suffix(')')) {
        let v = eval_expr(inner, n)?;
        return Ok((v as f64).sqrt().floor() as i64);
    }
    if let Some((a, b)) = s.split_once('*') {
        return Ok(eval_expr(a, n)? * eval_expr(b, n)?);
    }
    if let Some((a, b)) = s.split_once('/') {
        let d = eval_expr(b, n)?;
        if d == 0 {
            return Err("division by zero in size expression".into());
        }
        return Ok(eval_expr(a, n)? / d);
    }
    if let Some((a, b)) = s.split_once('+') {
        return Ok(eval_expr(a, n)? + eval_expr(b, n)?);
    }
    if let Some((a, b)) = s.rsplit_once('-')
        && !a.is_empty()
    {
        return Ok(eval_expr(a, n)? - eval_expr(b, n)?);
    }
    if s == "n" {
        return Ok(n);
    }
    s.parse::<i64>()
        .map_err(|_| format!("bad size expression '{s}'"))
}

fn num(t: &toml::Table, key: &str, default: (i64, i64), n: i64, rng: &Rng) -> Result<i64, String> {
    let (lo, hi) = bounds(t, key, default, n)?;
    Ok(rng.range(lo, hi))
}

fn bounds(t: &toml::Table, key: &str, default: (i64, i64), n: i64) -> Result<(i64, i64), String> {
    match t.get(key) {
        None => Ok(default),
        Some(toml::Value::Integer(i)) => Ok((*i, *i)),
        Some(toml::Value::String(s)) => {
            let v = eval_expr(s, n)?;
            Ok((v, v))
        }
        Some(toml::Value::Array(a)) if a.len() == 2 => {
            let g = |v: &toml::Value| -> Result<i64, String> {
                match v {
                    toml::Value::Integer(i) => Ok(*i),
                    toml::Value::String(s) => eval_expr(s, n),
                    _ => Err(format!("bad bound in {key}")),
                }
            };
            Ok((g(&a[0])?, g(&a[1])?))
        }
        _ => Err(format!("bad value for {key}")),
    }
}

fn flag(t: &toml::Table, key: &str) -> bool {
    t.get(key).and_then(toml::Value::as_bool).unwrap_or(false)
}

fn text_of(t: &toml::Table, key: &str, default: &str) -> String {
    t.get(key)
        .and_then(toml::Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn rand_string(rng: &Rng, len: i64, alphabet: &[u8]) -> String {
    (0..len.max(0))
        .map(|_| alphabet[rng.range(0, alphabet.len() as i64 - 1) as usize] as char)
        .collect()
}

fn words_string(rng: &Rng, count: i64, alphabet: &[u8], wlen: (i64, i64)) -> String {
    (0..count.max(0))
        .map(|_| {
            let l = rng.range(wlen.0, wlen.1).max(1);
            rand_string(rng, l, alphabet)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn gen_arg(t: &toml::Table, n: i64, rng: &Rng, earlier: &[Arg]) -> Result<Arg, String> {
    let kind = text_of(t, "kind", "int");
    let alphabet_s = text_of(t, "alphabet", "abcdefghijklmnopqrstuvwxyz");
    let alphabet = alphabet_s.as_bytes();
    if alphabet.is_empty() {
        return Err("empty alphabet".into());
    }
    let range = bounds(t, "range", (-20, 20), n)?;
    let ints_text = |v: &[i64]| {
        format!(
            "[{}]",
            v.iter().map(i64::to_string).collect::<Vec<_>>().join(", ")
        )
    };
    let dep = |key: &str| -> Result<Option<&Arg>, String> {
        match t.get(key).and_then(toml::Value::as_integer) {
            Some(i) => earlier
                .get(i as usize)
                .map(Some)
                .ok_or_else(|| format!("{key}: no such earlier argument")),
            None => Ok(None),
        }
    };
    match kind.as_str() {
        "int" => {
            let v = if let Some(a) = dep("pick_from")? {
                if a.ints.is_empty() || rng.chance(num(t, "miss", (0, 0), n, rng)?) {
                    rng.range(range.0, range.1)
                } else {
                    a.ints[rng.range(0, a.ints.len() as i64 - 1) as usize]
                }
            } else if let Some(a) = dep("sum_of")? {
                if a.ints.len() < 2 || rng.chance(num(t, "miss", (0, 0), n, rng)?) {
                    rng.range(range.0, range.1)
                } else {
                    let i = rng.range(0, a.ints.len() as i64 - 1) as usize;
                    let mut j = rng.range(0, a.ints.len() as i64 - 2) as usize;
                    if j >= i {
                        j += 1;
                    }
                    a.ints[i] + a.ints[j]
                }
            } else if let Some(a) = dep("index_of")? {
                rng.range(0, a.ints.len().max(1) as i64 - 1)
            } else if let Some(a) = dep("up_to_len_of")? {
                rng.range(0, a.ints.len() as i64)
            } else if let Some(a) = dep("in_len_of")? {
                let hi =
                    num(t, "max", (i64::MAX, i64::MAX), n, rng)?.min(a.ints.len().max(1) as i64);
                rng.range(1, hi)
            } else if let Some(a) = dep("below")? {
                rng.range(0, a.ints.first().copied().unwrap_or(1).max(1) - 1)
            } else {
                rng.range(range.0, range.1)
            };
            Ok(Arg {
                text: v.to_string(),
                ints: vec![v],
            })
        }
        "bool" => {
            let v = rng.chance(50);
            Ok(Arg {
                text: v.to_string(),
                ints: vec![i64::from(v)],
            })
        }
        "char" => {
            let c = alphabet[rng.range(0, alphabet.len() as i64 - 1) as usize] as char;
            Ok(Arg {
                text: format!("'{c}'"),
                ints: vec![c as i64],
            })
        }
        "string" => {
            let s = if let Some(a) = dep("anagram_of")? {
                let mut cs: Vec<char> = a.ints.iter().map(|&b| b as u8 as char).collect();
                rng.shuffle(&mut cs);
                if !cs.is_empty() && rng.chance(num(t, "change_pct", (0, 0), n, rng)?) {
                    let k = rng.range(0, cs.len() as i64 - 1) as usize;
                    cs[k] = alphabet[rng.range(0, alphabet.len() as i64 - 1) as usize] as char;
                }
                cs.into_iter().collect()
            } else if flag(t, "periodic") {
                let mut perm: Vec<u8> = alphabet.to_vec();
                rng.shuffle(&mut perm);
                let len = num(t, "len", (0, 8), n, rng)?;
                (0..len.max(0) as usize)
                    .map(|i| perm[i % perm.len()] as char)
                    .collect()
            } else if let Some(br) = t.get("brackets").and_then(toml::Value::as_str) {
                let pairs: Vec<(char, char)> = br
                    .chars()
                    .collect::<Vec<_>>()
                    .chunks(2)
                    .map(|c| (c[0], c[1]))
                    .collect();
                let len = num(t, "len", (0, 8), n, rng)?;
                let mut out = String::new();
                let mut stack: Vec<char> = Vec::new();
                if flag(t, "nested") {
                    for _ in 0..len / 2 {
                        let p = pairs[rng.range(0, pairs.len() as i64 - 1) as usize];
                        out.push(p.0);
                        stack.push(p.1);
                    }
                }
                while (out.len() as i64) < len && !flag(t, "nested") {
                    let remaining = len - out.len() as i64;
                    if !stack.is_empty() && (stack.len() as i64 >= remaining || rng.chance(50)) {
                        out.push(stack.pop().unwrap_or(')'));
                    } else if (stack.len() as i64) + 1 < remaining {
                        let p = pairs[rng.range(0, pairs.len() as i64 - 1) as usize];
                        out.push(p.0);
                        stack.push(p.1);
                    } else {
                        break;
                    }
                }
                while let Some(c) = stack.pop() {
                    out.push(c);
                }
                if !out.is_empty() && !rng.chance(num(t, "balanced_pct", (100, 100), n, rng)?) {
                    let mut cs: Vec<char> = out.chars().collect();
                    let k = rng.range(0, cs.len() as i64 - 1) as usize;
                    cs[k] = br
                        .chars()
                        .nth(rng.range(0, br.len() as i64 - 1) as usize)
                        .unwrap_or('(');
                    out = cs.into_iter().collect();
                }
                out
            } else if flag(t, "words") {
                let wl = bounds(t, "word_len", (1, 6), n)?;
                words_string(rng, num(t, "len", (0, 8), n, rng)?, alphabet, wl)
            } else if flag(t, "palindrome")
                || (t.contains_key("palindrome_pct")
                    && rng.chance(num(t, "palindrome_pct", (0, 0), n, rng)?))
            {
                let len = num(t, "len", (0, 8), n, rng)?;
                let half = rand_string(rng, (len + 1) / 2, alphabet);
                let mut full = half.clone();
                full.extend(half.chars().rev().skip((len % 2) as usize));
                full
            } else {
                rand_string(rng, num(t, "len", (0, 8), n, rng)?, alphabet)
            };
            let s = format!(
                "{}{s}{}",
                text_of(t, "prefix", ""),
                text_of(t, "suffix", "")
            );
            let ints = s.bytes().map(i64::from).collect();
            Ok(Arg {
                text: quote(&s),
                ints,
            })
        }
        "vec_int" => {
            let len = num(t, "len", (0, 8), n, rng)?;
            let mut v: Vec<i64> = if flag(t, "distinct") {
                let span = (range.1 - range.0 + 1).max(1);
                let mut pool: Vec<i64> = if span <= len * 8 + 64 {
                    (range.0..=range.1).collect()
                } else {
                    let mut set = BTreeSet::new();
                    while (set.len() as i64) < len {
                        set.insert(rng.range(range.0, range.1));
                    }
                    set.into_iter().collect()
                };
                rng.shuffle(&mut pool);
                pool.truncate(len.max(0) as usize);
                pool
            } else {
                (0..len.max(0))
                    .map(|_| rng.range(range.0, range.1))
                    .collect()
            };
            if flag(t, "perm_missing") {
                let top = num(t, "len", (1, 8), n, rng)?;
                v = (0..=top).collect();
                rng.shuffle(&mut v);
                v.pop();
            }
            if flag(t, "single_out") {
                let pairs = num(t, "len", (0, 4), n, rng)?;
                v.clear();
                let mut pool: Vec<i64> = (range.0..=range.1).collect();
                rng.shuffle(&mut pool);
                for x in pool.iter().take(pairs as usize + 1) {
                    v.push(*x);
                    v.push(*x);
                }
                v.pop();
                rng.shuffle(&mut v);
            }
            if flag(t, "sorted") {
                v.sort();
            }
            if flag(t, "descending") {
                v.sort();
                v.reverse();
            }
            if flag(t, "rotated") && !v.is_empty() {
                v.sort();
                let k = rng.range(0, v.len() as i64 - 1) as usize;
                v.rotate_left(k);
            }
            Ok(Arg {
                text: ints_text(&v),
                ints: v,
            })
        }
        "vec_bool" => {
            let len = num(t, "len", (0, 8), n, rng)?;
            let v: Vec<bool> = (0..len.max(0)).map(|_| rng.chance(50)).collect();
            Ok(Arg {
                text: format!(
                    "[{}]",
                    v.iter().map(bool::to_string).collect::<Vec<_>>().join(", ")
                ),
                ints: vec![],
            })
        }
        "vec_char" => {
            let len = num(t, "len", (0, 8), n, rng)?;
            let s = rand_string(rng, len, alphabet);
            Ok(Arg {
                text: format!(
                    "[{}]",
                    s.chars()
                        .map(|c| format!("'{c}'"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                ints: vec![],
            })
        }
        "vec_string" => {
            let len = num(t, "len", (0, 6), n, rng)?;
            let il = bounds(t, "item_len", (0, 5), n)?;
            let mut items: Vec<String> = Vec::new();
            let dup = num(t, "dup_pct", (0, 0), n, rng)?;
            let anagram = num(t, "anagram_pct", (0, 0), n, rng)?;
            for _ in 0..len.max(0) {
                if !items.is_empty() && rng.chance(dup) {
                    let k = rng.range(0, items.len() as i64 - 1) as usize;
                    items.push(items[k].clone());
                } else if !items.is_empty() && rng.chance(anagram) {
                    let k = rng.range(0, items.len() as i64 - 1) as usize;
                    let mut cs: Vec<char> = items[k].chars().collect();
                    rng.shuffle(&mut cs);
                    items.push(cs.into_iter().collect());
                } else {
                    items.push(rand_string(rng, rng.range(il.0, il.1), alphabet));
                }
            }
            if flag(t, "common_prefix") {
                let p = rand_string(rng, rng.range(0, 4), alphabet);
                for it in &mut items {
                    it.insert_str(0, &p);
                }
            }
            Ok(Arg {
                text: format!(
                    "[{}]",
                    items
                        .iter()
                        .map(|s| quote(s))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                ints: vec![],
            })
        }
        "sorted_lists" => {
            let count = num(t, "count", (1, 4), n, rng)?;
            let mut lists = Vec::new();
            let mut flat = Vec::new();
            for _ in 0..count {
                let len = num(t, "len", (0, 5), n, rng)?;
                let mut v: Vec<i64> = (0..len).map(|_| rng.range(range.0, range.1)).collect();
                v.sort();
                flat.extend(v.iter().copied());
                lists.push(ints_text(&v));
            }
            Ok(Arg {
                text: format!("[{}]", lists.join(", ")),
                ints: flat,
            })
        }
        "rpn" => {
            let numbers = num(t, "numbers", (1, 5), n, rng)?;
            let ops = text_of(t, "ops", "+-*");
            let ops: Vec<char> = ops.chars().collect();
            let mut tokens: Vec<String> = Vec::new();
            let mut vals: Vec<i128> = Vec::new();
            let mut pushed = 0;
            while pushed < numbers || vals.len() > 1 {
                if vals.len() >= 2 && (pushed >= numbers || rng.chance(45)) {
                    let (b, a) = (vals.pop().unwrap_or(1), vals.pop().unwrap_or(1));
                    let mut op = ops[rng.range(0, ops.len() as i64 - 1) as usize];
                    if op == '/' && b == 0 {
                        op = '+';
                    }
                    vals.push(match op {
                        '+' => a + b,
                        '-' => a - b,
                        '*' => a * b,
                        _ => a / b,
                    });
                    tokens.push(quote(&op.to_string()));
                } else {
                    let v = rng.range(range.0, range.1);
                    vals.push(i128::from(v));
                    tokens.push(quote(&v.to_string()));
                    pushed += 1;
                }
            }
            Ok(Arg {
                text: format!("[{}]", tokens.join(", ")),
                ints: vec![],
            })
        }
        "grid_int" => {
            let rows = num(t, "rows", (1, 4), n, rng)?;
            let cols = if flag(t, "square") {
                rows
            } else {
                num(t, "cols", (1, 4), n, rng)?
            };
            let rows_v: Vec<String> = (0..rows)
                .map(|_| {
                    ints_text(
                        &(0..cols)
                            .map(|_| rng.range(range.0, range.1))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect();
            Ok(Arg {
                text: format!("[{}]", rows_v.join(", ")),
                ints: vec![rows, cols],
            })
        }
        "grid_char" => {
            let rows = num(t, "rows", (1, 4), n, rng)?;
            let cols = if flag(t, "square") {
                rows
            } else {
                num(t, "cols", (1, 4), n, rng)?
            };
            let rows_v: Vec<String> = (0..rows)
                .map(|_| {
                    format!(
                        "[{}]",
                        (0..cols)
                            .map(|_| format!(
                                "'{}'",
                                alphabet[rng.range(0, alphabet.len() as i64 - 1) as usize] as char
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
                .collect();
            Ok(Arg {
                text: format!("[{}]", rows_v.join(", ")),
                ints: vec![rows, cols],
            })
        }
        "pairs" => {
            // vec<vec<int>> of [a, b] pairs; `intervals` makes a <= b
            let count = num(t, "count", (0, 6), n, rng)?;
            let mut ps = Vec::new();
            let mut flat = Vec::new();
            for _ in 0..count.max(0) {
                let a = rng.range(range.0, range.1);
                let b = if flag(t, "intervals") {
                    a + rng.range(0, num(t, "max_len", (3, 3), n, rng)?)
                } else {
                    rng.range(range.0, range.1)
                };
                ps.push(format!("[{a}, {b}]"));
                flat.push(a);
                flat.push(b);
            }
            Ok(Arg {
                text: format!("[{}]", ps.join(", ")),
                ints: flat,
            })
        }
        "edges" => {
            // random edges over `nodes` vertices (given as an earlier int argument or a number)
            let nodes = match dep("nodes_arg")? {
                Some(a) => a.ints.first().copied().unwrap_or(1),
                None => num(t, "nodes", (1, 6), n, rng)?,
            };
            let m = num(t, "m", (0, nodes * 2), n, rng)?;
            let directed = flag(t, "directed");
            let dag = flag(t, "dag");
            let mut seen = BTreeSet::new();
            let mut es = Vec::new();
            let mut tries = 0;
            let mut perm: Vec<i64> = (0..nodes).collect();
            if dag {
                rng.shuffle(&mut perm);
            }
            if flag(t, "chain") {
                for v in 1..nodes {
                    seen.insert((v - 1, v));
                    es.push(format!("[{}, {v}]", v - 1));
                }
            }
            if flag(t, "connected") {
                for v in 1..nodes {
                    let u = rng.range(0, v - 1);
                    seen.insert((u.min(v), u.max(v)));
                    es.push(format!("[{u}, {v}]"));
                }
            }
            while (es.len() as i64) < m && nodes > 1 && tries < 10 * m + 20 {
                tries += 1;
                let a = rng.range(0, nodes - 1);
                let b = rng.range(0, nodes - 1);
                if a == b {
                    continue;
                }
                let (a, b) = if dag {
                    let (pa, pb) = (perm[a as usize], perm[b as usize]);
                    if pa < pb { (a, b) } else { (b, a) }
                } else {
                    (a, b)
                };
                let key = if directed || dag {
                    (a, b)
                } else {
                    (a.min(b), a.max(b))
                };
                if seen.insert(key) {
                    es.push(format!("[{a}, {b}]"));
                }
            }
            Ok(Arg {
                text: format!("[{}]", es.join(", ")),
                ints: vec![nodes],
            })
        }
        "list" => {
            let len = num(t, "len", (0, 6), n, rng)?;
            let mut v: Vec<i64> = (0..len).map(|_| rng.range(range.0, range.1)).collect();
            if flag(t, "sorted") {
                v.sort();
            }
            let mut text = ints_text(&v);
            if len > 0
                && t.contains_key("cycle_pct")
                && rng.chance(num(t, "cycle_pct", (0, 0), n, rng)?)
            {
                text.push_str(&format!(" @{}", rng.range(0, len - 1)));
            }
            Ok(Arg { text, ints: v })
        }
        "tree" => {
            let count = num(t, "nodes", (0, 7), n, rng)?;
            let vals: Vec<i64> = if flag(t, "distinct") || flag(t, "bst") {
                let mut pool: Vec<i64> = (range.0..=range.1.max(range.0 + count)).collect();
                rng.shuffle(&mut pool);
                pool.truncate(count as usize);
                pool
            } else {
                (0..count).map(|_| rng.range(range.0, range.1)).collect()
            };
            let mut vals = vals;
            if vals.len() > 1 && rng.chance(num(t, "swap_pct", (0, 0), n, rng)?) {
                let (a, b) = (
                    rng.range(0, vals.len() as i64 - 1) as usize,
                    rng.range(0, vals.len() as i64 - 1) as usize,
                );
                vals.swap(a, b);
            }
            // structure: arrays of left/right child indices
            let cn = vals.len();
            let mut left = vec![usize::MAX; cn];
            let mut right = vec![usize::MAX; cn];
            if flag(t, "bst") {
                for i in 1..cn {
                    let mut cur = 0;
                    loop {
                        let slot = if vals[i] < vals[cur] {
                            &mut left[cur]
                        } else {
                            &mut right[cur]
                        };
                        if *slot == usize::MAX {
                            *slot = i;
                            break;
                        }
                        cur = *slot;
                    }
                }
            } else {
                let deep = flag(t, "deep");
                let mut open: Vec<(usize, bool)> = Vec::new();
                for i in 0..cn {
                    if i > 0 {
                        let k = if deep {
                            open.len() - 1 - rng.range(0, (open.len() as i64 - 1).min(1)) as usize
                        } else {
                            rng.range(0, open.len() as i64 - 1) as usize
                        };
                        let (p, is_left) = open.remove(k);
                        if is_left { left[p] = i } else { right[p] = i }
                    }
                    open.push((i, true));
                    open.push((i, false));
                }
            }
            let mut items: Vec<String> = Vec::new();
            if cn > 0 {
                let mut q = std::collections::VecDeque::new();
                q.push_back(0usize);
                while let Some(x) = q.pop_front() {
                    if x == usize::MAX {
                        items.push("null".into());
                        continue;
                    }
                    items.push(vals[x].to_string());
                    q.push_back(left[x]);
                    q.push_back(right[x]);
                }
                while items.last().is_some_and(|s| s == "null") {
                    items.pop();
                }
            }
            Ok(Arg {
                text: format!("[{}]", items.join(", ")),
                ints: vals,
            })
        }
        other => Err(format!("unknown generator kind '{other}'")),
    }
}

/// Expand one generator block into argument-literal lists.
pub fn expand(spec: &GenSpec, n_override: Option<i64>) -> Result<Vec<Vec<String>>, String> {
    let n = n_override.unwrap_or(spec.n);
    let rng = Rng::new(spec.seed);
    let mut out = Vec::new();
    for _ in 0..spec.count {
        let mut args: Vec<Arg> = Vec::new();
        for a in &spec.args {
            let g = gen_arg(a, n, &rng, &args)?;
            args.push(g);
        }
        out.push(args.into_iter().map(|a| a.text).collect());
    }
    Ok(out)
}

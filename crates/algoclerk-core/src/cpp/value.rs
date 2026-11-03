// SPDX-License-Identifier: Apache-2.0
//! Plain (owned, `Send + Sync`) values: what tests, results and trace snapshots hold, and the brace-list text
//! notation shared by level files, the judge, the watch panel and the code generators.
//!
//! The interpreter has its own internal runtime representation; it converts [`Plain`] arguments in and
//! results out. Pointers to structs (`ListNode*`, `TreeNode*`) are indices into a [`Heap`] so that cycles are
//! representable.
//!
//! Text notation (the C++ brace-initialiser spelling, nothing invented):
//! `42`, `-7`, `true`, `'a'`, `"text"`, `{1, 2, 3}`, `{{1, 2}, {3, 4}}`, a `pair`/`tuple` as `{a, b}`, a map as
//! `{{k, v}, {k, v}}`, an optional as `nullopt` or `{x}`, `nullptr`. A `ListNode*` is `{1, 2, 3}` (a list of
//! values, optionally followed by ` @k` meaning the last node points back to node `k`), a `TreeNode*` is the
//! level-order list with `nullptr` holes: `{1, 2, 3, nullptr, 4}`.

use super::ast::Ty;
use std::collections::BTreeSet;

/// A plain value. Which C++ type it has is known from the accompanying [`Ty`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Plain {
    /// No value (`void`).
    Void,
    /// A declared but never assigned scalar (only in trace snapshots).
    Uninit,
    /// `bool`, `char`, `int`, `unsigned`, `long long`, `unsigned long long`, `size_t`.
    Int(i128),
    /// `std::string` / `std::string_view` bytes.
    Str(Vec<u8>),
    /// `vector`, `array`, `deque`, `list`, `stack` (bottom first), `queue` (front first), `pair` / `tuple`
    /// fields, `set` / `unordered_set` elements (iteration order), `priority_queue` (pop order).
    Seq(Vec<Plain>),
    /// `map` / `unordered_map` entries in iteration order.
    Map(Vec<(Plain, Plain)>),
    /// `std::optional`.
    Opt(Option<Box<Plain>>),
    /// Pointer to a struct: index into [`Heap::nodes`]; 0 is `nullptr`.
    Ptr(u32),
    /// A struct held by value: its fields in declaration order.
    Struct(Vec<Plain>),
}

/// Struct instances addressed by [`Plain::Ptr`]. Index 0 is reserved for `nullptr`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heap {
    /// Field values of each instance (`ListNode`: `[val, next]`; `TreeNode`: `[val, left, right]`).
    pub nodes: Vec<Vec<Plain>>,
}

impl Default for Heap {
    fn default() -> Heap {
        Heap {
            nodes: vec![Vec::new()],
        }
    }
}

impl Heap {
    /// Allocate a node and return its pointer value.
    pub fn alloc(&mut self, fields: Vec<Plain>) -> u32 {
        self.nodes.push(fields);
        (self.nodes.len() - 1) as u32
    }
}

impl Plain {
    /// An integer value.
    pub fn int(v: i64) -> Plain {
        Plain::Int(v as i128)
    }
    /// A string value.
    pub fn str(s: &str) -> Plain {
        Plain::Str(s.as_bytes().to_vec())
    }
    /// Integer payload.
    pub fn as_int(&self) -> Option<i128> {
        if let Plain::Int(i) = self {
            Some(*i)
        } else {
            None
        }
    }
    /// Number of cells (leaf values) held, recursively; scalars count 1. Used for the space estimate.
    pub fn cells(&self) -> u64 {
        match self {
            Plain::Str(s) => s.len() as u64 + 1,
            Plain::Seq(v) | Plain::Struct(v) => 1 + v.iter().map(Plain::cells).sum::<u64>(),
            Plain::Map(m) => 1 + m.iter().map(|(k, v)| k.cells() + v.cells()).sum::<u64>(),
            Plain::Opt(Some(v)) => 1 + v.cells(),
            _ => 1,
        }
    }
}

const RENDER_CAP: usize = 1_000_000;

fn esc(c: u8, quote: u8, out: &mut String) {
    match c {
        b'\n' => out.push_str("\\n"),
        b'\t' => out.push_str("\\t"),
        b'\r' => out.push_str("\\r"),
        0 => out.push_str("\\0"),
        b'\\' => out.push_str("\\\\"),
        c if c == quote => {
            out.push('\\');
            out.push(c as char);
        }
        c if (0x20..0x7f).contains(&c) => out.push(c as char),
        c => out.push_str(&format!("\\x{c:02x}")),
    }
}

/// Text form of a value of type `ty` (see the module docs).
pub fn render(v: &Plain, ty: &Ty, heap: &Heap) -> String {
    let mut s = String::new();
    render_into(v, ty, heap, &mut s);
    s
}

fn elem_ty(ty: &Ty) -> Option<&Ty> {
    match ty {
        Ty::Vector(t)
        | Ty::Array(t, _)
        | Ty::Deque(t)
        | Ty::List(t)
        | Ty::Stack(t)
        | Ty::Queue(t)
        | Ty::Set(t)
        | Ty::UnorderedSet(t)
        | Ty::PriorityQueue(t, _) => Some(t),
        _ => None,
    }
}

fn render_into(v: &Plain, ty: &Ty, heap: &Heap, out: &mut String) {
    match (v, ty) {
        (Plain::Void, _) => out.push_str("void"),
        (Plain::Uninit, _) => out.push_str("<uninitialised>"),
        (Plain::Int(i), Ty::Bool) => out.push_str(if *i != 0 { "true" } else { "false" }),
        (Plain::Int(i), Ty::Char) => {
            out.push('\'');
            esc(*i as u8, b'\'', out);
            out.push('\'');
        }
        (Plain::Int(i), _) => out.push_str(&i.to_string()),
        (Plain::Str(s), _) => {
            out.push('"');
            for &c in s {
                esc(c, b'"', out);
            }
            out.push('"');
        }
        (Plain::Opt(None), _) => out.push_str("nullopt"),
        (Plain::Opt(Some(x)), Ty::Optional(t)) => {
            out.push('{');
            render_into(x, t, heap, out);
            out.push('}');
        }
        (Plain::Ptr(p), Ty::Ptr(inner)) => render_ptr(*p, inner, heap, out),
        (Plain::Ptr(0), _) => out.push_str("nullptr"),
        (Plain::Seq(xs), Ty::Pair(a, b)) if xs.len() == 2 => {
            out.push('{');
            render_into(&xs[0], a, heap, out);
            out.push_str(", ");
            render_into(&xs[1], b, heap, out);
            out.push('}');
        }
        (Plain::Seq(xs), Ty::Tuple(ts)) => {
            out.push('{');
            for (i, (x, t)) in xs.iter().zip(ts).enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                render_into(x, t, heap, out);
            }
            out.push('}');
        }
        (Plain::Seq(xs), t) => {
            let et = elem_ty(t).cloned().unwrap_or(Ty::Unknown);
            out.push('{');
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                render_into(x, &et, heap, out);
            }
            out.push('}');
        }
        (Plain::Map(es), Ty::Map(k, val) | Ty::UnorderedMap(k, val)) => {
            out.push('{');
            for (i, (a, b)) in es.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push('{');
                render_into(a, k, heap, out);
                out.push_str(", ");
                render_into(b, val, heap, out);
                out.push('}');
            }
            out.push('}');
        }
        (Plain::Struct(fs), _) => {
            out.push('{');
            for (i, f) in fs.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                render_into(f, &Ty::Unknown, heap, out);
            }
            out.push('}');
        }
        _ => out.push_str("?"),
    }
}

fn is_tree(inner: &Ty) -> bool {
    matches!(inner, Ty::Struct(n) if n == "TreeNode")
}

fn render_ptr(p: u32, inner: &Ty, heap: &Heap, out: &mut String) {
    if p == 0 {
        out.push_str("nullptr");
        return;
    }
    if is_tree(inner) {
        // level order with nullptr holes, trailing holes trimmed
        let mut items: Vec<Option<i128>> = Vec::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(p);
        let mut seen = 0usize;
        while let Some(n) = queue.pop_front() {
            seen += 1;
            if n == 0 || seen > RENDER_CAP {
                items.push(None);
                continue;
            }
            let f = heap.nodes.get(n as usize);
            let val = f
                .and_then(|f| f.first())
                .and_then(Plain::as_int)
                .unwrap_or(0);
            items.push(Some(val));
            let child = |i: usize| match f.and_then(|f| f.get(i)) {
                Some(Plain::Ptr(c)) => *c,
                _ => 0,
            };
            queue.push_back(child(1));
            queue.push_back(child(2));
        }
        while items.last() == Some(&None) {
            items.pop();
        }
        out.push('{');
        for (i, it) in items.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            match it {
                Some(v) => out.push_str(&v.to_string()),
                None => out.push_str("nullptr"),
            }
        }
        out.push('}');
    } else {
        let mut order: Vec<u32> = Vec::new();
        let mut cur = p;
        let mut cycle: Option<usize> = None;
        while cur != 0 && order.len() < RENDER_CAP {
            if let Some(i) = order.iter().position(|&x| x == cur).filter(|_| order.len() < 5000) {
                cycle = Some(i);
                break;
            }
            order.push(cur);
            cur = match heap.nodes.get(cur as usize).and_then(|f| f.get(1)) {
                Some(Plain::Ptr(n)) => *n,
                _ => 0,
            };
        }
        out.push('{');
        for (i, n) in order.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            let val = heap
                .nodes
                .get(*n as usize)
                .and_then(|f| f.first())
                .and_then(Plain::as_int)
                .unwrap_or(0);
            out.push_str(&val.to_string());
        }
        out.push('}');
        if let Some(k) = cycle {
            out.push_str(&format!(" @{k}"));
        }
    }
}

/// Parse text in the module's notation into a value of type `ty`, allocating nodes in `heap`.
pub fn parse_value(text: &str, ty: &Ty, heap: &mut Heap) -> Result<Plain, String> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    let v = p.value(ty, heap)?;
    p.ws();
    // optional cycle suffix is consumed by the list parser; anything else is junk
    if p.i < p.s.len() {
        return Err(format!("unexpected text after the value: {}", &text[p.i..]));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, c: u8) -> Result<(), String> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(format!("expected '{}' at offset {}", c as char, self.i))
        }
    }
    fn word(&mut self) -> String {
        self.ws();
        let st = self.i;
        while self.i < self.s.len()
            && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_')
        {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[st..self.i]).into_owned()
    }
    fn number(&mut self) -> Result<i128, String> {
        self.ws();
        let st = self.i;
        if self.s.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
            self.i += 1;
        }
        std::str::from_utf8(&self.s[st..self.i])
            .ok()
            .and_then(|t| t.parse::<i128>().ok())
            .ok_or_else(|| format!("expected a number at offset {st}"))
    }
    fn escaped(&mut self, quote: u8) -> Result<u8, String> {
        let c = *self.s.get(self.i).ok_or("unterminated literal")?;
        self.i += 1;
        if c != b'\\' {
            return Ok(c);
        }
        let e = *self.s.get(self.i).ok_or("bad escape")?;
        self.i += 1;
        Ok(match e {
            b'n' => b'\n',
            b't' => b'\t',
            b'r' => b'\r',
            b'0' => 0,
            b'\\' => b'\\',
            b'\'' => b'\'',
            b'"' => b'"',
            b'x' => {
                let h = std::str::from_utf8(self.s.get(self.i..self.i + 2).ok_or("bad \\x")?)
                    .map_err(|_| "bad \\x")?;
                self.i += 2;
                u8::from_str_radix(h, 16).map_err(|_| "bad \\x".to_string())?
            }
            _ => return Err(format!("unknown escape \\{}", e as char)).map_err(|e: String| {
                let _ = quote;
                e
            }),
        })
    }
    fn list<T>(
        &mut self,
        mut item: impl FnMut(&mut Self, usize) -> Result<T, String>,
    ) -> Result<Vec<T>, String> {
        self.expect(b'{')?;
        let mut out = Vec::new();
        if self.eat(b'}') {
            return Ok(out);
        }
        loop {
            let n = out.len();
            out.push(item(self, n)?);
            if self.eat(b',') {
                if self.peek() == Some(b'}') {
                    self.i += 1;
                    return Ok(out);
                }
                continue;
            }
            self.expect(b'}')?;
            return Ok(out);
        }
    }
    fn value(&mut self, ty: &Ty, heap: &mut Heap) -> Result<Plain, String> {
        match ty {
            Ty::Bool => match self.word().as_str() {
                "true" => Ok(Plain::Int(1)),
                "false" => Ok(Plain::Int(0)),
                w => Err(format!("expected true or false, found '{w}'")),
            },
            Ty::Char => {
                self.expect(b'\'')?;
                let c = self.escaped(b'\'')?;
                if self.s.get(self.i) != Some(&b'\'') {
                    return Err("unterminated char literal".into());
                }
                self.i += 1;
                Ok(Plain::Int(i128::from(c as i8)))
            }
            t if t.is_integral() => {
                let n = self.number()?;
                let (lo, hi) = int_range(t);
                if n < lo || n > hi {
                    return Err(format!("{n} does not fit in {t}"));
                }
                Ok(Plain::Int(n))
            }
            Ty::Str | Ty::StrView => {
                self.expect(b'"')?;
                let mut out = Vec::new();
                loop {
                    match self.s.get(self.i) {
                        None => return Err("unterminated string".into()),
                        Some(b'"') => {
                            self.i += 1;
                            return Ok(Plain::Str(out));
                        }
                        _ => out.push(self.escaped(b'"')?),
                    }
                }
            }
            Ty::Optional(t) => {
                if self.peek() == Some(b'{') {
                    let mut xs = self.list(|p, _| p.value(t, heap))?;
                    if xs.len() != 1 {
                        return Err("an optional holds exactly one value".into());
                    }
                    Ok(Plain::Opt(Some(Box::new(xs.remove(0)))))
                } else if self.word() == "nullopt" {
                    Ok(Plain::Opt(None))
                } else {
                    Err("expected nullopt or {value}".into())
                }
            }
            Ty::Pair(a, b) => {
                let xs = self.list(|p, i| if i == 0 { p.value(a, heap) } else { p.value(b, heap) })?;
                if xs.len() != 2 {
                    return Err("a pair has two values".into());
                }
                Ok(Plain::Seq(xs))
            }
            Ty::Tuple(ts) => {
                let xs = self.list(|p, i| match ts.get(i) {
                    Some(t) => p.value(t, heap),
                    None => Err("too many tuple fields".into()),
                })?;
                if xs.len() != ts.len() {
                    return Err("wrong number of tuple fields".into());
                }
                Ok(Plain::Seq(xs))
            }
            Ty::Map(k, v) | Ty::UnorderedMap(k, v) => {
                let es = self.list(|p, _| {
                    let kv = p.list(|p, i| {
                        if i == 0 {
                            p.value(k, heap)
                        } else {
                            p.value(v, heap)
                        }
                    })?;
                    let mut it = kv.into_iter();
                    match (it.next(), it.next(), it.next()) {
                        (Some(a), Some(b), None) => Ok((a, b)),
                        _ => Err("a map entry is {key, value}".to_string()),
                    }
                })?;
                Ok(Plain::Map(es))
            }
            Ty::Set(t) | Ty::UnorderedSet(t) => {
                let xs = self.list(|p, _| p.value(t, heap))?;
                let mut seen = BTreeSet::new();
                for x in &xs {
                    if !seen.insert(x.clone()) {
                        return Err("duplicate element in a set".into());
                    }
                }
                Ok(Plain::Seq(xs))
            }
            Ty::Ptr(inner) => self.ptr(inner, heap),
            t => match elem_ty(t) {
                Some(et) => {
                    let et = et.clone();
                    let xs = self.list(|p, _| p.value(&et, heap))?;
                    if let Ty::Array(_, n) = t {
                        if xs.len() != *n {
                            return Err(format!("expected {n} elements, found {}", xs.len()));
                        }
                    }
                    Ok(Plain::Seq(xs))
                }
                None => Err(format!("cannot write a value of type {t}")),
            },
        }
    }
    fn ptr(&mut self, inner: &Ty, heap: &mut Heap) -> Result<Plain, String> {
        if self.peek() == Some(b'n') {
            return if self.word() == "nullptr" {
                Ok(Plain::Ptr(0))
            } else {
                Err("expected nullptr".into())
            };
        }
        if is_tree(inner) {
            let items = self.list(|p, _| {
                if p.peek() == Some(b'n') {
                    if p.word() == "nullptr" {
                        Ok(None)
                    } else {
                        Err("expected nullptr".to_string())
                    }
                } else {
                    p.number().map(Some)
                }
            })?;
            if items.first().is_none_or(Option::is_none) {
                return Ok(Plain::Ptr(0));
            }
            let mut ids: Vec<u32> = Vec::new();
            for it in &items {
                ids.push(match it {
                    Some(v) => heap.alloc(vec![Plain::Int(*v), Plain::Ptr(0), Plain::Ptr(0)]),
                    None => 0,
                });
            }
            let mut next = 1usize;
            for i in 0..ids.len() {
                if ids[i] == 0 {
                    continue;
                }
                for slot in [1usize, 2] {
                    if next < ids.len() {
                        heap.nodes[ids[i] as usize][slot] = Plain::Ptr(ids[next]);
                        next += 1;
                    }
                }
            }
            Ok(Plain::Ptr(ids[0]))
        } else {
            let vals = self.list(|p, _| p.number())?;
            let ids: Vec<u32> = vals
                .iter()
                .map(|v| heap.alloc(vec![Plain::Int(*v), Plain::Ptr(0)]))
                .collect();
            for w in ids.windows(2) {
                heap.nodes[w[0] as usize][1] = Plain::Ptr(w[1]);
            }
            if self.peek() == Some(b'@') {
                self.i += 1;
                let k = self.number()?;
                let k = usize::try_from(k).map_err(|_| "bad cycle index")?;
                match (ids.last(), ids.get(k)) {
                    (Some(&last), Some(&target)) => {
                        heap.nodes[last as usize][1] = Plain::Ptr(target);
                    }
                    _ => return Err("cycle index out of range".into()),
                }
            }
            Ok(Plain::Ptr(ids.first().copied().unwrap_or(0)))
        }
    }
}

/// Inclusive value range of an integral type.
pub fn int_range(t: &Ty) -> (i128, i128) {
    match t {
        Ty::Bool => (0, 1),
        Ty::Char => (-128, 127),
        Ty::UChar => (0, 255),
        Ty::Int => (i128::from(i32::MIN), i128::from(i32::MAX)),
        Ty::UInt => (0, i128::from(u32::MAX)),
        Ty::LongLong => (i128::from(i64::MIN), i128::from(i64::MAX)),
        _ => (0, i128::from(u64::MAX)),
    }
}

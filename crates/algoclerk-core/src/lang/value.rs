// SPDX-License-Identifier: Apache-2.0
//! Runtime values, node arena and canonical literal text.

use super::ast::Ty;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;

/// A runtime value. Collections are copy-on-write; the cost model charges logical copies.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Value {
    /// No value.
    Unit,
    /// Integer.
    Int(i64),
    /// Boolean.
    Bool(bool),
    /// ASCII character.
    Char(u8),
    /// ASCII string.
    Str(Rc<Vec<u8>>),
    /// Shelf.
    Vec(Rc<Vec<Value>>),
    /// Bin.
    Set(Rc<BTreeSet<Value>>),
    /// Filing cabinet.
    Map(Rc<BTreeMap<Value, Value>>),
    /// Tuple.
    Tuple(Rc<Vec<Value>>),
    /// Option.
    Opt(Option<Rc<Value>>),
    /// Stack (top is the last element).
    Stack(Rc<Vec<Value>>),
    /// Queue or deque (front is the first element).
    Queue(Rc<VecDeque<Value>>),
    /// Max-heap stored as an implicit binary tree.
    Heap(Rc<Vec<Value>>),
    /// Linked list or tree node handle (0 is null).
    Node(u32),
    /// Reference to a variable in the interpreter stack (by-reference parameters).
    Ref(usize),
}

/// One heap-allocated list or tree node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeData {
    /// Payload.
    pub val: i64,
    /// `next` for lists, `left` for trees.
    pub a: u32,
    /// `right` for trees.
    pub b: u32,
}

impl Value {
    /// New string value.
    pub fn str(s: &str) -> Value {
        Value::Str(Rc::new(s.as_bytes().to_vec()))
    }
    /// New vec value.
    pub fn vec(v: Vec<Value>) -> Value {
        Value::Vec(Rc::new(v))
    }
    /// New tuple value.
    pub fn tuple(v: Vec<Value>) -> Value {
        Value::Tuple(Rc::new(v))
    }
    /// Integer payload.
    pub fn as_int(&self) -> Option<i64> {
        if let Value::Int(i) = self { Some(*i) } else { None }
    }
    /// Boolean payload.
    pub fn as_bool(&self) -> Option<bool> {
        if let Value::Bool(b) = self { Some(*b) } else { None }
    }
    /// Is this a collection that costs something to copy?
    pub fn is_bulky(&self) -> bool {
        matches!(
            self,
            Value::Str(_) | Value::Vec(_) | Value::Set(_) | Value::Map(_) | Value::Tuple(_) | Value::Stack(_) | Value::Queue(_) | Value::Heap(_)
        )
    }
    /// Number of cells (leaf values) held, recursively; scalars count 1.
    pub fn cells(&self) -> u64 {
        match self {
            Value::Str(s) => s.len() as u64 + 1,
            Value::Vec(v) | Value::Stack(v) | Value::Heap(v) | Value::Tuple(v) => 1 + v.iter().map(Value::cells).sum::<u64>(),
            Value::Set(s) => 1 + s.iter().map(Value::cells).sum::<u64>(),
            Value::Map(m) => 1 + m.iter().map(|(k, v)| k.cells() + v.cells()).sum::<u64>(),
            Value::Queue(q) => 1 + q.iter().map(Value::cells).sum::<u64>(),
            Value::Opt(Some(v)) => 1 + v.cells(),
            _ => 1,
        }
    }
    /// Top-level element count (for cheap cost estimates).
    pub fn shallow_len(&self) -> u64 {
        match self {
            Value::Str(s) => s.len() as u64,
            Value::Vec(v) | Value::Stack(v) | Value::Heap(v) | Value::Tuple(v) => v.len() as u64,
            Value::Set(s) => s.len() as u64,
            Value::Map(m) => m.len() as u64,
            Value::Queue(q) => q.len() as u64,
            _ => 1,
        }
    }
}

/// Maximum nodes rendered when printing linked structures.
const RENDER_CAP: usize = 1_000_000;

/// Text form of a value; the same syntax is accepted by [`parse_value`].
pub fn render(v: &Value, ty: &Ty, nodes: &[NodeData]) -> String {
    let mut s = String::new();
    render_into(v, ty, nodes, &mut s);
    s
}

fn esc_byte(c: u8, quote: u8, out: &mut String) {
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
        c if c == b' ' || c.is_ascii_graphic() => out.push(c as char),
        c => out.push_str(&format!("\\x{c:02x}")),
    }
}

fn render_into(v: &Value, ty: &Ty, nodes: &[NodeData], out: &mut String) {
    match (v, ty) {
        (Value::Unit, _) => out.push_str("()"),
        (Value::Int(i), _) => out.push_str(&i.to_string()),
        (Value::Bool(b), _) => out.push_str(if *b { "true" } else { "false" }),
        (Value::Char(c), _) => {
            out.push('\'');
            esc_byte(*c, b'\'', out);
            out.push('\'');
        }
        (Value::Str(s), _) => {
            out.push('"');
            for &c in s.iter() {
                esc_byte(c, b'"', out);
            }
            out.push('"');
        }
        (Value::Vec(items), Ty::Vec(t)) | (Value::Stack(items), Ty::Stack(t)) | (Value::Heap(items), Ty::Heap(t)) => {
            out.push('[');
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                render_into(x, t, nodes, out);
            }
            out.push(']');
        }
        (Value::Queue(items), Ty::Queue(t) | Ty::Deque(t)) => {
            out.push('[');
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                render_into(x, t, nodes, out);
            }
            out.push(']');
        }
        (Value::Set(items), Ty::Set(t)) => {
            out.push('{');
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                render_into(x, t, nodes, out);
            }
            out.push('}');
        }
        (Value::Map(items), Ty::Map(kt, vt)) => {
            out.push('{');
            for (i, (k, x)) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                render_into(k, kt, nodes, out);
                out.push_str(": ");
                render_into(x, vt, nodes, out);
            }
            out.push('}');
        }
        (Value::Tuple(items), Ty::Tuple(ts)) => {
            out.push('(');
            for (i, (x, t)) in items.iter().zip(ts.iter()).enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                render_into(x, t, nodes, out);
            }
            out.push(')');
        }
        (Value::Opt(None), _) => out.push_str("none"),
        (Value::Opt(Some(x)), Ty::Option(t)) => {
            out.push_str("some(");
            render_into(x, t, nodes, out);
            out.push(')');
        }
        (Value::Node(n), Ty::LNode) => render_list(*n, nodes, out),
        (Value::Node(n), Ty::TNode) => render_tree(*n, nodes, out),
        (Value::Node(n), _) => out.push_str(&format!("node#{n}")),
        (other, _) => out.push_str(&format!("{other:?}")),
    }
}

fn render_list(head: u32, nodes: &[NodeData], out: &mut String) {
    out.push('[');
    let mut cur = head;
    let mut seen = std::collections::HashMap::new();
    let mut i = 0usize;
    while cur != 0 {
        if let Some(&pos) = seen.get(&cur) {
            out.push_str(&format!("] @{pos}"));
            return;
        }
        seen.insert(cur, i);
        let Some(n) = nodes.get(cur as usize) else { break };
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&n.val.to_string());
        cur = n.a;
        i += 1;
        if i > RENDER_CAP {
            out.push_str(", ...");
            break;
        }
    }
    out.push(']');
}

fn render_tree(root: u32, nodes: &[NodeData], out: &mut String) {
    // level order with `null` holes, trailing nulls trimmed
    let mut items: Vec<Option<i64>> = Vec::new();
    let mut q: VecDeque<u32> = VecDeque::new();
    q.push_back(root);
    let mut count = 0;
    while let Some(n) = q.pop_front() {
        count += 1;
        if count > RENDER_CAP {
            break;
        }
        if n == 0 {
            items.push(None);
            continue;
        }
        let Some(d) = nodes.get(n as usize) else { items.push(None); continue };
        items.push(Some(d.val));
        q.push_back(d.a);
        q.push_back(d.b);
    }
    while items.last() == Some(&None) {
        items.pop();
    }
    out.push('[');
    for (i, x) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        match x {
            Some(v) => out.push_str(&v.to_string()),
            None => out.push_str("null"),
        }
    }
    out.push(']');
}

// ----- literal parsing -----

struct Lit<'a> {
    s: &'a [u8],
    i: usize,
    depth: u32,
}

type R<T> = Result<T, String>;

impl Lit<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn word(&mut self, w: &str) -> bool {
        self.ws();
        if self.s[self.i..].starts_with(w.as_bytes())
            && !self.s.get(self.i + w.len()).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
        {
            self.i += w.len();
            true
        } else {
            false
        }
    }
    fn quoted(&mut self, q: u8) -> R<Vec<u8>> {
        let mut out = Vec::new();
        loop {
            let Some(&c) = self.s.get(self.i) else { return Err("unterminated quote".into()) };
            self.i += 1;
            if c == q {
                return Ok(out);
            }
            if c == b'\\' {
                let Some(&e) = self.s.get(self.i) else { return Err("bad escape".into()) };
                self.i += 1;
                out.push(match e {
                    b'n' => b'\n',
                    b't' => b'\t',
                    b'r' => b'\r',
                    b'0' => 0,
                    b'x' => {
                        let h = self.s.get(self.i..self.i + 2).ok_or("bad escape")?;
                        self.i += 2;
                        u8::from_str_radix(std::str::from_utf8(h).map_err(|e| e.to_string())?, 16).map_err(|e| e.to_string())?
                    }
                    o => o,
                });
            } else {
                out.push(c);
            }
        }
    }
    fn int(&mut self) -> R<i64> {
        self.ws();
        let st = self.i;
        if self.s.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
            self.i += 1;
        }
        std::str::from_utf8(&self.s[st..self.i])
            .map_err(|e| e.to_string())?
            .parse::<i64>()
            .map_err(|_| format!("expected a number at position {st}"))
    }
    fn value(&mut self, ty: &Ty, nodes: &mut Vec<NodeData>) -> R<Value> {
        self.depth += 1;
        if self.depth > 64 {
            return Err("nested too deep".into());
        }
        let r = self.value_inner(ty, nodes);
        self.depth -= 1;
        r
    }
    fn seq(&mut self, close: u8, ty: &Ty, nodes: &mut Vec<NodeData>) -> R<Vec<Value>> {
        let mut out = Vec::new();
        if self.eat(close) {
            return Ok(out);
        }
        loop {
            out.push(self.value(ty, nodes)?);
            if self.eat(b',') {
                if self.eat(close) {
                    break;
                }
                continue;
            }
            if self.eat(close) {
                break;
            }
            return Err(format!("expected ',' or closing bracket at position {}", self.i));
        }
        Ok(out)
    }
    fn value_inner(&mut self, ty: &Ty, nodes: &mut Vec<NodeData>) -> R<Value> {
        self.ws();
        match ty {
            Ty::Int => Ok(Value::Int(self.int()?)),
            Ty::Bool => {
                if self.word("true") {
                    Ok(Value::Bool(true))
                } else if self.word("false") {
                    Ok(Value::Bool(false))
                } else {
                    Err("expected true or false".into())
                }
            }
            Ty::Char => {
                if !self.eat(b'\'') {
                    return Err("expected a 'c' character".into());
                }
                let t = self.quoted(b'\'')?;
                if t.len() != 1 {
                    return Err("a character literal holds one character".into());
                }
                Ok(Value::Char(t[0]))
            }
            Ty::Str => {
                if !self.eat(b'"') {
                    return Err("expected a \"string\"".into());
                }
                Ok(Value::Str(Rc::new(self.quoted(b'"')?)))
            }
            Ty::Vec(t) => {
                if !self.eat(b'[') {
                    return Err("expected '['".into());
                }
                Ok(Value::vec(self.seq(b']', t, nodes)?))
            }
            Ty::Stack(t) => {
                if !self.eat(b'[') {
                    return Err("expected '['".into());
                }
                Ok(Value::Stack(Rc::new(self.seq(b']', t, nodes)?)))
            }
            Ty::Queue(t) | Ty::Deque(t) => {
                if !self.eat(b'[') {
                    return Err("expected '['".into());
                }
                Ok(Value::Queue(Rc::new(self.seq(b']', t, nodes)?.into())))
            }
            Ty::Heap(t) => {
                if !self.eat(b'[') {
                    return Err("expected '['".into());
                }
                let mut h = Vec::new();
                for x in self.seq(b']', t, nodes)? {
                    heap_push(&mut h, x);
                }
                Ok(Value::Heap(Rc::new(h)))
            }
            Ty::Set(t) => {
                if !self.eat(b'{') {
                    return Err("expected '{'".into());
                }
                Ok(Value::Set(Rc::new(self.seq(b'}', t, nodes)?.into_iter().collect())))
            }
            Ty::Map(kt, vt) => {
                if !self.eat(b'{') {
                    return Err("expected '{'".into());
                }
                let mut m = BTreeMap::new();
                if self.eat(b'}') {
                    return Ok(Value::Map(Rc::new(m)));
                }
                loop {
                    let k = self.value(kt, nodes)?;
                    if !self.eat(b':') {
                        return Err("expected ':'".into());
                    }
                    let v = self.value(vt, nodes)?;
                    m.insert(k, v);
                    if self.eat(b',') {
                        if self.eat(b'}') {
                            break;
                        }
                        continue;
                    }
                    if self.eat(b'}') {
                        break;
                    }
                    return Err("expected ',' or '}'".into());
                }
                Ok(Value::Map(Rc::new(m)))
            }
            Ty::Tuple(ts) => {
                if !self.eat(b'(') {
                    return Err("expected '('".into());
                }
                let mut out = Vec::new();
                for (i, t) in ts.iter().enumerate() {
                    if i > 0 && !self.eat(b',') {
                        return Err("expected ','".into());
                    }
                    out.push(self.value(t, nodes)?);
                }
                self.eat(b',');
                if !self.eat(b')') {
                    return Err("expected ')'".into());
                }
                Ok(Value::tuple(out))
            }
            Ty::Option(t) => {
                if self.word("none") {
                    return Ok(Value::Opt(None));
                }
                if self.word("some") && self.eat(b'(') {
                    let v = self.value(t, nodes)?;
                    if !self.eat(b')') {
                        return Err("expected ')'".into());
                    }
                    return Ok(Value::Opt(Some(Rc::new(v))));
                }
                Err("expected none or some(..)".into())
            }
            Ty::LNode => {
                if self.word("null") {
                    return Ok(Value::Node(0));
                }
                if !self.eat(b'[') {
                    return Err("expected a list like [1, 2, 3]".into());
                }
                let vals = self.seq(b']', &Ty::Int, nodes)?;
                let mut handles = Vec::new();
                for v in &vals {
                    nodes.push(NodeData { val: v.as_int().unwrap_or(0), a: 0, b: 0 });
                    handles.push(nodes.len() as u32 - 1);
                }
                for w in handles.windows(2) {
                    nodes[w[0] as usize].a = w[1];
                }
                self.ws();
                if self.eat(b'@') {
                    let p = self.int()?;
                    if p < 0 || p as usize >= handles.len() {
                        return Err("cycle position out of range".into());
                    }
                    let last = handles[handles.len() - 1];
                    nodes[last as usize].a = handles[p as usize];
                }
                Ok(Value::Node(handles.first().copied().unwrap_or(0)))
            }
            Ty::TNode => {
                if self.word("null") {
                    return Ok(Value::Node(0));
                }
                if !self.eat(b'[') {
                    return Err("expected a tree like [1, 2, 3, null, 4]".into());
                }
                let mut items: Vec<Option<i64>> = Vec::new();
                if !self.eat(b']') {
                    loop {
                        if self.word("null") {
                            items.push(None);
                        } else {
                            items.push(Some(self.int()?));
                        }
                        if self.eat(b',') {
                            if self.eat(b']') {
                                break;
                            }
                            continue;
                        }
                        if self.eat(b']') {
                            break;
                        }
                        return Err("expected ',' or ']'".into());
                    }
                }
                Ok(Value::Node(build_tree(&items, nodes)))
            }
            other => Err(format!("cannot write a literal of type {other}")),
        }
    }
}

/// Build a binary tree from level-order values with `None` holes; returns the root handle (0 when empty).
pub fn build_tree(items: &[Option<i64>], nodes: &mut Vec<NodeData>) -> u32 {
    let Some(Some(root_v)) = items.first() else { return 0 };
    nodes.push(NodeData { val: *root_v, a: 0, b: 0 });
    let root = nodes.len() as u32 - 1;
    let mut q: VecDeque<u32> = VecDeque::new();
    q.push_back(root);
    let mut i = 1;
    while let Some(n) = q.pop_front() {
        for side in 0..2 {
            if i >= items.len() {
                return root;
            }
            if let Some(v) = items[i] {
                nodes.push(NodeData { val: v, a: 0, b: 0 });
                let h = nodes.len() as u32 - 1;
                if side == 0 {
                    nodes[n as usize].a = h;
                } else {
                    nodes[n as usize].b = h;
                }
                q.push_back(h);
            }
            i += 1;
        }
    }
    root
}

/// Parse the text form of a value of the given type. Linked structures are allocated in `nodes`
/// (index 0 is reserved for null: pass a vec that already starts with one dummy node).
pub fn parse_value(text: &str, ty: &Ty, nodes: &mut Vec<NodeData>) -> Result<Value, String> {
    if nodes.is_empty() {
        nodes.push(NodeData::default());
    }
    let mut l = Lit { s: text.as_bytes(), i: 0, depth: 0 };
    let v = l.value(ty, nodes)?;
    l.ws();
    if l.i != l.s.len() {
        return Err(format!("unexpected text after the value at position {}", l.i));
    }
    Ok(v)
}

/// Push onto a max-heap stored in a vec.
pub fn heap_push(h: &mut Vec<Value>, x: Value) {
    h.push(x);
    let mut i = h.len() - 1;
    while i > 0 {
        let p = (i - 1) / 2;
        if h[i] > h[p] {
            h.swap(i, p);
            i = p;
        } else {
            break;
        }
    }
}

/// Pop the largest element from a max-heap stored in a vec.
pub fn heap_pop(h: &mut Vec<Value>) -> Option<Value> {
    if h.is_empty() {
        return None;
    }
    let last = h.len() - 1;
    h.swap(0, last);
    let top = h.pop();
    let n = h.len();
    let mut i = 0;
    loop {
        let l = 2 * i + 1;
        let r = l + 1;
        let mut m = i;
        if l < n && h[l] > h[m] {
            m = l;
        }
        if r < n && h[r] > h[m] {
            m = r;
        }
        if m == i {
            break;
        }
        h.swap(i, m);
        i = m;
    }
    top
}

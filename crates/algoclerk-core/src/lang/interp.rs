// SPDX-License-Identifier: Apache-2.0
//! Deterministic, sandboxed tree-walking interpreter with the abstract cost model.
//!
//! Cost model (see `docs/LANGUAGE.md`): every statement costs 1, every operation costs 1, and
//! operations on collections cost what they cost in real code (vec index 1, vec `contains` n, map
//! lookup 1, sort n log n, string concatenation length, slice length, heap push log n, ...).

use super::ast::*;
use super::check::{TypeInfo, is_mutating};
use super::value::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;

/// Resource limits for one run.
#[derive(Clone, Debug)]
pub struct Limits {
    /// Abstract cost budget ("time limit").
    pub steps: u64,
    /// Maximum call depth.
    pub depth: u32,
    /// Maximum number of live elements ("shelf space").
    pub cells: i64,
    /// Sample memory use for the space estimate.
    pub measure: bool,
    /// Record up to this many trace events (0 disables tracing).
    pub trace_events: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits { steps: 5_000_000, depth: 200, cells: 4_000_000, measure: false, trace_events: 0 }
    }
}

/// What stopped a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RtKind {
    /// Cost budget exhausted.
    Time,
    /// Too many live elements.
    Memory,
    /// Too many nested calls.
    Depth,
    /// Division by zero.
    DivZero,
    /// Arithmetic overflow.
    Overflow,
    /// Index outside a shelf or word.
    Index,
    /// Bad slice bounds.
    Slice,
    /// Missing map key.
    Key,
    /// Pop or peek on an empty container.
    Empty,
    /// Unwrapping `none`.
    Unwrap,
    /// Touching a null node.
    Null,
    /// `to_int` on a non-number.
    BadInt,
    /// `chr` outside ASCII.
    BadChar,
    /// Shift count out of range.
    Shift,
    /// Invalid argument (range step 0, negative repeat count...).
    BadArg,
    /// Interpreter invariant broken (never expected after a successful check).
    Internal,
}

/// A runtime error, worded for the speech bubble.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RtError {
    /// Category.
    pub kind: RtKind,
    /// Node being executed.
    pub node: NodeId,
    /// Message for the clerk to say.
    pub msg: String,
}

impl std::fmt::Display for RtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

/// One step of the recorded execution (for the animated replay).
#[derive(Clone, Debug)]
pub struct Event {
    /// Statement that just ran (or is about to run its body).
    pub node: NodeId,
    /// Function index.
    pub func: u32,
    /// Call depth.
    pub depth: u32,
    /// Cumulative cost.
    pub cost: u64,
    /// All local slots of the frame (references resolved).
    pub vars: Vec<Value>,
    /// Value being returned, for `return` statements.
    pub returned: Option<Value>,
}

/// A recorded run.
#[derive(Clone, Debug, Default)]
pub struct Trace {
    /// Events in order.
    pub events: Vec<Event>,
    /// More events happened than were recorded.
    pub truncated: bool,
}

/// Everything a run produced.
#[derive(Clone, Debug)]
pub struct RunReport {
    /// Return value or the error.
    pub result: Result<Value, RtError>,
    /// Abstract cost spent.
    pub cost: u64,
    /// Peak sampled live cells (when `measure`), including the call stack.
    pub peak_cells: u64,
    /// Cells held by the inputs at the start.
    pub input_cells: u64,
    /// Recorded trace when requested.
    pub trace: Option<Trace>,
    /// Final node arena (for rendering list and tree values).
    pub nodes: Vec<NodeData>,
}

enum Flow {
    Next,
    Break,
    Continue,
    Return(Value),
}

enum Step {
    At(Value),
    Tup(usize),
}

type Fail = (RtKind, String);
type Res<T> = Result<T, Box<RtError>>;

struct Machine<'a> {
    prog: &'a Program,
    info: &'a TypeInfo,
    limits: &'a Limits,
    cost: u64,
    stack: Vec<Value>,
    base: usize,
    depth: u32,
    func: u32,
    nodes: Vec<NodeData>,
    live: i64,
    stmts: u64,
    peak: u64,
    trace: Option<Trace>,
}

fn rt_err(node: NodeId, f: Fail) -> Box<RtError> {
    Box::new(RtError { kind: f.0, node, msg: f.1 })
}

fn lg(n: u64) -> u64 {
    64 - n.leading_zeros() as u64
}

fn idx_fail(i: i64, len: usize) -> Fail {
    if len == 0 {
        (RtKind::Index, format!("Slot {i} does not exist: this shelf is empty!"))
    } else {
        (RtKind::Index, format!("Slot {i} does not exist: this shelf only has slots 0 to {}.", len - 1))
    }
}

fn step_mut<'v>(v: &'v mut Value, st: &Step) -> Result<&'v mut Value, Fail> {
    match (v, st) {
        (Value::Vec(rc), Step::At(Value::Int(i))) => {
            let vv = Rc::make_mut(rc);
            let len = vv.len();
            if *i < 0 || *i as usize >= len {
                Err(idx_fail(*i, len))
            } else {
                Ok(&mut vv[*i as usize])
            }
        }
        (Value::Map(rc), Step::At(k)) => {
            Rc::make_mut(rc).get_mut(k).ok_or_else(|| (RtKind::Key, "There is no drawer with that label!".to_string()))
        }
        (Value::Tuple(rc), Step::Tup(n)) => {
            let t = Rc::make_mut(rc);
            t.get_mut(*n).ok_or((RtKind::Internal, "bad tuple index".to_string()))
        }
        _ => Err((RtKind::Internal, "cannot step into this value".to_string())),
    }
}

fn walk<'v>(mut v: &'v mut Value, steps: &[Step]) -> Result<&'v mut Value, Fail> {
    for st in steps {
        v = step_mut(v, st)?;
    }
    Ok(v)
}

fn empty(what: &str) -> Fail {
    (RtKind::Empty, format!("The {what} is empty, so there is nothing to take!"))
}

impl<'a> Machine<'a> {
    fn fail(&self, node: NodeId, kind: RtKind, msg: impl Into<String>) -> Box<RtError> {
        Box::new(RtError { kind, node, msg: msg.into() })
    }
    fn at(&self, node: NodeId, f: Fail) -> Box<RtError> {
        self.fail(node, f.0, f.1)
    }
    fn charge(&mut self, node: NodeId, n: u64) -> Res<()> {
        self.cost = self.cost.saturating_add(n);
        if self.cost > self.limits.steps {
            return Err(self.fail(node, RtKind::Time, "The clock ran out! This is taking too long. Is there a faster way?"));
        }
        Ok(())
    }
    fn grow(&mut self, node: NodeId, by: i64) -> Res<()> {
        self.live = (self.live + by).max(0);
        if self.live > self.limits.cells {
            return Err(self.fail(node, RtKind::Memory, "The shelves are full! This uses far too much space."));
        }
        Ok(())
    }
    fn abs_slot(&self, slot: u32) -> usize {
        let mut i = self.base + slot as usize;
        while let Value::Ref(j) = &self.stack[i] {
            i = *j;
        }
        i
    }
    fn get_slot(&self, slot: u32) -> Value {
        self.stack[self.abs_slot(slot)].clone()
    }
    fn set_slot(&mut self, slot: u32, v: Value) {
        if slot == u32::MAX {
            return;
        }
        let i = self.abs_slot(slot);
        self.stack[i] = v;
    }
    fn alloc_node(&mut self, val: i64, a: u32, b: u32) -> Value {
        self.nodes.push(NodeData { val, a, b });
        Value::Node(self.nodes.len() as u32 - 1)
    }

    // ----- sampling and tracing -----
    fn sample(&mut self) {
        let mut cells: u64 = self.nodes.len() as u64 + self.depth as u64 * 2;
        for v in &self.stack {
            if !matches!(v, Value::Ref(_)) {
                cells += v.cells();
            }
        }
        self.peak = self.peak.max(cells);
    }
    fn tick(&mut self) {
        self.stmts += 1;
        if self.limits.measure && self.stmts % (1 + self.stmts / 128) == 0 {
            self.sample();
        }
    }
    fn record(&mut self, node: NodeId, returned: Option<Value>) {
        let Some(tr) = self.trace.as_mut() else { return };
        if tr.events.len() >= self.limits.trace_events {
            tr.truncated = true;
            return;
        }
        let n = self.info.fns[self.func as usize].nslots;
        let mut vars = Vec::with_capacity(n as usize);
        for s in 0..n {
            let mut i = self.base + s as usize;
            while let Value::Ref(j) = &self.stack[i] {
                i = *j;
            }
            vars.push(self.stack[i].clone());
        }
        let ev = Event { node, func: self.func, depth: self.depth, cost: self.cost, vars, returned };
        if let Some(tr) = self.trace.as_mut() {
            tr.events.push(ev);
        }
    }

    // ----- statements -----
    fn block(&mut self, b: &Block) -> Res<Flow> {
        for s in &b.stmts {
            match self.stmt(s)? {
                Flow::Next => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Next)
    }

    fn eval_owned(&mut self, e: &Expr) -> Res<Value> {
        let v = self.eval(e)?;
        if v.is_bulky() && matches!(e.kind, ExprKind::Var(_) | ExprKind::Index(..) | ExprKind::TupleIdx(..)) {
            self.charge(e.id, v.cells())?;
        }
        Ok(v)
    }

    fn stmt(&mut self, s: &Stmt) -> Res<Flow> {
        self.charge(s.id, 1)?;
        self.tick();
        match &s.kind {
            StmtKind::Let { pat, init, .. } => {
                let v = self.eval_owned(init)?;
                self.bind_pat(s.id, pat, v);
                self.record(s.id, None);
                Ok(Flow::Next)
            }
            StmtKind::Assign { target, op, value } => {
                let v = self.eval_owned(value)?;
                self.assign(target, *op, v)?;
                self.record(s.id, None);
                Ok(Flow::Next)
            }
            StmtKind::If { cond, then, els } => {
                let c = self.eval(cond)?;
                self.record(s.id, None);
                if c == Value::Bool(true) {
                    self.block(then)
                } else if let Some(e) = els {
                    self.block(e)
                } else {
                    Ok(Flow::Next)
                }
            }
            StmtKind::While { cond, body } => {
                loop {
                    let c = self.eval(cond)?;
                    if c != Value::Bool(true) {
                        break;
                    }
                    self.record(s.id, None);
                    match self.block(body)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Next | Flow::Continue => {}
                    }
                    self.charge(s.id, 1)?;
                    self.tick();
                }
                Ok(Flow::Next)
            }
            StmtKind::ForRange { start, end, step, body, .. } => {
                let a = match start {
                    Some(e) => self.eval(e)?.as_int().unwrap_or(0),
                    None => 0,
                };
                let b = self.eval(end)?.as_int().unwrap_or(0);
                let st = match step {
                    Some(e) => self.eval(e)?.as_int().unwrap_or(1),
                    None => 1,
                };
                if st == 0 {
                    return Err(self.fail(s.id, RtKind::BadArg, "A range cannot step by 0; I would never get anywhere!"));
                }
                let slot = self.info.slots[s.id as usize];
                let mut i = a;
                while (st > 0 && i < b) || (st < 0 && i > b) {
                    self.set_slot(slot, Value::Int(i));
                    self.record(s.id, None);
                    match self.block(body)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Next | Flow::Continue => {}
                    }
                    self.charge(s.id, 1)?;
                    self.tick();
                    i = match i.checked_add(st) {
                        Some(x) => x,
                        None => break,
                    };
                }
                Ok(Flow::Next)
            }
            StmtKind::ForEach { pat, iter, body } => {
                let items: Vec<Value> = match &iter.kind {
                    ExprKind::Call(n, args) if (n == "enumerate" || n == "zip") && self.info.slots[iter.id as usize] == u32::MAX => {
                        if n == "enumerate" {
                            let v = self.eval(&args[0])?;
                            let Value::Vec(v) = v else { return Err(self.fail(s.id, RtKind::Internal, "enumerate needs a shelf")) };
                            self.charge(iter.id, v.len() as u64 + 1)?;
                            v.iter().enumerate().map(|(i, x)| Value::tuple(vec![Value::Int(i as i64), x.clone()])).collect()
                        } else {
                            let a = self.eval(&args[0])?;
                            let b = self.eval(&args[1])?;
                            let (Value::Vec(a), Value::Vec(b)) = (a, b) else {
                                return Err(self.fail(s.id, RtKind::Internal, "zip needs two shelves"));
                            };
                            let n = a.len().min(b.len());
                            self.charge(iter.id, n as u64 + 1)?;
                            (0..n).map(|i| Value::tuple(vec![a[i].clone(), b[i].clone()])).collect()
                        }
                    }
                    _ => match self.eval(iter)? {
                        Value::Vec(v) => v.as_ref().clone(),
                        Value::Str(v) => v.iter().map(|c| Value::Char(*c)).collect(),
                        _ => return Err(self.fail(s.id, RtKind::Internal, "cannot walk this value")),
                    },
                };
                for item in items {
                    self.bind_pat(s.id, pat, item);
                    self.record(s.id, None);
                    match self.block(body)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Next | Flow::Continue => {}
                    }
                    self.charge(s.id, 1)?;
                    self.tick();
                }
                Ok(Flow::Next)
            }
            StmtKind::Break => Ok(Flow::Break),
            StmtKind::Continue => Ok(Flow::Continue),
            StmtKind::Return(e) => {
                let v = match e {
                    Some(e) if matches!(e.kind, ExprKind::Var(_)) => self.eval(e)?,
                    Some(e) => self.eval_owned(e)?,
                    None => Value::Unit,
                };
                self.record(s.id, Some(v.clone()));
                Ok(Flow::Return(v))
            }
            StmtKind::Expr(e) => {
                self.eval(e)?;
                self.record(s.id, None);
                Ok(Flow::Next)
            }
        }
    }

    fn bind_pat(&mut self, stmt: NodeId, pat: &Pat, v: Value) {
        let slots = self.info.pat_slots.get(&stmt).cloned().unwrap_or_default();
        let mut it = slots.into_iter();
        self.bind_into(pat, v, &mut it);
    }

    fn bind_into(&mut self, pat: &Pat, v: Value, slots: &mut std::vec::IntoIter<u32>) {
        match pat {
            Pat::Var(_) | Pat::Wild => {
                if let Some(s) = slots.next() {
                    self.set_slot(s, v);
                }
            }
            Pat::Tuple(ps) => {
                if let Value::Tuple(t) = v {
                    for (p, x) in ps.iter().zip(t.iter()) {
                        self.bind_into(p, x.clone(), slots);
                    }
                }
            }
        }
    }

    // ----- places -----
    fn place(&mut self, e: &Expr) -> Res<(usize, Vec<Step>)> {
        match &e.kind {
            ExprKind::Var(_) => Ok((self.abs_slot(self.info.slots[e.id as usize]), vec![])),
            ExprKind::Index(a, i) => {
                let (r, mut st) = self.place(a)?;
                let iv = self.eval(i)?;
                self.charge(e.id, 1)?;
                st.push(Step::At(iv));
                Ok((r, st))
            }
            ExprKind::TupleIdx(a, n) => {
                let (r, mut st) = self.place(a)?;
                st.push(Step::Tup(*n));
                Ok((r, st))
            }
            _ => Err(self.fail(e.id, RtKind::Internal, "not a place")),
        }
    }

    fn assign(&mut self, target: &Expr, op: Option<BinOp>, v: Value) -> Res<()> {
        let id = target.id;
        if let ExprKind::Field(a, f) = &target.kind {
            let h = match self.eval(a)? {
                Value::Node(h) if h != 0 => h as usize,
                _ => return Err(self.fail(id, RtKind::Null, "That node is missing (null), so I cannot change its fields!")),
            };
            self.charge(id, 1)?;
            let new = match op {
                None => v,
                Some(op) => {
                    let old = match f.as_str() {
                        "val" => Value::Int(self.nodes[h].val),
                        "next" | "left" => Value::Node(self.nodes[h].a),
                        _ => Value::Node(self.nodes[h].b),
                    };
                    self.binop(id, op, old, v)?
                }
            };
            match (f.as_str(), new) {
                ("val", Value::Int(x)) => self.nodes[h].val = x,
                ("next" | "left", Value::Node(x)) => self.nodes[h].a = x,
                ("right", Value::Node(x)) => self.nodes[h].b = x,
                _ => return Err(self.fail(id, RtKind::Internal, "bad field store")),
            }
            return Ok(());
        }
        let (root, steps) = self.place(target)?;
        if steps.is_empty() {
            let new = match op {
                None => v,
                Some(op) => {
                    let old = self.stack[root].clone();
                    self.binop(id, op, old, v)?
                }
            };
            self.stack[root] = new;
            return Ok(());
        }
        let (last, parents) = steps.split_last().map_or((None, &steps[..]), |(l, p)| (Some(l), p));
        let new = match op {
            None => v,
            Some(op) => {
                let old = {
                    let t = walk(&mut self.stack[root], &steps).map_err(|f| rt_err(id, f))?;
                    t.clone()
                };
                self.binop(id, op, old, v)?
            }
        };
        let parent = walk(&mut self.stack[root], parents).map_err(|f| rt_err(id, f))?;
        match (parent, last) {
            (Value::Map(rc), Some(Step::At(k))) => {
                let m = Rc::make_mut(rc);
                let fresh = !m.contains_key(k);
                m.insert(k.clone(), new);
                if fresh {
                    self.grow(id, 1)?;
                }
            }
            (p, Some(st)) => {
                let slot = step_mut(p, st).map_err(|f| rt_err(id, f))?;
                *slot = new;
            }
            _ => {}
        }
        Ok(())
    }

    // ----- expressions -----
    fn binop(&mut self, id: NodeId, op: BinOp, a: Value, b: Value) -> Res<Value> {
        use BinOp::*;
        self.charge(id, 1)?;
        let ov = |m: &Self| m.fail(id, RtKind::Overflow, "That number is too big for my pocket calculator!");
        Ok(match (op, a, b) {
            (Add, Value::Int(x), Value::Int(y)) => Value::Int(x.checked_add(y).ok_or_else(|| ov(self))?),
            (Sub, Value::Int(x), Value::Int(y)) => Value::Int(x.checked_sub(y).ok_or_else(|| ov(self))?),
            (Mul, Value::Int(x), Value::Int(y)) => Value::Int(x.checked_mul(y).ok_or_else(|| ov(self))?),
            (Div | Rem, Value::Int(_), Value::Int(0)) => {
                return Err(self.fail(id, RtKind::DivZero, "Dividing by zero? Even my calculator refuses!"));
            }
            (Div, Value::Int(x), Value::Int(y)) => Value::Int(x.checked_div(y).ok_or_else(|| ov(self))?),
            (Rem, Value::Int(x), Value::Int(y)) => Value::Int(x.checked_rem(y).ok_or_else(|| ov(self))?),
            (BitAnd, Value::Int(x), Value::Int(y)) => Value::Int(x & y),
            (BitOr, Value::Int(x), Value::Int(y)) => Value::Int(x | y),
            (BitXor, Value::Int(x), Value::Int(y)) => Value::Int(x ^ y),
            (Shl | Shr, Value::Int(x), Value::Int(y)) => {
                if !(0..64).contains(&y) {
                    return Err(self.fail(id, RtKind::Shift, "I can only shift by 0 to 63 places."));
                }
                if op == Shl { Value::Int(x.wrapping_shl(y as u32)) } else { Value::Int(x >> y) }
            }
            (Add, Value::Str(x), Value::Str(y)) => {
                self.charge(id, (x.len() + y.len()) as u64)?;
                let mut v = x.as_ref().clone();
                v.extend_from_slice(&y);
                Value::Str(Rc::new(v))
            }
            (Add, Value::Str(x), Value::Char(c)) => {
                self.charge(id, x.len() as u64 + 1)?;
                let mut v = x.as_ref().clone();
                v.push(c);
                Value::Str(Rc::new(v))
            }
            (Add, Value::Vec(x), Value::Vec(y)) => {
                self.charge(id, (x.len() + y.len()) as u64)?;
                let mut v = x.as_ref().clone();
                v.extend(y.iter().cloned());
                self.grow(id, y.len() as i64)?;
                Value::vec(v)
            }
            (Eq, x, y) => {
                if x.is_bulky() {
                    self.charge(id, x.shallow_len())?;
                }
                Value::Bool(x == y)
            }
            (Ne, x, y) => {
                if x.is_bulky() {
                    self.charge(id, x.shallow_len())?;
                }
                Value::Bool(x != y)
            }
            (Lt, x, y) => Value::Bool(x < y),
            (Le, x, y) => Value::Bool(x <= y),
            (Gt, x, y) => Value::Bool(x > y),
            (Ge, x, y) => Value::Bool(x >= y),
            _ => return Err(self.fail(id, RtKind::Internal, "operands do not fit the operator")),
        })
    }

    fn eval(&mut self, e: &Expr) -> Res<Value> {
        let id = e.id;
        match &e.kind {
            ExprKind::Int(n) => Ok(Value::Int(*n)),
            ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            ExprKind::Char(c) => Ok(Value::Char(*c)),
            ExprKind::Str(s) => Ok(Value::str(s)),
            ExprKind::Null => Ok(Value::Node(0)),
            ExprKind::NoneLit => Ok(Value::Opt(None)),
            ExprKind::Var(_) => Ok(self.get_slot(self.info.slots[id as usize])),
            ExprKind::Unary(op, a) => {
                let v = self.eval(a)?;
                self.charge(id, 1)?;
                match (op, v) {
                    (UnOp::Neg, Value::Int(x)) => x
                        .checked_neg()
                        .map(Value::Int)
                        .ok_or_else(|| self.fail(id, RtKind::Overflow, "That number is too big for my pocket calculator!")),
                    (UnOp::BitNot, Value::Int(x)) => Ok(Value::Int(!x)),
                    (UnOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                    _ => Err(self.fail(id, RtKind::Internal, "bad unary operand")),
                }
            }
            ExprKind::Binary(BinOp::And, a, b) => {
                self.charge(id, 1)?;
                if self.eval(a)? == Value::Bool(false) {
                    Ok(Value::Bool(false))
                } else {
                    self.eval(b)
                }
            }
            ExprKind::Binary(BinOp::Or, a, b) => {
                self.charge(id, 1)?;
                if self.eval(a)? == Value::Bool(true) {
                    Ok(Value::Bool(true))
                } else {
                    self.eval(b)
                }
            }
            ExprKind::Binary(op, a, b) => {
                let x = self.eval(a)?;
                let y = self.eval(b)?;
                self.binop(id, *op, x, y)
            }
            ExprKind::Index(a, i) => {
                let av = self.eval(a)?;
                let iv = self.eval(i)?;
                self.charge(id, 1)?;
                match (av, iv) {
                    (Value::Vec(v), Value::Int(i)) => {
                        v.get(usize::try_from(i).unwrap_or(usize::MAX)).cloned().ok_or_else(|| self.at(id, idx_fail(i, v.len())))
                    }
                    (Value::Str(v), Value::Int(i)) => v
                        .get(usize::try_from(i).unwrap_or(usize::MAX))
                        .map(|c| Value::Char(*c))
                        .ok_or_else(|| self.at(id, (RtKind::Index, format!("Letter {i} does not exist: this word only has {} letters.", v.len())))),
                    (Value::Map(m), k) => m
                        .get(&k)
                        .cloned()
                        .ok_or_else(|| self.fail(id, RtKind::Key, "There is no drawer with that label! Check with contains() first.")),
                    _ => Err(self.fail(id, RtKind::Internal, "bad index operands")),
                }
            }
            ExprKind::Slice(a, lo, hi) => {
                let av = self.eval(a)?;
                let len = av.shallow_len() as i64;
                let l = match lo {
                    Some(x) => self.eval(x)?.as_int().unwrap_or(0),
                    None => 0,
                };
                let h = match hi {
                    Some(x) => self.eval(x)?.as_int().unwrap_or(len),
                    None => len,
                };
                if l < 0 || h < l || h > len {
                    return Err(self.fail(id, RtKind::Slice, format!("I cannot cut from {l} to {h}: the length is only {len}.")));
                }
                self.charge(id, 1 + (h - l) as u64)?;
                let (l, h) = (l as usize, h as usize);
                match av {
                    Value::Vec(v) => Ok(Value::vec(v[l..h].to_vec())),
                    Value::Str(v) => Ok(Value::Str(Rc::new(v[l..h].to_vec()))),
                    _ => Err(self.fail(id, RtKind::Internal, "bad slice operand")),
                }
            }
            ExprKind::Field(a, f) => {
                let av = self.eval(a)?;
                self.charge(id, 1)?;
                match av {
                    Value::Node(h) if h != 0 => {
                        let n = self.nodes[h as usize];
                        Ok(match f.as_str() {
                            "val" => Value::Int(n.val),
                            "next" | "left" => Value::Node(n.a),
                            _ => Value::Node(n.b),
                        })
                    }
                    Value::Node(_) => Err(self.fail(id, RtKind::Null, "That node is missing (null), so it has nothing to read!")),
                    _ => Err(self.fail(id, RtKind::Internal, "bad field operand")),
                }
            }
            ExprKind::TupleIdx(a, n) => match self.eval(a)? {
                Value::Tuple(t) => Ok(t[*n].clone()),
                _ => Err(self.fail(id, RtKind::Internal, "bad tuple operand")),
            },
            ExprKind::VecLit(items) => {
                self.charge(id, 1 + items.len() as u64)?;
                let mut v = Vec::with_capacity(items.len());
                for it in items {
                    v.push(self.eval_owned(it)?);
                }
                self.grow(id, v.len() as i64)?;
                Ok(Value::vec(v))
            }
            ExprKind::VecRepeat(x, n) => {
                let xv = self.eval_owned(x)?;
                let n = self.eval(n)?.as_int().unwrap_or(0);
                if n < 0 {
                    return Err(self.fail(id, RtKind::BadArg, "I cannot build a shelf with a negative number of slots."));
                }
                self.grow(id, n.min(i64::from(u32::MAX)))?;
                self.charge(id, 1 + n as u64)?;
                Ok(Value::vec(vec![xv; n as usize]))
            }
            ExprKind::TupleLit(items) => {
                self.charge(id, 1)?;
                let mut v = Vec::with_capacity(items.len());
                for it in items {
                    v.push(self.eval_owned(it)?);
                }
                Ok(Value::tuple(v))
            }
            ExprKind::Ctor(t) => {
                self.charge(id, 1)?;
                Ok(match t {
                    Ty::Vec(_) => Value::vec(vec![]),
                    Ty::Set(_) => Value::Set(Rc::new(BTreeSet::new())),
                    Ty::Map(..) => Value::Map(Rc::new(BTreeMap::new())),
                    Ty::Stack(_) => Value::Stack(Rc::new(vec![])),
                    Ty::Queue(_) | Ty::Deque(_) => Value::Queue(Rc::new(VecDeque::new())),
                    Ty::Heap(_) => Value::Heap(Rc::new(vec![])),
                    Ty::Option(_) => Value::Opt(None),
                    _ => return Err(self.fail(id, RtKind::Internal, "cannot construct this type")),
                })
            }
            ExprKind::Call(name, args) => self.call(e, name, args),
            ExprKind::Method(recv, name, args) => self.method(e, recv, name, args),
        }
    }

    fn call(&mut self, e: &Expr, name: &str, args: &[Expr]) -> Res<Value> {
        let id = e.id;
        let callee = self.info.slots[id as usize];
        if callee != u32::MAX {
            return self.call_user(id, callee as usize, args);
        }
        let mut vals = Vec::with_capacity(args.len());
        for a in args {
            vals.push(self.eval_owned(a)?);
        }
        self.charge(id, 1)?;
        self.builtin_fn(id, name, vals)
    }

    fn call_user(&mut self, id: NodeId, idx: usize, args: &[Expr]) -> Res<Value> {
        let prog = self.prog;
        let info = self.info;
        let f = &prog.fns[idx];
        let finfo = &info.fns[idx];
        if self.depth >= self.limits.depth {
            return Err(self.fail(id, RtKind::Depth, "Too many notes-to-self in a row! My desk is buried in call slips."));
        }
        let mut vals = Vec::with_capacity(args.len());
        for (i, a) in args.iter().enumerate() {
            let p = &f.params[i];
            if p.by_ref {
                let ExprKind::Var(_) = a.kind else { return Err(self.fail(id, RtKind::Internal, "ref arg")) };
                vals.push(Value::Ref(self.abs_slot(self.info.slots[a.id as usize])));
            } else if finfo.mutated.get(i).copied().unwrap_or(false) {
                vals.push(self.eval_owned(a)?);
            } else {
                vals.push(self.eval(a)?);
            }
        }
        self.charge(id, 1)?;
        self.invoke(idx, vals, id)
    }

    fn invoke(&mut self, idx: usize, vals: Vec<Value>, id: NodeId) -> Res<Value> {
        let nslots = self.info.fns[idx].nslots as usize;
        let saved_base = self.base;
        let saved_func = self.func;
        let new_base = self.stack.len();
        self.stack.resize(new_base + nslots, Value::Unit);
        for (i, v) in vals.into_iter().enumerate() {
            self.stack[new_base + i] = v;
        }
        self.base = new_base;
        self.func = idx as u32;
        self.depth += 1;
        let prog = self.prog;
        let r = self.block(&prog.fns[idx].body);
        self.depth -= 1;
        self.stack.truncate(new_base);
        self.base = saved_base;
        self.func = saved_func;
        let _ = id;
        match r? {
            Flow::Return(v) => Ok(v),
            _ => Ok(Value::Unit),
        }
    }

    fn builtin_fn(&mut self, id: NodeId, name: &str, mut a: Vec<Value>) -> Res<Value> {
        let internal = |m: &Self| m.fail(id, RtKind::Internal, format!("bad arguments for {name}"));
        match (name, a.as_mut_slice()) {
            ("len", [v]) => Ok(Value::Int(v.shallow_len() as i64)),
            ("min" | "max", [x, y]) => {
                let pick_first = if name == "min" { *x <= *y } else { *x >= *y };
                Ok(if pick_first { x.clone() } else { y.clone() })
            }
            ("min" | "max", [Value::Vec(v)]) => {
                self.charge(id, v.len() as u64)?;
                let it = v.iter();
                let r = if name == "min" { it.min() } else { it.max() };
                r.cloned().ok_or_else(|| self.at(id, empty("shelf")))
            }
            ("abs", [Value::Int(x)]) => x
                .checked_abs()
                .map(Value::Int)
                .ok_or_else(|| self.fail(id, RtKind::Overflow, "That number is too big for my pocket calculator!")),
            ("sum", [Value::Vec(v)]) => {
                self.charge(id, v.len() as u64)?;
                let mut t: i64 = 0;
                for x in v.iter() {
                    t = t
                        .checked_add(x.as_int().unwrap_or(0))
                        .ok_or_else(|| self.fail(id, RtKind::Overflow, "That number is too big for my pocket calculator!"))?;
                }
                Ok(Value::Int(t))
            }
            ("sorted", [Value::Vec(v)]) => {
                self.charge(id, v.len() as u64 * lg(v.len() as u64).max(1))?;
                let mut c = v.as_ref().clone();
                c.sort();
                self.grow(id, c.len() as i64)?;
                Ok(Value::vec(c))
            }
            ("sorted", [Value::Set(s)]) => {
                self.charge(id, s.len() as u64 * lg(s.len() as u64).max(1))?;
                self.grow(id, s.len() as i64)?;
                Ok(Value::vec(s.iter().cloned().collect()))
            }
            ("sorted", [Value::Str(s)]) => {
                self.charge(id, s.len() as u64 * lg(s.len() as u64).max(1))?;
                let mut c = s.as_ref().clone();
                c.sort_unstable();
                Ok(Value::Str(Rc::new(c)))
            }
            ("reversed", [Value::Vec(v)]) => {
                self.charge(id, v.len() as u64)?;
                self.grow(id, v.len() as i64)?;
                Ok(Value::vec(v.iter().rev().cloned().collect()))
            }
            ("reversed", [Value::Str(s)]) => {
                self.charge(id, s.len() as u64)?;
                Ok(Value::Str(Rc::new(s.iter().rev().copied().collect())))
            }
            ("to_string", [Value::Int(x)]) => {
                let s = x.to_string();
                self.charge(id, s.len() as u64)?;
                Ok(Value::str(&s))
            }
            ("to_string", [Value::Char(c)]) => Ok(Value::Str(Rc::new(vec![*c]))),
            ("to_int", [Value::Str(s)]) => {
                self.charge(id, s.len() as u64)?;
                let t = String::from_utf8_lossy(s).into_owned();
                match t.parse::<i64>() {
                    Ok(n) if !t.starts_with('+') => Ok(Value::Int(n)),
                    _ => Err(self.fail(id, RtKind::BadInt, format!("\"{t}\" is not a number I can read."))),
                }
            }
            ("ord", [Value::Char(c)]) => Ok(Value::Int(i64::from(*c))),
            ("chr", [Value::Int(x)]) => {
                if (0..128).contains(x) {
                    Ok(Value::Char(*x as u8))
                } else {
                    Err(self.fail(id, RtKind::BadChar, "My typewriter only has keys 0 to 127."))
                }
            }
            ("join", [Value::Vec(v), Value::Str(sep)]) => {
                let mut out = Vec::new();
                for (i, s) in v.iter().enumerate() {
                    if i > 0 {
                        out.extend_from_slice(sep);
                    }
                    if let Value::Str(s) = s {
                        out.extend_from_slice(s);
                    }
                }
                self.charge(id, out.len() as u64 + 1)?;
                self.grow(id, out.len() as i64)?;
                Ok(Value::Str(Rc::new(out)))
            }
            ("from_chars", [Value::Vec(v)]) => {
                self.charge(id, v.len() as u64)?;
                Ok(Value::Str(Rc::new(v.iter().map(|c| if let Value::Char(c) = c { *c } else { b'?' }).collect())))
            }
            ("some", [v]) => Ok(Value::Opt(Some(Rc::new(v.clone())))),
            ("lnode", [Value::Int(v), Value::Node(n)]) => {
                self.grow(id, 1)?;
                Ok(self.alloc_node(*v, *n, 0))
            }
            ("tnode", [Value::Int(v), Value::Node(l), Value::Node(r)]) => {
                self.grow(id, 1)?;
                Ok(self.alloc_node(*v, *l, *r))
            }
            _ => Err(internal(self)),
        }
    }

    fn method(&mut self, e: &Expr, recv: &Expr, name: &str, args: &[Expr]) -> Res<Value> {
        let id = e.id;
        let rty = self.info.ty(recv.id).clone();
        let mut vals = Vec::with_capacity(args.len());
        let mutating = is_mutating(&rty, name);
        for a in args {
            vals.push(if mutating || matches!(rty, Ty::Option(_)) { self.eval_owned(a)? } else { self.eval(a)? });
        }
        self.charge(id, 1)?;
        if mutating {
            let (root, steps) = self.place(recv)?;
            let len_before = walk(&mut self.stack[root], &steps).map_err(|f| rt_err(id, f))?.shallow_len();
            let target = walk(&mut self.stack[root], &steps).map_err(|f| rt_err(id, f))?;
            let (v, cost, grow) = apply_mut(target, name, vals, len_before).map_err(|f| rt_err(id, f))?;
            self.charge(id, cost)?;
            self.grow(id, grow)?;
            return Ok(v);
        }
        let rv = self.eval(recv)?;
        let (v, cost, grow) = apply_ref(&rv, name, vals).map_err(|f| rt_err(id, f))?;
        self.charge(id, cost)?;
        self.grow(id, grow)?;
        Ok(v)
    }
}

/// Mutating method on a place. Returns (result, extra cost, live-cell growth).
fn apply_mut(t: &mut Value, name: &str, a: Vec<Value>, len: u64) -> Result<(Value, u64, i64), Fail> {
    let bad = || (RtKind::Internal, format!("bad arguments for {name}"));
    let mut a = a;
    let r = match (t, name) {
        (Value::Vec(rc), "push") => {
            Rc::make_mut(rc).push(a.pop().ok_or_else(bad)?);
            (Value::Unit, 0, 1)
        }
        (Value::Vec(rc), "pop") => (Rc::make_mut(rc).pop().ok_or_else(|| empty("shelf"))?, 0, -1),
        (Value::Vec(rc), "insert") => {
            let x = a.pop().ok_or_else(bad)?;
            let i = a.pop().and_then(|v| v.as_int()).ok_or_else(bad)?;
            let v = Rc::make_mut(rc);
            if i < 0 || i as usize > v.len() {
                return Err(idx_fail(i, v.len()));
            }
            let c = (v.len() - i as usize) as u64;
            v.insert(i as usize, x);
            (Value::Unit, c, 1)
        }
        (Value::Vec(rc), "remove") => {
            let i = a.pop().and_then(|v| v.as_int()).ok_or_else(bad)?;
            let v = Rc::make_mut(rc);
            if i < 0 || i as usize >= v.len() {
                return Err(idx_fail(i, v.len()));
            }
            let c = (v.len() - i as usize) as u64;
            (v.remove(i as usize), c, -1)
        }
        (Value::Vec(rc), "reverse") => {
            Rc::make_mut(rc).reverse();
            (Value::Unit, len, 0)
        }
        (Value::Vec(rc), "sort") => {
            Rc::make_mut(rc).sort();
            (Value::Unit, len * lg(len).max(1), 0)
        }
        (Value::Vec(rc), "swap") => {
            let j = a.pop().and_then(|v| v.as_int()).ok_or_else(bad)?;
            let i = a.pop().and_then(|v| v.as_int()).ok_or_else(bad)?;
            let v = Rc::make_mut(rc);
            for k in [i, j] {
                if k < 0 || k as usize >= v.len() {
                    return Err(idx_fail(k, v.len()));
                }
            }
            v.swap(i as usize, j as usize);
            (Value::Unit, 0, 0)
        }
        (Value::Vec(rc), "clear") => {
            Rc::make_mut(rc).clear();
            (Value::Unit, 0, -(len as i64))
        }
        (Value::Vec(rc), "extend") => {
            let Value::Vec(o) = a.pop().ok_or_else(bad)? else { return Err(bad()) };
            Rc::make_mut(rc).extend(o.iter().cloned());
            (Value::Unit, o.len() as u64, o.len() as i64)
        }
        (Value::Str(rc), "push") => {
            let Value::Char(c) = a.pop().ok_or_else(bad)? else { return Err(bad()) };
            Rc::make_mut(rc).push(c);
            (Value::Unit, 0, 1)
        }
        (Value::Set(rc), "add") => {
            let fresh = Rc::make_mut(rc).insert(a.pop().ok_or_else(bad)?);
            (Value::Unit, 0, i64::from(fresh))
        }
        (Value::Set(rc), "remove") => {
            let had = Rc::make_mut(rc).remove(&a.pop().ok_or_else(bad)?);
            (Value::Unit, 0, -i64::from(had))
        }
        (Value::Set(rc), "clear") => {
            Rc::make_mut(rc).clear();
            (Value::Unit, 0, -(len as i64))
        }
        (Value::Map(rc), "remove") => {
            let had = Rc::make_mut(rc).remove(&a.pop().ok_or_else(bad)?).is_some();
            (Value::Unit, 0, -i64::from(had))
        }
        (Value::Map(rc), "clear") => {
            Rc::make_mut(rc).clear();
            (Value::Unit, 0, -(len as i64))
        }
        (Value::Stack(rc), "push") => {
            Rc::make_mut(rc).push(a.pop().ok_or_else(bad)?);
            (Value::Unit, 0, 1)
        }
        (Value::Stack(rc), "pop") => (Rc::make_mut(rc).pop().ok_or_else(|| empty("stack"))?, 0, -1),
        (Value::Stack(rc), "clear") => {
            Rc::make_mut(rc).clear();
            (Value::Unit, 0, -(len as i64))
        }
        (Value::Queue(rc), "push" | "push_back") => {
            Rc::make_mut(rc).push_back(a.pop().ok_or_else(bad)?);
            (Value::Unit, 0, 1)
        }
        (Value::Queue(rc), "push_front") => {
            Rc::make_mut(rc).push_front(a.pop().ok_or_else(bad)?);
            (Value::Unit, 0, 1)
        }
        (Value::Queue(rc), "pop" | "pop_front") => (Rc::make_mut(rc).pop_front().ok_or_else(|| empty("queue"))?, 0, -1),
        (Value::Queue(rc), "pop_back") => (Rc::make_mut(rc).pop_back().ok_or_else(|| empty("queue"))?, 0, -1),
        (Value::Queue(rc), "clear") => {
            Rc::make_mut(rc).clear();
            (Value::Unit, 0, -(len as i64))
        }
        (Value::Heap(rc), "push") => {
            heap_push(Rc::make_mut(rc), a.pop().ok_or_else(bad)?);
            (Value::Unit, lg(len + 1), 1)
        }
        (Value::Heap(rc), "pop") => (heap_pop(Rc::make_mut(rc)).ok_or_else(|| empty("heap"))?, lg(len), -1),
        (Value::Heap(rc), "clear") => {
            Rc::make_mut(rc).clear();
            (Value::Unit, 0, -(len as i64))
        }
        _ => return Err(bad()),
    };
    Ok(r)
}

fn find_sub(h: &[u8], n: &[u8]) -> i64 {
    if n.is_empty() {
        return 0;
    }
    h.windows(n.len()).position(|w| w == n).map_or(-1, |p| p as i64)
}

fn needle(v: &Value) -> Vec<u8> {
    match v {
        Value::Char(c) => vec![*c],
        Value::Str(s) => s.as_ref().clone(),
        _ => vec![],
    }
}

/// Non-mutating method. Returns (result, extra cost, live-cell growth).
fn apply_ref(t: &Value, name: &str, a: Vec<Value>) -> Result<(Value, u64, i64), Fail> {
    let bad = || (RtKind::Internal, format!("bad arguments for {name}"));
    let b = |x: bool| Value::Bool(x);
    let r = match (t, name, a.as_slice()) {
        (Value::Vec(v), "contains", [x]) => {
            let p = v.iter().position(|y| y == x);
            (b(p.is_some()), p.map_or(v.len(), |p| p + 1) as u64, 0)
        }
        (Value::Vec(v), "index_of", [x]) => {
            let p = v.iter().position(|y| y == x);
            (Value::Int(p.map_or(-1, |p| p as i64)), p.map_or(v.len(), |p| p + 1) as u64, 0)
        }
        (Value::Vec(v), "count", [x]) => (Value::Int(v.iter().filter(|y| *y == x).count() as i64), v.len() as u64, 0),
        (Value::Vec(v), "is_empty", []) => (b(v.is_empty()), 0, 0),
        (Value::Str(s), "is_empty", []) => (b(s.is_empty()), 0, 0),
        (Value::Str(s), "upper", []) => (Value::Str(Rc::new(s.to_ascii_uppercase())), s.len() as u64, 0),
        (Value::Str(s), "lower", []) => (Value::Str(Rc::new(s.to_ascii_lowercase())), s.len() as u64, 0),
        (Value::Str(s), "trim", []) => {
            let st = s.iter().position(|c| !c.is_ascii_whitespace()).unwrap_or(s.len());
            let en = s.iter().rposition(|c| !c.is_ascii_whitespace()).map_or(st, |p| p + 1);
            (Value::Str(Rc::new(s[st..en.max(st)].to_vec())), s.len() as u64, 0)
        }
        (Value::Str(s), "find", [x]) => {
            let n = needle(x);
            (Value::Int(find_sub(s, &n)), s.len() as u64, 0)
        }
        (Value::Str(s), "contains", [x]) => {
            let n = needle(x);
            (b(find_sub(s, &n) >= 0), s.len() as u64, 0)
        }
        (Value::Str(s), "starts_with", [Value::Str(p)]) => (b(s.starts_with(p)), p.len() as u64, 0),
        (Value::Str(s), "ends_with", [Value::Str(p)]) => (b(s.ends_with(p)), p.len() as u64, 0),
        (Value::Str(s), "split", [x]) => {
            let n = needle(x);
            let mut parts: Vec<Value> = Vec::new();
            if n.is_empty() {
                parts.push(Value::Str(s.clone()));
            } else {
                let mut cur = 0;
                let mut i = 0;
                while i + n.len() <= s.len() {
                    if s[i..i + n.len()] == n[..] {
                        parts.push(Value::Str(Rc::new(s[cur..i].to_vec())));
                        i += n.len();
                        cur = i;
                    } else {
                        i += 1;
                    }
                }
                parts.push(Value::Str(Rc::new(s[cur..].to_vec())));
            }
            let g = parts.len() as i64;
            (Value::vec(parts), s.len() as u64, g)
        }
        (Value::Str(s), "chars", []) => (Value::vec(s.iter().map(|c| Value::Char(*c)).collect()), s.len() as u64, s.len() as i64),
        (Value::Str(s), "count", [Value::Char(c)]) => (Value::Int(s.iter().filter(|x| *x == c).count() as i64), s.len() as u64, 0),
        (Value::Str(s), "replace", [Value::Str(from), Value::Str(to)]) => {
            let mut out = Vec::new();
            let mut i = 0;
            while i < s.len() {
                if !from.is_empty() && s[i..].starts_with(from) {
                    out.extend_from_slice(to);
                    i += from.len();
                } else {
                    out.push(s[i]);
                    i += 1;
                }
            }
            (Value::Str(Rc::new(out)), s.len() as u64, 0)
        }
        (Value::Str(s), "repeat", [Value::Int(k)]) => {
            if *k < 0 || (*k as u64).saturating_mul(s.len() as u64) > 4_000_000 {
                return Err((RtKind::BadArg, "That would make a word far too long.".into()));
            }
            let out: Vec<u8> = s.iter().copied().cycle().take(s.len() * *k as usize).collect();
            let l = out.len() as u64;
            (Value::Str(Rc::new(out)), l, 0)
        }
        (Value::Char(c), "is_digit", []) => (b(c.is_ascii_digit()), 0, 0),
        (Value::Char(c), "is_alpha", []) => (b(c.is_ascii_alphabetic()), 0, 0),
        (Value::Char(c), "is_alnum", []) => (b(c.is_ascii_alphanumeric()), 0, 0),
        (Value::Char(c), "is_upper", []) => (b(c.is_ascii_uppercase()), 0, 0),
        (Value::Char(c), "is_lower", []) => (b(c.is_ascii_lowercase()), 0, 0),
        (Value::Char(c), "is_space", []) => (b(c.is_ascii_whitespace()), 0, 0),
        (Value::Char(c), "upper", []) => (Value::Char(c.to_ascii_uppercase()), 0, 0),
        (Value::Char(c), "lower", []) => (Value::Char(c.to_ascii_lowercase()), 0, 0),
        (Value::Set(s), "contains", [x]) => (b(s.contains(x)), 0, 0),
        (Value::Set(s), "is_empty", []) => (b(s.is_empty()), 0, 0),
        (Value::Set(s), "union", [Value::Set(o)]) => {
            let r: BTreeSet<Value> = s.union(o).cloned().collect();
            let g = r.len() as i64;
            (Value::Set(Rc::new(r)), (s.len() + o.len()) as u64, g)
        }
        (Value::Set(s), "intersection", [Value::Set(o)]) => {
            let r: BTreeSet<Value> = s.intersection(o).cloned().collect();
            let g = r.len() as i64;
            (Value::Set(Rc::new(r)), (s.len() + o.len()) as u64, g)
        }
        (Value::Set(s), "difference", [Value::Set(o)]) => {
            let r: BTreeSet<Value> = s.difference(o).cloned().collect();
            let g = r.len() as i64;
            (Value::Set(Rc::new(r)), (s.len() + o.len()) as u64, g)
        }
        (Value::Map(m), "get", [k, d]) => (m.get(k).cloned().unwrap_or_else(|| d.clone()), 0, 0),
        (Value::Map(m), "get", [k]) => (Value::Opt(m.get(k).map(|v| Rc::new(v.clone()))), 0, 0),
        (Value::Map(m), "contains", [k]) => (b(m.contains_key(k)), 0, 0),
        (Value::Map(m), "keys", []) => (Value::vec(m.keys().cloned().collect()), m.len() as u64, m.len() as i64),
        (Value::Map(m), "values", []) => (Value::vec(m.values().cloned().collect()), m.len() as u64, m.len() as i64),
        (Value::Map(m), "items", []) => (
            Value::vec(m.iter().map(|(k, v)| Value::tuple(vec![k.clone(), v.clone()])).collect()),
            m.len() as u64,
            m.len() as i64,
        ),
        (Value::Map(m), "is_empty", []) => (b(m.is_empty()), 0, 0),
        (Value::Opt(o), "is_some", []) => (b(o.is_some()), 0, 0),
        (Value::Opt(o), "is_none", []) => (b(o.is_none()), 0, 0),
        (Value::Opt(o), "unwrap", []) => match o {
            Some(v) => ((**v).clone(), 0, 0),
            None => return Err((RtKind::Unwrap, "I opened the envelope and it was empty (none)!".into())),
        },
        (Value::Opt(o), "unwrap_or", [d]) => (o.as_ref().map_or_else(|| d.clone(), |v| (**v).clone()), 0, 0),
        (Value::Stack(v), "peek", []) => (v.last().cloned().ok_or_else(|| empty("stack"))?, 0, 0),
        (Value::Stack(v), "is_empty", []) => (b(v.is_empty()), 0, 0),
        (Value::Queue(v), "front", []) => (v.front().cloned().ok_or_else(|| empty("queue"))?, 0, 0),
        (Value::Queue(v), "back", []) => (v.back().cloned().ok_or_else(|| empty("queue"))?, 0, 0),
        (Value::Queue(v), "is_empty", []) => (b(v.is_empty()), 0, 0),
        (Value::Heap(v), "peek", []) => (v.first().cloned().ok_or_else(|| empty("heap"))?, 0, 0),
        (Value::Heap(v), "is_empty", []) => (b(v.is_empty()), 0, 0),
        _ => return Err(bad()),
    };
    Ok(r)
}

/// Run `entry` with the given argument values. Never panics.
pub fn run(prog: &Program, info: &TypeInfo, entry: &str, args: Vec<Value>, nodes: Vec<NodeData>, limits: &Limits) -> RunReport {
    let mut nodes = nodes;
    if nodes.is_empty() {
        nodes.push(NodeData::default());
    }
    let Some(idx) = prog.fns.iter().position(|f| f.name == entry) else {
        return RunReport {
            result: Err(RtError { kind: RtKind::Internal, node: 0, msg: format!("There is no function called {entry}.") }),
            cost: 0,
            peak_cells: 0,
            input_cells: 0,
            trace: None,
            nodes,
        };
    };
    let input_cells: u64 = args.iter().map(Value::cells).sum::<u64>() + nodes.len() as u64;
    let live0 = args.iter().map(Value::cells).sum::<u64>() as i64;
    let mut m = Machine {
        prog,
        info,
        limits,
        cost: 0,
        stack: Vec::new(),
        base: 0,
        depth: 0,
        func: idx as u32,
        nodes,
        live: live0,
        stmts: 0,
        peak: input_cells,
        trace: (limits.trace_events > 0).then(Trace::default),
    };
    let result = m.invoke(idx, args, 0).map_err(|e| *e);
    if limits.measure {
        m.sample();
    }
    RunReport { result, cost: m.cost, peak_cells: m.peak, input_cells, trace: m.trace, nodes: m.nodes }
}

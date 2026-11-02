// SPDX-License-Identifier: Apache-2.0
//! Interpreter for the C++17 subset with the abstract cost model and undefined behaviour detection.
//!
//! The interpreter is dynamically typed with exact static type tags on every value (see [`value`]); it does
//! not need the checker. Cost model: see `docs/COST_MODEL.md`.

use super::ast::*;
use super::value::{Heap, Plain};

/// Iteration order of unordered containers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnorderedOrder {
    /// Insertion order.
    Insertion,
    /// Reverse insertion order.
    Reverse,
}

/// Resource limits for one run.
#[derive(Clone, Debug)]
pub struct Limits {
    /// Abstract cost budget.
    pub steps: u64,
    /// Maximum call depth.
    pub depth: u32,
    /// Maximum live elements.
    pub cells: i64,
    /// Sample memory for the space estimate.
    pub measure: bool,
    /// Record up to this many trace events.
    pub trace_events: usize,
    /// Order of unordered containers.
    pub unordered: UnorderedOrder,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            steps: 5_000_000,
            depth: 200,
            cells: 4_000_000,
            measure: false,
            trace_events: 0,
            unordered: UnorderedOrder::Insertion,
        }
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
    /// Signed overflow.
    Overflow,
    /// Index out of range.
    Index,
    /// Bad shift.
    Shift,
    /// Null dereference.
    Null,
    /// Dangling pointer or use after delete.
    Dangling,
    /// Uninitialised read.
    Uninit,
    /// Iterator or reference invalidated.
    Invalidated,
    /// Fell off the end of a non-void function.
    MissingReturn,
    /// Empty container access.
    Empty,
    /// Invalid comparator.
    BadCompare,
    /// Other invalid argument.
    BadArg,
    /// Interpreter invariant broken.
    Internal,
}

/// A runtime error worded for the speech bubble.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RtError {
    /// Category.
    pub kind: RtKind,
    /// Statement or expression being executed.
    pub node: NodeId,
    /// Message.
    pub msg: String,
}

/// A variable in a trace snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct VarSnap {
    /// Name.
    pub name: String,
    /// Declared type.
    pub ty: Ty,
    /// Value.
    pub value: Plain,
}

/// One recorded step.
#[derive(Clone, Debug)]
pub struct Event {
    /// Statement that ran.
    pub node: NodeId,
    /// Function index.
    pub func: u32,
    /// Call depth.
    pub depth: u32,
    /// Cumulative cost.
    pub cost: u64,
    /// Visible variables.
    pub vars: Vec<VarSnap>,
    /// Returned value for `return`.
    pub returned: Option<Plain>,
    /// Struct instances reachable from the variables (for drawing lists and trees).
    pub heap: Heap,
}

/// A recorded run.
#[derive(Clone, Debug, Default)]
pub struct Trace {
    /// Events.
    pub events: Vec<Event>,
    /// More happened than was recorded.
    pub truncated: bool,
}

/// Everything a run produced.
#[derive(Clone, Debug)]
pub struct RunReport {
    /// Result or error.
    pub result: Result<Plain, RtError>,
    /// Abstract cost.
    pub cost: u64,
    /// Peak live cells.
    pub peak_cells: u64,
    /// Cells of the inputs.
    pub input_cells: u64,
    /// Trace if requested.
    pub trace: Option<Trace>,
    /// Heap the result's pointers refer to.
    pub heap: Heap,
}

mod algo;
mod assoc;
mod conv;
mod expr;
mod iter;
mod methods;
mod ops;
mod plain;
mod resolve;
mod rval;
mod seqs;

use ops::Fail;
use resolve::{Resolved, VarRef};
use rval::*;
use std::collections::HashMap;
use std::rc::Rc;

impl std::fmt::Display for RtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

type R<T> = Result<T, Box<RtError>>;

/// Result of evaluating an expression: a value, or the object it designates.
#[derive(Debug)]
enum Rv {
    Val(Value),
    Place(Place),
}

enum Flow {
    Next,
    Break,
    Continue,
    Ret,
}

struct Slot {
    gn: u32,
    v: Value,
}

struct HObj {
    gn: u32,
    v: Option<Value>,
}

enum Loc<'a> {
    V(&'a Value),
    B(u8),
}

enum LocM<'a> {
    V(&'a mut Value),
    B(&'a mut u8),
}

struct TraceBuf {
    events: Vec<Event>,
    truncated: bool,
    cells_left: i64,
}

const BIG_STACK: usize = 192 << 20;
const STACK_MARGIN: usize = 4 << 20;

fn invalid_msg() -> Fail {
    (
        RtKind::Invalidated,
        "that iterator or reference points into a container that was modified since (push_back, insert, \
         erase, clear, resize ...): it may have moved or died"
            .into(),
    )
}

fn dangling_msg() -> Fail {
    (
        RtKind::Dangling,
        "that pointer or reference refers to a variable that no longer exists (or to a deleted object)"
            .into(),
    )
}

fn index_msg(i: usize, len: usize) -> Fail {
    let what = if len == 0 {
        "the container is empty".to_string()
    } else {
        format!("valid indexes are 0 to {}", len - 1)
    };
    (
        RtKind::Index,
        format!("index {i} is out of range: {what} (a container of size {len})"),
    )
}

fn step_ref<'a>(cur: Loc<'a>, s: &Step) -> Result<Loc<'a>, Fail> {
    let Loc::V(v) = cur else {
        return Err((RtKind::Internal, "step into a character".into()));
    };
    match s {
        Step::Idx(i, ver) => match v {
            Value::Vector(c) => {
                if c.ver != *ver {
                    return Err(invalid_msg());
                }
                c.v.get(*i)
                    .map(Loc::V)
                    .ok_or_else(|| index_msg(*i, c.v.len()))
            }
            Value::Deque(c) => {
                if c.ver != *ver {
                    return Err(invalid_msg());
                }
                c.v.get(*i)
                    .map(Loc::V)
                    .ok_or_else(|| index_msg(*i, c.v.len()))
            }
            Value::Pq(c) => {
                if c.ver != *ver {
                    return Err(invalid_msg());
                }
                c.v.get(*i)
                    .map(Loc::V)
                    .ok_or_else(|| index_msg(*i, c.v.len()))
            }
            Value::Str(c) => {
                if c.ver != *ver {
                    return Err(invalid_msg());
                }
                c.s.get(*i)
                    .map(|b| Loc::B(*b))
                    .ok_or_else(|| index_msg(*i, c.s.len()))
            }
            Value::View(b) => b
                .get(*i)
                .map(|b| Loc::B(*b))
                .ok_or_else(|| index_msg(*i, b.len())),
            _ => Err((RtKind::Internal, "indexing a non-container".into())),
        },
        Step::Key(k) => match v {
            Value::Map(m) => m.m.get(k).map(Loc::V).ok_or_else(invalid_msg),
            Value::Set(m) => {
                m.m.get_key_value(k)
                    .map(|(k, _)| Loc::V(&k.0))
                    .ok_or_else(invalid_msg)
            }
            Value::UMap(u) | Value::USet(u) => u
                .idx
                .get(k)
                .and_then(|&i| u.ents.get(i))
                .and_then(Option::as_ref)
                .map(Loc::V)
                .ok_or_else(invalid_msg),
            _ => Err((RtKind::Internal, "key step on a non-map".into())),
        },
        Step::Field(n) => {
            let n = *n as usize;
            match v {
                Value::Pair(p) => p.get(n).map(Loc::V),
                Value::Tuple(t) => t.get(n).map(Loc::V),
                Value::Struct(s) => s.f.get(n).map(Loc::V),
                _ => None,
            }
            .ok_or_else(|| (RtKind::Internal, "no such field".into()))
        }
        Step::Opt => match v {
            Value::Opt(o) => o.v.as_ref().map(Loc::V).ok_or_else(empty_opt),
            _ => Err((RtKind::Internal, "not an optional".into())),
        },
        Step::Node(id, gn) => match v {
            Value::List(l) => {
                if !l.live(*id, *gn) {
                    return Err(invalid_msg());
                }
                l.nodes[*id as usize]
                    .val
                    .as_ref()
                    .map(Loc::V)
                    .ok_or_else(invalid_msg)
            }
            _ => Err((RtKind::Internal, "not a list".into())),
        },
    }
}

fn empty_opt() -> Fail {
    (
        RtKind::Empty,
        "this std::optional is empty: there is no value to read (value() would throw)".into(),
    )
}

fn step_mut<'a>(cur: LocM<'a>, s: &Step) -> Result<LocM<'a>, Fail> {
    let LocM::V(v) = cur else {
        return Err((RtKind::Internal, "step into a character".into()));
    };
    match s {
        Step::Idx(i, ver) => match v {
            Value::Vector(c) => {
                if c.ver != *ver {
                    return Err(invalid_msg());
                }
                let n = c.v.len();
                c.v.get_mut(*i).map(LocM::V).ok_or_else(|| index_msg(*i, n))
            }
            Value::Deque(c) => {
                if c.ver != *ver {
                    return Err(invalid_msg());
                }
                let n = c.v.len();
                c.v.get_mut(*i).map(LocM::V).ok_or_else(|| index_msg(*i, n))
            }
            Value::Pq(c) => {
                if c.ver != *ver {
                    return Err(invalid_msg());
                }
                let n = c.v.len();
                c.v.get_mut(*i).map(LocM::V).ok_or_else(|| index_msg(*i, n))
            }
            Value::Str(c) => {
                if c.ver != *ver {
                    return Err(invalid_msg());
                }
                let n = c.s.len();
                c.s.get_mut(*i).map(LocM::B).ok_or_else(|| index_msg(*i, n))
            }
            _ => Err((RtKind::Internal, "indexing a non-container".into())),
        },
        Step::Key(k) => match v {
            Value::Map(m) => m.m.get_mut(k).map(LocM::V).ok_or_else(invalid_msg),
            Value::UMap(u) => match u.idx.get(k).copied() {
                Some(i) => u
                    .ents
                    .get_mut(i)
                    .and_then(Option::as_mut)
                    .map(LocM::V)
                    .ok_or_else(invalid_msg),
                None => Err(invalid_msg()),
            },
            Value::Set(_) | Value::USet(_) => Err((
                RtKind::Internal,
                "the elements of a set cannot be modified".into(),
            )),
            _ => Err((RtKind::Internal, "key step on a non-map".into())),
        },
        Step::Field(n) => {
            let n = *n as usize;
            match v {
                Value::Pair(p) => p.get_mut(n).map(LocM::V),
                Value::Tuple(t) => t.get_mut(n).map(LocM::V),
                Value::Struct(s) => s.f.get_mut(n).map(LocM::V),
                _ => None,
            }
            .ok_or_else(|| (RtKind::Internal, "no such field".into()))
        }
        Step::Opt => match v {
            Value::Opt(o) => o.v.as_mut().map(LocM::V).ok_or_else(empty_opt),
            _ => Err((RtKind::Internal, "not an optional".into())),
        },
        Step::Node(id, gn) => match v {
            Value::List(l) => {
                if !l.live(*id, *gn) {
                    return Err(invalid_msg());
                }
                l.nodes[*id as usize]
                    .val
                    .as_mut()
                    .map(LocM::V)
                    .ok_or_else(invalid_msg)
            }
            _ => Err((RtKind::Internal, "not a list".into())),
        },
    }
}

pub(crate) struct Machine<'p> {
    prog: &'p Program,
    lim: &'p Limits,
    rs: Resolved<'p>,
    cost: u64,
    trip: u64,
    next_sample: u64,
    peak: u64,
    live_hint: u64,
    stack: Vec<Slot>,
    base: usize,
    cap_off: usize,
    temps: Vec<Slot>,
    globals: Vec<Slot>,
    heap: Vec<HObj>,
    heap_free: Vec<u32>,
    gn: u32,
    cur: NodeId,
    depth: u32,
    ret: Option<Rv>,
    ret_spec: Option<&'p TypeSpec>,
    struct_ix: HashMap<&'p str, u32>,
    stack_top: usize,
    stack_budget: usize,
    func: u32,
    trace: Option<TraceBuf>,
    vis: Vec<(&'p str, u32)>,
    vis_base: usize,
    input_cells: u64,
}

fn addr_here() -> usize {
    let m = 0u8;
    std::hint::black_box(&m) as *const u8 as usize
}

impl<'p> Machine<'p> {
    fn new(p: &'p Program, lim: &'p Limits, budget: usize) -> Machine<'p> {
        let rs = resolve::resolve(p);
        let mut struct_ix = HashMap::new();
        for (i, s) in rs.structs.iter().enumerate() {
            struct_ix.insert(s.decl.name.as_str(), i as u32);
        }
        let trip = lim.steps.saturating_add(1).min(512);
        Machine {
            prog: p,
            lim,
            rs,
            cost: 0,
            trip,
            next_sample: 512,
            peak: 0,
            live_hint: 0,
            stack: Vec::new(),
            base: 0,
            cap_off: 0,
            temps: Vec::new(),
            globals: Vec::new(),
            heap: Vec::new(),
            heap_free: Vec::new(),
            gn: 1,
            cur: 0,
            depth: 0,
            ret: None,
            ret_spec: None,
            struct_ix,
            stack_top: addr_here(),
            stack_budget: budget,
            func: 0,
            trace: (lim.trace_events > 0).then(|| TraceBuf {
                events: Vec::new(),
                truncated: false,
                cells_left: 3_000_000,
            }),
            vis: Vec::new(),
            vis_base: 0,
            input_cells: 0,
        }
    }

    // ----- errors, cost, memory -----

    fn err(&self, kind: RtKind, msg: impl Into<String>) -> Box<RtError> {
        Box::new(RtError {
            kind,
            node: self.cur,
            msg: msg.into(),
        })
    }
    fn fail(&self, f: Fail) -> Box<RtError> {
        self.err(f.0, f.1)
    }
    fn internal(&self, msg: &str) -> Box<RtError> {
        self.err(RtKind::Internal, msg)
    }
    #[inline]
    fn charge(&mut self, n: u64) -> R<()> {
        self.cost = self.cost.saturating_add(n);
        if self.cost >= self.trip {
            return self.slow_charge();
        }
        Ok(())
    }
    #[cold]
    fn slow_charge(&mut self) -> R<()> {
        if self.cost > self.lim.steps {
            return Err(self.err(
                RtKind::Time,
                "the clock ran out: this program takes too long (an endless loop, or an approach that is too slow)",
            ));
        }
        if self.cost >= self.next_sample {
            self.sample()?;
        }
        self.trip = self.lim.steps.saturating_add(1).min(self.next_sample);
        Ok(())
    }
    fn live_cells(&self) -> u64 {
        let mut n = 0u64;
        for s in &self.stack {
            if s.gn != 0 {
                n += cells(&s.v);
            }
        }
        for s in &self.temps {
            n += cells(&s.v);
        }
        for s in &self.globals {
            n += cells(&s.v);
        }
        for h in &self.heap {
            if let Some(v) = &h.v {
                n += cells(v);
            }
        }
        n + u64::from(self.depth) * 2
    }
    fn sample(&mut self) -> R<()> {
        let n = self.live_cells();
        self.live_hint = n;
        self.peak = self.peak.max(n);
        let gap = if self.lim.measure {
            (n / 32).max(128)
        } else {
            (n / 4).max(4096)
        };
        self.next_sample = self.cost.saturating_add(gap);
        if n as i64 > self.lim.cells {
            return Err(self.err(
                RtKind::Memory,
                "the shelves are full: this uses far too much memory",
            ));
        }
        self.trip = self.lim.steps.saturating_add(1).min(self.next_sample);
        Ok(())
    }
    /// Check a requested allocation of `n` more elements.
    fn reserve(&mut self, n: u64) -> R<()> {
        if n.saturating_add(self.live_hint) > self.lim.cells as u64 {
            return Err(self.err(
                RtKind::Memory,
                format!("asking for {n} more elements is too much: the shelves would overflow"),
            ));
        }
        Ok(())
    }
    #[inline]
    fn deep(&self) -> bool {
        self.stack_top.abs_diff(addr_here()) > self.stack_budget
    }
    fn too_deep(&self) -> Box<RtError> {
        self.err(
            RtKind::Depth,
            "too many nested calls or too deeply nested code: this recursion never stops, or goes too deep",
        )
    }

    // ----- slots and places -----

    fn new_gen(&mut self) -> u32 {
        self.gn = self.gn.wrapping_add(1).max(1);
        self.gn
    }
    fn temp(&mut self, v: Value) -> Place {
        let gn = self.new_gen();
        self.temps.push(Slot { gn, v });
        Place::root(Root::Temp((self.temps.len() - 1) as u32, gn))
    }
    /// The place a slot designates (following reference slots).
    fn slot_place(&self, abs: usize) -> R<Place> {
        match self.stack.get(abs) {
            Some(Slot { gn, v }) if *gn != 0 => match v {
                Value::Ref(q) => Ok((**q).clone()),
                _ => Ok(Place::root(Root::Local(abs as u32, *gn))),
            },
            _ => Err(self.err(
                RtKind::Uninit,
                "this variable was never declared on this path (a jump skipped its declaration)",
            )),
        }
    }
    fn var_place(&self, vr: VarRef) -> R<Place> {
        match vr {
            VarRef::Local(s) => self.slot_place(self.base + s as usize),
            VarRef::Capture(i) => self.slot_place(self.base + self.cap_off + i as usize),
            VarRef::Global(g) => Ok(Place::root(Root::Global(g))),
            VarRef::Field(i) => Ok(self.slot_place(self.base)?.with(Step::Field(i as u16))),
            _ => Err(self.internal("name is not a variable")),
        }
    }
    fn declare(&mut self, slot: u32, v: Value) -> u32 {
        let gn = self.new_gen();
        let i = self.base + slot as usize;
        if let Some(s) = self.stack.get_mut(i) {
            *s = Slot { gn, v };
        }
        gn
    }
    fn root_ref(&self, r: &Root) -> Result<&Value, Fail> {
        match *r {
            Root::Local(i, g) => match self.stack.get(i as usize) {
                Some(s) if s.gn == g => Ok(&s.v),
                _ => Err(dangling_msg()),
            },
            Root::Temp(i, g) => match self.temps.get(i as usize) {
                Some(s) if s.gn == g => Ok(&s.v),
                _ => Err(dangling_msg()),
            },
            Root::Global(i) => self
                .globals
                .get(i as usize)
                .map(|s| &s.v)
                .ok_or_else(dangling_msg),
            Root::Heap(i, g) => match self.heap.get(i as usize) {
                Some(h) if h.gn == g => h.v.as_ref().ok_or_else(dangling_msg),
                _ => Err(dangling_msg()),
            },
        }
    }
    fn root_mut(&mut self, r: &Root) -> Result<&mut Value, Fail> {
        match *r {
            Root::Local(i, g) => match self.stack.get_mut(i as usize) {
                Some(s) if s.gn == g => Ok(&mut s.v),
                _ => Err(dangling_msg()),
            },
            Root::Temp(i, g) => match self.temps.get_mut(i as usize) {
                Some(s) if s.gn == g => Ok(&mut s.v),
                _ => Err(dangling_msg()),
            },
            Root::Global(i) => self
                .globals
                .get_mut(i as usize)
                .map(|s| &mut s.v)
                .ok_or_else(dangling_msg),
            Root::Heap(i, g) => match self.heap.get_mut(i as usize) {
                Some(h) if h.gn == g => h.v.as_mut().ok_or_else(dangling_msg),
                _ => Err(dangling_msg()),
            },
        }
    }
    fn loc(&self, p: &Place) -> Result<Loc<'_>, Fail> {
        let mut cur = Loc::V(self.root_ref(&p.root)?);
        for s in &p.path {
            cur = step_ref(cur, s)?;
        }
        Ok(cur)
    }
    fn loc_mut(&mut self, p: &Place) -> Result<LocM<'_>, Fail> {
        let mut cur = LocM::V(self.root_mut(&p.root)?);
        for s in &p.path {
            cur = step_mut(cur, s)?;
        }
        Ok(cur)
    }
    /// Borrow the value at a place.
    fn get(&self, p: &Place) -> Result<&Value, Fail> {
        match self.loc(p)? {
            Loc::V(v) => Ok(v),
            Loc::B(_) => Err((
                RtKind::Internal,
                "a character inside a string cannot be borrowed".into(),
            )),
        }
    }
    fn get_mut(&mut self, p: &Place) -> R<&mut Value> {
        let cur = self.cur;
        let mk = move |f: Fail| {
            Box::new(RtError {
                kind: f.0,
                node: cur,
                msg: f.1,
            })
        };
        match self.loc_mut(p).map_err(mk)? {
            LocM::V(v) => Ok(v),
            LocM::B(_) => Err(Box::new(RtError {
                kind: RtKind::Internal,
                node: cur,
                msg: "a character inside a string cannot be borrowed".into(),
            })),
        }
    }
    fn loc_mut_r(&mut self, p: &Place) -> R<LocM<'_>> {
        let cur = self.cur;
        self.loc_mut(p).map_err(move |f| {
            Box::new(RtError {
                kind: f.0,
                node: cur,
                msg: f.1,
            })
        })
    }
    /// Clone the value at a place (characters of strings become `char`).
    fn load(&self, p: &Place) -> Result<Value, Fail> {
        match self.loc(p)? {
            Loc::V(v) => Ok(v.clone()),
            Loc::B(b) => Ok(Value::Char(b as i8)),
        }
    }
    /// Overwrite the value at a place (bumping the container version when a container is replaced).
    fn store(&mut self, p: &Place, mut v: Value) -> Result<(), Fail> {
        match self.loc_mut(p)? {
            LocM::V(slot) => {
                if let (Some(old), true) = (slot.ver(), v.ver().is_some()) {
                    v.set_ver(old.wrapping_add(1));
                }
                *slot = v;
                Ok(())
            }
            LocM::B(b) => match v.as_i128() {
                Some(x) => {
                    *b = x as u8;
                    Ok(())
                }
                None => Err((
                    RtKind::Internal,
                    "storing a non-character in a string".into(),
                )),
            },
        }
    }
    /// Read a scalar-like value for an operation: errors on uninitialised variables.
    fn read(&self, p: &Place) -> R<Value> {
        let v = self.load(p).map_err(|f| self.fail(f))?;
        self.check_init(v)
    }
    fn check_init(&self, v: Value) -> R<Value> {
        if let Value::Uninit(t) = &v {
            return Err(self.err(
                RtKind::Uninit,
                format!("this {t} variable was never given a value, so reading it is undefined behaviour"),
            ));
        }
        Ok(v)
    }
    /// Copy the value at a place, charging the copy.
    fn materialize(&mut self, p: &Place) -> R<Value> {
        let (c, scalar) = {
            let v = self.get_or_char(p)?;
            match v {
                Some(v) => (leaves(v), v.sk().is_some()),
                None => (0, true),
            }
        };
        if !scalar && c > 0 {
            self.charge(c)?;
        }
        self.read(p)
    }
    fn get_or_char(&self, p: &Place) -> R<Option<&Value>> {
        match self.loc(p).map_err(|f| self.fail(f))? {
            Loc::V(v) => Ok(Some(v)),
            Loc::B(_) => Ok(None),
        }
    }
    fn rv_value(&mut self, rv: Rv) -> R<Value> {
        match rv {
            Rv::Val(v) => Ok(v),
            Rv::Place(p) => self.materialize(&p),
        }
    }
    fn rv_place(&mut self, rv: Rv) -> Place {
        match rv {
            Rv::Place(p) => p,
            Rv::Val(v) => self.temp(v),
        }
    }
    // heap objects
    fn alloc_heap(&mut self, v: Value) -> Value {
        if let Some(i) = self.heap_free.pop() {
            let h = &mut self.heap[i as usize];
            h.v = Some(v);
            return Value::PHeap(i, h.gn);
        }
        self.heap.push(HObj { gn: 1, v: Some(v) });
        Value::PHeap((self.heap.len() - 1) as u32, 1)
    }
    fn free_heap(&mut self, i: u32, gn: u32) -> Result<(), Fail> {
        match self.heap.get_mut(i as usize) {
            Some(h) if h.gn == gn && h.v.is_some() => {
                h.v = None;
                h.gn = h.gn.wrapping_add(1).max(1);
                self.heap_free.push(i);
                Ok(())
            }
            _ => Err((
                RtKind::Dangling,
                "this object was already deleted: deleting it twice is undefined behaviour".into(),
            )),
        }
    }
    /// The place a pointer value designates.
    fn deref_ptr(&self, v: &Value) -> R<Place> {
        match v {
            Value::PHeap(i, g) => Ok(Place::root(Root::Heap(*i, *g))),
            Value::PPlace(p) => Ok((**p).clone()),
            Value::PNull => Err(self.err(
                RtKind::Null,
                "this pointer is null (nullptr): there is nothing there to look at",
            )),
            Value::Uninit(_) => Err(self.err(
                RtKind::Uninit,
                "this pointer was never given a value, so using it is undefined behaviour",
            )),
            _ => Err(self.err(RtKind::Internal, "dereferencing a non-pointer")),
        }
    }

    // ----- calls -----

    fn call_fn(&mut self, fi: u32, bound: Vec<Value>) -> R<Rv> {
        if self.depth >= self.lim.depth || self.deep() {
            return Err(self.too_deep());
        }
        let (decl, nslots) = {
            let f = &self.rs.fns[fi as usize];
            (f.decl, f.nslots as usize)
        };
        let body = match &decl.body {
            Some(b) => b,
            None => return Err(self.internal("function has no body")),
        };
        let saved = (
            self.base,
            self.cap_off,
            self.ret_spec,
            self.func,
            self.vis_base,
            self.cur,
        );
        let new_base = self.stack.len();
        self.stack
            .resize_with(new_base + nslots.max(bound.len()), || Slot {
                gn: 0,
                v: Value::Void,
            });
        self.base = new_base;
        self.cap_off = 0;
        self.ret_spec = Some(&decl.ret);
        self.func = fi;
        self.depth += 1;
        self.vis_base = self.vis.len();
        for (i, v) in bound.into_iter().enumerate() {
            self.declare(i as u32, v);
            if self.trace.is_some() {
                let nm = decl.params.get(i).map_or("", |p| p.name.as_str());
                self.vis.push((nm, i as u32));
            }
        }
        let tmark = self.temps.len();
        let r = self.exec_stmts(&body.stmts);
        let out = match r {
            Ok(Flow::Ret) => Ok(self.ret.take().unwrap_or(Rv::Val(Value::Void))),
            Ok(_) => {
                if decl.ret.ty == Ty::Void || decl.name == "main" {
                    Ok(Rv::Val(Value::Void))
                } else {
                    Err(Box::new(RtError {
                        kind: RtKind::MissingReturn,
                        node: decl.id,
                        msg: format!(
                            "`{}` reached its closing brace without returning a value: undefined behaviour",
                            decl.name
                        ),
                    }))
                }
            }
            Err(e) => Err(e),
        };
        self.temps.truncate(tmark);
        self.depth -= 1;
        self.stack.truncate(new_base);
        self.vis.truncate(self.vis_base);
        (
            self.base,
            self.cap_off,
            self.ret_spec,
            self.func,
            self.vis_base,
            self.cur,
        ) = saved;
        out
    }

    fn call_closure(&mut self, c: &Rc<Closure>, bound: Vec<Value>) -> R<Rv> {
        if self.depth >= self.lim.depth || self.deep() {
            return Err(self.too_deep());
        }
        let li = &self.rs.lams[c.lam as usize];
        let lam = li.lam;
        let nslots = li.nslots as usize;
        let ncaps = li.caps.len();
        let saved = (
            self.base,
            self.cap_off,
            self.ret_spec,
            self.func,
            self.vis_base,
            self.cur,
        );
        let new_base = self.stack.len();
        let total = nslots.max(bound.len()) + ncaps;
        self.stack.resize_with(new_base + total, || Slot {
            gn: 0,
            v: Value::Void,
        });
        let cap_off = nslots.max(bound.len());
        self.base = new_base;
        self.cap_off = cap_off;
        self.ret_spec = lam.ret.as_ref();
        self.func = (self.rs.fns.len() + c.lam as usize) as u32;
        self.depth += 1;
        self.vis_base = self.vis.len();
        for (i, v) in bound.into_iter().enumerate() {
            self.declare(i as u32, v);
            if self.trace.is_some() {
                let nm = lam.params.get(i).map_or("", |p| p.name.as_str());
                self.vis.push((nm, i as u32));
            }
        }
        {
            let caps = c.caps.borrow();
            for (i, v) in caps.iter().enumerate().take(ncaps) {
                self.declare((cap_off + i) as u32, v.clone());
                if self.trace.is_some() {
                    self.vis.push((
                        self.rs.lams[c.lam as usize].caps[i].name,
                        (cap_off + i) as u32,
                    ));
                }
            }
        }
        let tmark = self.temps.len();
        let r = self.exec_stmts(&lam.body.stmts);
        let out = match r {
            Ok(Flow::Ret) => Ok(self.ret.take().unwrap_or(Rv::Val(Value::Void))),
            Ok(_) => Ok(Rv::Val(Value::Void)),
            Err(e) => Err(e),
        };
        if lam.mutable {
            let mut caps = c.caps.borrow_mut();
            for (i, slot) in caps.iter_mut().enumerate().take(ncaps) {
                let by_ref = self.rs.lams[c.lam as usize].caps[i].by_ref;
                if by_ref {
                    continue;
                }
                if let Some(s) = self.stack.get_mut(new_base + cap_off + i)
                    && !matches!(slot, Value::Ref(_))
                    && s.gn != 0
                {
                    *slot = std::mem::replace(&mut s.v, Value::Void);
                }
            }
        }
        self.temps.truncate(tmark);
        self.depth -= 1;
        self.stack.truncate(new_base);
        self.vis.truncate(self.vis_base);
        (
            self.base,
            self.cap_off,
            self.ret_spec,
            self.func,
            self.vis_base,
            self.cur,
        ) = saved;
        out
    }

    /// Call a callable value with already evaluated arguments (used by the library).
    fn call_value(&mut self, f: &Value, args: Vec<Rv>) -> R<Rv> {
        let tm = self.temps.len();
        let r = self.call_value_inner(f, args);
        if !matches!(
            &r,
            Ok(Rv::Place(Place {
                root: Root::Temp(..),
                ..
            }))
        ) {
            self.temps.truncate(tm);
        }
        r
    }

    fn call_value_inner(&mut self, f: &Value, args: Vec<Rv>) -> R<Rv> {
        match f {
            Value::Closure(c) => {
                let specs: Vec<&'p TypeSpec> = self.rs.lams[c.lam as usize]
                    .lam
                    .params
                    .iter()
                    .map(|p| &p.spec)
                    .collect();
                if specs.len() != args.len() {
                    return Err(
                        self.err(RtKind::BadArg, "wrong number of arguments for the function")
                    );
                }
                let mut bound = Vec::with_capacity(args.len());
                for (s, a) in specs.into_iter().zip(args) {
                    bound.push(self.bind_rv(s, a)?);
                }
                self.charge(1)?;
                let c = c.clone();
                self.call_closure(&c, bound)
            }
            Value::FnRef(g) => {
                let fi = self.pick_overload_rv(*g, &args)?;
                let specs: Vec<&'p TypeSpec> = self.rs.fns[fi as usize]
                    .decl
                    .params
                    .iter()
                    .map(|p| &p.spec)
                    .collect();
                let mut bound = Vec::with_capacity(args.len());
                for (s, a) in specs.into_iter().zip(args) {
                    bound.push(self.bind_rv(s, a)?);
                }
                self.charge(1)?;
                self.call_fn(fi, bound)
            }
            Value::Functor(kind, _) => {
                if args.len() != 2 {
                    return Err(
                        self.err(RtKind::BadArg, "a comparison functor takes two arguments")
                    );
                }
                let mut it = args.into_iter();
                let a = it.next().map(|a| self.rv_value(a)).transpose()?;
                let b = it.next().map(|a| self.rv_value(a)).transpose()?;
                let (Some(a), Some(b)) = (a, b) else {
                    return Err(self.internal("missing argument"));
                };
                let o = cmp_val(&a, &b);
                let r = match kind {
                    Functor::Less => o == std::cmp::Ordering::Less,
                    Functor::Greater => o == std::cmp::Ordering::Greater,
                };
                Ok(Rv::Val(Value::Bool(r)))
            }
            Value::Uninit(_) => Err(self.err(
                RtKind::Uninit,
                "this std::function holds nothing: calling it would throw bad_function_call",
            )),
            _ => Err(self.err(RtKind::BadArg, "this value cannot be called")),
        }
    }

    /// Bind an evaluated argument to a parameter.
    fn bind_rv(&mut self, spec: &'p TypeSpec, rv: Rv) -> R<Value> {
        let auto = matches!(spec.ty, Ty::Auto | Ty::Unknown);
        if spec.is_ref {
            match rv {
                Rv::Place(p) => {
                    if !auto && spec.konst {
                        let v = self.get_or_char(&p)?;
                        let ok = v.is_none_or(|v| conv::matches_ty(v, &spec.ty));
                        if !ok {
                            let v = self.materialize(&p)?;
                            let v = self.convert(v, &spec.ty)?;
                            let t = self.temp(v);
                            return Ok(Value::Ref(Box::new(t)));
                        }
                    }
                    Ok(Value::Ref(Box::new(p)))
                }
                Rv::Val(v) => {
                    let v = if auto { v } else { self.convert(v, &spec.ty)? };
                    let t = self.temp(v);
                    Ok(Value::Ref(Box::new(t)))
                }
            }
        } else {
            let v = self.rv_value(rv)?;
            let v = self.check_init(v)?;
            if auto {
                Ok(v)
            } else {
                self.convert(v, &spec.ty)
            }
        }
    }

    fn pick_overload_rv(&mut self, g: u32, args: &[Rv]) -> R<u32> {
        let cands: Vec<u32> = self.rs.groups[g as usize]
            .iter()
            .copied()
            .filter(|&f| self.rs.fns[f as usize].decl.params.len() == args.len())
            .collect();
        match cands.len() {
            0 => Err(self.err(RtKind::BadArg, "no function takes that many arguments")),
            1 => Ok(cands[0]),
            _ => {
                let mut best = (i32::MIN, cands[0]);
                for &f in &cands {
                    let mut score = 0;
                    for (p, a) in self.rs.fns[f as usize].decl.params.iter().zip(args) {
                        let v = match a {
                            Rv::Val(v) => Some(v),
                            Rv::Place(pl) => self.get_or_char(pl)?,
                        };
                        if let Some(v) = v {
                            score += conv::match_score(v, &p.spec.ty);
                        }
                    }
                    if score > best.0 {
                        best = (score, f);
                    }
                }
                Ok(best.1)
            }
        }
    }

    // ----- statements -----

    fn scope_exit(&mut self, id: NodeId, tmark: usize, vmark: usize) {
        let (lo, hi) = self.rs.scope(id);
        for s in lo..hi {
            if let Some(sl) = self.stack.get_mut(self.base + s as usize) {
                sl.gn = 0;
                sl.v = Value::Void;
            }
        }
        self.temps.truncate(tmark);
        self.vis.truncate(vmark);
    }

    fn exec_stmts(&mut self, stmts: &'p [Stmt]) -> R<Flow> {
        for s in stmts {
            match self.exec(s)? {
                Flow::Next => {}
                f => return Ok(f),
            }
        }
        Ok(Flow::Next)
    }

    fn record(&mut self, node: NodeId, returned: Option<&Value>) {
        let Some(tb) = &self.trace else { return };
        if tb.events.len() >= self.lim.trace_events || tb.cells_left <= 0 {
            if let Some(tb) = self.trace.as_mut() {
                tb.truncated = true;
            }
            return;
        }
        let ev = self.snapshot(node, returned);
        if let Some(tb) = self.trace.as_mut() {
            tb.cells_left -= ev.vars.iter().map(|v| v.value.cells() as i64).sum::<i64>() + 4;
            tb.events.push(ev);
        }
    }

    fn exec(&mut self, s: &'p Stmt) -> R<Flow> {
        self.cur = s.id;
        if self.deep() {
            return Err(self.too_deep());
        }
        match &s.kind {
            StmtKind::Block(b) => {
                let (tm, vm) = (self.temps.len(), self.vis.len());
                let r = self.exec_stmts(&b.stmts)?;
                self.scope_exit(s.id, tm, vm);
                return Ok(r);
            }
            StmtKind::Case(_) => return Ok(Flow::Next),
            _ => {}
        }
        self.charge(1)?;
        if self.trace.is_some() && !matches!(s.kind, StmtKind::Return(_)) {
            self.record(s.id, None);
        }
        match &s.kind {
            StmtKind::Decl { spec, vars } => {
                for d in vars {
                    self.exec_decl(spec, d)?;
                }
                Ok(Flow::Next)
            }
            StmtKind::Structured { spec, names, init } => {
                self.exec_structured(s, spec, names, init)?;
                Ok(Flow::Next)
            }
            StmtKind::Expr(e) => {
                let tm = self.temps.len();
                let _ = self.eval_rv(e)?;
                self.temps.truncate(tm);
                Ok(Flow::Next)
            }
            StmtKind::If {
                init,
                cond,
                then,
                els,
            } => {
                let (tm, vm) = (self.temps.len(), self.vis.len());
                if let Some(i) = init {
                    self.exec(i)?;
                    self.cur = s.id;
                }
                let c = self.eval_cond(cond)?;
                let r = if c {
                    self.exec(then)?
                } else if let Some(e) = els {
                    self.exec(e)?
                } else {
                    Flow::Next
                };
                self.scope_exit(s.id, tm, vm);
                Ok(r)
            }
            StmtKind::Switch { cond, body } => self.exec_switch(s, cond, body),
            StmtKind::For {
                init,
                cond,
                incr,
                body,
            } => {
                let (tm, vm) = (self.temps.len(), self.vis.len());
                if let Some(i) = init {
                    self.exec(i)?;
                }
                let tm2 = self.temps.len();
                let out = loop {
                    self.cur = s.id;
                    self.charge(1)?;
                    if let Some(c) = cond {
                        let ok = self.eval_cond(c)?;
                        self.temps.truncate(tm2);
                        if !ok {
                            break Flow::Next;
                        }
                    }
                    match self.exec(body)? {
                        Flow::Break => break Flow::Next,
                        Flow::Ret => break Flow::Ret,
                        Flow::Next | Flow::Continue => {}
                    }
                    self.cur = s.id;
                    if let Some(i) = incr {
                        let _ = self.eval_rv(i)?;
                        self.temps.truncate(tm2);
                    }
                };
                self.scope_exit(s.id, tm, vm);
                Ok(out)
            }
            StmtKind::RangeFor {
                spec,
                names,
                structured,
                range,
                body,
            } => self.exec_range_for(s, spec, names, *structured, range, body),
            StmtKind::While { cond, body } => {
                let tm = self.temps.len();
                loop {
                    self.cur = s.id;
                    self.charge(1)?;
                    let ok = self.eval_cond(cond)?;
                    self.temps.truncate(tm);
                    if !ok {
                        return Ok(Flow::Next);
                    }
                    match self.exec(body)? {
                        Flow::Break => return Ok(Flow::Next),
                        Flow::Ret => return Ok(Flow::Ret),
                        Flow::Next | Flow::Continue => {}
                    }
                }
            }
            StmtKind::DoWhile { body, cond } => {
                let tm = self.temps.len();
                loop {
                    self.charge(1)?;
                    match self.exec(body)? {
                        Flow::Break => return Ok(Flow::Next),
                        Flow::Ret => return Ok(Flow::Ret),
                        Flow::Next | Flow::Continue => {}
                    }
                    self.cur = s.id;
                    let ok = self.eval_cond(cond)?;
                    self.temps.truncate(tm);
                    if !ok {
                        return Ok(Flow::Next);
                    }
                }
            }
            StmtKind::Return(e) => {
                self.exec_return(s, e.as_ref())?;
                Ok(Flow::Ret)
            }
            StmtKind::Break => Ok(Flow::Break),
            StmtKind::Continue => Ok(Flow::Continue),
            StmtKind::Empty => Ok(Flow::Next),
            StmtKind::Block(_) | StmtKind::Case(_) => Ok(Flow::Next),
        }
    }

    fn exec_return(&mut self, s: &'p Stmt, e: Option<&'p Expr>) -> R<()> {
        let tm = self.temps.len();
        let spec = self.ret_spec;
        let rv = match e {
            None => Rv::Val(Value::Void),
            Some(e) => match spec {
                Some(sp) if sp.is_ref => {
                    let rv = self.eval_rv(e)?;
                    Rv::Place(self.rv_place(rv))
                }
                Some(sp) if !matches!(sp.ty, Ty::Auto | Ty::Unknown | Ty::Void) => {
                    let v = self.eval_return_value(e, Some(&sp.ty))?;
                    Rv::Val(v)
                }
                _ => {
                    let v = self.eval_return_value(e, None)?;
                    Rv::Val(v)
                }
            },
        };
        // temporaries die with the statement, but a returned place into one must survive
        if !matches!(
            &rv,
            Rv::Place(Place {
                root: Root::Temp(..),
                ..
            })
        ) {
            self.temps.truncate(tm);
        }
        if self.trace.is_some() {
            let shown = match &rv {
                Rv::Val(v) => Some(v.clone()),
                Rv::Place(p) => self.load(p).ok(),
            };
            self.record(s.id, shown.as_ref());
        }
        self.ret = Some(rv);
        Ok(())
    }

    /// Value of a `return` operand: moves out of a plain local variable instead of copying it.
    fn eval_return_value(&mut self, e: &'p Expr, ty: Option<&Ty>) -> R<Value> {
        if let ExprKind::Var(_) = &e.kind
            && let VarRef::Local(s) = self.rs.var(e.id)
        {
            let abs = self.base + s as usize;
            if let Some(sl) = self.stack.get_mut(abs)
                && sl.gn != 0
                && !matches!(sl.v, Value::Ref(_) | Value::Uninit(_) | Value::Void)
            {
                let v = std::mem::replace(&mut sl.v, Value::Void);
                return match ty {
                    Some(t) => self.convert(v, t),
                    None => Ok(v),
                };
            }
        }
        match ty {
            Some(t) => self.eval_conv(e, t),
            None => self.eval(e),
        }
    }

    fn exec_decl(&mut self, spec: &'p TypeSpec, d: &'p Declarator) -> R<()> {
        let slot = self.rs.slot(d.id);
        if slot == u32::MAX {
            return Err(self.internal("unresolved declaration"));
        }
        let gn = self.declare(slot, Value::Void);
        let tm = self.temps.len();
        let v = self.decl_value(spec, d, slot, gn)?;
        let _ = tm;
        if let Some(sl) = self.stack.get_mut(self.base + slot as usize) {
            *sl = Slot { gn, v };
        }
        if self.trace.is_some() {
            self.vis.push((d.name.as_str(), slot));
        }
        Ok(())
    }

    fn decl_value(
        &mut self,
        spec: &'p TypeSpec,
        d: &'p Declarator,
        _slot: u32,
        _gen: u32,
    ) -> R<Value> {
        let auto = matches!(spec.ty, Ty::Auto);
        if spec.is_ref {
            let e = match &d.init {
                Init::Copy(e) => e,
                Init::Direct(a) if a.len() == 1 => &a[0],
                Init::Brace { elems, .. } if elems.len() == 1 => &elems[0],
                _ => return Err(self.internal("reference without an initialiser")),
            };
            let rv = match (&e.kind, auto) {
                (ExprKind::InitList(_), false) => Rv::Val(self.eval_conv(e, &spec.ty)?),
                _ => self.eval_rv(e)?,
            };
            return self.bind_rv(spec, rv);
        }
        if let Some(n) = &d.array {
            return self.decl_array(spec, d, n);
        }
        match &d.init {
            Init::None => self.default_value(&spec.ty),
            Init::Copy(e) => {
                if auto {
                    let v = self.eval(e)?;
                    self.check_init(v)
                } else {
                    let v = self.eval_conv(e, &spec.ty)?;
                    self.check_init(v)
                }
            }
            Init::Direct(args) => {
                if auto {
                    match args.first() {
                        Some(a) => self.eval(a),
                        None => Err(self.internal("auto without a value")),
                    }
                } else {
                    self.construct_ty(&spec.ty, args, false)
                }
            }
            Init::Brace { elems, with_eq } => {
                if auto {
                    match elems.first() {
                        Some(a) => self.eval(a),
                        None => Err(self.internal("auto without a value")),
                    }
                } else if *with_eq {
                    self.list_value(elems, &spec.ty)
                } else {
                    self.construct_ty(&spec.ty, elems, true)
                }
            }
        }
    }

    fn decl_array(&mut self, spec: &'p TypeSpec, d: &'p Declarator, n: &'p Expr) -> R<Value> {
        let nv = self.eval(n)?;
        let Some(len) = nv.as_i128().filter(|&x| x >= 0) else {
            return Err(self.err(RtKind::BadArg, "an array needs a non-negative size"));
        };
        self.reserve(len as u64)?;
        self.charge(len as u64)?;
        let ety: TyRc = Rc::new(spec.ty.clone());
        let elems: &[Expr] = match &d.init {
            Init::None => &[],
            Init::Copy(e) => std::slice::from_ref(e),
            Init::Direct(a) | Init::Brace { elems: a, .. } => a,
        };
        let has_init = !matches!(d.init, Init::None);
        let mut v = Vec::with_capacity(len as usize);
        let flat: Vec<&'p Expr> = match elems {
            [
                Expr {
                    kind: ExprKind::InitList(inner),
                    ..
                },
            ] => inner.iter().collect(),
            _ => elems.iter().collect(),
        };
        if flat.len() as i128 > len {
            return Err(self.err(RtKind::BadArg, "too many initialisers for this array"));
        }
        for e in &flat {
            v.push(self.eval_conv(e, &ety)?);
        }
        while (v.len() as i128) < len {
            v.push(if has_init {
                self.zero_value(&ety)?
            } else {
                self.default_value(&ety)?
            });
        }
        Ok(Value::Vector(Box::new(VecV {
            ety,
            v,
            ver: 0,
            kind: VKind::Array,
        })))
    }

    fn exec_structured(
        &mut self,
        s: &'p Stmt,
        spec: &'p TypeSpec,
        names: &'p [String],
        init: &'p Expr,
    ) -> R<()> {
        let first = self.rs.slot(s.id);
        if first == u32::MAX {
            return Err(self.internal("unresolved binding"));
        }
        let rv = self.eval_rv(init)?;
        let obj = if spec.is_ref {
            self.rv_place(rv)
        } else {
            let v = self.rv_value(rv)?;
            self.temp(v)
        };
        self.bind_names(&obj, first, names)
    }

    fn bind_names(&mut self, obj: &Place, first: u32, names: &'p [String]) -> R<()> {
        let steps: Vec<Step> = match self.get(obj).map_err(|f| self.fail(f))? {
            Value::Pair(_) => (0..2).map(Step::Field).collect(),
            Value::Tuple(t) => (0..t.len() as u16).map(Step::Field).collect(),
            Value::Struct(st) => (0..st.f.len() as u16).map(Step::Field).collect(),
            Value::Vector(v) if v.kind == VKind::Array => {
                (0..v.v.len()).map(|i| Step::Idx(i, v.ver)).collect()
            }
            _ => {
                return Err(self.err(
                    RtKind::BadArg,
                    "this value cannot be taken apart by a structured binding",
                ));
            }
        };
        if steps.len() != names.len() {
            return Err(self.err(
                RtKind::BadArg,
                format!(
                    "the structured binding names {} things but the value has {}",
                    names.len(),
                    steps.len()
                ),
            ));
        }
        for (i, (st, n)) in steps.into_iter().zip(names).enumerate() {
            let p = obj.with(st);
            self.declare(first + i as u32, Value::Ref(Box::new(p)));
            if self.trace.is_some() {
                self.vis.push((n.as_str(), first + i as u32));
            }
        }
        Ok(())
    }

    fn exec_switch(&mut self, s: &'p Stmt, cond: &'p Expr, body: &'p Block) -> R<Flow> {
        let (tm, vm) = (self.temps.len(), self.vis.len());
        let cv = self.eval(cond)?;
        let Some(cx) = cv.as_i128() else {
            return Err(self.err(RtKind::BadArg, "a switch needs an integral value"));
        };
        let mut start = None;
        let mut default = None;
        for (i, st) in body.stmts.iter().enumerate() {
            if let StmtKind::Case(c) = &st.kind {
                match c {
                    None => default = Some(i),
                    Some(e) => {
                        self.cur = st.id;
                        let v = self.eval(e)?;
                        if v.as_i128() == Some(cx) {
                            start = Some(i);
                            break;
                        }
                    }
                }
            }
        }
        self.cur = s.id;
        let Some(start) = start.or(default) else {
            self.scope_exit(s.id, tm, vm);
            return Ok(Flow::Next);
        };
        let mut out = Flow::Next;
        for st in &body.stmts[start..] {
            match self.exec(st)? {
                Flow::Next => {}
                Flow::Break => break,
                f => {
                    out = f;
                    break;
                }
            }
        }
        self.scope_exit(s.id, tm, vm);
        Ok(out)
    }

    fn exec_range_for(
        &mut self,
        s: &'p Stmt,
        spec: &'p TypeSpec,
        names: &'p [String],
        structured: bool,
        range: &'p Expr,
        body: &'p Stmt,
    ) -> R<Flow> {
        let (tm, vm) = (self.temps.len(), self.vis.len());
        let first = self.rs.slot(s.id);
        if first == u32::MAX {
            return Err(self.internal("unresolved loop variable"));
        }
        let rv = if let ExprKind::InitList(_) = &range.kind {
            Rv::Val(self.eval(range)?)
        } else {
            self.eval_rv(range)?
        };
        let cont = self.rv_place(rv);
        let tm2 = self.temps.len();
        let mut it = self.range_iter(&cont)?;
        let mut out = Flow::Next;
        loop {
            self.cur = s.id;
            self.charge(1)?;
            let Some(elem) = self.range_next(&mut it, &cont)? else {
                break;
            };
            let _ = &elem;
            // bind the loop variable
            let auto = matches!(spec.ty, Ty::Auto);
            if structured || spec.is_ref {
                let obj_place = elem;
                if structured {
                    let obj = if spec.is_ref {
                        obj_place
                    } else {
                        let v = self.materialize(&obj_place)?;
                        self.temp(v)
                    };
                    self.bind_names(&obj, first, names)?;
                } else {
                    let v = self.bind_rv(spec, Rv::Place(obj_place))?;
                    self.declare(first, v);
                }
            } else {
                let v = self.materialize(&elem)?;
                let v = if auto { v } else { self.convert(v, &spec.ty)? };
                self.declare(first, v);
            }
            if self.trace.is_some() {
                self.vis.truncate(vm);
                if !structured && let Some(n) = names.first() {
                    self.vis.push((n.as_str(), first));
                }
            }
            let f = self.exec(body)?;
            self.temps.truncate(tm2);
            match f {
                Flow::Break => break,
                Flow::Ret => {
                    out = Flow::Ret;
                    break;
                }
                Flow::Next | Flow::Continue => {}
            }
        }
        self.scope_exit(s.id, tm, vm);
        Ok(out)
    }
}

/// Run `entry` with the given arguments.
pub fn run(p: &Program, entry: &str, args: Vec<Plain>, heap: Heap, limits: &Limits) -> RunReport {
    let input = std::sync::Mutex::new(Some((args, heap)));
    let take = || input.lock().ok().and_then(|mut g| g.take());
    let threaded = std::thread::scope(|sc| {
        let b = std::thread::Builder::new().stack_size(BIG_STACK);
        let h = b.spawn_scoped(sc, || {
            let (a, h) = take()?;
            Some(run_inner(p, entry, a, h, limits, BIG_STACK - STACK_MARGIN))
        });
        match h {
            Ok(h) => match h.join() {
                Ok(r) => Some(r),
                Err(_) => Some(Some(internal_report(
                    "the interpreter crashed on this program",
                ))),
            },
            Err(_) => None,
        }
    });
    match threaded {
        Some(Some(r)) => r,
        Some(None) => internal_report("interpreter input was lost"),
        None => match take() {
            Some((a, h)) => run_inner(p, entry, a, h, limits, 512 << 10),
            None => internal_report("interpreter input was lost"),
        },
    }
}

fn internal_report(msg: &str) -> RunReport {
    RunReport {
        result: Err(RtError {
            kind: RtKind::Internal,
            node: 0,
            msg: msg.into(),
        }),
        cost: 0,
        peak_cells: 0,
        input_cells: 0,
        trace: None,
        heap: Heap::default(),
    }
}

fn run_inner(
    p: &Program,
    entry: &str,
    args: Vec<Plain>,
    heap: Heap,
    limits: &Limits,
    budget: usize,
) -> RunReport {
    let mut m = Machine::new(p, limits, budget);
    let r = m.run_entry(entry, args, &heap);
    let (result, out_heap) = match r {
        Ok((v, h)) => (Ok(v), h),
        Err(e) => (Err(*e), Heap::default()),
    };
    let live = m.live_cells();
    m.peak = m.peak.max(live);
    let trace = m.trace.take().map(|t| Trace {
        events: t.events,
        truncated: t.truncated,
    });
    RunReport {
        result,
        cost: m.cost,
        peak_cells: m.peak,
        input_cells: m.input_cells,
        trace,
        heap: out_heap,
    }
}

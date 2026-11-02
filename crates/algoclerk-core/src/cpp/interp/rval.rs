// SPDX-License-Identifier: Apache-2.0
//! Runtime values of the interpreter: scalars with exact C++ type tags, containers that remember their
//! element types, places (references, pointers, iterator targets) and the ordering used by `std::map`.

use crate::cpp::ast::{Functor, Ty};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::{BuildHasherDefault, Hash, Hasher};
use std::rc::Rc;

/// Shared element type.
pub type TyRc = Rc<Ty>;

/// Scalar kinds with C++ conversion rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sk {
    /// `bool`
    Bool,
    /// `char`
    Char,
    /// `int`
    Int,
    /// `unsigned`
    UInt,
    /// `long long`
    LL,
    /// `unsigned long long`
    Ull,
    /// `size_t` (`unsigned long`)
    Size,
}

impl Sk {
    /// The AST type.
    pub fn ty(self) -> Ty {
        match self {
            Sk::Bool => Ty::Bool,
            Sk::Char => Ty::Char,
            Sk::Int => Ty::Int,
            Sk::UInt => Ty::UInt,
            Sk::LL => Ty::LongLong,
            Sk::Ull => Ty::ULongLong,
            Sk::Size => Ty::Size,
        }
    }
    /// Scalar kind of an integral AST type.
    pub fn of_ty(t: &Ty) -> Option<Sk> {
        Some(match t {
            Ty::Bool => Sk::Bool,
            Ty::Char => Sk::Char,
            Ty::Int => Sk::Int,
            Ty::UInt => Sk::UInt,
            Ty::LongLong => Sk::LL,
            Ty::ULongLong => Sk::Ull,
            Ty::Size => Sk::Size,
            _ => return None,
        })
    }
    /// Signed types.
    pub fn signed(self) -> bool {
        matches!(self, Sk::Char | Sk::Int | Sk::LL)
    }
    /// Width in bits (`bool` is 1).
    pub fn bits(self) -> u32 {
        match self {
            Sk::Bool => 1,
            Sk::Char => 8,
            Sk::Int | Sk::UInt => 32,
            _ => 64,
        }
    }
    /// Smallest value.
    pub fn min(self) -> i128 {
        match self {
            Sk::Char => -128,
            Sk::Int => i128::from(i32::MIN),
            Sk::LL => i128::from(i64::MIN),
            _ => 0,
        }
    }
    /// Largest value.
    pub fn max(self) -> i128 {
        match self {
            Sk::Bool => 1,
            Sk::Char => 127,
            Sk::Int => i128::from(i32::MAX),
            Sk::UInt => i128::from(u32::MAX),
            Sk::LL => i128::from(i64::MAX),
            _ => i128::from(u64::MAX),
        }
    }
    /// Does `x` fit without wrapping?
    pub fn fits(self, x: i128) -> bool {
        x >= self.min() && x <= self.max()
    }
    /// Build a value of this kind from `x`, wrapping modulo 2^bits (booleans become `x != 0`).
    pub fn make(self, x: i128) -> Value {
        match self {
            Sk::Bool => Value::Bool(x != 0),
            Sk::Char => Value::Char(x as i8),
            Sk::Int => Value::Int(x as i32),
            Sk::UInt => Value::UInt(x as u32),
            Sk::LL => Value::LL(x as i64),
            Sk::Ull => Value::Ull(x as u64),
            Sk::Size => Value::Size(x as u64),
        }
    }
}

/// One string.
#[derive(Clone, Debug)]
pub struct StrV {
    /// Bytes.
    pub s: Vec<u8>,
    /// Invalidation counter.
    pub ver: u32,
}

/// Which `vector`-like type a [`VecV`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VKind {
    /// `std::vector`
    Vector,
    /// `std::array` and C arrays
    Array,
    /// `std::stack`
    Stack,
}

/// `vector`, `array`, `stack`.
#[derive(Clone, Debug)]
pub struct VecV {
    /// Element type.
    pub ety: TyRc,
    /// Elements.
    pub v: Vec<Value>,
    /// Invalidation counter.
    pub ver: u32,
    /// Flavour.
    pub kind: VKind,
}

/// Which deque-like type a [`DqV`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DKind {
    /// `std::deque`
    Deque,
    /// `std::queue`
    Queue,
}

/// `deque`, `queue`.
#[derive(Clone, Debug)]
pub struct DqV {
    /// Element type.
    pub ety: TyRc,
    /// Elements.
    pub v: VecDeque<Value>,
    /// Invalidation counter.
    pub ver: u32,
    /// Flavour.
    pub kind: DKind,
}

/// Sentinel for "no node".
pub const NIL: u32 = u32::MAX;

/// A node of a `std::list`.
#[derive(Clone, Debug)]
pub struct LNode {
    /// Payload (`None` for a free node).
    pub val: Option<Value>,
    /// Previous node.
    pub prev: u32,
    /// Next node.
    pub next: u32,
    /// Generation (bumped when the node is erased).
    pub gn: u32,
}

/// `std::list`.
#[derive(Clone, Debug)]
pub struct ListV {
    /// Element type.
    pub ety: TyRc,
    /// Node slab.
    pub nodes: Vec<LNode>,
    /// First node.
    pub head: u32,
    /// Last node.
    pub tail: u32,
    /// Element count.
    pub len: usize,
    /// Free node indices.
    pub free: Vec<u32>,
}

/// Comparator of a priority queue.
#[derive(Clone, Debug)]
pub enum PqCmp {
    /// `std::less`
    Less,
    /// `std::greater`
    Greater,
    /// A closure or function.
    Fn(Value),
}

/// `std::priority_queue` (binary heap array, top at index 0).
#[derive(Clone, Debug)]
pub struct PqV {
    /// Element type.
    pub ety: TyRc,
    /// The heap array.
    pub v: Vec<Value>,
    /// Comparator.
    pub cmp: PqCmp,
    /// Invalidation counter.
    pub ver: u32,
}

/// `std::map` / `std::set`: ordered by key. Map values are `Pair(key, value)`, set values are `Void`.
#[derive(Clone, Debug)]
pub struct OrdV {
    /// Key type.
    pub kty: TyRc,
    /// Mapped type (`None` for sets).
    pub vty: Option<TyRc>,
    /// Entries.
    pub m: BTreeMap<Key, Value>,
}

/// Cheap multiplicative hasher.
#[derive(Default)]
pub struct Fx(u64);

impl Hasher for Fx {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ u64::from(b)).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
    }
    fn write_u64(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(5) ^ x).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

/// Hash map with the cheap hasher.
pub type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<Fx>>;

/// `std::unordered_map` / `std::unordered_set` with a deterministic iteration order (insertion order).
#[derive(Clone, Debug)]
pub struct UnV {
    /// Key type.
    pub kty: TyRc,
    /// Mapped type (`None` for sets).
    pub vty: Option<TyRc>,
    /// Key to entry index.
    pub idx: FxMap<Key, usize>,
    /// Entries in insertion order (`Pair` for maps, the element for sets); `None` once erased.
    pub ents: Vec<Option<Value>>,
    /// Live entries.
    pub live: usize,
    /// Iterator invalidation counter (bumped by inserts of new keys and `clear`).
    pub ver: u32,
}

/// `std::optional`.
#[derive(Clone, Debug)]
pub struct OptV {
    /// Contained type.
    pub ety: TyRc,
    /// Value if engaged.
    pub v: Option<Value>,
}

/// A struct value.
#[derive(Clone, Debug)]
pub struct StructV {
    /// Index into the interpreter's struct table.
    pub sid: u32,
    /// Struct name.
    pub name: Rc<str>,
    /// Field values in declaration order.
    pub f: Vec<Value>,
}

/// A lambda value with its captures.
#[derive(Debug)]
pub struct Closure {
    /// Index of the lambda in the interpreter's table.
    pub lam: u32,
    /// Captured values (`Value::Ref` for by-reference captures).
    pub caps: RefCell<Vec<Value>>,
}

/// Where a place lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Root {
    /// Absolute stack slot and its generation.
    Local(u32, u32),
    /// Global constant.
    Global(u32),
    /// Heap object and generation.
    Heap(u32, u32),
    /// Temporary slot and generation.
    Temp(u32, u32),
}

/// One step into a composite value.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Element of a vector-like container (index, container version).
    Idx(usize, u32),
    /// Element of a map, set or unordered container by key.
    Key(Key),
    /// Field of a struct, pair or tuple.
    Field(u16),
    /// The value inside an optional.
    Opt,
    /// Node of a `std::list` (slab index, node generation).
    Node(u32, u32),
}

/// An addressable object: a variable, an element, a field or a heap object.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    /// Root object.
    pub root: Root,
    /// Steps from the root.
    pub path: Vec<Step>,
}

impl Place {
    /// A place with no steps.
    pub fn root(r: Root) -> Place {
        Place {
            root: r,
            path: Vec::new(),
        }
    }
    /// This place extended by one step.
    pub fn with(&self, s: Step) -> Place {
        let mut path = Vec::with_capacity(self.path.len() + 1);
        path.extend(self.path.iter().cloned());
        path.push(s);
        Place {
            root: self.root,
            path,
        }
    }
}

/// Position of an iterator.
#[derive(Clone, Debug, PartialEq)]
pub enum Pos {
    /// Index (for reverse iterators: number of elements already passed from the back).
    Idx(usize),
    /// Key of an ordered container element.
    Key(Key),
    /// Entry of an unordered container.
    Ent(usize),
    /// Node of a list.
    Node(u32),
    /// One past the last element of an ordered, unordered or list container.
    End,
}

/// An iterator value.
#[derive(Clone, Debug)]
pub struct IterV {
    /// The container.
    pub cont: Place,
    /// Where it points.
    pub pos: Pos,
    /// Reverse iterator.
    pub rev: bool,
    /// Container version at creation (vector-like and unordered containers).
    pub ver: u32,
}

/// A runtime value. Every value knows its exact C++ type.
#[derive(Clone, Debug)]
pub enum Value {
    /// No value.
    Void,
    /// Declared but not initialised (type remembered).
    Uninit(Box<Ty>),
    /// `bool`
    Bool(bool),
    /// `char`
    Char(i8),
    /// `int`
    Int(i32),
    /// `unsigned`
    UInt(u32),
    /// `long long`
    LL(i64),
    /// `unsigned long long`
    Ull(u64),
    /// `size_t`
    Size(u64),
    /// `std::string`
    Str(Box<StrV>),
    /// `std::string_view`
    View(Rc<Vec<u8>>),
    /// `vector`, `array`, `stack`
    Vector(Box<VecV>),
    /// `deque`, `queue`
    Deque(Box<DqV>),
    /// `list`
    List(Box<ListV>),
    /// `priority_queue`
    Pq(Box<PqV>),
    /// `map`
    Map(Box<OrdV>),
    /// `set`
    Set(Box<OrdV>),
    /// `unordered_map`
    UMap(Box<UnV>),
    /// `unordered_set`
    USet(Box<UnV>),
    /// `pair`
    Pair(Box<[Value; 2]>),
    /// `tuple`
    #[allow(clippy::box_collection)]
    Tuple(Box<Vec<Value>>),
    /// `optional`
    Opt(Box<OptV>),
    /// A struct by value.
    Struct(Box<StructV>),
    /// A lambda.
    Closure(Rc<Closure>),
    /// A named function used as a value (index of the overload group).
    FnRef(u32),
    /// `std::less<T>()` / `std::greater<T>()`
    Functor(Functor, TyRc),
    /// Null pointer.
    PNull,
    /// Pointer to a heap object (id, generation).
    PHeap(u32, u32),
    /// Pointer to anything else.
    PPlace(Box<Place>),
    /// Iterator.
    Iter(Box<IterV>),
    /// A reference (only held by variable slots and captures).
    Ref(Box<Place>),
}

impl Value {
    /// Integer payload of any integral scalar.
    pub fn as_i128(&self) -> Option<i128> {
        Some(match self {
            Value::Bool(b) => i128::from(*b),
            Value::Char(c) => i128::from(*c),
            Value::Int(i) => i128::from(*i),
            Value::UInt(u) => i128::from(*u),
            Value::LL(i) => i128::from(*i),
            Value::Ull(u) | Value::Size(u) => i128::from(*u),
            _ => return None,
        })
    }
    /// Scalar kind.
    pub fn sk(&self) -> Option<Sk> {
        Some(match self {
            Value::Bool(_) => Sk::Bool,
            Value::Char(_) => Sk::Char,
            Value::Int(_) => Sk::Int,
            Value::UInt(_) => Sk::UInt,
            Value::LL(_) => Sk::LL,
            Value::Ull(_) => Sk::Ull,
            Value::Size(_) => Sk::Size,
            _ => return None,
        })
    }
    /// A string value.
    pub fn string(s: Vec<u8>) -> Value {
        Value::Str(Box::new(StrV { s, ver: 0 }))
    }
    /// A `std::vector` of `ety`.
    pub fn vector(ety: TyRc, v: Vec<Value>) -> Value {
        Value::Vector(Box::new(VecV {
            ety,
            v,
            ver: 0,
            kind: VKind::Vector,
        }))
    }
    /// A pair.
    pub fn pair(a: Value, b: Value) -> Value {
        Value::Pair(Box::new([a, b]))
    }
    /// The bytes of a string or string_view.
    pub fn bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Str(s) => Some(&s.s),
            Value::View(v) => Some(v),
            _ => None,
        }
    }
    /// Is this a pointer value?
    pub fn is_ptr(&self) -> bool {
        matches!(self, Value::PNull | Value::PHeap(..) | Value::PPlace(_))
    }
    /// Invalidation counter of a container value.
    pub fn ver(&self) -> Option<u32> {
        match self {
            Value::Str(s) => Some(s.ver),
            Value::Vector(v) => Some(v.ver),
            Value::Deque(d) => Some(d.ver),
            Value::Pq(p) => Some(p.ver),
            Value::UMap(u) | Value::USet(u) => Some(u.ver),
            _ => None,
        }
    }
    /// Set the invalidation counter of a container value.
    pub fn set_ver(&mut self, n: u32) {
        match self {
            Value::Str(s) => s.ver = n,
            Value::Vector(v) => v.ver = n,
            Value::Deque(d) => d.ver = n,
            Value::Pq(p) => p.ver = n,
            Value::UMap(u) | Value::USet(u) => u.ver = n,
            _ => {}
        }
    }
    /// Bump the invalidation counter.
    pub fn bump(&mut self) {
        if let Some(v) = self.ver() {
            self.set_ver(v.wrapping_add(1));
        }
    }
    /// The static type of the value.
    pub fn ty(&self) -> Ty {
        match self {
            Value::Void => Ty::Void,
            Value::Uninit(t) => (**t).clone(),
            Value::Bool(_) => Ty::Bool,
            Value::Char(_) => Ty::Char,
            Value::Int(_) => Ty::Int,
            Value::UInt(_) => Ty::UInt,
            Value::LL(_) => Ty::LongLong,
            Value::Ull(_) => Ty::ULongLong,
            Value::Size(_) => Ty::Size,
            Value::Str(_) => Ty::Str,
            Value::View(_) => Ty::StrView,
            Value::Vector(v) => match v.kind {
                VKind::Vector => Ty::Vector(Box::new((*v.ety).clone())),
                VKind::Array => Ty::Array(Box::new((*v.ety).clone()), v.v.len()),
                VKind::Stack => Ty::Stack(Box::new((*v.ety).clone())),
            },
            Value::Deque(d) => match d.kind {
                DKind::Deque => Ty::Deque(Box::new((*d.ety).clone())),
                DKind::Queue => Ty::Queue(Box::new((*d.ety).clone())),
            },
            Value::List(l) => Ty::List(Box::new((*l.ety).clone())),
            Value::Pq(p) => {
                let c = match &p.cmp {
                    PqCmp::Less => Ty::Functor(Functor::Less, Box::new((*p.ety).clone())),
                    PqCmp::Greater => Ty::Functor(Functor::Greater, Box::new((*p.ety).clone())),
                    PqCmp::Fn(_) => Ty::Lambda(0),
                };
                Ty::PriorityQueue(Box::new((*p.ety).clone()), Box::new(c))
            }
            Value::Map(m) => Ty::Map(
                Box::new((*m.kty).clone()),
                Box::new(m.vty.as_ref().map_or(Ty::Unknown, |t| (**t).clone())),
            ),
            Value::Set(m) => Ty::Set(Box::new((*m.kty).clone())),
            Value::UMap(m) => Ty::UnorderedMap(
                Box::new((*m.kty).clone()),
                Box::new(m.vty.as_ref().map_or(Ty::Unknown, |t| (**t).clone())),
            ),
            Value::USet(m) => Ty::UnorderedSet(Box::new((*m.kty).clone())),
            Value::Pair(p) => Ty::Pair(Box::new(p[0].ty()), Box::new(p[1].ty())),
            Value::Tuple(t) => Ty::Tuple(t.iter().map(Value::ty).collect()),
            Value::Opt(o) => Ty::Optional(Box::new((*o.ety).clone())),
            Value::Struct(s) => Ty::Struct(s.name.to_string()),
            Value::Closure(c) => Ty::Lambda(c.lam),
            Value::FnRef(_) => Ty::Unknown,
            Value::Functor(f, t) => Ty::Functor(*f, Box::new((**t).clone())),
            Value::PNull => Ty::NullPtr,
            Value::PHeap(..) | Value::PPlace(_) => Ty::Ptr(Box::new(Ty::Unknown)),
            Value::Iter(_) => Ty::Unknown,
            Value::Ref(_) => Ty::Unknown,
        }
    }
}

fn scalar_elems(t: &Ty) -> bool {
    matches!(
        t,
        Ty::Bool
            | Ty::Char
            | Ty::Int
            | Ty::UInt
            | Ty::LongLong
            | Ty::ULongLong
            | Ty::Size
            | Ty::Ptr(_)
    )
}

/// Leaf values held (strings count their bytes): the cost of copying.
pub fn leaves(v: &Value) -> u64 {
    match v {
        Value::Str(s) => s.s.len() as u64,
        Value::View(_) => 1,
        Value::Vector(c) => {
            if scalar_elems(&c.ety) {
                c.v.len() as u64
            } else {
                c.v.iter().map(leaves).sum()
            }
        }
        Value::Deque(c) => {
            if scalar_elems(&c.ety) {
                c.v.len() as u64
            } else {
                c.v.iter().map(leaves).sum()
            }
        }
        Value::List(l) => l
            .nodes
            .iter()
            .filter_map(|n| n.val.as_ref())
            .map(leaves)
            .sum(),
        Value::Pq(p) => {
            if scalar_elems(&p.ety) {
                p.v.len() as u64
            } else {
                p.v.iter().map(leaves).sum()
            }
        }
        Value::Map(m) | Value::Set(m) => {
            m.m.iter().map(|(k, x)| leaves(&k.0) + sub_leaves(x)).sum()
        }
        Value::UMap(u) | Value::USet(u) => u.ents.iter().flatten().map(leaves).sum(),
        Value::Pair(p) => leaves(&p[0]) + leaves(&p[1]),
        Value::Tuple(t) => t.iter().map(leaves).sum(),
        Value::Opt(o) => o.v.as_ref().map_or(0, leaves),
        Value::Struct(s) => s.f.iter().map(leaves).sum(),
        Value::Void | Value::Ref(_) => 0,
        _ => 1,
    }
}

fn sub_leaves(v: &Value) -> u64 {
    match v {
        // a map entry is stored as Pair(key, value): the key is counted separately
        Value::Pair(p) => leaves(&p[1]),
        _ => 0,
    }
}

/// Memory cells held by a value (strings: bytes / 8 + 1, containers: 1 + elements).
pub fn cells(v: &Value) -> u64 {
    match v {
        Value::Str(s) => s.s.len() as u64 / 8 + 1,
        Value::View(_) => 1,
        Value::Vector(c) => {
            1 + if scalar_elems(&c.ety) {
                c.v.len() as u64
            } else {
                c.v.iter().map(cells).sum()
            }
        }
        Value::Deque(c) => {
            1 + if scalar_elems(&c.ety) {
                c.v.len() as u64
            } else {
                c.v.iter().map(cells).sum()
            }
        }
        Value::List(l) => {
            1 + l
                .nodes
                .iter()
                .filter_map(|n| n.val.as_ref())
                .map(cells)
                .sum::<u64>()
        }
        Value::Pq(p) => 1 + p.v.iter().map(cells).sum::<u64>(),
        Value::Map(m) | Value::Set(m) => {
            1 + m
                .m
                .iter()
                .map(|(k, x)| cells(&k.0) + sub_cells(x))
                .sum::<u64>()
        }
        Value::UMap(u) | Value::USet(u) => 1 + u.ents.iter().flatten().map(cells).sum::<u64>(),
        Value::Pair(p) => 1 + cells(&p[0]) + cells(&p[1]),
        Value::Tuple(t) => 1 + t.iter().map(cells).sum::<u64>(),
        Value::Opt(o) => 1 + o.v.as_ref().map_or(0, cells),
        Value::Struct(s) => 1 + s.f.iter().map(cells).sum::<u64>(),
        Value::Ref(_) | Value::Void => 0,
        _ => 1,
    }
}

fn sub_cells(v: &Value) -> u64 {
    match v {
        Value::Pair(p) => cells(&p[1]),
        _ => 0,
    }
}

/// A value used as an ordered or hashed key.
#[derive(Clone, Debug)]
pub struct Key(pub Value);

/// Total order on values of one C++ type (what `operator<` gives for the supported types).
pub fn cmp_val(a: &Value, b: &Value) -> Ordering {
    if let (Some(x), Some(y)) = (a.as_i128(), b.as_i128()) {
        return x.cmp(&y);
    }
    if let (Some(x), Some(y)) = (a.bytes(), b.bytes()) {
        return x.cmp(y);
    }
    match (a, b) {
        (Value::Pair(x), Value::Pair(y)) => {
            cmp_val(&x[0], &y[0]).then_with(|| cmp_val(&x[1], &y[1]))
        }
        (Value::Tuple(x), Value::Tuple(y)) => cmp_seq(x.iter(), y.iter()),
        (Value::Vector(x), Value::Vector(y)) => cmp_seq(x.v.iter(), y.v.iter()),
        (Value::Deque(x), Value::Deque(y)) => cmp_seq(x.v.iter(), y.v.iter()),
        (Value::Opt(x), Value::Opt(y)) => match (&x.v, &y.v) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(p), Some(q)) => cmp_val(p, q),
        },
        (Value::Struct(x), Value::Struct(y)) => cmp_seq(x.f.iter(), y.f.iter()),
        (Value::PNull, Value::PNull) => Ordering::Equal,
        (Value::PNull, _) => Ordering::Less,
        (_, Value::PNull) => Ordering::Greater,
        (Value::PHeap(i, g), Value::PHeap(j, h)) => (i, g).cmp(&(j, h)),
        (Value::PHeap(..), _) => Ordering::Less,
        (_, Value::PHeap(..)) => Ordering::Greater,
        _ => Ordering::Equal,
    }
}

fn cmp_seq<'a>(
    mut x: impl Iterator<Item = &'a Value>,
    mut y: impl Iterator<Item = &'a Value>,
) -> Ordering {
    loop {
        match (x.next(), y.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(p), Some(q)) => {
                let c = cmp_val(p, q);
                if c != Ordering::Equal {
                    return c;
                }
            }
        }
    }
}

impl PartialEq for Key {
    fn eq(&self, o: &Key) -> bool {
        cmp_val(&self.0, &o.0) == Ordering::Equal
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, o: &Key) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Key {
    fn cmp(&self, o: &Key) -> Ordering {
        cmp_val(&self.0, &o.0)
    }
}

fn hash_val<H: Hasher>(v: &Value, h: &mut H) {
    if let Some(i) = v.as_i128() {
        h.write_u64(i as u64);
        return;
    }
    if let Some(b) = v.bytes() {
        h.write(b);
        h.write_u64(b.len() as u64);
        return;
    }
    match v {
        Value::Pair(p) => {
            hash_val(&p[0], h);
            hash_val(&p[1], h);
        }
        Value::Tuple(t) => t.iter().for_each(|x| hash_val(x, h)),
        Value::Vector(c) => c.v.iter().for_each(|x| hash_val(x, h)),
        Value::Deque(c) => c.v.iter().for_each(|x| hash_val(x, h)),
        Value::Opt(o) => {
            if let Some(x) = &o.v {
                hash_val(x, h);
            }
        }
        Value::Struct(s) => s.f.iter().for_each(|x| hash_val(x, h)),
        Value::PHeap(i, g) => {
            h.write_u64(u64::from(*i));
            h.write_u64(u64::from(*g));
        }
        _ => h.write_u64(0),
    }
}

impl Hash for Key {
    fn hash<H: Hasher>(&self, h: &mut H) {
        hash_val(&self.0, h);
    }
}

/// Pointer equality.
pub fn ptr_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::PNull, Value::PNull) => true,
        (Value::PHeap(i, g), Value::PHeap(j, h)) => i == j && g == h,
        (Value::PPlace(p), Value::PPlace(q)) => p == q,
        _ => false,
    }
}

/// Structural equality of two values of the same type (`operator==`).
pub fn eq_val(a: &Value, b: &Value) -> bool {
    if a.is_ptr() || b.is_ptr() {
        return ptr_eq(a, b);
    }
    cmp_val(a, b) == Ordering::Equal
}

impl ListV {
    /// An empty list.
    pub fn new(ety: TyRc) -> ListV {
        ListV {
            ety,
            nodes: Vec::new(),
            head: NIL,
            tail: NIL,
            len: 0,
            free: Vec::new(),
        }
    }
    /// Insert before `before` (`NIL` = at the end); returns the new node.
    pub fn insert_before(&mut self, before: u32, v: Value) -> u32 {
        let prev = if before == NIL {
            self.tail
        } else {
            self.nodes[before as usize].prev
        };
        let node = LNode {
            val: Some(v),
            prev,
            next: before,
            gn: 0,
        };
        let id = if let Some(f) = self.free.pop() {
            let gn = self.nodes[f as usize].gn;
            self.nodes[f as usize] = LNode { gn, ..node };
            f
        } else {
            self.nodes.push(node);
            (self.nodes.len() - 1) as u32
        };
        if prev == NIL {
            self.head = id;
        } else {
            self.nodes[prev as usize].next = id;
        }
        if before == NIL {
            self.tail = id;
        } else {
            self.nodes[before as usize].prev = id;
        }
        self.len += 1;
        id
    }
    /// Unlink a node and return its value and its successor.
    pub fn remove(&mut self, id: u32) -> Option<(Value, u32)> {
        let n = self.nodes.get_mut(id as usize)?;
        let val = n.val.take()?;
        let (prev, next) = (n.prev, n.next);
        n.gn = n.gn.wrapping_add(1);
        if prev == NIL {
            self.head = next;
        } else {
            self.nodes[prev as usize].next = next;
        }
        if next == NIL {
            self.tail = prev;
        } else {
            self.nodes[next as usize].prev = prev;
        }
        self.free.push(id);
        self.len -= 1;
        Some((val, next))
    }
    /// Node indices in order.
    pub fn order(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.len);
        let mut cur = self.head;
        while cur != NIL {
            out.push(cur);
            cur = self.nodes[cur as usize].next;
        }
        out
    }
    /// Values in order.
    pub fn values(&self) -> Vec<&Value> {
        self.order()
            .into_iter()
            .filter_map(|i| self.nodes[i as usize].val.as_ref())
            .collect()
    }
    /// Is the node live?
    pub fn live(&self, id: u32, gn: u32) -> bool {
        self.nodes
            .get(id as usize)
            .is_some_and(|n| n.val.is_some() && n.gn == gn)
    }
}

impl UnV {
    /// An empty unordered container.
    pub fn new(kty: TyRc, vty: Option<TyRc>) -> UnV {
        UnV {
            kty,
            vty,
            idx: FxMap::default(),
            ents: Vec::new(),
            live: 0,
            ver: 0,
        }
    }
    /// Key of an entry value.
    pub fn key_of(&self, e: &Value) -> Key {
        match e {
            Value::Pair(p) => Key(p[0].clone()),
            o => Key(o.clone()),
        }
    }
    /// Insert a new entry (the caller checked it is absent).
    pub fn push(&mut self, k: Key, e: Value) {
        if self.ents.len() > 32 && self.live * 2 < self.ents.len() {
            self.compact();
        }
        self.idx.insert(k, self.ents.len());
        self.ents.push(Some(e));
        self.live += 1;
        self.ver = self.ver.wrapping_add(1);
    }
    /// Remove the entry with this key.
    pub fn remove(&mut self, k: &Key) -> Option<Value> {
        let i = self.idx.remove(k)?;
        self.live -= 1;
        self.ents[i].take()
    }
    fn compact(&mut self) {
        let old = std::mem::take(&mut self.ents);
        self.idx.clear();
        for e in old.into_iter().flatten() {
            let k = self.key_of(&e);
            self.idx.insert(k, self.ents.len());
            self.ents.push(Some(e));
        }
    }
    /// Index of the first live entry at or after `i` (or before, going backwards).
    pub fn next_live(&self, i: usize, back: bool) -> Option<usize> {
        if self.ents.is_empty() {
            return None;
        }
        if back {
            (0..=i.min(self.ents.len() - 1))
                .rev()
                .find(|&j| self.ents.get(j).is_some_and(Option::is_some))
        } else {
            (i..self.ents.len()).find(|&j| self.ents[j].is_some())
        }
    }
}

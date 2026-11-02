// SPDX-License-Identifier: Apache-2.0
//! Static name resolution: turns every variable use into a frame slot, a capture, a global or a function,
//! and computes the frame size of every function, constructor and lambda.

use crate::cpp::ast::*;
use std::collections::HashMap;

/// What a name expression refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarRef {
    /// Not resolved (library name or unknown).
    Unset,
    /// Slot of the current frame.
    Local(u32),
    /// Capture of the current lambda.
    Capture(u32),
    /// Global constant.
    Global(u32),
    /// Overload group of free functions.
    Func(u32),
    /// Field of the object under construction (inside a constructor).
    Field(u32),
    /// `this` inside a constructor.
    This,
}

/// Where a capture takes its value from in the creating frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Src {
    /// A slot.
    Local(u32),
    /// A capture of the enclosing lambda.
    Capture(u32),
}

/// One captured variable.
#[derive(Clone, Debug)]
pub struct CapInfo<'p> {
    /// Variable name.
    pub name: &'p str,
    /// Source in the creating frame.
    pub src: Src,
    /// Captured by reference.
    pub by_ref: bool,
}

/// A lambda with its frame layout: `nslots` locals (parameters first) followed by the captures.
#[derive(Debug)]
pub struct LamInfo<'p> {
    /// The lambda.
    pub lam: &'p Lambda,
    /// Locals including parameters.
    pub nslots: u32,
    /// Captures.
    pub caps: Vec<CapInfo<'p>>,
}

/// A free function with a body.
#[derive(Debug)]
pub struct FnInfo<'p> {
    /// Declaration.
    pub decl: &'p FnDecl,
    /// Frame size.
    pub nslots: u32,
}

/// A constructor: slot 0 is the object under construction, parameters follow.
#[derive(Debug)]
pub struct CtorInfo<'p> {
    /// Declaration.
    pub ctor: &'p Ctor,
    /// Frame size.
    pub nslots: u32,
}

/// A struct with resolved constructors.
#[derive(Debug)]
pub struct StructInfo<'p> {
    /// Declaration.
    pub decl: &'p StructDecl,
    /// Constructors.
    pub ctors: Vec<CtorInfo<'p>>,
}

/// A global constant.
#[derive(Debug)]
pub struct GlobalInfo<'p> {
    /// Declared type.
    pub spec: &'p TypeSpec,
    /// Declarator.
    pub decl: &'p Declarator,
}

/// All resolution results.
#[derive(Debug, Default)]
pub struct Resolved<'p> {
    var: Vec<VarRef>,
    slot: Vec<u32>,
    scope: Vec<(u32, u32)>,
    lam_idx: Vec<u32>,
    /// Free functions with bodies.
    pub fns: Vec<FnInfo<'p>>,
    /// Overload groups (indices into `fns`).
    pub groups: Vec<Vec<u32>>,
    /// Name of each group.
    pub group_names: Vec<&'p str>,
    /// Lambdas.
    pub lams: Vec<LamInfo<'p>>,
    /// Structs.
    pub structs: Vec<StructInfo<'p>>,
    /// Globals.
    pub globals: Vec<GlobalInfo<'p>>,
}

impl Resolved<'_> {
    /// Resolution of a `Var` expression.
    pub fn var(&self, id: NodeId) -> VarRef {
        self.var.get(id as usize).copied().unwrap_or(VarRef::Unset)
    }
    /// First slot of a declarator, parameter, structured binding or range-for statement.
    pub fn slot(&self, id: NodeId) -> u32 {
        self.slot.get(id as usize).copied().unwrap_or(u32::MAX)
    }
    /// Slots `[lo, hi)` opened by a scope statement.
    pub fn scope(&self, id: NodeId) -> (u32, u32) {
        self.scope.get(id as usize).copied().unwrap_or((0, 0))
    }
    /// Index into `lams` of a lambda expression.
    pub fn lam(&self, id: NodeId) -> Option<u32> {
        self.lam_idx
            .get(id as usize)
            .copied()
            .filter(|&i| i != u32::MAX)
    }
}

fn put<T: Clone>(v: &mut Vec<T>, id: NodeId, val: T, fill: T) {
    let i = id as usize;
    if i >= v.len() {
        if i > 50_000_000 {
            return;
        }
        v.resize(i + 1, fill);
    }
    v[i] = val;
}

enum Kind<'p> {
    Func,
    Ctor(&'p [Field]),
    Lambda {
        lam: &'p Lambda,
        caps: Vec<CapInfo<'p>>,
    },
}

struct Ctx<'p> {
    scopes: Vec<Vec<(&'p str, u32)>>,
    saved_max: Vec<u32>,
    next: u32,
    max: u32,
    kind: Kind<'p>,
}

impl<'p> Ctx<'p> {
    fn new(kind: Kind<'p>) -> Ctx<'p> {
        Ctx {
            scopes: vec![Vec::new()],
            saved_max: Vec::new(),
            next: 0,
            max: 0,
            kind,
        }
    }
}

struct Builder<'p> {
    out: Resolved<'p>,
    ctxs: Vec<Ctx<'p>>,
    globals: HashMap<&'p str, u32>,
    groups: HashMap<&'p str, u32>,
}

/// Resolve a whole program.
pub fn resolve(p: &Program) -> Resolved<'_> {
    let mut b = Builder {
        out: Resolved {
            var: Vec::with_capacity(p.next_id as usize),
            slot: Vec::with_capacity(p.next_id as usize),
            scope: Vec::with_capacity(p.next_id as usize),
            lam_idx: Vec::with_capacity(p.next_id as usize),
            ..Resolved::default()
        },
        ctxs: Vec::new(),
        globals: HashMap::new(),
        groups: HashMap::new(),
    };
    // pass 1: names of everything that can be referred to
    for it in &p.items {
        match &it.kind {
            ItemKind::Func(f) if f.body.is_some() => {
                let g = *b.groups.entry(f.name.as_str()).or_insert_with(|| {
                    b.out.groups.push(Vec::new());
                    b.out.group_names.push(f.name.as_str());
                    (b.out.groups.len() - 1) as u32
                });
                b.out.groups[g as usize].push(b.out.fns.len() as u32);
                b.out.fns.push(FnInfo { decl: f, nslots: 0 });
            }
            ItemKind::Struct(s) => b.out.structs.push(StructInfo {
                decl: s,
                ctors: Vec::new(),
            }),
            _ => {}
        }
    }
    // pass 2: bodies in program order (globals are visible only after their declaration)
    let mut fi = 0usize;
    let mut si = 0usize;
    for it in &p.items {
        match &it.kind {
            ItemKind::Func(f) if f.body.is_some() => {
                b.func(f, fi);
                fi += 1;
            }
            ItemKind::Struct(s) => {
                b.strukt(s, si);
                si += 1;
            }
            ItemKind::Global { spec, vars, .. } => {
                for d in vars {
                    b.ctxs.push(Ctx::new(Kind::Func));
                    b.declarator_exprs(d);
                    b.ctxs.pop();
                    let g = b.out.globals.len() as u32;
                    b.out.globals.push(GlobalInfo { spec, decl: d });
                    b.globals.insert(d.name.as_str(), g);
                }
            }
            _ => {}
        }
    }
    b.out
}

impl<'p> Builder<'p> {
    fn top(&mut self) -> &mut Ctx<'p> {
        let n = self.ctxs.len() - 1;
        &mut self.ctxs[n]
    }
    fn alloc(&mut self) -> u32 {
        let c = self.top();
        let s = c.next;
        c.next += 1;
        c.max = c.max.max(c.next);
        s
    }
    fn declare(&mut self, name: &'p str) -> u32 {
        let s = self.alloc();
        if !name.is_empty() {
            let c = self.top();
            if let Some(sc) = c.scopes.last_mut() {
                sc.push((name, s));
            }
        }
        s
    }
    fn push_scope(&mut self) -> u32 {
        let c = self.top();
        c.scopes.push(Vec::new());
        c.saved_max.push(c.max);
        c.max = c.next;
        c.next
    }
    fn pop_scope(&mut self, id: NodeId, lo: u32) {
        let c = self.top();
        c.scopes.pop();
        let hi = c.max;
        c.max = c.saved_max.pop().unwrap_or(0).max(hi);
        c.next = lo;
        put(&mut self.out.scope, id, (lo, hi), (0, 0));
    }
    fn func(&mut self, f: &'p FnDecl, idx: usize) {
        self.ctxs.push(Ctx::new(Kind::Func));
        for p in &f.params {
            let s = self.declare(&p.name);
            put(&mut self.out.slot, p.id, s, u32::MAX);
        }
        if let Some(b) = &f.body {
            self.block_stmts(b);
        }
        let c = self.ctxs.pop();
        self.out.fns[idx].nslots = c.map_or(0, |c| c.max);
    }
    fn strukt(&mut self, s: &'p StructDecl, idx: usize) {
        for f in &s.fields {
            if let Some(d) = &f.default {
                self.ctxs.push(Ctx::new(Kind::Func));
                self.expr(d);
                self.ctxs.pop();
            }
        }
        for c in &s.ctors {
            self.ctxs.push(Ctx::new(Kind::Ctor(&s.fields)));
            self.alloc(); // this
            for p in &c.params {
                let sl = self.declare(&p.name);
                put(&mut self.out.slot, p.id, sl, u32::MAX);
            }
            for (_, args) in &c.inits {
                for a in args {
                    self.expr(a);
                }
            }
            self.block_stmts(&c.body);
            let cx = self.ctxs.pop();
            self.out.structs[idx].ctors.push(CtorInfo {
                ctor: c,
                nslots: cx.map_or(1, |c| c.max),
            });
        }
    }
    fn block_stmts(&mut self, b: &'p Block) {
        for s in &b.stmts {
            self.stmt(s);
        }
    }
    fn declarator_exprs(&mut self, d: &'p Declarator) {
        let s = self.declare(&d.name);
        put(&mut self.out.slot, d.id, s, u32::MAX);
        if let Some(a) = &d.array {
            self.expr(a);
        }
        match &d.init {
            Init::None => {}
            Init::Copy(e) => self.expr(e),
            Init::Direct(a) | Init::Brace { elems: a, .. } => a.iter().for_each(|e| self.expr(e)),
        }
    }
    fn stmt(&mut self, s: &'p Stmt) {
        match &s.kind {
            StmtKind::Decl { vars, .. } => {
                for d in vars {
                    self.declarator_exprs(d);
                }
            }
            StmtKind::Structured { names, init, .. } => {
                self.expr(init);
                let first = self.alloc();
                put(&mut self.out.slot, s.id, first, u32::MAX);
                for (i, n) in names.iter().enumerate() {
                    if i > 0 {
                        self.alloc();
                    }
                    let c = self.top();
                    if let Some(sc) = c.scopes.last_mut() {
                        sc.push((n.as_str(), first + i as u32));
                    }
                }
            }
            StmtKind::Expr(e) => self.expr(e),
            StmtKind::If {
                init,
                cond,
                then,
                els,
            } => {
                let lo = self.push_scope();
                if let Some(i) = init {
                    self.stmt(i);
                }
                self.expr(cond);
                self.stmt(then);
                if let Some(e) = els {
                    self.stmt(e);
                }
                self.pop_scope(s.id, lo);
            }
            StmtKind::Switch { cond, body } => {
                self.expr(cond);
                let lo = self.push_scope();
                self.block_stmts(body);
                self.pop_scope(s.id, lo);
            }
            StmtKind::Case(e) => {
                if let Some(e) = e {
                    self.expr(e);
                }
            }
            StmtKind::For {
                init,
                cond,
                incr,
                body,
            } => {
                let lo = self.push_scope();
                if let Some(i) = init {
                    self.stmt(i);
                }
                if let Some(c) = cond {
                    self.expr(c);
                }
                if let Some(i) = incr {
                    self.expr(i);
                }
                self.stmt(body);
                self.pop_scope(s.id, lo);
            }
            StmtKind::RangeFor {
                names, range, body, ..
            } => {
                self.expr(range);
                let lo = self.push_scope();
                let first = self.alloc();
                put(&mut self.out.slot, s.id, first, u32::MAX);
                for (i, n) in names.iter().enumerate() {
                    if i > 0 {
                        self.alloc();
                    }
                    let c = self.top();
                    if let Some(sc) = c.scopes.last_mut() {
                        sc.push((n.as_str(), first + i as u32));
                    }
                }
                self.stmt(body);
                self.pop_scope(s.id, lo);
            }
            StmtKind::While { cond, body } => {
                self.expr(cond);
                self.stmt(body);
            }
            StmtKind::DoWhile { body, cond } => {
                self.stmt(body);
                self.expr(cond);
            }
            StmtKind::Return(e) => {
                if let Some(e) = e {
                    self.expr(e);
                }
            }
            StmtKind::Block(b) => {
                let lo = self.push_scope();
                self.block_stmts(b);
                self.pop_scope(s.id, lo);
            }
            StmtKind::Break | StmtKind::Continue | StmtKind::Empty => {}
        }
    }

    fn lookup(&mut self, d: usize, name: &'p str) -> Option<VarRef> {
        let c = &self.ctxs[d];
        for sc in c.scopes.iter().rev() {
            if let Some((_, s)) = sc.iter().rev().find(|(n, _)| *n == name) {
                return Some(VarRef::Local(*s));
            }
        }
        let lam: &'p Lambda = match &c.kind {
            Kind::Func => return None,
            Kind::Ctor(fields) => {
                if name == "this" {
                    return Some(VarRef::This);
                }
                return fields
                    .iter()
                    .position(|f| f.name == name)
                    .map(|i| VarRef::Field(i as u32));
            }
            Kind::Lambda { lam, caps } => {
                if let Some(i) = caps.iter().position(|c| c.name == name) {
                    return Some(VarRef::Capture(i as u32));
                }
                lam
            }
        };
        if d == 0 {
            return None;
        }
        let src = match self.lookup(d - 1, name)? {
            VarRef::Local(s) => Src::Local(s),
            VarRef::Capture(i) => Src::Capture(i),
            _ => return None,
        };
        let by_ref = match lam.captures.items.iter().find(|(n, _)| n == name) {
            Some((_, r)) => *r,
            None => lam.captures.default == CaptureDefault::Ref,
        };
        if let Kind::Lambda { caps, .. } = &mut self.ctxs[d].kind {
            caps.push(CapInfo { name, src, by_ref });
            Some(VarRef::Capture((caps.len() - 1) as u32))
        } else {
            None
        }
    }

    fn expr(&mut self, e: &'p Expr) {
        match &e.kind {
            ExprKind::Var(name) => {
                let d = self.ctxs.len().wrapping_sub(1);
                let r = if self.ctxs.is_empty() {
                    None
                } else {
                    self.lookup(d, name)
                };
                let r = r
                    .or_else(|| self.globals.get(name.as_str()).map(|&g| VarRef::Global(g)))
                    .or_else(|| self.groups.get(name.as_str()).map(|&g| VarRef::Func(g)))
                    .unwrap_or(VarRef::Unset);
                put(&mut self.out.var, e.id, r, VarRef::Unset);
            }
            ExprKind::Lambda(l) => {
                self.ctxs.push(Ctx::new(Kind::Lambda {
                    lam: l,
                    caps: Vec::new(),
                }));
                for p in &l.params {
                    let s = self.declare(&p.name);
                    put(&mut self.out.slot, p.id, s, u32::MAX);
                }
                self.block_stmts(&l.body);
                if let Some(c) = self.ctxs.pop()
                    && let Kind::Lambda { caps, .. } = c.kind
                {
                    self.out.lams.push(LamInfo {
                        lam: l,
                        nslots: c.max,
                        caps,
                    });
                    let idx = (self.out.lams.len() - 1) as u32;
                    put(&mut self.out.lam_idx, e.id, idx, u32::MAX);
                }
            }
            _ => expr_children(e, &mut |c| self.expr(c)),
        }
    }
}

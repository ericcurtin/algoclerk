// SPDX-License-Identifier: Apache-2.0
//! The ClerkScript syntax tree. Every statement and expression has a [`NodeId`] so that the
//! checker, the interpreter trace, the block editor and the error squiggles can all point at it.

use std::fmt;

/// Identity of a statement or expression inside one [`Program`].
pub type NodeId = u32;

/// A byte range in the program text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    /// First byte.
    pub start: u32,
    /// One past the last byte.
    pub end: u32,
}

/// A ClerkScript type. `Var` and `Null` only exist during inference.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ty {
    /// 64-bit signed integer.
    Int,
    /// true / false.
    Bool,
    /// One ASCII character.
    Char,
    /// ASCII text.
    Str,
    /// No value (functions that return nothing).
    Unit,
    /// Growable array.
    Vec(Box<Ty>),
    /// Set of hashable values.
    Set(Box<Ty>),
    /// Map from hashable keys to values.
    Map(Box<Ty>, Box<Ty>),
    /// Fixed-size group of values (two or more).
    Tuple(Vec<Ty>),
    /// Maybe a value.
    Option(Box<Ty>),
    /// Last in, first out.
    Stack(Box<Ty>),
    /// First in, first out.
    Queue(Box<Ty>),
    /// Double-ended queue.
    Deque(Box<Ty>),
    /// Max-heap priority queue.
    Heap(Box<Ty>),
    /// Singly linked list node (nullable handle).
    LNode,
    /// Binary tree node (nullable handle).
    TNode,
    /// The `null` literal before it meets a node type.
    Null,
    /// Inference variable.
    Var(u32),
}

impl Ty {
    /// `vec<t>`.
    pub fn vec(t: Ty) -> Ty {
        Ty::Vec(Box::new(t))
    }
    /// Scalar types that are cheap to copy and need no cloning in generated code.
    pub fn is_scalar(&self) -> bool {
        matches!(
            self,
            Ty::Int | Ty::Bool | Ty::Char | Ty::LNode | Ty::TNode | Ty::Null | Ty::Unit
        )
    }
    /// The unlock name of the type (`"vec"`, `"map"`, ...).
    pub fn family(&self) -> &'static str {
        match self {
            Ty::Int => "int",
            Ty::Bool => "bool",
            Ty::Char => "char",
            Ty::Str => "string",
            Ty::Unit => "unit",
            Ty::Vec(_) => "vec",
            Ty::Set(_) => "set",
            Ty::Map(..) => "map",
            Ty::Tuple(_) => "tuple",
            Ty::Option(_) => "option",
            Ty::Stack(_) => "stack",
            Ty::Queue(_) => "queue",
            Ty::Deque(_) => "deque",
            Ty::Heap(_) => "heap",
            Ty::LNode => "lnode",
            Ty::TNode => "tnode",
            Ty::Null => "null",
            Ty::Var(_) => "?",
        }
    }
    /// Every type family appearing inside this type (including itself).
    pub fn families(&self, out: &mut Vec<&'static str>) {
        out.push(self.family());
        match self {
            Ty::Vec(t)
            | Ty::Set(t)
            | Ty::Option(t)
            | Ty::Stack(t)
            | Ty::Queue(t)
            | Ty::Deque(t)
            | Ty::Heap(t) => t.families(out),
            Ty::Map(k, v) => {
                k.families(out);
                v.families(out);
            }
            Ty::Tuple(ts) => ts.iter().for_each(|t| t.families(out)),
            _ => {}
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Int => f.write_str("int"),
            Ty::Bool => f.write_str("bool"),
            Ty::Char => f.write_str("char"),
            Ty::Str => f.write_str("string"),
            Ty::Unit => f.write_str("()"),
            Ty::Vec(t) => write!(f, "vec<{t}>"),
            Ty::Set(t) => write!(f, "set<{t}>"),
            Ty::Map(k, v) => write!(f, "map<{k}, {v}>"),
            Ty::Tuple(ts) => {
                f.write_str("(")?;
                for (i, t) in ts.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{t}")?;
                }
                f.write_str(")")
            }
            Ty::Option(t) => write!(f, "option<{t}>"),
            Ty::Stack(t) => write!(f, "stack<{t}>"),
            Ty::Queue(t) => write!(f, "queue<{t}>"),
            Ty::Deque(t) => write!(f, "deque<{t}>"),
            Ty::Heap(t) => write!(f, "heap<{t}>"),
            Ty::LNode => f.write_str("lnode"),
            Ty::TNode => f.write_str("tnode"),
            Ty::Null => f.write_str("null"),
            Ty::Var(_) => f.write_str("_"),
        }
    }
}

/// Binary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `and`
    And,
    /// `or`
    Or,
}

impl BinOp {
    /// Source spelling.
    pub fn text(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "and",
            BinOp::Or => "or",
        }
    }
    /// Binding strength; larger binds tighter.
    pub fn prec(self) -> u8 {
        match self {
            BinOp::Or => 1,
            BinOp::And => 2,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 4,
            BinOp::BitOr => 5,
            BinOp::BitXor => 6,
            BinOp::BitAnd => 7,
            BinOp::Shl | BinOp::Shr => 8,
            BinOp::Add | BinOp::Sub => 9,
            BinOp::Mul | BinOp::Div | BinOp::Rem => 10,
        }
    }
    /// All operators that may be used in compound assignment (`+=` ...).
    pub fn assignable(self) -> bool {
        !matches!(
            self,
            BinOp::Eq
                | BinOp::Ne
                | BinOp::Lt
                | BinOp::Le
                | BinOp::Gt
                | BinOp::Ge
                | BinOp::And
                | BinOp::Or
        )
    }
}

/// Unary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    /// `-x`
    Neg,
    /// `not x`
    Not,
    /// `~x`
    BitNot,
}

/// An expression.
#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    /// Identity.
    pub id: NodeId,
    /// Shape.
    pub kind: ExprKind,
}

/// Expression shapes.
#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    /// Integer literal.
    Int(i64),
    /// `true` / `false`.
    Bool(bool),
    /// Character literal.
    Char(u8),
    /// String literal.
    Str(String),
    /// `null` node handle.
    Null,
    /// `none` option.
    NoneLit,
    /// Variable.
    Var(String),
    /// Unary operation.
    Unary(UnOp, Box<Expr>),
    /// Binary operation.
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `a[i]`.
    Index(Box<Expr>, Box<Expr>),
    /// `a[i..j]`, `a[..j]`, `a[i..]`.
    Slice(Box<Expr>, Option<Box<Expr>>, Option<Box<Expr>>),
    /// `node.val`, `node.next`, `node.left`, `node.right`.
    Field(Box<Expr>, String),
    /// `pair.0`.
    TupleIdx(Box<Expr>, usize),
    /// `name(args)`: built-in function, constructor-like built-in or user function.
    Call(String, Vec<Expr>),
    /// `recv.name(args)`.
    Method(Box<Expr>, String, Vec<Expr>),
    /// `[a, b, c]`.
    VecLit(Vec<Expr>),
    /// `[x; n]`.
    VecRepeat(Box<Expr>, Box<Expr>),
    /// `(a, b)`.
    TupleLit(Vec<Expr>),
    /// `stack<int>()`, `map<string, int>()` ...
    Ctor(Ty),
}

/// A binding pattern.
#[derive(Clone, Debug, PartialEq)]
pub enum Pat {
    /// Name a value.
    Var(String),
    /// Ignore a value.
    Wild,
    /// Destructure a tuple.
    Tuple(Vec<Pat>),
}

/// A statement.
#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    /// Identity.
    pub id: NodeId,
    /// Shape.
    pub kind: StmtKind,
}

/// Statement shapes.
#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
    /// `let pat [: ty] = init;`
    Let {
        /// Pattern.
        pat: Pat,
        /// Optional annotation.
        ty: Option<Ty>,
        /// Value.
        init: Expr,
    },
    /// `target = value;` or `target op= value;`
    Assign {
        /// Place being written.
        target: Expr,
        /// Compound operator.
        op: Option<BinOp>,
        /// Value.
        value: Expr,
    },
    /// `if cond { } [else { }]`; `else if` is an else block holding one `If`.
    If {
        /// Condition.
        cond: Expr,
        /// Then body.
        then: Block,
        /// Else body.
        els: Option<Block>,
    },
    /// `while cond { }`
    While {
        /// Condition.
        cond: Expr,
        /// Body.
        body: Block,
    },
    /// `for i in range(a, b, step) { }`
    ForRange {
        /// Loop variable.
        var: String,
        /// Start (default 0).
        start: Option<Expr>,
        /// End (exclusive).
        end: Expr,
        /// Step (default 1; may be negative).
        step: Option<Expr>,
        /// Body.
        body: Block,
    },
    /// `for pat in collection { }`
    ForEach {
        /// Pattern.
        pat: Pat,
        /// Collection.
        iter: Expr,
        /// Body.
        body: Block,
    },
    /// `break;`
    Break,
    /// `continue;`
    Continue,
    /// `return [expr];`
    Return(Option<Expr>),
    /// `expr;`
    Expr(Expr),
}

/// A list of statements.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Block {
    /// Statements in order.
    pub stmts: Vec<Stmt>,
}

/// A function parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    /// Name.
    pub name: String,
    /// Type.
    pub ty: Ty,
    /// `&mut` parameter: the callee changes the caller's variable.
    pub by_ref: bool,
}

/// A function.
#[derive(Clone, Debug, PartialEq)]
pub struct FnDecl {
    /// Identity.
    pub id: NodeId,
    /// Name.
    pub name: String,
    /// Parameters.
    pub params: Vec<Param>,
    /// Return type (`Ty::Unit` when absent).
    pub ret: Ty,
    /// Body.
    pub body: Block,
}

/// A whole program: the player's functions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Program {
    /// Functions in source order.
    pub fns: Vec<FnDecl>,
    /// Next unused [`NodeId`].
    pub next_id: NodeId,
    /// Source spans by node id (empty entries for synthetic nodes).
    pub spans: Vec<Span>,
}

impl Program {
    /// Allocate a fresh node id (for programmatic construction and block editing).
    pub fn fresh_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    /// Source span of a node, if the node came from parsed text.
    pub fn span(&self, id: NodeId) -> Option<Span> {
        self.spans
            .get(id as usize)
            .copied()
            .filter(|s| s.end > s.start)
    }
    /// Find a function by name.
    pub fn find(&self, name: &str) -> Option<&FnDecl> {
        self.fns.iter().find(|f| f.name == name)
    }
    /// Number of statements and expression nodes ("size" used by the SIZE medal counts statements and
    /// operations; see [`Program::size`]).
    pub fn size(&self) -> u32 {
        fn expr(e: &Expr) -> u32 {
            1 + match &e.kind {
                ExprKind::Unary(_, a) | ExprKind::Field(a, _) | ExprKind::TupleIdx(a, _) => expr(a),
                ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::VecRepeat(a, b) => {
                    expr(a) + expr(b)
                }
                ExprKind::Slice(a, b, c) => {
                    expr(a) + b.as_ref().map_or(0, |x| expr(x)) + c.as_ref().map_or(0, |x| expr(x))
                }
                ExprKind::Call(_, a) | ExprKind::VecLit(a) | ExprKind::TupleLit(a) => {
                    a.iter().map(expr).sum()
                }
                ExprKind::Method(r, _, a) => expr(r) + a.iter().map(expr).sum::<u32>(),
                _ => 0,
            }
        }
        fn block(b: &Block) -> u32 {
            b.stmts.iter().map(stmt).sum()
        }
        fn stmt(s: &Stmt) -> u32 {
            1 + match &s.kind {
                StmtKind::Let { init, .. } => expr(init),
                StmtKind::Assign { target, value, .. } => expr(target) + expr(value),
                StmtKind::If { cond, then, els } => {
                    expr(cond) + block(then) + els.as_ref().map_or(0, block)
                }
                StmtKind::While { cond, body } => expr(cond) + block(body),
                StmtKind::ForRange {
                    start,
                    end,
                    step,
                    body,
                    ..
                } => {
                    start.as_ref().map_or(0, expr)
                        + expr(end)
                        + step.as_ref().map_or(0, expr)
                        + block(body)
                }
                StmtKind::ForEach { iter, body, .. } => expr(iter) + block(body),
                StmtKind::Return(e) => e.as_ref().map_or(0, expr),
                StmtKind::Expr(e) => expr(e),
                StmtKind::Break | StmtKind::Continue => 0,
            }
        }
        self.fns.iter().map(|f| block(&f.body)).sum()
    }
}

/// Walk every statement (depth first, in source order).
pub fn walk_stmts<'a>(b: &'a Block, f: &mut dyn FnMut(&'a Stmt)) {
    for s in &b.stmts {
        f(s);
        match &s.kind {
            StmtKind::If { then, els, .. } => {
                walk_stmts(then, f);
                if let Some(e) = els {
                    walk_stmts(e, f);
                }
            }
            StmtKind::While { body, .. }
            | StmtKind::ForRange { body, .. }
            | StmtKind::ForEach { body, .. } => walk_stmts(body, f),
            _ => {}
        }
    }
}

/// Walk an expression tree (pre-order).
pub fn walk_expr<'a>(e: &'a Expr, f: &mut dyn FnMut(&'a Expr)) {
    f(e);
    match &e.kind {
        ExprKind::Unary(_, a) | ExprKind::Field(a, _) | ExprKind::TupleIdx(a, _) => walk_expr(a, f),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::VecRepeat(a, b) => {
            walk_expr(a, f);
            walk_expr(b, f);
        }
        ExprKind::Slice(a, b, c) => {
            walk_expr(a, f);
            if let Some(b) = b {
                walk_expr(b, f);
            }
            if let Some(c) = c {
                walk_expr(c, f);
            }
        }
        ExprKind::Call(_, a) | ExprKind::VecLit(a) | ExprKind::TupleLit(a) => {
            a.iter().for_each(|x| walk_expr(x, f))
        }
        ExprKind::Method(r, _, a) => {
            walk_expr(r, f);
            a.iter().for_each(|x| walk_expr(x, f));
        }
        _ => {}
    }
}

/// Visit every expression directly owned by a statement (not those of nested blocks).
pub fn stmt_exprs<'a>(s: &'a Stmt, f: &mut dyn FnMut(&'a Expr)) {
    match &s.kind {
        StmtKind::Let { init, .. } => f(init),
        StmtKind::Assign { target, value, .. } => {
            f(target);
            f(value);
        }
        StmtKind::If { cond, .. } | StmtKind::While { cond, .. } => f(cond),
        StmtKind::ForRange {
            start, end, step, ..
        } => {
            if let Some(x) = start {
                f(x);
            }
            f(end);
            if let Some(x) = step {
                f(x);
            }
        }
        StmtKind::ForEach { iter, .. } => f(iter),
        StmtKind::Return(Some(e)) | StmtKind::Expr(e) => f(e),
        _ => {}
    }
}

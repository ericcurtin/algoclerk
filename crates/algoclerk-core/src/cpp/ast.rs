// SPDX-License-Identifier: Apache-2.0
//! Syntax tree of the C++17 subset that players write (see `docs/CPP_SUBSET.md`).
//!
//! Every statement, expression, declarator and parameter has a [`NodeId`] so that the checker, the
//! interpreter trace, the card editor and the error squiggles can all point at it. The tree is a plain
//! syntax tree: types are *spelled* by the player ([`TypeSpec`]), semantic information (the type of every
//! expression) lives in the checker's side tables ([`crate::cpp::check::TypeInfo`]).

use std::fmt;

/// Identity of a node inside one [`Program`].
pub type NodeId = u32;

/// A byte range in the player's source text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    /// First byte.
    pub start: u32,
    /// One past the last byte.
    pub end: u32,
}

/// A C++ type of the subset. `Auto`, `InitList`, `NullPtr` and `Unknown` mostly exist for the checker.
///
/// Only types whose size is identical on every supported platform exist: `int` is 32 bits, `long long`
/// 64 bits, `size_t` 64 bits. `long`, `double`, `float`, `short` and the `<cstdint>` types are not part of
/// the subset (see `docs/CPP_SUBSET.md`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    /// `void`
    Void,
    /// `bool`
    Bool,
    /// `char` (signed 8 bit; the subset is ASCII only)
    Char,
    /// `int` (32 bit signed)
    Int,
    /// `unsigned` / `unsigned int` (32 bit)
    UInt,
    /// `long long`
    LongLong,
    /// `unsigned long long`
    ULongLong,
    /// `size_t` / `std::size_t` (64 bit unsigned; distinct from `unsigned long long` for deduction)
    Size,
    /// `std::string`
    Str,
    /// `std::string_view`
    StrView,
    /// `std::vector<T>`
    Vector(Box<Ty>),
    /// `std::array<T, N>` and C arrays `T a[N]`
    Array(Box<Ty>, usize),
    /// `std::pair<A, B>`
    Pair(Box<Ty>, Box<Ty>),
    /// `std::tuple<A, B, ...>`
    Tuple(Vec<Ty>),
    /// `std::optional<T>`
    Optional(Box<Ty>),
    /// `std::unordered_map<K, V>`
    UnorderedMap(Box<Ty>, Box<Ty>),
    /// `std::map<K, V>`
    Map(Box<Ty>, Box<Ty>),
    /// `std::unordered_set<T>`
    UnorderedSet(Box<Ty>),
    /// `std::set<T>`
    Set(Box<Ty>),
    /// `std::stack<T>`
    Stack(Box<Ty>),
    /// `std::queue<T>`
    Queue(Box<Ty>),
    /// `std::deque<T>`
    Deque(Box<Ty>),
    /// `std::list<T>`
    List(Box<Ty>),
    /// `std::priority_queue<T, std::vector<T>, Cmp>`; `Cmp` is a [`Ty::Functor`] or a [`Ty::Lambda`]
    PriorityQueue(Box<Ty>, Box<Ty>),
    /// `std::less<T>` / `std::greater<T>`
    Functor(Functor, Box<Ty>),
    /// `std::function<R(A, B)>`
    Function(Vec<Ty>, Box<Ty>),
    /// The unnamed closure type of the lambda with this node id
    Lambda(NodeId),
    /// `T*` (only pointers to structs are usable)
    Ptr(Box<Ty>),
    /// A struct declared in the prelude (`ListNode`, `TreeNode`) or by the player when unlocked
    Struct(String),
    /// Iterator into a container of this type; `reverse` for `rbegin()` / `rend()`
    Iter {
        /// The container type (never a reference).
        container: Box<Ty>,
        /// A reverse iterator.
        reverse: bool,
    },
    /// `auto` (only in declarations before checking)
    Auto,
    /// `std::nullptr_t`
    NullPtr,
    /// Type of a bare `{...}` before it meets its target (checker only)
    InitList,
    /// A type the checker could not work out (already reported)
    Unknown,
}

/// The comparison functor templates of `<functional>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Functor {
    /// `std::less<T>`
    Less,
    /// `std::greater<T>`
    Greater,
}

impl Ty {
    /// `std::vector<t>`.
    pub fn vector(t: Ty) -> Ty {
        Ty::Vector(Box::new(t))
    }
    /// Integral types (including `bool` and `char`).
    pub fn is_integral(&self) -> bool {
        matches!(
            self,
            Ty::Bool
                | Ty::Char
                | Ty::Int
                | Ty::UInt
                | Ty::LongLong
                | Ty::ULongLong
                | Ty::Size
        )
    }
    /// Unsigned integer types (`bool` is treated as unsigned).
    pub fn is_unsigned(&self) -> bool {
        matches!(self, Ty::Bool | Ty::UInt | Ty::ULongLong | Ty::Size)
    }
    /// Bit width of an integral type.
    pub fn bits(&self) -> u32 {
        match self {
            Ty::Bool => 1,
            Ty::Char => 8,
            Ty::Int | Ty::UInt => 32,
            _ => 64,
        }
    }
    /// The library header that provides the type's name, if any.
    pub fn header(&self) -> Option<&'static str> {
        Some(match self {
            Ty::Str => "<string>",
            Ty::StrView => "<string_view>",
            Ty::Vector(_) => "<vector>",
            Ty::Array(..) => "<array>",
            Ty::Pair(..) => "<utility>",
            Ty::Tuple(_) => "<tuple>",
            Ty::Optional(_) => "<optional>",
            Ty::UnorderedMap(..) => "<unordered_map>",
            Ty::Map(..) => "<map>",
            Ty::UnorderedSet(_) => "<unordered_set>",
            Ty::Set(_) => "<set>",
            Ty::Stack(_) => "<stack>",
            Ty::Queue(_) => "<queue>",
            Ty::Deque(_) => "<deque>",
            Ty::List(_) => "<list>",
            Ty::PriorityQueue(..) => "<queue>",
            Ty::Functor(..) | Ty::Function(..) => "<functional>",
            _ => return None,
        })
    }
    /// Every type nested inside this one (including itself), pre-order.
    pub fn walk(&self, f: &mut dyn FnMut(&Ty)) {
        f(self);
        match self {
            Ty::Vector(t)
            | Ty::Array(t, _)
            | Ty::Optional(t)
            | Ty::UnorderedSet(t)
            | Ty::Set(t)
            | Ty::Stack(t)
            | Ty::Queue(t)
            | Ty::Deque(t)
            | Ty::List(t)
            | Ty::Ptr(t)
            | Ty::Functor(_, t) => t.walk(f),
            Ty::Pair(a, b) | Ty::UnorderedMap(a, b) | Ty::Map(a, b) | Ty::PriorityQueue(a, b) => {
                a.walk(f);
                b.walk(f);
            }
            Ty::Tuple(ts) => ts.iter().for_each(|t| t.walk(f)),
            Ty::Function(args, r) => {
                args.iter().for_each(|t| t.walk(f));
                r.walk(f);
            }
            Ty::Iter { container, .. } => container.walk(f),
            _ => {}
        }
    }
}

impl fmt::Display for Ty {
    /// The type as C++ spells it with the `std::` prefix (what the watch panel shows).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let list = |f: &mut fmt::Formatter<'_>, ts: &[Ty]| -> fmt::Result {
            for (i, t) in ts.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{t}")?;
            }
            Ok(())
        };
        match self {
            Ty::Void => f.write_str("void"),
            Ty::Bool => f.write_str("bool"),
            Ty::Char => f.write_str("char"),
            Ty::Int => f.write_str("int"),
            Ty::UInt => f.write_str("unsigned"),
            Ty::LongLong => f.write_str("long long"),
            Ty::ULongLong => f.write_str("unsigned long long"),
            Ty::Size => f.write_str("size_t"),
            Ty::Str => f.write_str("std::string"),
            Ty::StrView => f.write_str("std::string_view"),
            Ty::Vector(t) => write!(f, "std::vector<{t}>"),
            Ty::Array(t, n) => write!(f, "std::array<{t}, {n}>"),
            Ty::Pair(a, b) => write!(f, "std::pair<{a}, {b}>"),
            Ty::Tuple(ts) => {
                f.write_str("std::tuple<")?;
                list(f, ts)?;
                f.write_str(">")
            }
            Ty::Optional(t) => write!(f, "std::optional<{t}>"),
            Ty::UnorderedMap(k, v) => write!(f, "std::unordered_map<{k}, {v}>"),
            Ty::Map(k, v) => write!(f, "std::map<{k}, {v}>"),
            Ty::UnorderedSet(t) => write!(f, "std::unordered_set<{t}>"),
            Ty::Set(t) => write!(f, "std::set<{t}>"),
            Ty::Stack(t) => write!(f, "std::stack<{t}>"),
            Ty::Queue(t) => write!(f, "std::queue<{t}>"),
            Ty::Deque(t) => write!(f, "std::deque<{t}>"),
            Ty::List(t) => write!(f, "std::list<{t}>"),
            Ty::PriorityQueue(t, c) => match &**c {
                Ty::Functor(Functor::Less, _) => write!(f, "std::priority_queue<{t}>"),
                c => write!(f, "std::priority_queue<{t}, std::vector<{t}>, {c}>"),
            },
            Ty::Functor(Functor::Less, t) => write!(f, "std::less<{t}>"),
            Ty::Functor(Functor::Greater, t) => write!(f, "std::greater<{t}>"),
            Ty::Function(args, r) => {
                write!(f, "std::function<{r}(")?;
                list(f, args)?;
                f.write_str(")>")
            }
            Ty::Lambda(_) => f.write_str("(lambda)"),
            Ty::Ptr(t) => write!(f, "{t}*"),
            Ty::Struct(n) => f.write_str(n),
            Ty::Iter { container, reverse } => {
                if *reverse {
                    write!(f, "{container}::reverse_iterator")
                } else {
                    write!(f, "{container}::iterator")
                }
            }
            Ty::Auto => f.write_str("auto"),
            Ty::NullPtr => f.write_str("std::nullptr_t"),
            Ty::InitList => f.write_str("{...}"),
            Ty::Unknown => f.write_str("?"),
        }
    }
}

/// A type as the player wrote it in a declaration: the type plus `const` and `&`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeSpec {
    /// The type (`Ty::Auto` for `auto`).
    pub ty: Ty,
    /// Top-level `const` (`const T`, `const T&`, `const auto&`).
    pub konst: bool,
    /// An lvalue reference (`T&`, `const T&`, `auto&`).
    pub is_ref: bool,
    /// How the name was spelled when it differs from the canonical `std::` form (alias use, `using
    /// namespace std`): the exact text, so the printer can reproduce the player's code. `None` means print
    /// the canonical form.
    pub spelled: Option<String>,
}

impl TypeSpec {
    /// A plain, non-const, non-reference type.
    pub fn plain(ty: Ty) -> TypeSpec {
        TypeSpec {
            ty,
            konst: false,
            is_ref: false,
            spelled: None,
        }
    }
}

/// Binary operators (`&&` and `||` short-circuit; `Comma` is the comma operator).
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
    /// `&&`
    And,
    /// `||`
    Or,
    /// `,`
    Comma,
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
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::Comma => ",",
        }
    }
    /// C++ precedence level; larger binds tighter (comma 1 ... multiplicative 13).
    pub fn prec(self) -> u8 {
        match self {
            BinOp::Comma => 1,
            BinOp::Or => 4,
            BinOp::And => 5,
            BinOp::BitOr => 6,
            BinOp::BitXor => 7,
            BinOp::BitAnd => 8,
            BinOp::Eq | BinOp::Ne => 9,
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 10,
            BinOp::Shl | BinOp::Shr => 11,
            BinOp::Add | BinOp::Sub => 12,
            BinOp::Mul | BinOp::Div | BinOp::Rem => 13,
        }
    }
    /// May be used as a compound assignment (`+=` ...).
    pub fn assignable(self) -> bool {
        matches!(
            self,
            BinOp::Add
                | BinOp::Sub
                | BinOp::Mul
                | BinOp::Div
                | BinOp::Rem
                | BinOp::BitAnd
                | BinOp::BitOr
                | BinOp::BitXor
                | BinOp::Shl
                | BinOp::Shr
        )
    }
}

/// Unary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    /// `-x`
    Neg,
    /// `+x`
    Plus,
    /// `!x`
    Not,
    /// `~x`
    BitNot,
    /// `*p`
    Deref,
    /// `&x`
    AddrOf,
    /// `++x`
    PreInc,
    /// `--x`
    PreDec,
    /// `x++`
    PostInc,
    /// `x--`
    PostDec,
}

/// Integer literal suffix kind (the type follows the C++ rules for the value and suffix).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntSuffix {
    /// No suffix.
    None,
    /// `u`
    U,
    /// `ll`
    LL,
    /// `ull`
    ULL,
}

/// How a literal was written (decimal, hex, binary), kept so the printer reproduces it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntBase {
    /// `42`
    Dec,
    /// `0x2a`
    Hex,
    /// `0b101010`
    Bin,
    /// `052`
    Oct,
}

/// A class of captures for lambdas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureDefault {
    /// `[]` / `[x]`
    None,
    /// `[&]`
    Ref,
    /// `[=]`
    Copy,
}

/// A lambda capture list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Captures {
    /// `&` or `=` default.
    pub default: CaptureDefault,
    /// Explicit items: `(name, by_ref)`.
    pub items: Vec<(String, bool)>,
}

/// A lambda expression `[caps](params) mutable -> ret { body }`.
#[derive(Clone, Debug, PartialEq)]
pub struct Lambda {
    /// Capture list.
    pub captures: Captures,
    /// Parameters (`auto` parameters make the lambda generic).
    pub params: Vec<Param>,
    /// Declared return type (`-> T`).
    pub ret: Option<TypeSpec>,
    /// `mutable` was written.
    pub mutable: bool,
    /// Body.
    pub body: Block,
}

/// An expression.
#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    /// Identity.
    pub id: NodeId,
    /// Shape.
    pub kind: ExprKind,
}

/// One segment of a qualified name: `std`, `sort`, `numeric_limits<int>`, `max`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathSeg {
    /// The identifier.
    pub name: String,
    /// Explicit template arguments (`<int>`), if written.
    pub targs: Option<Vec<TArg>>,
}

/// A template argument: a type or an integer (`std::get<0>`, `std::array<int, 5>`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TArg {
    /// A type.
    Ty(Ty),
    /// An integer constant.
    Int(i64),
}

/// Expression shapes.
#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    /// Integer literal; the checker decides the type from base, suffix and value.
    Int {
        /// Magnitude.
        value: u64,
        /// Suffix.
        suffix: IntSuffix,
        /// Spelling base.
        base: IntBase,
    },
    /// `true` / `false`.
    Bool(bool),
    /// Character literal (ASCII).
    Char(u8),
    /// String literal (ASCII), unescaped.
    Str(String),
    /// `nullptr`
    NullPtr,
    /// A plain unqualified name: a variable, a function, a constant.
    Var(String),
    /// A qualified or templated name: `std::sort`, `std::numeric_limits<int>::max`, `std::string::npos`,
    /// `std::vector<int>`.
    Path(Vec<PathSeg>),
    /// Unary operation (including `*`, `&`, `++`, `--`).
    Unary(UnOp, Box<Expr>),
    /// Binary operation.
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `lhs = rhs` or `lhs op= rhs`.
    Assign {
        /// Compound operator (`None` for plain `=`).
        op: Option<BinOp>,
        /// Target.
        lhs: Box<Expr>,
        /// Value.
        rhs: Box<Expr>,
    },
    /// `c ? a : b`
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    /// `a[i]`
    Index(Box<Expr>, Box<Expr>),
    /// `callee(args)`: free function, `std::` function, lambda call, or method call when the callee is a
    /// [`ExprKind::Member`].
    Call {
        /// What is called.
        callee: Box<Expr>,
        /// Arguments.
        args: Vec<Expr>,
    },
    /// `obj.name` or `obj->name`.
    Member {
        /// Object.
        obj: Box<Expr>,
        /// Member name.
        name: String,
        /// `->` instead of `.`.
        arrow: bool,
    },
    /// `static_cast<T>(e)`, `(T)e` or `T(e)` for scalar types.
    Cast {
        /// Spelling.
        style: CastStyle,
        /// Target type.
        ty: TypeSpec,
        /// Operand.
        e: Box<Expr>,
    },
    /// A lambda.
    Lambda(Box<Lambda>),
    /// A bare braced list `{a, b}` (target type decided by context).
    InitList(Vec<Expr>),
    /// `T(args)` or `T{args}` with an explicit type: `std::vector<int>(n, 0)`, `std::string{"a"}`,
    /// `ListNode(1)`.
    Construct {
        /// The constructed type.
        ty: TypeSpec,
        /// Constructor arguments.
        args: Vec<Expr>,
        /// Written with braces.
        brace: bool,
    },
    /// `new T(args)` / `new T{args}`
    New {
        /// Allocated type.
        ty: TypeSpec,
        /// Arguments.
        args: Vec<Expr>,
        /// Written with braces.
        brace: bool,
    },
    /// `delete p`
    Delete(Box<Expr>),
}

/// How a cast was written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastStyle {
    /// `static_cast<T>(e)`
    Static,
    /// `(T)e`
    CStyle,
    /// `T(e)`
    Functional,
}

/// How a variable is initialised.
#[derive(Clone, Debug, PartialEq)]
pub enum Init {
    /// `T x;`
    None,
    /// `T x = e;`
    Copy(Expr),
    /// `T x(a, b);`
    Direct(Vec<Expr>),
    /// `T x{a, b};` and `T x = {a, b};`
    Brace {
        /// The elements.
        elems: Vec<Expr>,
        /// `= {` was written.
        with_eq: bool,
    },
}

/// One declared name: `x`, `x = 3`, `v(n, 0)`, `a[26] = {}`.
#[derive(Clone, Debug, PartialEq)]
pub struct Declarator {
    /// Identity.
    pub id: NodeId,
    /// Name.
    pub name: String,
    /// C array dimension (`int a[26]`) as an expression.
    pub array: Option<Expr>,
    /// Initialiser.
    pub init: Init,
}

/// A statement. `comments` are the full-line comments written right above it; `trailing` a `// ...`
/// comment at the end of its first line. Both exist so the text/card/text round trip keeps comments.
#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    /// Identity.
    pub id: NodeId,
    /// Shape.
    pub kind: StmtKind,
    /// Comment lines above the statement (text after `//`, leading space kept).
    pub comments: Vec<String>,
    /// Trailing comment on the same line (text after `//`).
    pub trailing: Option<String>,
}

impl Stmt {
    /// A statement without comments.
    pub fn new(id: NodeId, kind: StmtKind) -> Stmt {
        Stmt {
            id,
            kind,
            comments: Vec::new(),
            trailing: None,
        }
    }
}

/// Statement shapes.
#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
    /// `T a = 1, b;` (also `auto`, `const`, references)
    Decl {
        /// Declared type.
        spec: TypeSpec,
        /// One or more declarators.
        vars: Vec<Declarator>,
    },
    /// `auto [a, b] = e;` / `auto& [k, v] = e;`
    Structured {
        /// `auto`, `const auto&`, ... (`ty` is `Auto`).
        spec: TypeSpec,
        /// Bound names.
        names: Vec<String>,
        /// Initialiser.
        init: Expr,
    },
    /// An expression statement.
    Expr(Expr),
    /// `if (init; cond) then else`
    If {
        /// C++17 init statement.
        init: Option<Box<Stmt>>,
        /// Condition.
        cond: Expr,
        /// Then branch (a `Block` or a single statement).
        then: Box<Stmt>,
        /// Else branch (a `Block`, a single statement or another `If`).
        els: Option<Box<Stmt>>,
    },
    /// `switch (cond) { case 1: ... default: ... }`; the labels are `Case` statements inside the block.
    Switch {
        /// Controlling expression (integral).
        cond: Expr,
        /// The body block holding `Case` markers.
        body: Block,
    },
    /// A `case x:` or `default:` marker inside a switch body (`None` is `default`).
    Case(Option<Expr>),
    /// `for (init; cond; incr) body`
    For {
        /// Init statement (declaration or expression statement).
        init: Option<Box<Stmt>>,
        /// Condition.
        cond: Option<Expr>,
        /// Increment (may use the comma operator).
        incr: Option<Expr>,
        /// Body.
        body: Box<Stmt>,
    },
    /// `for (const auto& x : range) body` or `for (auto& [k, v] : m) body`
    RangeFor {
        /// Declared element type (`auto`, `const auto&`, `int`, ...).
        spec: TypeSpec,
        /// A single name or a structured binding list.
        names: Vec<String>,
        /// `names` is a structured binding `[a, b]`.
        structured: bool,
        /// Range expression.
        range: Expr,
        /// Body.
        body: Box<Stmt>,
    },
    /// `while (cond) body`
    While {
        /// Condition.
        cond: Expr,
        /// Body.
        body: Box<Stmt>,
    },
    /// `do body while (cond);`
    DoWhile {
        /// Body.
        body: Box<Stmt>,
        /// Condition.
        cond: Expr,
    },
    /// `return;` / `return e;`
    Return(Option<Expr>),
    /// `break;`
    Break,
    /// `continue;`
    Continue,
    /// `{ ... }`
    Block(Block),
    /// `;`
    Empty,
}

/// A brace-delimited list of statements.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Block {
    /// Statements in order.
    pub stmts: Vec<Stmt>,
    /// Comment lines after the last statement (before the closing brace).
    pub tail_comments: Vec<String>,
}

/// A function or lambda parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    /// Identity.
    pub id: NodeId,
    /// Name (may be empty for an unnamed parameter).
    pub name: String,
    /// Type with `const` / `&`.
    pub spec: TypeSpec,
}

/// A free function.
#[derive(Clone, Debug, PartialEq)]
pub struct FnDecl {
    /// Identity.
    pub id: NodeId,
    /// Name.
    pub name: String,
    /// Parameters.
    pub params: Vec<Param>,
    /// Return type (`void` allowed; `const` / `&` allowed).
    pub ret: TypeSpec,
    /// `static` / `inline` was written (ignored).
    pub is_static: bool,
    /// Body; `None` for a forward declaration `int f(int);`.
    pub body: Option<Block>,
    /// Comment lines above the function.
    pub comments: Vec<String>,
}

/// A field of a struct.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// Field type.
    pub spec: TypeSpec,
    /// Field name.
    pub name: String,
    /// Default member initialiser (`int x = 0;`).
    pub default: Option<Expr>,
}

/// A constructor `Name(params) : a(x), b(y) { body }`.
#[derive(Clone, Debug, PartialEq)]
pub struct Ctor {
    /// Identity.
    pub id: NodeId,
    /// Parameters.
    pub params: Vec<Param>,
    /// Member initialiser list: `(field, args)`.
    pub inits: Vec<(String, Vec<Expr>)>,
    /// Body.
    pub body: Block,
}

/// `struct Name { fields; constructors; };` (data only: no methods, no inheritance, no templates).
#[derive(Clone, Debug, PartialEq)]
pub struct StructDecl {
    /// Identity.
    pub id: NodeId,
    /// Name.
    pub name: String,
    /// Fields in order.
    pub fields: Vec<Field>,
    /// Constructors.
    pub ctors: Vec<Ctor>,
}

/// A top-level item.
#[derive(Clone, Debug, PartialEq)]
pub enum ItemKind {
    /// `#include <vector>` / `#include "x"`.
    Include {
        /// `vector` (without brackets).
        path: String,
        /// `<...>` (true) or `"..."`.
        angle: bool,
    },
    /// `using namespace std;`
    UsingNamespaceStd,
    /// `using Name = Type;` or `typedef Type Name;`
    Alias {
        /// New name.
        name: String,
        /// Aliased type.
        ty: Ty,
        /// Written as `typedef`.
        typedef: bool,
    },
    /// A function.
    Func(FnDecl),
    /// A struct (prelude, or player when unlocked).
    Struct(StructDecl),
    /// A global constant `const int N = 5;` / `constexpr int N = 5;`
    Global {
        /// Declared type (`konst` set).
        spec: TypeSpec,
        /// `constexpr` was written.
        constexpr_: bool,
        /// Declarators.
        vars: Vec<Declarator>,
    },
}

/// A top-level item with its identity and comments.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// Identity.
    pub id: NodeId,
    /// Shape.
    pub kind: ItemKind,
    /// Comment lines above the item.
    pub comments: Vec<String>,
}

/// A whole translation unit: the level prelude's items followed by the player's items.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Program {
    /// Items in order: the first `prelude_items` come from the (read-only) prelude.
    pub items: Vec<Item>,
    /// How many leading items belong to the prelude.
    pub prelude_items: usize,
    /// Next unused [`NodeId`].
    pub next_id: NodeId,
    /// Source spans by node id into the *player's* text (empty for prelude and synthetic nodes).
    pub spans: Vec<Span>,
}

impl Program {
    /// Allocate a fresh node id (for programmatic construction and card editing).
    pub fn fresh_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    /// Source span of a node when it came from the player's text.
    pub fn span(&self, id: NodeId) -> Option<Span> {
        self.spans
            .get(id as usize)
            .copied()
            .filter(|s| s.end > s.start)
    }
    /// The player's items.
    pub fn player_items(&self) -> &[Item] {
        &self.items[self.prelude_items.min(self.items.len())..]
    }
    /// Every function (prelude and player) in source order.
    pub fn fns(&self) -> impl Iterator<Item = &FnDecl> {
        self.items.iter().filter_map(|i| match &i.kind {
            ItemKind::Func(f) => Some(f),
            _ => None,
        })
    }
    /// Find a function with a body by name.
    pub fn find(&self, name: &str) -> Option<&FnDecl> {
        self.fns().find(|f| f.name == name && f.body.is_some())
    }
    /// Find a struct by name.
    pub fn find_struct(&self, name: &str) -> Option<&StructDecl> {
        self.items.iter().find_map(|i| match &i.kind {
            ItemKind::Struct(s) if s.name == name => Some(s),
            _ => None,
        })
    }
}

/// Walk every statement (depth first, source order), including those nested in lambdas' bodies is NOT
/// done: lambdas are expressions, use [`walk_expr`] to reach them.
pub fn walk_stmts<'a>(b: &'a Block, f: &mut dyn FnMut(&'a Stmt)) {
    for s in &b.stmts {
        walk_stmt(s, f);
    }
}

/// Walk one statement and everything nested in it (statements only).
pub fn walk_stmt<'a>(s: &'a Stmt, f: &mut dyn FnMut(&'a Stmt)) {
    f(s);
    match &s.kind {
        StmtKind::If {
            init, then, els, ..
        } => {
            if let Some(i) = init {
                walk_stmt(i, f);
            }
            walk_stmt(then, f);
            if let Some(e) = els {
                walk_stmt(e, f);
            }
        }
        StmtKind::For { init, body, .. } => {
            if let Some(i) = init {
                walk_stmt(i, f);
            }
            walk_stmt(body, f);
        }
        StmtKind::RangeFor { body, .. }
        | StmtKind::While { body, .. }
        | StmtKind::DoWhile { body, .. } => walk_stmt(body, f),
        StmtKind::Switch { body, .. } | StmtKind::Block(body) => walk_stmts(body, f),
        _ => {}
    }
}

/// Visit the direct sub-expressions of an expression.
pub fn expr_children<'a>(e: &'a Expr, f: &mut dyn FnMut(&'a Expr)) {
    match &e.kind {
        ExprKind::Unary(_, a) | ExprKind::Delete(a) => f(a),
        ExprKind::Cast { e: a, .. } => f(a),
        ExprKind::Member { obj, .. } => f(obj),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) => {
            f(a);
            f(b);
        }
        ExprKind::Assign { lhs, rhs, .. } => {
            f(lhs);
            f(rhs);
        }
        ExprKind::Ternary(a, b, c) => {
            f(a);
            f(b);
            f(c);
        }
        ExprKind::Call { callee, args } => {
            f(callee);
            args.iter().for_each(&mut *f);
        }
        ExprKind::InitList(a)
        | ExprKind::Construct { args: a, .. }
        | ExprKind::New { args: a, .. } => a.iter().for_each(f),
        ExprKind::Lambda(l) => walk_block_exprs(&l.body, f),
        _ => {}
    }
}

/// Walk an expression tree (pre-order), descending into lambda bodies.
pub fn walk_expr<'a>(e: &'a Expr, f: &mut dyn FnMut(&'a Expr)) {
    f(e);
    expr_children(e, &mut |c| walk_expr(c, f));
}

/// Visit every expression owned directly by a statement (not by nested statements).
pub fn stmt_exprs<'a>(s: &'a Stmt, f: &mut dyn FnMut(&'a Expr)) {
    let init_exprs = |i: &'a Init, f: &mut dyn FnMut(&'a Expr)| match i {
        Init::None => {}
        Init::Copy(e) => f(e),
        Init::Direct(a) | Init::Brace { elems: a, .. } => a.iter().for_each(f),
    };
    match &s.kind {
        StmtKind::Decl { vars, .. } => {
            for v in vars {
                if let Some(a) = &v.array {
                    f(a);
                }
                init_exprs(&v.init, f);
            }
        }
        StmtKind::Structured { init, .. } => f(init),
        StmtKind::Expr(e) => f(e),
        StmtKind::If { cond, .. }
        | StmtKind::Switch { cond, .. }
        | StmtKind::While { cond, .. }
        | StmtKind::DoWhile { cond, .. } => f(cond),
        StmtKind::Case(Some(e)) => f(e),
        StmtKind::For { cond, incr, .. } => {
            if let Some(c) = cond {
                f(c);
            }
            if let Some(i) = incr {
                f(i);
            }
        }
        StmtKind::RangeFor { range, .. } => f(range),
        StmtKind::Return(Some(e)) => f(e),
        _ => {}
    }
}

fn walk_block_exprs<'a>(b: &'a Block, f: &mut dyn FnMut(&'a Expr)) {
    walk_stmts(b, &mut |s| stmt_exprs(s, &mut |e| walk_expr(e, &mut *f)));
}

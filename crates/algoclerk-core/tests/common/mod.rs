// SPDX-License-Identifier: Apache-2.0
//! TEMPORARY test-support front end: a small C++ subset parser used by the interpreter tests until the real
//! parser (`cpp::parse`) is available. It builds the AST contract of `cpp::ast` and records statement and
//! expression spans. It is deliberately forgiving and not part of the product.

#![allow(dead_code, missing_docs)]

use algoclerk_core::cpp::ast::*;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Id(String),
    Num(u64, IntSuffix, IntBase),
    Chr(u8),
    Str(String),
    P(&'static str),
    Inc(String, bool),
    Eof,
}

const PUNCTS: &[&str] = &[
    "<<=", ">>=", "->", "++", "--", "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "+=", "-=",
    "*=", "/=", "%=", "&=", "|=", "^=", "::", "+", "-", "*", "/", "%", "&", "|", "^", "~", "!",
    "<", ">", "=", "?", ":", ";", ",", ".", "(", ")", "[", "]", "{", "}",
];

fn lex(src: &str) -> Result<Vec<(Tok, u32, u32)>, String> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        if c == b'#' {
            let end = src[i..].find('\n').map_or(b.len(), |p| i + p);
            let line = src[i..end].trim();
            if let Some(rest) = line.strip_prefix("#include") {
                let rest = rest.trim();
                let angle = rest.starts_with('<');
                out.push((
                    Tok::Inc(rest[1..rest.len() - 1].to_string(), angle),
                    start as u32,
                    end as u32,
                ));
            }
            i = end;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push((Tok::Id(src[start..i].to_string()), start as u32, i as u32));
            continue;
        }
        if c.is_ascii_digit() {
            let (mut base, mut j) = (IntBase::Dec, i);
            let mut digits = String::new();
            if c == b'0' && matches!(b.get(i + 1), Some(b'x' | b'X')) {
                base = IntBase::Hex;
                j += 2;
                while j < b.len() && b[j].is_ascii_hexdigit() {
                    digits.push(b[j] as char);
                    j += 1;
                }
            } else if c == b'0' && matches!(b.get(i + 1), Some(b'b' | b'B')) {
                base = IntBase::Bin;
                j += 2;
                while j < b.len() && matches!(b[j], b'0' | b'1') {
                    digits.push(b[j] as char);
                    j += 1;
                }
            } else {
                while j < b.len() && b[j].is_ascii_digit() {
                    digits.push(b[j] as char);
                    j += 1;
                }
                if digits.len() > 1 && digits.starts_with('0') {
                    base = IntBase::Oct;
                }
            }
            let radix = match base {
                IntBase::Hex => 16,
                IntBase::Bin => 2,
                IntBase::Oct => 8,
                IntBase::Dec => 10,
            };
            let value = u64::from_str_radix(&digits, radix).map_err(|e| e.to_string())?;
            let mut suf = String::new();
            while j < b.len() && matches!(b[j], b'u' | b'U' | b'l' | b'L') {
                suf.push(b[j].to_ascii_lowercase() as char);
                j += 1;
            }
            let suffix = match suf.as_str() {
                "" => IntSuffix::None,
                "u" => IntSuffix::U,
                "ll" | "l" => IntSuffix::LL,
                "ull" | "lu" | "llu" | "ul" => IntSuffix::ULL,
                s => return Err(format!("bad suffix {s}")),
            };
            if j < b.len() && b[j] == b'.' && b.get(j + 1).is_some_and(u8::is_ascii_digit) {
                return Err("floating point literals are not in the subset".into());
            }
            i = j;
            out.push((Tok::Num(value, suffix, base), start as u32, i as u32));
            continue;
        }
        if c == b'\'' || c == b'"' {
            let q = c;
            i += 1;
            let mut s = Vec::new();
            while i < b.len() && b[i] != q {
                if b[i] == b'\\' {
                    i += 1;
                    s.push(match b[i] {
                        b'n' => b'\n',
                        b't' => b'\t',
                        b'r' => b'\r',
                        b'0' => 0,
                        o => o,
                    });
                } else {
                    s.push(b[i]);
                }
                i += 1;
            }
            i += 1;
            if q == b'\'' {
                out.push((
                    Tok::Chr(s.first().copied().unwrap_or(0)),
                    start as u32,
                    i as u32,
                ));
            } else {
                out.push((
                    Tok::Str(String::from_utf8_lossy(&s).into_owned()),
                    start as u32,
                    i as u32,
                ));
            }
            continue;
        }
        let mut matched = false;
        for p in PUNCTS {
            if src[i..].starts_with(p) {
                i += p.len();
                out.push((Tok::P(p), start as u32, i as u32));
                matched = true;
                break;
            }
        }
        if !matched {
            return Err(format!("unexpected character {:?}", c as char));
        }
    }
    out.push((Tok::Eof, b.len() as u32, b.len() as u32));
    Ok(out)
}

struct P {
    t: Vec<(Tok, u32, u32)>,
    i: usize,
    next: NodeId,
    spans: Vec<Span>,
    types: HashMap<String, Ty>,
    structs: Vec<String>,
    std_ns: bool,
}

type R<T> = Result<T, String>;

const TEMPLATES: &[&str] = &[
    "vector",
    "map",
    "set",
    "unordered_map",
    "unordered_set",
    "pair",
    "tuple",
    "optional",
    "function",
    "stack",
    "queue",
    "deque",
    "list",
    "priority_queue",
    "array",
    "greater",
    "less",
    "numeric_limits",
];

impl P {
    fn peek(&self) -> &Tok {
        &self.t[self.i].0
    }
    fn peek_at(&self, k: usize) -> &Tok {
        &self.t[(self.i + k).min(self.t.len() - 1)].0
    }
    fn pos(&self) -> u32 {
        self.t[self.i].1
    }
    fn last_end(&self) -> u32 {
        self.t[self.i.saturating_sub(1)].2
    }
    fn bump(&mut self) -> Tok {
        let t = self.t[self.i].0.clone();
        if self.i + 1 < self.t.len() {
            self.i += 1;
        }
        t
    }
    fn is_p(&self, p: &str) -> bool {
        matches!(self.peek(), Tok::P(q) if *q == p)
    }
    fn is_id(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Id(q) if q == s)
    }
    fn eat_p(&mut self, p: &str) -> bool {
        if self.is_p(p) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn eat_id(&mut self, s: &str) -> bool {
        if self.is_id(s) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn expect_p(&mut self, p: &str) -> R<()> {
        if self.eat_p(p) {
            Ok(())
        } else {
            Err(format!(
                "expected `{p}` at byte {} but found {:?}",
                self.pos(),
                self.peek()
            ))
        }
    }
    fn ident(&mut self) -> R<String> {
        match self.bump() {
            Tok::Id(s) => Ok(s),
            t => Err(format!(
                "expected a name, found {t:?} at byte {}",
                self.pos()
            )),
        }
    }
    fn id(&mut self, from: u32) -> NodeId {
        let id = self.next;
        self.next += 1;
        let end = self.last_end();
        if self.spans.len() <= id as usize {
            self.spans.resize(id as usize + 1, Span::default());
        }
        self.spans[id as usize] = Span {
            start: from,
            end: end.max(from),
        };
        id
    }
    fn expr(&mut self, from: u32, kind: ExprKind) -> Expr {
        Expr {
            id: self.id(from),
            kind,
        }
    }

    // ---- types ----
    fn is_type_start(&self) -> bool {
        match self.peek() {
            Tok::Id(s) => match s.as_str() {
                "const" | "constexpr" | "static" | "inline" | "unsigned" | "long" | "int"
                | "char" | "bool" | "void" | "auto" | "size_t" | "decltype" => true,
                "std" => matches!(self.peek_at(1), Tok::P("::")),
                s => {
                    self.types.contains_key(s)
                        || self.structs.iter().any(|n| n == s)
                        || (self.std_ns
                            && (TEMPLATES.contains(&s) || s == "string" || s == "string_view"))
                }
            },
            _ => false,
        }
    }

    fn targs(&mut self) -> R<Vec<TArg>> {
        self.expect_p("<")?;
        let mut out = Vec::new();
        if !self.is_p(">") {
            loop {
                if let Tok::Num(v, _, _) = self.peek().clone() {
                    self.bump();
                    out.push(TArg::Int(v as i64));
                } else {
                    let t = self.ty_core()?;
                    out.push(TArg::Ty(t));
                }
                if !self.eat_p(",") {
                    break;
                }
            }
        }
        // `>>` closes two template argument lists
        self.close_angle()?;
        Ok(out)
    }

    /// Consume one `>`, splitting a `>>` token into two.
    fn close_angle(&mut self) -> R<()> {
        if self.is_p(">>") {
            let (_, s, e) = self.t[self.i].clone();
            self.t[self.i] = (Tok::P(">"), s, s + 1);
            self.t.insert(self.i + 1, (Tok::P(">"), s + 1, e));
        }
        self.expect_p(">")
    }

    fn ty_arg(&mut self, a: &[TArg], i: usize) -> R<Ty> {
        match a.get(i) {
            Some(TArg::Ty(t)) => Ok(t.clone()),
            _ => Err("expected a type argument".into()),
        }
    }

    /// A type without const/&.
    fn ty_core(&mut self) -> R<Ty> {
        let mut konst_seen = false;
        while matches!(self.peek(), Tok::Id(s) if matches!(s.as_str(), "const" | "constexpr" | "static" | "inline"))
        {
            self.bump();
            konst_seen = true;
        }
        let _ = konst_seen;
        let name = self.ident()?;
        let mut t = match name.as_str() {
            "int" => Ty::Int,
            "char" => Ty::Char,
            "bool" => Ty::Bool,
            "void" => Ty::Void,
            "auto" => Ty::Auto,
            "size_t" => Ty::Size,
            "unsigned" => {
                if self.eat_id("long") {
                    self.eat_id("long");
                    self.eat_id("int");
                    Ty::ULongLong
                } else {
                    self.eat_id("int");
                    Ty::UInt
                }
            }
            "long" => {
                if self.eat_id("long") {
                    self.eat_id("int");
                    Ty::LongLong
                } else {
                    return Err("`long` is not in the subset".into());
                }
            }
            "decltype" => {
                self.expect_p("(")?;
                self.ident()?;
                self.expect_p(")")?;
                Ty::Lambda(0)
            }
            "std" => {
                self.expect_p("::")?;
                let n = self.ident()?;
                self.std_ty(&n)?
            }
            n if self.types.contains_key(n) => self.types[n].clone(),
            n if self.structs.iter().any(|s| s == n) => Ty::Struct(n.to_string()),
            n if self.std_ns => self.std_ty(n)?,
            n => return Err(format!("unknown type `{n}`")),
        };
        if self.is_p("::") && matches!(self.peek_at(1), Tok::Id(n) if n.ends_with("iterator")) {
            self.bump();
            let n = self.ident()?;
            t = Ty::Iter {
                container: Box::new(t),
                reverse: n.starts_with("reverse") || n.starts_with("const_reverse"),
            };
        }
        while self.is_p("*") {
            self.bump();
            t = Ty::Ptr(Box::new(t));
        }
        Ok(t)
    }

    fn std_ty(&mut self, n: &str) -> R<Ty> {
        Ok(match n {
            "string" => Ty::Str,
            "string_view" => Ty::StrView,
            "size_t" => Ty::Size,
            "vector" | "set" | "unordered_set" | "stack" | "queue" | "deque" | "list"
            | "optional" => {
                let a = self.targs()?;
                let t = Box::new(self.ty_arg(&a, 0)?);
                match n {
                    "vector" => Ty::Vector(t),
                    "set" => Ty::Set(t),
                    "unordered_set" => Ty::UnorderedSet(t),
                    "stack" => Ty::Stack(t),
                    "queue" => Ty::Queue(t),
                    "deque" => Ty::Deque(t),
                    "list" => Ty::List(t),
                    _ => Ty::Optional(t),
                }
            }
            "array" => {
                let a = self.targs()?;
                let t = Box::new(self.ty_arg(&a, 0)?);
                let n = match a.get(1) {
                    Some(TArg::Int(n)) => *n as usize,
                    _ => return Err("array needs a size".into()),
                };
                Ty::Array(t, n)
            }
            "map" | "unordered_map" | "pair" => {
                let a = self.targs()?;
                let (x, y) = (Box::new(self.ty_arg(&a, 0)?), Box::new(self.ty_arg(&a, 1)?));
                match n {
                    "map" => Ty::Map(x, y),
                    "unordered_map" => Ty::UnorderedMap(x, y),
                    _ => Ty::Pair(x, y),
                }
            }
            "tuple" => {
                let a = self.targs()?;
                let mut ts = Vec::new();
                for i in 0..a.len() {
                    ts.push(self.ty_arg(&a, i)?);
                }
                Ty::Tuple(ts)
            }
            "greater" | "less" => {
                let a = self.targs()?;
                let t = Box::new(self.ty_arg(&a, 0)?);
                Ty::Functor(
                    if n == "greater" {
                        Functor::Greater
                    } else {
                        Functor::Less
                    },
                    t,
                )
            }
            "priority_queue" => {
                let a = self.targs()?;
                let t = self.ty_arg(&a, 0)?;
                let c = if a.len() >= 3 {
                    self.ty_arg(&a, 2)?
                } else {
                    Ty::Functor(Functor::Less, Box::new(t.clone()))
                };
                Ty::PriorityQueue(Box::new(t), Box::new(c))
            }
            "function" => {
                self.expect_p("<")?;
                let ret = self.ty_core()?;
                self.expect_p("(")?;
                let mut args = Vec::new();
                while !self.is_p(")") {
                    args.push(self.type_spec()?.ty);
                    if !self.eat_p(",") {
                        break;
                    }
                }
                self.expect_p(")")?;
                self.close_angle()?;
                Ty::Function(args, Box::new(ret))
            }
            other => return Err(format!("`std::{other}` is not in the subset")),
        })
    }

    fn type_spec(&mut self) -> R<TypeSpec> {
        let mut konst = false;
        while matches!(self.peek(), Tok::Id(s) if matches!(s.as_str(), "const" | "constexpr" | "static" | "inline"))
        {
            konst |= self.is_id("const") || self.is_id("constexpr");
            self.bump();
        }
        let ty = self.ty_core()?;
        while self.eat_id("const") {
            konst = true;
        }
        let is_ref = self.eat_p("&");
        Ok(TypeSpec {
            ty,
            konst,
            is_ref,
            spelled: None,
        })
    }

    // ---- expressions ----
    fn parse_expr(&mut self) -> R<Expr> {
        let from = self.pos();
        let mut e = self.assign()?;
        while self.is_p(",") {
            self.bump();
            let r = self.assign()?;
            e = self.expr(
                from,
                ExprKind::Binary(BinOp::Comma, Box::new(e), Box::new(r)),
            );
        }
        Ok(e)
    }

    fn assign(&mut self) -> R<Expr> {
        let from = self.pos();
        let lhs = self.ternary()?;
        let op = match self.peek() {
            Tok::P("=") => Some(None),
            Tok::P("+=") => Some(Some(BinOp::Add)),
            Tok::P("-=") => Some(Some(BinOp::Sub)),
            Tok::P("*=") => Some(Some(BinOp::Mul)),
            Tok::P("/=") => Some(Some(BinOp::Div)),
            Tok::P("%=") => Some(Some(BinOp::Rem)),
            Tok::P("&=") => Some(Some(BinOp::BitAnd)),
            Tok::P("|=") => Some(Some(BinOp::BitOr)),
            Tok::P("^=") => Some(Some(BinOp::BitXor)),
            Tok::P("<<=") => Some(Some(BinOp::Shl)),
            Tok::P(">>=") => Some(Some(BinOp::Shr)),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let rhs = if self.is_p("{") {
                self.init_list()?
            } else {
                self.assign()?
            };
            return Ok(self.expr(
                from,
                ExprKind::Assign {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
            ));
        }
        Ok(lhs)
    }

    fn ternary(&mut self) -> R<Expr> {
        let from = self.pos();
        let c = self.binary(1)?;
        if self.eat_p("?") {
            let a = self.assign()?;
            self.expect_p(":")?;
            let b = self.assign()?;
            return Ok(self.expr(
                from,
                ExprKind::Ternary(Box::new(c), Box::new(a), Box::new(b)),
            ));
        }
        Ok(c)
    }

    fn binop(&self) -> Option<BinOp> {
        Some(match self.peek() {
            Tok::P("||") => BinOp::Or,
            Tok::P("&&") => BinOp::And,
            Tok::P("|") => BinOp::BitOr,
            Tok::P("^") => BinOp::BitXor,
            Tok::P("&") => BinOp::BitAnd,
            Tok::P("==") => BinOp::Eq,
            Tok::P("!=") => BinOp::Ne,
            Tok::P("<") => BinOp::Lt,
            Tok::P("<=") => BinOp::Le,
            Tok::P(">") => BinOp::Gt,
            Tok::P(">=") => BinOp::Ge,
            Tok::P("<<") => BinOp::Shl,
            Tok::P(">>") => BinOp::Shr,
            Tok::P("+") => BinOp::Add,
            Tok::P("-") => BinOp::Sub,
            Tok::P("*") => BinOp::Mul,
            Tok::P("/") => BinOp::Div,
            Tok::P("%") => BinOp::Rem,
            _ => return None,
        })
    }

    fn binary(&mut self, min: u8) -> R<Expr> {
        let from = self.pos();
        let mut l = self.unary()?;
        while let Some(op) = self.binop() {
            if op.prec() < min.max(4) {
                break;
            }
            self.bump();
            let r = self.binary(op.prec() + 1)?;
            l = self.expr(from, ExprKind::Binary(op, Box::new(l), Box::new(r)));
        }
        Ok(l)
    }

    fn unary(&mut self) -> R<Expr> {
        let from = self.pos();
        let op = match self.peek() {
            Tok::P("-") => Some(UnOp::Neg),
            Tok::P("+") => Some(UnOp::Plus),
            Tok::P("!") => Some(UnOp::Not),
            Tok::P("~") => Some(UnOp::BitNot),
            Tok::P("*") => Some(UnOp::Deref),
            Tok::P("&") => Some(UnOp::AddrOf),
            Tok::P("++") => Some(UnOp::PreInc),
            Tok::P("--") => Some(UnOp::PreDec),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let e = self.unary()?;
            return Ok(self.expr(from, ExprKind::Unary(op, Box::new(e))));
        }
        if self.is_id("new") {
            self.bump();
            let ty = self.type_spec()?;
            let (args, brace) = if self.is_p("(") {
                (self.call_args()?, false)
            } else if self.is_p("{") {
                (self.brace_args()?, true)
            } else {
                (vec![], false)
            };
            return Ok(self.expr(from, ExprKind::New { ty, args, brace }));
        }
        if self.is_id("delete") {
            self.bump();
            let e = self.unary()?;
            return Ok(self.expr(from, ExprKind::Delete(Box::new(e))));
        }
        // C style cast
        if self.is_p("(")
            && matches!(self.peek_at(1), Tok::Id(s) if matches!(s.as_str(), "int" | "char" | "bool" | "unsigned" | "long" | "size_t"))
        {
            let save = self.i;
            self.bump();
            if let Ok(ty) = self.type_spec()
                && self.eat_p(")")
            {
                let e = self.unary()?;
                return Ok(self.expr(
                    from,
                    ExprKind::Cast {
                        style: CastStyle::CStyle,
                        ty,
                        e: Box::new(e),
                    },
                ));
            }
            self.i = save;
        }
        self.postfix()
    }

    fn call_args(&mut self) -> R<Vec<Expr>> {
        self.expect_p("(")?;
        let mut v = Vec::new();
        while !self.is_p(")") {
            v.push(if self.is_p("{") {
                self.init_list()?
            } else {
                self.assign()?
            });
            if !self.eat_p(",") {
                break;
            }
        }
        self.expect_p(")")?;
        Ok(v)
    }

    fn brace_args(&mut self) -> R<Vec<Expr>> {
        self.expect_p("{")?;
        let mut v = Vec::new();
        while !self.is_p("}") {
            v.push(if self.is_p("{") {
                self.init_list()?
            } else {
                self.assign()?
            });
            if !self.eat_p(",") {
                break;
            }
        }
        self.expect_p("}")?;
        Ok(v)
    }

    fn init_list(&mut self) -> R<Expr> {
        let from = self.pos();
        let a = self.brace_args()?;
        Ok(self.expr(from, ExprKind::InitList(a)))
    }

    fn postfix(&mut self) -> R<Expr> {
        let from = self.pos();
        let mut e = self.primary()?;
        loop {
            if self.is_p("(") {
                let args = self.call_args()?;
                e = self.expr(
                    from,
                    ExprKind::Call {
                        callee: Box::new(e),
                        args,
                    },
                );
            } else if self.is_p("[") {
                self.bump();
                let i = self.parse_expr()?;
                self.expect_p("]")?;
                e = self.expr(from, ExprKind::Index(Box::new(e), Box::new(i)));
            } else if self.is_p(".") || self.is_p("->") {
                let arrow = self.is_p("->");
                self.bump();
                let name = self.ident()?;
                e = self.expr(
                    from,
                    ExprKind::Member {
                        obj: Box::new(e),
                        name,
                        arrow,
                    },
                );
            } else if self.is_p("++") {
                self.bump();
                e = self.expr(from, ExprKind::Unary(UnOp::PostInc, Box::new(e)));
            } else if self.is_p("--") {
                self.bump();
                e = self.expr(from, ExprKind::Unary(UnOp::PostDec, Box::new(e)));
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn lambda(&mut self) -> R<Expr> {
        let from = self.pos();
        self.expect_p("[")?;
        let mut default = CaptureDefault::None;
        let mut items = Vec::new();
        while !self.is_p("]") {
            if self.is_p("&") {
                self.bump();
                if let Tok::Id(n) = self.peek().clone() {
                    self.bump();
                    items.push((n, true));
                } else {
                    default = CaptureDefault::Ref;
                }
            } else if self.is_p("=") {
                self.bump();
                default = CaptureDefault::Copy;
            } else {
                let n = self.ident()?;
                items.push((n, false));
            }
            if !self.eat_p(",") {
                break;
            }
        }
        self.expect_p("]")?;
        let mut params = Vec::new();
        if self.eat_p("(") {
            while !self.is_p(")") {
                let pf = self.pos();
                let spec = self.type_spec()?;
                let name = if let Tok::Id(_) = self.peek() {
                    self.ident()?
                } else {
                    String::new()
                };
                params.push(Param {
                    id: self.id(pf),
                    name,
                    spec,
                });
                if !self.eat_p(",") {
                    break;
                }
            }
            self.expect_p(")")?;
        }
        let mutable = self.eat_id("mutable");
        let ret = if self.eat_p("->") {
            Some(self.type_spec()?)
        } else {
            None
        };
        let body = self.block()?;
        Ok(self.expr(
            from,
            ExprKind::Lambda(Box::new(Lambda {
                captures: Captures { default, items },
                params,
                ret,
                mutable,
                body,
            })),
        ))
    }

    fn primary(&mut self) -> R<Expr> {
        let from = self.pos();
        match self.peek().clone() {
            Tok::Num(value, suffix, base) => {
                self.bump();
                Ok(self.expr(
                    from,
                    ExprKind::Int {
                        value,
                        suffix,
                        base,
                    },
                ))
            }
            Tok::Chr(c) => {
                self.bump();
                Ok(self.expr(from, ExprKind::Char(c)))
            }
            Tok::Str(s) => {
                self.bump();
                let mut s = s;
                while let Tok::Str(t) = self.peek().clone() {
                    self.bump();
                    s.push_str(&t);
                }
                Ok(self.expr(from, ExprKind::Str(s)))
            }
            Tok::P("(") => {
                self.bump();
                let e = self.parse_expr()?;
                self.expect_p(")")?;
                Ok(e)
            }
            Tok::P("[") => self.lambda(),
            Tok::P("{") => self.init_list(),
            Tok::Id(name) => {
                match name.as_str() {
                    "true" | "false" => {
                        self.bump();
                        return Ok(self.expr(from, ExprKind::Bool(name == "true")));
                    }
                    "nullptr" => {
                        self.bump();
                        return Ok(self.expr(from, ExprKind::NullPtr));
                    }
                    "static_cast" => {
                        self.bump();
                        self.expect_p("<")?;
                        let ty = self.type_spec()?;
                        self.close_angle()?;
                        self.expect_p("(")?;
                        let e = self.parse_expr()?;
                        self.expect_p(")")?;
                        return Ok(self.expr(
                            from,
                            ExprKind::Cast {
                                style: CastStyle::Static,
                                ty,
                                e: Box::new(e),
                            },
                        ));
                    }
                    "int" | "char" | "bool" | "size_t" | "unsigned" => {
                        let ty = self.type_spec()?;
                        self.expect_p("(")?;
                        let e = self.parse_expr()?;
                        self.expect_p(")")?;
                        return Ok(self.expr(
                            from,
                            ExprKind::Cast {
                                style: CastStyle::Functional,
                                ty,
                                e: Box::new(e),
                            },
                        ));
                    }
                    _ => {}
                }
                // a type used as a constructor: vector<int>(..), std::string(..), ListNode(..), Point{..}
                let is_ctor_type = (name == "std"
                    && matches!(self.peek_at(2), Tok::Id(n) if TEMPLATES.contains(&n.as_str()) || n == "string" || n == "string_view")
                    && !self.path_is_function_call())
                    || (self.std_ns
                        && (TEMPLATES.contains(&name.as_str())
                            || name == "string"
                            || name == "string_view")
                        && name != "numeric_limits")
                    || self.structs.contains(&name)
                    || self.types.contains_key(&name);
                if is_ctor_type && !matches!(self.peek_at(1), Tok::P("=")) {
                    let save = self.i;
                    if let Ok(ty) = self.type_spec()
                        && (self.is_p("(") || self.is_p("{"))
                    {
                        let brace = self.is_p("{");
                        let args = if brace {
                            self.brace_args()?
                        } else {
                            self.call_args()?
                        };
                        return Ok(self.expr(from, ExprKind::Construct { ty, args, brace }));
                    }
                    self.i = save;
                }
                // a (qualified) name
                let mut segs = vec![PathSeg {
                    name: self.ident()?,
                    targs: None,
                }];
                let mut qualified = false;
                loop {
                    if self.is_p("::") {
                        self.bump();
                        qualified = true;
                        let n = self.ident()?;
                        segs.push(PathSeg {
                            name: n,
                            targs: None,
                        });
                    } else if self.is_p("<") && self.targs_follow(&segs) {
                        let a = self.targs()?;
                        if let Some(s) = segs.last_mut() {
                            s.targs = Some(a);
                        }
                    } else {
                        break;
                    }
                }
                if qualified || segs.iter().any(|s| s.targs.is_some()) {
                    Ok(self.expr(from, ExprKind::Path(segs)))
                } else {
                    Ok(self.expr(from, ExprKind::Var(segs.remove(0).name)))
                }
            }
            t => Err(format!(
                "unexpected {t:?} in an expression at byte {}",
                self.pos()
            )),
        }
    }

    fn path_is_function_call(&self) -> bool {
        false
    }

    /// Does `<` after this name open template arguments (rather than being a comparison)?
    fn targs_follow(&self, segs: &[PathSeg]) -> bool {
        let last = segs.last().map_or("", |s| s.name.as_str());
        if !(TEMPLATES.contains(&last) || matches!(last, "get" | "max" | "min")) {
            return false;
        }
        match self.peek_at(1) {
            Tok::Num(..) => true,
            Tok::Id(s) => {
                matches!(
                    s.as_str(),
                    "int" | "long" | "unsigned" | "char" | "bool" | "size_t" | "std"
                ) || self.types.contains_key(s)
            }
            _ => false,
        }
    }

    // ---- statements ----
    fn block(&mut self) -> R<Block> {
        self.expect_p("{")?;
        let mut stmts = Vec::new();
        while !self.is_p("}") {
            if matches!(self.peek(), Tok::Eof) {
                return Err("unterminated block".into());
            }
            stmts.push(self.stmt()?);
        }
        self.bump();
        Ok(Block {
            stmts,
            tail_comments: vec![],
        })
    }

    fn mk(&mut self, from: u32, kind: StmtKind) -> Stmt {
        Stmt::new(self.id(from), kind)
    }

    fn looks_like_decl(&mut self) -> bool {
        if !self.is_type_start() {
            return false;
        }
        let save = self.i;
        let ok = (|| -> R<bool> {
            let _ = self.type_spec()?;
            Ok(matches!(self.peek(), Tok::Id(_))
                || self.is_p("[") && matches!(self.peek_at(1), Tok::Id(_))
                || (self.is_p("&") && matches!(self.peek_at(1), Tok::P("[") | Tok::Id(_))))
        })()
        .unwrap_or(false);
        self.i = save;
        ok
    }

    fn declarators(&mut self) -> R<Vec<Declarator>> {
        let mut vars = Vec::new();
        loop {
            let from = self.pos();
            while self.eat_p("*") {}
            let name = self.ident()?;
            let mut array = None;
            if self.is_p("[") {
                self.bump();
                array = Some(self.parse_expr()?);
                self.expect_p("]")?;
            }
            let init = if self.eat_p("=") {
                if self.is_p("{") {
                    let a = self.brace_args()?;
                    Init::Brace {
                        elems: a,
                        with_eq: true,
                    }
                } else {
                    Init::Copy(self.assign()?)
                }
            } else if self.is_p("(") {
                Init::Direct(self.call_args()?)
            } else if self.is_p("{") {
                Init::Brace {
                    elems: self.brace_args()?,
                    with_eq: false,
                }
            } else {
                Init::None
            };
            vars.push(Declarator {
                id: self.id(from),
                name,
                array,
                init,
            });
            if !self.eat_p(",") {
                break;
            }
        }
        Ok(vars)
    }

    fn decl_stmt(&mut self) -> R<Stmt> {
        let from = self.pos();
        let spec = self.type_spec()?;
        if self.is_p("[") {
            self.bump();
            let mut names = vec![self.ident()?];
            while self.eat_p(",") {
                names.push(self.ident()?);
            }
            self.expect_p("]")?;
            self.expect_p("=")?;
            let init = self.assign()?;
            self.expect_p(";")?;
            return Ok(self.mk(from, StmtKind::Structured { spec, names, init }));
        }
        let vars = self.declarators()?;
        self.expect_p(";")?;
        Ok(self.mk(from, StmtKind::Decl { spec, vars }))
    }

    fn body(&mut self) -> R<Box<Stmt>> {
        Ok(Box::new(self.stmt()?))
    }

    fn stmt(&mut self) -> R<Stmt> {
        let from = self.pos();
        if self.is_p("{") {
            let b = self.block()?;
            return Ok(self.mk(from, StmtKind::Block(b)));
        }
        if self.eat_p(";") {
            return Ok(self.mk(from, StmtKind::Empty));
        }
        if let Tok::Id(k) = self.peek().clone() {
            match k.as_str() {
                "if" => {
                    self.bump();
                    self.expect_p("(")?;
                    let mut init = None;
                    if self.looks_like_decl() {
                        init = Some(Box::new(self.decl_stmt()?));
                    } else {
                        // `if (expr; cond)` with an expression init is rare: try to find `;` at depth 0
                        let save = self.i;
                        let e = self.parse_expr();
                        if e.is_ok() && self.is_p(";") {
                            self.bump();
                            let ef = self.last_end();
                            let _ = ef;
                            self.i = save;
                            let ex = self.parse_expr()?;
                            self.expect_p(";")?;
                            init = Some(Box::new(self.mk(from, StmtKind::Expr(ex))));
                        } else {
                            self.i = save;
                        }
                    }
                    let cond = self.parse_expr()?;
                    self.expect_p(")")?;
                    let then = self.body()?;
                    let els = if self.eat_id("else") {
                        Some(self.body()?)
                    } else {
                        None
                    };
                    return Ok(self.mk(
                        from,
                        StmtKind::If {
                            init,
                            cond,
                            then,
                            els,
                        },
                    ));
                }
                "switch" => {
                    self.bump();
                    self.expect_p("(")?;
                    let init = if self.looks_like_decl() {
                        Some(self.decl_stmt()?)
                    } else {
                        None
                    };
                    let cond = self.parse_expr()?;
                    self.expect_p(")")?;
                    let body = self.block()?;
                    let sw = self.mk(from, StmtKind::Switch { cond, body });
                    return Ok(match init {
                        None => sw,
                        Some(i) => self.mk(
                            from,
                            StmtKind::Block(Block {
                                stmts: vec![i, sw],
                                tail_comments: vec![],
                            }),
                        ),
                    });
                }
                "case" => {
                    self.bump();
                    let e = self.ternary()?;
                    self.expect_p(":")?;
                    return Ok(self.mk(from, StmtKind::Case(Some(e))));
                }
                "default" => {
                    self.bump();
                    self.expect_p(":")?;
                    return Ok(self.mk(from, StmtKind::Case(None)));
                }
                "while" => {
                    self.bump();
                    self.expect_p("(")?;
                    let cond = self.parse_expr()?;
                    self.expect_p(")")?;
                    let body = self.body()?;
                    return Ok(self.mk(from, StmtKind::While { cond, body }));
                }
                "do" => {
                    self.bump();
                    let body = self.body()?;
                    if !self.eat_id("while") {
                        return Err("expected while".into());
                    }
                    self.expect_p("(")?;
                    let cond = self.parse_expr()?;
                    self.expect_p(")")?;
                    self.expect_p(";")?;
                    return Ok(self.mk(from, StmtKind::DoWhile { body, cond }));
                }
                "for" => return self.for_stmt(from),
                "return" => {
                    self.bump();
                    let e = if self.is_p(";") {
                        None
                    } else {
                        Some(if self.is_p("{") {
                            self.init_list()?
                        } else {
                            self.parse_expr()?
                        })
                    };
                    self.expect_p(";")?;
                    return Ok(self.mk(from, StmtKind::Return(e)));
                }
                "break" => {
                    self.bump();
                    self.expect_p(";")?;
                    return Ok(self.mk(from, StmtKind::Break));
                }
                "continue" => {
                    self.bump();
                    self.expect_p(";")?;
                    return Ok(self.mk(from, StmtKind::Continue));
                }
                _ => {}
            }
        }
        if self.looks_like_decl() {
            return self.decl_stmt();
        }
        let e = self.parse_expr()?;
        self.expect_p(";")?;
        Ok(self.mk(from, StmtKind::Expr(e)))
    }

    fn for_stmt(&mut self, from: u32) -> R<Stmt> {
        self.bump();
        self.expect_p("(")?;
        // range for?
        if self.looks_like_decl() {
            let save = self.i;
            let spec = self.type_spec()?;
            let mut names = Vec::new();
            let mut structured = false;
            if self.is_p("[") {
                self.bump();
                structured = true;
                names.push(self.ident()?);
                while self.eat_p(",") {
                    names.push(self.ident()?);
                }
                self.expect_p("]")?;
            } else {
                names.push(self.ident()?);
            }
            if self.eat_p(":") {
                let range = if self.is_p("{") {
                    self.init_list()?
                } else {
                    self.parse_expr()?
                };
                self.expect_p(")")?;
                let body = self.body()?;
                return Ok(self.mk(
                    from,
                    StmtKind::RangeFor {
                        spec,
                        names,
                        structured,
                        range,
                        body,
                    },
                ));
            }
            self.i = save;
        }
        let init = if self.eat_p(";") {
            None
        } else if self.looks_like_decl() {
            Some(Box::new(self.decl_stmt()?))
        } else {
            let f = self.pos();
            let e = self.parse_expr()?;
            self.expect_p(";")?;
            Some(Box::new(self.mk(f, StmtKind::Expr(e))))
        };
        let cond = if self.is_p(";") {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect_p(";")?;
        let incr = if self.is_p(")") {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect_p(")")?;
        let body = self.body()?;
        Ok(self.mk(
            from,
            StmtKind::For {
                init,
                cond,
                incr,
                body,
            },
        ))
    }

    // ---- items ----
    fn params(&mut self) -> R<Vec<Param>> {
        self.expect_p("(")?;
        let mut v = Vec::new();
        while !self.is_p(")") {
            let pf = self.pos();
            let spec = self.type_spec()?;
            let name = if let Tok::Id(_) = self.peek() {
                self.ident()?
            } else {
                String::new()
            };
            v.push(Param {
                id: self.id(pf),
                name,
                spec,
            });
            if !self.eat_p(",") {
                break;
            }
        }
        self.expect_p(")")?;
        Ok(v)
    }

    fn item(&mut self, items: &mut Vec<Item>) -> R<()> {
        let from = self.pos();
        if let Tok::Inc(path, angle) = self.peek().clone() {
            self.bump();
            let id = self.id(from);
            items.push(Item {
                id,
                kind: ItemKind::Include { path, angle },
                comments: vec![],
            });
            return Ok(());
        }
        if self.is_id("using") {
            self.bump();
            if self.eat_id("namespace") {
                self.ident()?;
                self.expect_p(";")?;
                self.std_ns = true;
                let id = self.id(from);
                items.push(Item {
                    id,
                    kind: ItemKind::UsingNamespaceStd,
                    comments: vec![],
                });
                return Ok(());
            }
            let name = self.ident()?;
            self.expect_p("=")?;
            let ty = self.type_spec()?.ty;
            self.expect_p(";")?;
            self.types.insert(name.clone(), ty.clone());
            let id = self.id(from);
            items.push(Item {
                id,
                kind: ItemKind::Alias {
                    name,
                    ty,
                    typedef: false,
                },
                comments: vec![],
            });
            return Ok(());
        }
        if self.is_id("typedef") {
            self.bump();
            let ty = self.type_spec()?.ty;
            let name = self.ident()?;
            self.expect_p(";")?;
            self.types.insert(name.clone(), ty.clone());
            let id = self.id(from);
            items.push(Item {
                id,
                kind: ItemKind::Alias {
                    name,
                    ty,
                    typedef: true,
                },
                comments: vec![],
            });
            return Ok(());
        }
        if self.is_id("struct") {
            self.bump();
            let name = self.ident()?;
            self.structs.push(name.clone());
            self.expect_p("{")?;
            let mut fields = Vec::new();
            let mut ctors = Vec::new();
            while !self.is_p("}") {
                // constructor?
                if self.is_id(&name) && matches!(self.peek_at(1), Tok::P("(")) {
                    let cf = self.pos();
                    self.bump();
                    let params = self.params()?;
                    let mut inits = Vec::new();
                    if self.eat_p(":") {
                        loop {
                            let f = self.ident()?;
                            let args = if self.is_p("{") {
                                self.brace_args()?
                            } else {
                                self.call_args()?
                            };
                            inits.push((f, args));
                            if !self.eat_p(",") {
                                break;
                            }
                        }
                    }
                    let body = self.block()?;
                    ctors.push(Ctor {
                        id: self.id(cf),
                        params,
                        inits,
                        body,
                    });
                    continue;
                }
                let spec = self.type_spec()?;
                loop {
                    let n = self.ident()?;
                    let default = if self.eat_p("=") {
                        Some(if self.is_p("{") {
                            self.init_list()?
                        } else {
                            self.assign()?
                        })
                    } else if self.is_p("{") {
                        Some(self.init_list()?)
                    } else {
                        None
                    };
                    fields.push(Field {
                        spec: spec.clone(),
                        name: n,
                        default,
                    });
                    if !self.eat_p(",") {
                        break;
                    }
                }
                self.expect_p(";")?;
            }
            self.expect_p("}")?;
            self.expect_p(";")?;
            let id = self.id(from);
            let sid = self.id(from);
            items.push(Item {
                id,
                kind: ItemKind::Struct(StructDecl {
                    id: sid,
                    name,
                    fields,
                    ctors,
                }),
                comments: vec![],
            });
            return Ok(());
        }
        // function, forward declaration or global
        let is_const = self.is_id("const") || self.is_id("constexpr");
        let constexpr_ = self.is_id("constexpr");
        let is_static = self.is_id("static") || self.is_id("inline");
        let spec = self.type_spec()?;
        let name = self.ident()?;
        if self.is_p("(") {
            let params = self.params()?;
            let body = if self.eat_p(";") {
                None
            } else {
                Some(self.block()?)
            };
            let fid = self.id(from);
            let id = self.id(from);
            items.push(Item {
                id,
                kind: ItemKind::Func(FnDecl {
                    id: fid,
                    name,
                    params,
                    ret: spec,
                    is_static,
                    body,
                    comments: vec![],
                }),
                comments: vec![],
            });
            return Ok(());
        }
        if !is_const {
            return Err(format!("unsupported global `{name}`"));
        }
        // global constant: re-parse declarators from the name
        self.i -= 1;
        let vars = self.declarators()?;
        self.expect_p(";")?;
        let id = self.id(from);
        items.push(Item {
            id,
            kind: ItemKind::Global {
                spec,
                constexpr_,
                vars,
            },
            comments: vec![],
        });
        Ok(())
    }
}

/// Parse a translation unit.
pub fn parse_mini(src: &str, known: &[&str]) -> Result<Program, String> {
    let t = lex(src)?;
    let mut p = P {
        t,
        i: 0,
        next: 1,
        spans: vec![Span::default()],
        types: HashMap::new(),
        structs: known.iter().map(|s| (*s).to_string()).collect(),
        std_ns: false,
    };
    let mut items = Vec::new();
    while !matches!(p.peek(), Tok::Eof) {
        p.item(&mut items)?;
    }
    let mut spans = p.spans;
    spans.resize(p.next as usize, Span::default());
    Ok(Program {
        items,
        prelude_items: 0,
        next_id: p.next,
        spans,
    })
}

/// Parse with whatever parser is current.
pub fn parse_any(src: &str, known: &[&str]) -> Result<Program, String> {
    parse_mini(src, known)
}

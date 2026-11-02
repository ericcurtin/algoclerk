// SPDX-License-Identifier: Apache-2.0
//! Expressions: precedence climbing for binary operators, assignment, `?:`, unary operators and casts,
//! postfix operators, `new` / `delete`, lambdas, brace lists, constructor calls and template-id calls.
//!
//! Grammar, loosest first (C++ precedence): comma, assignment and `?:` (right associative), `||`, `&&`,
//! `|`, `^`, `&`, `== !=`, `< <= > >=`, `<< >>`, `+ -`, `* / %`, unary and casts, postfix, primary.
//! Parentheses leave no node: the inner expression's span is widened to include them.

use super::types::is_std_type_name;
use super::*;

/// Library functions that take explicit template arguments, usable unqualified under `using namespace
/// std` (`max<int>(a, b)`, `get<0>(t)`).
const STD_TEMPLATE_FNS: &[&str] = &[
    "get",
    "max",
    "min",
    "numeric_limits",
    "make_pair",
    "make_tuple",
    "make_optional",
    "swap",
];

/// Keywords that start an expression the subset does not stock, with what to tell the player.
fn unsupported_kw(k: &str) -> Option<&'static str> {
    Some(match k {
        "sizeof" => "`sizeof`",
        "alignof" => "`alignof`",
        "throw" => "`throw` (exceptions)",
        "this" => "`this` (structs have no methods)",
        "dynamic_cast" | "reinterpret_cast" | "const_cast" => {
            "that kind of cast (use `static_cast`)"
        }
        "typeid" => "`typeid`",
        "operator" => "operator overloading",
        "noexcept" => "`noexcept`",
        "template" | "typename" => "templates",
        _ => return None,
    })
}

fn assign_op(p: &str) -> Option<Option<BinOp>> {
    Some(match p {
        "=" => None,
        "+=" => Some(BinOp::Add),
        "-=" => Some(BinOp::Sub),
        "*=" => Some(BinOp::Mul),
        "/=" => Some(BinOp::Div),
        "%=" => Some(BinOp::Rem),
        "&=" => Some(BinOp::BitAnd),
        "|=" => Some(BinOp::BitOr),
        "^=" => Some(BinOp::BitXor),
        "<<=" => Some(BinOp::Shl),
        ">>=" => Some(BinOp::Shr),
        _ => return None,
    })
}

fn bin_op(p: &str) -> Option<BinOp> {
    Some(match p {
        "||" => BinOp::Or,
        "&&" => BinOp::And,
        "|" => BinOp::BitOr,
        "^" => BinOp::BitXor,
        "&" => BinOp::BitAnd,
        "==" => BinOp::Eq,
        "!=" => BinOp::Ne,
        "<" => BinOp::Lt,
        "<=" => BinOp::Le,
        ">" => BinOp::Gt,
        ">=" => BinOp::Ge,
        "<<" => BinOp::Shl,
        ">>" => BinOp::Shr,
        "+" => BinOp::Add,
        "-" => BinOp::Sub,
        "*" => BinOp::Mul,
        "/" => BinOp::Div,
        "%" => BinOp::Rem,
        _ => return None,
    })
}

fn un_op(p: &str) -> Option<UnOp> {
    Some(match p {
        "-" => UnOp::Neg,
        "+" => UnOp::Plus,
        "!" => UnOp::Not,
        "~" => UnOp::BitNot,
        "*" => UnOp::Deref,
        "&" => UnOp::AddrOf,
        "++" => UnOp::PreInc,
        "--" => UnOp::PreDec,
        _ => return None,
    })
}

fn is_scalar(t: &Ty) -> bool {
    matches!(
        t,
        Ty::Bool | Ty::Char | Ty::Int | Ty::UInt | Ty::LongLong | Ty::ULongLong | Ty::Size
    )
}

impl Parser<'_> {
    /// One more level of tree height for a chain built in a loop (`a + b + c ...`, `a.b().c()`), counted
    /// against the nesting limit so that no expression is deeper than [`MAX_DEPTH`].
    fn deepen(&mut self, extra: &mut u32) -> PResult<()> {
        self.enter()?;
        *extra += 1;
        Ok(())
    }

    fn undeepen(&mut self, extra: u32) {
        self.depth = self.depth.saturating_sub(extra);
    }

    /// Expression with the comma operator: `a, b`.
    pub(crate) fn parse_expr(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_assign()?;
        let mut extra = 0;
        while self.at_punct(",") {
            self.bump();
            self.deepen(&mut extra)?;
            let rhs = self.parse_assign()?;
            let start = self.start_of(&lhs);
            lhs = self.mk_expr(
                start,
                ExprKind::Binary(BinOp::Comma, Box::new(lhs), Box::new(rhs)),
            );
        }
        self.undeepen(extra);
        Ok(lhs)
    }

    /// Assignment expression (no comma operator): call arguments, brace list elements, initialisers.
    pub(crate) fn parse_assign(&mut self) -> PResult<Expr> {
        self.enter()?;
        let r = self.assign_inner();
        self.leave();
        r
    }

    fn assign_inner(&mut self) -> PResult<Expr> {
        let lhs = self.parse_ternary()?;
        let op = match self.tok().map(|t| &t.kind) {
            Some(Tok::Punct(p)) => assign_op(p),
            _ => None,
        };
        let Some(op) = op else {
            return Ok(lhs);
        };
        self.bump();
        let rhs = self.parse_assign()?;
        let start = self.start_of(&lhs);
        Ok(self.mk_expr(
            start,
            ExprKind::Assign {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        ))
    }

    fn parse_ternary(&mut self) -> PResult<Expr> {
        let cond = self.parse_bin(BinOp::Or.prec())?;
        if !self.at_punct("?") {
            return Ok(cond);
        }
        self.bump();
        let a = self.parse_expr()?;
        self.expect_punct(":")?;
        let b = self.parse_assign()?;
        let start = self.start_of(&cond);
        Ok(self.mk_expr(
            start,
            ExprKind::Ternary(Box::new(cond), Box::new(a), Box::new(b)),
        ))
    }

    /// Binary operators of precedence `min` and tighter, left associative (precedence climbing).
    fn parse_bin(&mut self, min: u8) -> PResult<Expr> {
        let mut lhs = self.parse_unary()?;
        let mut extra = 0;
        loop {
            let op = match self.tok().map(|t| &t.kind) {
                Some(Tok::Punct(p)) => bin_op(p),
                _ => None,
            };
            let Some(op) = op.filter(|o| o.prec() >= min) else {
                break;
            };
            self.bump();
            self.deepen(&mut extra)?;
            let rhs = self.parse_bin(op.prec() + 1)?;
            let start = self.start_of(&lhs);
            lhs = self.mk_expr(start, ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)));
        }
        self.undeepen(extra);
        Ok(lhs)
    }

    /// Prefix operators, casts, `new`, `delete`, then a postfix expression.
    pub(crate) fn parse_unary(&mut self) -> PResult<Expr> {
        let start = self.cur_start();
        let op = match self.tok().map(|t| &t.kind) {
            Some(Tok::Punct(p)) => un_op(p),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let e = self.parse_operand()?;
            return Ok(self.mk_expr(start, ExprKind::Unary(op, Box::new(e))));
        }
        if self.at_kw("new") {
            return self.parse_new(start);
        }
        if self.at_kw("delete") {
            let span = self.cur_span();
            self.bump();
            if self.at_punct("[") {
                return Err(self.unsupported(span, "`delete[]` (arrays)"));
            }
            let e = self.parse_operand()?;
            return Ok(self.mk_expr(start, ExprKind::Delete(Box::new(e))));
        }
        if self.at_punct("(")
            && let Some(e) = self.try_c_cast(start)?
        {
            return Ok(e);
        }
        self.parse_postfix()
    }

    /// The operand of a prefix operator (one nesting level deeper).
    fn parse_operand(&mut self) -> PResult<Expr> {
        self.enter()?;
        let r = self.parse_unary();
        self.leave();
        r
    }

    /// `new T`, `new T(args)`, `new T{args}`.
    fn parse_new(&mut self, start: u32) -> PResult<Expr> {
        let span = self.cur_span();
        self.bump();
        if self.at_punct("(") {
            return Err(self.unsupported(span, "placement `new`"));
        }
        let ty = self.parse_simple_type()?;
        if self.at_punct("[") {
            return Err(self.unsupported(span, "`new T[n]` (use `std::vector`)"));
        }
        let brace = self.at_punct("{");
        let args = if brace {
            self.parse_brace_elems()?
        } else if self.at_punct("(") {
            self.parse_call_args()?
        } else {
            Vec::new()
        };
        Ok(self.mk_expr(start, ExprKind::New { ty, args, brace }))
    }

    /// Can this token begin the operand of a cast? `weak`: a `-`, `*`, `&` ... would more likely be a
    /// binary operator after a parenthesised variable.
    fn starts_operand(&self, weak: bool) -> bool {
        match self.tok().map(|t| &t.kind) {
            Some(Tok::Ident(_) | Tok::Int { .. } | Tok::Char(_) | Tok::Str(_) | Tok::Float(_)) => {
                true
            }
            Some(Tok::Kw(k)) => matches!(
                *k,
                "true" | "false" | "nullptr" | "this" | "new" | "static_cast" | "sizeof"
            ),
            Some(Tok::Punct(p)) => match *p {
                "(" | "!" | "~" => true,
                "-" | "+" | "*" | "&" | "++" | "--" => !weak,
                _ => false,
            },
            _ => false,
        }
    }

    /// `(T)e` when a parenthesised type is followed by an operand (cursor on `(`).
    fn try_c_cast(&mut self, start: u32) -> PResult<Option<Expr>> {
        let kw_start = self.tok_at(1).is_some_and(|t| match &t.kind {
            Tok::Kw(k) => matches!(
                *k,
                "void"
                    | "bool"
                    | "char"
                    | "int"
                    | "unsigned"
                    | "signed"
                    | "long"
                    | "short"
                    | "float"
                    | "double"
                    | "const"
            ),
            _ => false,
        });
        let weak = match self.ident_at(1) {
            Some("std") if self.at_punct_at(2, "::") => Some(false),
            Some("size_t") => Some(false),
            Some(n) if self.is_known_type(n) => Some(false),
            Some(n) if self.using_std && is_std_type_name(n) => Some(true),
            _ => None,
        };
        if !kw_start && weak.is_none() {
            return Ok(None);
        }
        let m = self.mark();
        self.bump();
        let ty = match self.parse_type_spec() {
            Ok(t) => t,
            Err(d) if kw_start => return Err(d),
            Err(_) => {
                self.restore(m);
                return Ok(None);
            }
        };
        if !self.at_punct(")") || (!kw_start && !self.looks_operand_after_paren(weak == Some(true)))
        {
            self.restore(m);
            return Ok(None);
        }
        self.bump();
        let e = self.parse_operand()?;
        Ok(Some(self.mk_expr(
            start,
            ExprKind::Cast {
                style: CastStyle::CStyle,
                ty,
                e: Box::new(e),
            },
        )))
    }

    /// Is the token after the `)` at the cursor an operand start?
    fn looks_operand_after_paren(&mut self, weak: bool) -> bool {
        self.pos += 1;
        let r = self.starts_operand(weak);
        self.pos -= 1;
        r
    }

    /// Call, index, member access and postfix `++` / `--` after a primary expression.
    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut e = self.parse_primary()?;
        let mut extra = 0;
        loop {
            let start = self.start_of(&e);
            let kind = if self.at_punct("(") {
                let args = self.parse_call_args()?;
                ExprKind::Call {
                    callee: Box::new(e),
                    args,
                }
            } else if self.at_punct("[") {
                self.bump();
                self.enter()?;
                let idx = self.parse_expr()?;
                self.leave();
                self.expect_punct("]")?;
                ExprKind::Index(Box::new(e), Box::new(idx))
            } else if self.at_punct(".") || self.at_punct("->") {
                let arrow = self.at_punct("->");
                self.bump();
                let name = self.expect_ident("a member name")?;
                ExprKind::Member {
                    obj: Box::new(e),
                    name,
                    arrow,
                }
            } else if self.at_punct("++") || self.at_punct("--") {
                let op = if self.at_punct("++") {
                    UnOp::PostInc
                } else {
                    UnOp::PostDec
                };
                self.bump();
                ExprKind::Unary(op, Box::new(e))
            } else {
                break;
            };
            self.deepen(&mut extra)?;
            e = self.mk_expr(start, kind);
        }
        self.undeepen(extra);
        Ok(e)
    }

    /// `(a, b)` call arguments (cursor on `(`).
    pub(crate) fn parse_call_args(&mut self) -> PResult<Vec<Expr>> {
        self.expect_punct("(")?;
        let mut args = Vec::new();
        if !self.at_punct(")") {
            loop {
                args.push(self.parse_assign()?);
                if !self.eat_punct(",") {
                    break;
                }
            }
        }
        self.expect_punct(")")?;
        Ok(args)
    }

    /// `{a, b}` elements, a trailing comma is allowed (cursor on `{`).
    pub(crate) fn parse_brace_elems(&mut self) -> PResult<Vec<Expr>> {
        self.expect_punct("{")?;
        let mut elems = Vec::new();
        while !self.at_punct("}") {
            elems.push(self.parse_assign()?);
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct("}")?;
        Ok(elems)
    }

    /// Literals, names, parentheses, brace lists, lambdas, `static_cast`, type constructions.
    fn parse_primary(&mut self) -> PResult<Expr> {
        let start = self.cur_start();
        let span = self.cur_span();
        let Some(kind) = self.tok().map(|t| t.kind.clone()) else {
            return Err(self.expected("an expression"));
        };
        match kind {
            Tok::Int {
                value,
                suffix,
                base,
            } => {
                self.bump();
                Ok(self.mk_expr(
                    start,
                    ExprKind::Int {
                        value,
                        suffix,
                        base,
                    },
                ))
            }
            Tok::Char(c) => {
                self.bump();
                Ok(self.mk_expr(start, ExprKind::Char(c)))
            }
            Tok::Str(mut s) => {
                self.bump();
                // adjacent literals are one: "ab" "cd"
                while let Some(Tok::Str(more)) = self.tok().map(|t| &t.kind) {
                    s.push_str(more);
                    self.bump();
                }
                Ok(self.mk_expr(start, ExprKind::Str(s)))
            }
            Tok::Float(_) => Err(self.unsupported(span, "decimal numbers (`double`)")),
            Tok::Kw("true" | "false") => {
                let v = self.at_kw("true");
                self.bump();
                Ok(self.mk_expr(start, ExprKind::Bool(v)))
            }
            Tok::Kw("nullptr") => {
                self.bump();
                Ok(self.mk_expr(start, ExprKind::NullPtr))
            }
            Tok::Kw("static_cast") => self.parse_static_cast(start),
            Tok::Kw(k) if unsupported_kw(k).is_some() => {
                Err(self.unsupported(span, unsupported_kw(k).unwrap_or("that")))
            }
            Tok::Kw(_) if self.at_builtin_type() => self.parse_type_construct(start),
            Tok::Ident(_) => {
                if let Some(e) = self.try_type_expr(start)? {
                    return Ok(e);
                }
                self.parse_path(start)
            }
            Tok::Punct("(") => {
                self.bump();
                let e = self.parse_expr()?;
                self.expect_punct(")")?;
                // the parentheses leave no node: widen the span instead
                let span = self.span_from(start);
                if let Some(slot) = self.spans.get_mut(e.id as usize) {
                    *slot = span;
                }
                Ok(e)
            }
            Tok::Punct("{") => {
                let elems = self.parse_brace_elems()?;
                Ok(self.mk_expr(start, ExprKind::InitList(elems)))
            }
            Tok::Punct("[") => self.parse_lambda(start),
            _ => Err(self.expected("an expression")),
        }
    }

    /// `static_cast<T>(e)` (cursor on the keyword).
    fn parse_static_cast(&mut self, start: u32) -> PResult<Expr> {
        self.bump();
        self.expect_punct("<")?;
        let ty = self.parse_type_spec()?;
        self.close_angle()?;
        self.expect_punct("(")?;
        let e = self.parse_expr()?;
        self.expect_punct(")")?;
        Ok(self.mk_expr(
            start,
            ExprKind::Cast {
                style: CastStyle::Static,
                ty,
                e: Box::new(e),
            },
        ))
    }

    /// `T(args)` / `T{args}` after the type was parsed. A scalar type with one parenthesised argument
    /// is a functional cast.
    fn build_construct(&mut self, start: u32, ty: TypeSpec) -> PResult<Expr> {
        let brace = self.at_punct("{");
        let mut args = if brace {
            self.parse_brace_elems()?
        } else {
            self.parse_call_args()?
        };
        if !brace && args.len() == 1 && is_scalar(&ty.ty) {
            let e = args.remove(0);
            return Ok(self.mk_expr(
                start,
                ExprKind::Cast {
                    style: CastStyle::Functional,
                    ty,
                    e: Box::new(e),
                },
            ));
        }
        Ok(self.mk_expr(start, ExprKind::Construct { ty, args, brace }))
    }

    /// A builtin type keyword in an expression: `int(x)`, `unsigned(x)`, `char{}`.
    fn parse_type_construct(&mut self, start: u32) -> PResult<Expr> {
        let ty = self.parse_simple_type()?;
        if self.at_punct("(") || self.at_punct("{") {
            self.build_construct(start, ty)
        } else {
            Err(self.expected("a '(' after the type name"))
        }
    }

    /// A library type, alias or struct name followed by `(` or `{`: `std::vector<int>(n, 0)`,
    /// `ListNode(1)`, `std::string{"a"}`. `None` (cursor unchanged) when the name is an ordinary one.
    fn try_type_expr(&mut self, start: u32) -> PResult<Option<Expr>> {
        let Some(name) = self.ident().map(str::to_string) else {
            return Ok(None);
        };
        let call_next = self.at_punct_at(1, "(") || self.at_punct_at(1, "{");
        let (candidate, strict) = if name == "std" && self.at_punct_at(1, "::") {
            (self.ident_at(2).is_some_and(is_std_type_name), true)
        } else if self.is_known_type(&name) || name == "size_t" {
            (call_next, false)
        } else if self.using_std && is_std_type_name(&name) {
            (call_next || self.at_punct_at(1, "<"), false)
        } else {
            (false, false)
        };
        if !candidate {
            return Ok(None);
        }
        let m = self.mark();
        match self.parse_simple_type() {
            Ok(ty) if self.at_punct("(") || self.at_punct("{") => {
                self.build_construct(start, ty).map(Some)
            }
            Ok(_) => {
                self.restore(m);
                Ok(None)
            }
            Err(d) if strict => Err(d),
            Err(_) => {
                self.restore(m);
                Ok(None)
            }
        }
    }

    /// May the `<` after `name` open template arguments (rather than be a comparison)?
    fn may_template(&self, done: &[PathSeg], name: &str) -> bool {
        match done.first() {
            Some(first) => first.name == "std",
            None => self.using_std && STD_TEMPLATE_FNS.contains(&name),
        }
    }

    /// A name, possibly qualified and with template arguments: `x`, `std::sort`, `std::get<0>`,
    /// `std::numeric_limits<int>::max`, `std::string::npos`.
    fn parse_path(&mut self, start: u32) -> PResult<Expr> {
        let mut segs: Vec<PathSeg> = Vec::new();
        loop {
            let name = self.expect_ident("a name")?;
            let mut targs = None;
            if self.at_punct("<") && self.may_template(&segs, &name) {
                let m = self.mark();
                match self.parse_template_args() {
                    Ok(a) if self.at_punct("(") || self.at_punct("{") || self.at_punct("::") => {
                        targs = Some(a);
                    }
                    _ => self.restore(m),
                }
            }
            segs.push(PathSeg { name, targs });
            if self.at_punct("::") && self.ident_at(1).is_some() {
                self.bump();
                continue;
            }
            break;
        }
        if segs.len() == 1 && segs[0].targs.is_none() {
            let name = segs.remove(0).name;
            return Ok(self.mk_expr(start, ExprKind::Var(name)));
        }
        Ok(self.mk_expr(start, ExprKind::Path(segs)))
    }

    /// `[captures](params) mutable -> ret { body }` (cursor on `[`).
    fn parse_lambda(&mut self, start: u32) -> PResult<Expr> {
        self.expect_punct("[")?;
        let mut default = CaptureDefault::None;
        let mut items: Vec<(String, bool)> = Vec::new();
        let mut first = true;
        while !self.at_punct("]") {
            if self.at_punct("&") {
                self.bump();
                if let Some(n) = self.ident().map(str::to_string) {
                    self.bump();
                    items.push((n, true));
                } else if first {
                    default = CaptureDefault::Ref;
                } else {
                    return Err(self.expected("a name after '&'"));
                }
            } else if self.at_punct("=") && first {
                self.bump();
                default = CaptureDefault::Copy;
            } else if let Some(n) = self.ident().map(str::to_string) {
                self.bump();
                items.push((n, false));
            } else if self.at_kw("this") {
                return Err(self.unsupported(self.cur_span(), "capturing `this`"));
            } else {
                return Err(self.expected("a captured name"));
            }
            if self.at_punct("=") {
                return Err(self.unsupported(self.cur_span(), "captures with an initialiser"));
            }
            first = false;
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct("]")?;
        let params = if self.at_punct("(") {
            self.parse_params()?
        } else {
            Vec::new()
        };
        let mut mutable = false;
        loop {
            if self.eat_kw("mutable") {
                mutable = true;
            } else if self.at_kw("noexcept") || self.at_kw("constexpr") {
                self.bump();
            } else {
                break;
            }
        }
        let ret = if self.eat_punct("->") {
            Some(self.parse_type_spec()?)
        } else {
            None
        };
        if !self.at_punct("{") {
            return Err(self.expected("the '{' of the lambda body"));
        }
        let saved = std::mem::take(&mut self.switch_depth);
        let body = self.parse_block();
        self.switch_depth = saved;
        let body = body?;
        let lambda = Lambda {
            captures: Captures { default, items },
            params,
            ret,
            mutable,
            body,
        };
        Ok(self.mk_expr(start, ExprKind::Lambda(Box::new(lambda))))
    }
}

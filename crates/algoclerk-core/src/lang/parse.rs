// SPDX-License-Identifier: Apache-2.0
//! Lexer and parser for the ClerkScript text view.

use super::ast::*;
use super::diag::Diag;

/// Deepest nesting of blocks, parentheses and types the parser accepts.
pub const MAX_NEST: u32 = 48;
/// Longest program text accepted (bytes).
pub const MAX_SOURCE: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Int(u64),
    Char(u8),
    Str(String),
    P(&'static str),
    Eof,
}

#[derive(Clone, Debug)]
struct Token {
    tok: Tok,
    span: Span,
    /// The next token starts immediately after this one (used to glue `> >` into `>>`).
    glued: bool,
}

const KEYWORDS: &[&str] = &[
    "fn", "let", "if", "else", "while", "for", "in", "break", "continue", "return", "true", "false", "null", "none",
    "and", "or", "not", "mut",
];

/// Words that start a type and therefore may not be used as variable names.
pub const TYPE_WORDS: &[&str] = &[
    "int", "bool", "char", "string", "vec", "set", "map", "option", "stack", "queue", "deque", "heap", "lnode", "tnode",
];

/// Is this word reserved (keyword or type name)?
pub fn is_reserved(w: &str) -> bool {
    KEYWORDS.contains(&w) || TYPE_WORDS.contains(&w)
}

/// Can this text be a variable or function name?
pub fn is_ident(w: &str) -> bool {
    let mut cs = w.chars();
    matches!(cs.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && cs.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !is_reserved(w)
}

const PUNCT: &[&str] = &[
    "<<=", ">>=", "..", "->", "==", "!=", "<=", ">=", "&&", "||", "<<", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=",
    "(", ")", "[", "]", "{", "}", ",", ";", ":", ".", "=", "<", ">", "+", "-", "*", "/", "%", "&", "|", "^", "~", "!",
];

fn lex(src: &str) -> Result<Vec<Token>, Diag> {
    let b = src.as_bytes();
    let mut i = 0usize;
    let mut out: Vec<Token> = Vec::new();
    let sp = |s: usize, e: usize| Span { start: s as u32, end: e as u32 };
    while i < b.len() {
        let c = b[i];
        if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
            i += 1;
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'/') || c == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if !c.is_ascii() {
            let ch = src[i..].chars().next().unwrap_or('?');
            return Err(Diag::at(
                sp(i, i + ch.len_utf8()),
                "lex.ascii",
                format!("My typewriter only has plain ASCII keys, and '{ch}' is not one of them."),
            ));
        }
        let start = i;
        if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(Token { tok: Tok::Ident(src[start..i].to_string()), span: sp(start, i), glued: false });
        } else if c.is_ascii_digit() {
            let (radix, digits_start) = if c == b'0' && matches!(b.get(i + 1), Some(b'x' | b'X')) {
                (16, i + 2)
            } else if c == b'0' && matches!(b.get(i + 1), Some(b'b' | b'B')) {
                (2, i + 2)
            } else {
                (10, i)
            };
            i = digits_start;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let text: String = src[digits_start..i].chars().filter(|&c| c != '_').collect();
            match u64::from_str_radix(&text, radix) {
                Ok(v) if !text.is_empty() => out.push(Token { tok: Tok::Int(v), span: sp(start, i), glued: false }),
                _ => {
                    return Err(Diag::at(
                        sp(start, i),
                        "lex.number",
                        format!("'{}' is not a number my calculator understands (or it is too big).", &src[start..i]),
                    ))
                }
            }
        } else if c == b'\'' || c == b'"' {
            let quote = c;
            i += 1;
            let mut text = Vec::new();
            loop {
                match b.get(i) {
                    None | Some(b'\n') => {
                        return Err(Diag::at(sp(start, i), "lex.unterminated", "This quote never closes."));
                    }
                    Some(&q) if q == quote => {
                        i += 1;
                        break;
                    }
                    Some(b'\\') => {
                        let e = match b.get(i + 1) {
                            Some(b'n') => b'\n',
                            Some(b't') => b'\t',
                            Some(b'r') => b'\r',
                            Some(b'0') => 0,
                            Some(b'\\') => b'\\',
                            Some(b'\'') => b'\'',
                            Some(b'"') => b'"',
                            _ => {
                                return Err(Diag::at(
                                    sp(i, (i + 2).min(b.len())),
                                    "lex.escape",
                                    "I only know the escapes \\n \\t \\r \\0 \\\\ \\' and \\\".",
                                ))
                            }
                        };
                        text.push(e);
                        i += 2;
                    }
                    Some(&ch) if ch.is_ascii() && (ch == b' ' || ch.is_ascii_graphic()) => {
                        text.push(ch);
                        i += 1;
                    }
                    Some(_) => {
                        return Err(Diag::at(
                            sp(i, i + 1),
                            "lex.ascii",
                            "My typewriter only has plain ASCII keys inside quotes too.",
                        ))
                    }
                }
            }
            if quote == b'\'' {
                if text.len() != 1 {
                    return Err(Diag::at(
                        sp(start, i),
                        "lex.char",
                        "A character in single quotes holds exactly one letter. Use double quotes for words.",
                    ));
                }
                out.push(Token { tok: Tok::Char(text[0]), span: sp(start, i), glued: false });
            } else {
                out.push(Token {
                    tok: Tok::Str(String::from_utf8_lossy(&text).into_owned()),
                    span: sp(start, i),
                    glued: false,
                });
            }
        } else {
            // `>>` is lexed as two glued `>` so that `map<int, vec<int>>` works; `>>=` and `>=` stay whole.
            let rest = &src[i..];
            let p = PUNCT.iter().find(|p| rest.starts_with(**p));
            let Some(p) = p else {
                return Err(Diag::at(sp(i, i + 1), "lex.char", format!("I do not know what '{}' means here.", c as char)));
            };
            i += p.len();
            out.push(Token { tok: Tok::P(p), span: sp(start, i), glued: false });
        }
        if let Some(last) = out.last_mut() {
            last.glued = b.get(i).is_some_and(|n| !n.is_ascii_whitespace());
        }
    }
    out.push(Token { tok: Tok::Eof, span: sp(b.len(), b.len()), glued: false });
    Ok(out)
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
    next_id: NodeId,
    spans: Vec<Span>,
    depth: u32,
    heights: Vec<u16>,
    too_tall: bool,
}

/// Tallest expression tree the parser accepts (keeps every later recursion shallow).
pub const MAX_EXPR_HEIGHT: u16 = 100;

type R<T> = Result<T, Diag>;

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }
    fn peek_at(&self, n: usize) -> &Tok {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)].tok
    }
    fn span(&self) -> Span {
        self.toks[self.pos].span
    }
    fn prev_end(&self) -> u32 {
        if self.pos == 0 { 0 } else { self.toks[self.pos - 1].span.end }
    }
    fn bump(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }
    fn is_p(&self, p: &str) -> bool {
        matches!(self.peek(), Tok::P(q) if *q == p)
    }
    fn is_kw(&self, k: &str) -> bool {
        matches!(self.peek(), Tok::Ident(q) if q == k)
    }
    fn eat_p(&mut self, p: &str) -> bool {
        if self.is_p(p) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn eat_kw(&mut self, k: &str) -> bool {
        if self.is_kw(k) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn describe(&self) -> String {
        match self.peek() {
            Tok::Ident(s) => format!("'{s}'"),
            Tok::Int(n) => format!("'{n}'"),
            Tok::Char(_) | Tok::Str(_) => "a quoted value".into(),
            Tok::P(p) => format!("'{p}'"),
            Tok::Eof => "the end of the page".into(),
        }
    }
    fn expect_p(&mut self, p: &str) -> R<()> {
        if self.eat_p(p) {
            Ok(())
        } else {
            let what = match p {
                ";" => "I expected a ';' to finish the sentence".to_string(),
                "{" => "I expected a '{' to open the body".to_string(),
                "}" => "I expected a '}' to close the body".to_string(),
                ")" => "I expected a ')' to close the bracket".to_string(),
                "]" => "I expected a ']' to close the bracket".to_string(),
                _ => format!("I expected '{p}'"),
            };
            Err(Diag::at(self.span(), "parse.expected", format!("{what}, but found {}.", self.describe())))
        }
    }
    fn expect_kw(&mut self, k: &str) -> R<()> {
        if self.eat_kw(k) {
            Ok(())
        } else {
            Err(Diag::at(self.span(), "parse.expected", format!("I expected '{k}', but found {}.", self.describe())))
        }
    }
    fn ident(&mut self, what: &str) -> R<String> {
        match self.peek().clone() {
            Tok::Ident(s) if !is_reserved(&s) => {
                self.bump();
                Ok(s)
            }
            Tok::Ident(s) => Err(Diag::at(
                self.span(),
                "parse.reserved",
                format!("'{s}' is a reserved word, so it cannot be {what}."),
            )),
            _ => Err(Diag::at(self.span(), "parse.expected", format!("I expected {what}, but found {}.", self.describe()))),
        }
    }
    fn enter(&mut self) -> R<()> {
        self.depth += 1;
        if self.depth > MAX_NEST {
            return Err(Diag::at(self.span(), "parse.deep", "That is nested too deep for my desk. Try a helper function."));
        }
        Ok(())
    }
    fn id(&mut self, start: u32) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        self.spans.resize(self.next_id as usize, Span::default());
        self.spans[id as usize] = Span { start, end: self.prev_end().max(start) };
        id
    }
    fn expr(&mut self, kind: ExprKind, start: u32) -> Expr {
        let h = |p: &Parser, e: &Expr| p.heights.get(e.id as usize).copied().unwrap_or(0);
        let below = match &kind {
            ExprKind::Unary(_, a) | ExprKind::Field(a, _) | ExprKind::TupleIdx(a, _) => h(self, a),
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::VecRepeat(a, b) => h(self, a).max(h(self, b)),
            ExprKind::Slice(a, b, c) => h(self, a).max(b.as_ref().map_or(0, |x| h(self, x))).max(c.as_ref().map_or(0, |x| h(self, x))),
            ExprKind::Call(_, a) | ExprKind::VecLit(a) | ExprKind::TupleLit(a) => a.iter().map(|x| h(self, x)).max().unwrap_or(0),
            ExprKind::Method(r, _, a) => a.iter().map(|x| h(self, x)).max().unwrap_or(0).max(h(self, r)),
            _ => 0,
        };
        let id = self.id(start);
        let height = below.saturating_add(1);
        if height > MAX_EXPR_HEIGHT {
            self.too_tall = true;
        }
        if self.heights.len() <= id as usize {
            self.heights.resize(id as usize + 1, 0);
        }
        self.heights[id as usize] = height.min(MAX_EXPR_HEIGHT + 1);
        Expr { id, kind }
    }
    fn stmt(&mut self, kind: StmtKind, start: u32) -> Stmt {
        Stmt { id: self.id(start), kind }
    }

    // ----- types -----
    fn ty(&mut self) -> R<Ty> {
        self.enter()?;
        let r = self.ty_inner();
        self.depth -= 1;
        r
    }
    fn generic1(&mut self) -> R<Ty> {
        self.expect_p("<")?;
        let t = self.ty()?;
        self.expect_gt()?;
        Ok(t)
    }
    fn expect_gt(&mut self) -> R<()> {
        self.expect_p(">")
    }
    fn ty_inner(&mut self) -> R<Ty> {
        if self.eat_p("(") {
            let mut ts = vec![self.ty()?];
            while self.eat_p(",") {
                ts.push(self.ty()?);
            }
            self.expect_p(")")?;
            if ts.len() < 2 {
                return Err(Diag::at(self.span(), "parse.tuple", "A tuple type needs at least two parts, like (int, int)."));
            }
            return Ok(Ty::Tuple(ts));
        }
        let sp = self.span();
        let Tok::Ident(w) = self.peek().clone() else {
            return Err(Diag::at(sp, "parse.type", format!("I expected a type here, but found {}.", self.describe())));
        };
        let t = match w.as_str() {
            "int" => Ty::Int,
            "bool" => Ty::Bool,
            "char" => Ty::Char,
            "string" => Ty::Str,
            "lnode" => Ty::LNode,
            "tnode" => Ty::TNode,
            "vec" | "set" | "option" | "stack" | "queue" | "deque" | "heap" => {
                self.bump();
                let inner = Box::new(self.generic1()?);
                return Ok(match w.as_str() {
                    "vec" => Ty::Vec(inner),
                    "set" => Ty::Set(inner),
                    "option" => Ty::Option(inner),
                    "stack" => Ty::Stack(inner),
                    "queue" => Ty::Queue(inner),
                    "deque" => Ty::Deque(inner),
                    _ => Ty::Heap(inner),
                });
            }
            "map" => {
                self.bump();
                self.expect_p("<")?;
                let k = self.ty()?;
                self.expect_p(",")?;
                let v = self.ty()?;
                self.expect_gt()?;
                return Ok(Ty::Map(Box::new(k), Box::new(v)));
            }
            _ => {
                return Err(Diag::at(
                    sp,
                    "parse.type",
                    format!("'{w}' is not a type I know. Try int, bool, char, string, vec<..>, map<..>, set<..> ..."),
                ))
            }
        };
        self.bump();
        Ok(t)
    }

    // ----- items -----
    fn program(&mut self) -> R<Vec<FnDecl>> {
        let mut fns = Vec::new();
        while *self.peek() != Tok::Eof {
            fns.push(self.func()?);
        }
        Ok(fns)
    }
    fn func(&mut self) -> R<FnDecl> {
        let start = self.span().start;
        self.expect_kw("fn")?;
        let name = self.ident("a function name")?;
        self.expect_p("(")?;
        let mut params = Vec::new();
        while !self.is_p(")") {
            let pname = self.ident("a parameter name")?;
            self.expect_p(":")?;
            let by_ref = if self.is_p("&") && matches!(self.peek_at(1), Tok::Ident(m) if m == "mut") {
                self.bump();
                self.bump();
                true
            } else {
                false
            };
            let ty = self.ty()?;
            params.push(Param { name: pname, ty, by_ref });
            if !self.eat_p(",") {
                break;
            }
        }
        self.expect_p(")")?;
        let ret = if self.eat_p("->") { self.ty()? } else { Ty::Unit };
        let body = self.block()?;
        Ok(FnDecl { id: self.id(start), name, params, ret, body })
    }
    fn block(&mut self) -> R<Block> {
        self.enter()?;
        self.expect_p("{")?;
        let mut stmts = Vec::new();
        while !self.is_p("}") {
            if *self.peek() == Tok::Eof {
                return Err(Diag::at(self.span(), "parse.expected", "I expected a '}' to close the body, but the page ended."));
            }
            stmts.push(self.statement()?);
        }
        self.expect_p("}")?;
        self.depth -= 1;
        Ok(Block { stmts })
    }
    fn pattern(&mut self) -> R<Pat> {
        if self.eat_p("(") {
            let mut ps = vec![self.pattern()?];
            while self.eat_p(",") {
                ps.push(self.pattern()?);
            }
            self.expect_p(")")?;
            if ps.len() < 2 {
                return Err(Diag::at(self.span(), "parse.tuple", "A tuple pattern needs at least two names, like (a, b)."));
            }
            return Ok(Pat::Tuple(ps));
        }
        if matches!(self.peek(), Tok::Ident(s) if s == "_") {
            self.bump();
            return Ok(Pat::Wild);
        }
        Ok(Pat::Var(self.ident("a name")?))
    }
    fn statement(&mut self) -> R<Stmt> {
        let start = self.span().start;
        let kind = match self.peek().clone() {
            Tok::Ident(k) if k == "let" => {
                self.bump();
                let pat = self.pattern()?;
                let ty = if self.eat_p(":") { Some(self.ty()?) } else { None };
                self.expect_p("=")?;
                let init = self.expr_top()?;
                self.expect_p(";")?;
                StmtKind::Let { pat, ty, init }
            }
            Tok::Ident(k) if k == "if" => return self.if_stmt(),
            Tok::Ident(k) if k == "while" => {
                self.bump();
                let cond = self.expr_top()?;
                let body = self.block()?;
                StmtKind::While { cond, body }
            }
            Tok::Ident(k) if k == "for" => {
                self.bump();
                let pat = self.pattern()?;
                self.expect_kw("in")?;
                let is_range = matches!(self.peek(), Tok::Ident(r) if r == "range") && *self.peek_at(1) == Tok::P("(");
                if is_range && let Pat::Var(var) = &pat {
                    self.bump();
                    self.bump();
                    let mut args = vec![self.expr_top()?];
                    while self.eat_p(",") {
                        args.push(self.expr_top()?);
                    }
                    self.expect_p(")")?;
                    let body = self.block()?;
                    let mut it = args.into_iter();
                    let (start_e, end, step) = match (it.next(), it.next(), it.next(), it.next()) {
                        (Some(e), None, ..) => (None, e, None),
                        (Some(a), Some(b), None, _) => (Some(a), b, None),
                        (Some(a), Some(b), Some(c), None) => (Some(a), b, Some(c)),
                        _ => {
                            return Err(Diag::at(
                                Span { start, end: self.prev_end() },
                                "parse.range",
                                "range takes one, two or three numbers: range(n), range(a, b) or range(a, b, step).",
                            ))
                        }
                    };
                    StmtKind::ForRange { var: var.clone(), start: start_e, end, step, body }
                } else {
                    let iter = self.expr_top()?;
                    let body = self.block()?;
                    StmtKind::ForEach { pat, iter, body }
                }
            }
            Tok::Ident(k) if k == "break" => {
                self.bump();
                self.expect_p(";")?;
                StmtKind::Break
            }
            Tok::Ident(k) if k == "continue" => {
                self.bump();
                self.expect_p(";")?;
                StmtKind::Continue
            }
            Tok::Ident(k) if k == "return" => {
                self.bump();
                let e = if self.is_p(";") { None } else { Some(self.expr_top()?) };
                self.expect_p(";")?;
                StmtKind::Return(e)
            }
            _ => {
                let e = self.expr_top()?;
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
                    if !matches!(e.kind, ExprKind::Var(_) | ExprKind::Index(..) | ExprKind::Field(..)) {
                        return Err(Diag::at(
                            self.span(),
                            "parse.place",
                            "You can only store into a variable, a shelf slot (a[i]) or a node field.",
                        ));
                    }
                    self.bump();
                    let value = self.expr_top()?;
                    self.expect_p(";")?;
                    StmtKind::Assign { target: e, op, value }
                } else {
                    self.expect_p(";")?;
                    StmtKind::Expr(e)
                }
            }
        };
        Ok(self.stmt(kind, start))
    }
    fn if_stmt(&mut self) -> R<Stmt> {
        self.enter()?;
        let start = self.span().start;
        self.expect_kw("if")?;
        let cond = self.expr_top()?;
        let then = self.block()?;
        let els = if self.eat_kw("else") {
            if self.is_kw("if") {
                let nested = self.if_stmt()?;
                Some(Block { stmts: vec![nested] })
            } else {
                Some(self.block()?)
            }
        } else {
            None
        };
        self.depth -= 1;
        Ok(self.stmt(StmtKind::If { cond, then, els }, start))
    }

    // ----- expressions -----
    fn expr_top(&mut self) -> R<Expr> {
        self.enter()?;
        let e = self.or_expr();
        self.depth -= 1;
        e
    }
    fn or_expr(&mut self) -> R<Expr> {
        let start = self.span().start;
        let mut l = self.and_expr()?;
        while self.eat_kw("or") || self.eat_p("||") {
            let r = self.and_expr()?;
            l = self.expr(ExprKind::Binary(BinOp::Or, Box::new(l), Box::new(r)), start);
        }
        Ok(l)
    }
    fn and_expr(&mut self) -> R<Expr> {
        let start = self.span().start;
        let mut l = self.not_expr()?;
        while self.eat_kw("and") || self.eat_p("&&") {
            let r = self.not_expr()?;
            l = self.expr(ExprKind::Binary(BinOp::And, Box::new(l), Box::new(r)), start);
        }
        Ok(l)
    }
    fn not_expr(&mut self) -> R<Expr> {
        let start = self.span().start;
        if self.eat_kw("not") || self.eat_p("!") {
            self.enter()?;
            let e = self.not_expr()?;
            self.depth -= 1;
            return Ok(self.expr(ExprKind::Unary(UnOp::Not, Box::new(e)), start));
        }
        self.bin_expr(4)
    }
    /// Look at the next binary operator without consuming it. Returns the operator and the token count.
    fn peek_binop(&self) -> Option<(BinOp, usize)> {
        let t = &self.toks[self.pos];
        let Tok::P(p) = t.tok else { return None };
        Some(match p {
            "+" => (BinOp::Add, 1),
            "-" => (BinOp::Sub, 1),
            "*" => (BinOp::Mul, 1),
            "/" => (BinOp::Div, 1),
            "%" => (BinOp::Rem, 1),
            "&" => (BinOp::BitAnd, 1),
            "|" => (BinOp::BitOr, 1),
            "^" => (BinOp::BitXor, 1),
            "<<" => (BinOp::Shl, 1),
            "==" => (BinOp::Eq, 1),
            "!=" => (BinOp::Ne, 1),
            "<" => (BinOp::Lt, 1),
            "<=" => (BinOp::Le, 1),
            ">=" => (BinOp::Ge, 1),
            ">" => {
                if t.glued && self.toks.get(self.pos + 1).is_some_and(|n| n.tok == Tok::P(">")) {
                    (BinOp::Shr, 2)
                } else {
                    (BinOp::Gt, 1)
                }
            }
            _ => return None,
        })
    }
    fn bin_expr(&mut self, min_prec: u8) -> R<Expr> {
        let start = self.span().start;
        let mut l = self.unary()?;
        while let Some((op, ntok)) = self.peek_binop() {
            if op.prec() < min_prec {
                break;
            }
            for _ in 0..ntok {
                self.bump();
            }
            let r = self.bin_expr(op.prec() + 1)?;
            l = self.expr(ExprKind::Binary(op, Box::new(l), Box::new(r)), start);
        }
        Ok(l)
    }
    fn unary(&mut self) -> R<Expr> {
        let start = self.span().start;
        if self.is_p("-") {
            self.bump();
            if let Tok::Int(n) = self.peek().clone() {
                // Fold `-literal` into a negative literal (also covers i64::MIN) unless a postfix follows.
                if !matches!(self.peek_at(1), Tok::P("." | "[")) {
                    self.bump();
                    let v = if n == 1u64 << 63 {
                        i64::MIN
                    } else {
                        i64::try_from(n)
                            .map(|x| -x)
                            .map_err(|_| Diag::at(self.span(), "lex.number", "That number is too big for my calculator."))?
                    };
                    return Ok(self.expr(ExprKind::Int(v), start));
                }
            }
            self.enter()?;
            let e = self.unary()?;
            self.depth -= 1;
            return Ok(self.expr(ExprKind::Unary(UnOp::Neg, Box::new(e)), start));
        }
        if self.eat_p("~") {
            self.enter()?;
            let e = self.unary()?;
            self.depth -= 1;
            return Ok(self.expr(ExprKind::Unary(UnOp::BitNot, Box::new(e)), start));
        }
        self.postfix()
    }
    fn postfix(&mut self) -> R<Expr> {
        let start = self.span().start;
        let mut e = self.primary()?;
        loop {
            if self.eat_p(".") {
                match self.peek().clone() {
                    Tok::Int(n) => {
                        self.bump();
                        e = self.expr(ExprKind::TupleIdx(Box::new(e), n as usize), start);
                    }
                    Tok::Ident(name) => {
                        self.bump();
                        if self.eat_p("(") {
                            let args = self.args(")")?;
                            e = self.expr(ExprKind::Method(Box::new(e), name, args), start);
                        } else {
                            e = self.expr(ExprKind::Field(Box::new(e), name), start);
                        }
                    }
                    _ => {
                        return Err(Diag::at(
                            self.span(),
                            "parse.expected",
                            format!("I expected a method or field name after the dot, but found {}.", self.describe()),
                        ))
                    }
                }
            } else if self.eat_p("[") {
                self.enter()?;
                let mut lo = None;
                if !self.is_p("..") {
                    lo = Some(Box::new(self.expr_top()?));
                }
                if self.eat_p("..") {
                    let hi = if self.is_p("]") { None } else { Some(Box::new(self.expr_top()?)) };
                    self.expect_p("]")?;
                    e = self.expr(ExprKind::Slice(Box::new(e), lo, hi), start);
                } else {
                    self.expect_p("]")?;
                    let idx = lo.ok_or_else(|| Diag::at(self.span(), "parse.index", "Empty index brackets."))?;
                    e = self.expr(ExprKind::Index(Box::new(e), idx), start);
                }
                self.depth -= 1;
            } else {
                return Ok(e);
            }
        }
    }
    fn args(&mut self, close: &str) -> R<Vec<Expr>> {
        let mut v = Vec::new();
        while !self.is_p(close) {
            v.push(self.expr_top()?);
            if !self.eat_p(",") {
                break;
            }
        }
        self.expect_p(close)?;
        Ok(v)
    }
    fn primary(&mut self) -> R<Expr> {
        let start = self.span().start;
        let sp = self.span();
        match self.peek().clone() {
            Tok::Int(n) => {
                self.bump();
                let v = i64::try_from(n)
                    .map_err(|_| Diag::at(sp, "lex.number", "That number is too big for my calculator."))?;
                Ok(self.expr(ExprKind::Int(v), start))
            }
            Tok::Char(c) => {
                self.bump();
                Ok(self.expr(ExprKind::Char(c), start))
            }
            Tok::Str(s) => {
                self.bump();
                Ok(self.expr(ExprKind::Str(s), start))
            }
            Tok::P("(") => {
                self.bump();
                self.enter()?;
                let first = self.expr_top()?;
                let r = if self.eat_p(",") {
                    let mut items = vec![first];
                    while !self.is_p(")") {
                        items.push(self.expr_top()?);
                        if !self.eat_p(",") {
                            break;
                        }
                    }
                    self.expect_p(")")?;
                    if items.len() < 2 {
                        return Err(Diag::at(sp, "parse.tuple", "A tuple needs at least two parts."));
                    }
                    self.expr(ExprKind::TupleLit(items), start)
                } else {
                    self.expect_p(")")?;
                    // Keep parenthesised literals distinguishable only through the printer's precedence rules.
                    first
                };
                self.depth -= 1;
                Ok(r)
            }
            Tok::P("[") => {
                self.bump();
                self.enter()?;
                let r = if self.eat_p("]") {
                    self.expr(ExprKind::VecLit(vec![]), start)
                } else {
                    let first = self.expr_top()?;
                    if self.eat_p(";") {
                        let n = self.expr_top()?;
                        self.expect_p("]")?;
                        self.expr(ExprKind::VecRepeat(Box::new(first), Box::new(n)), start)
                    } else {
                        let mut items = vec![first];
                        while self.eat_p(",") {
                            if self.is_p("]") {
                                break;
                            }
                            items.push(self.expr_top()?);
                        }
                        self.expect_p("]")?;
                        self.expr(ExprKind::VecLit(items), start)
                    }
                };
                self.depth -= 1;
                Ok(r)
            }
            Tok::Ident(w) => match w.as_str() {
                "true" | "false" => {
                    self.bump();
                    Ok(self.expr(ExprKind::Bool(w == "true"), start))
                }
                "null" => {
                    self.bump();
                    Ok(self.expr(ExprKind::Null, start))
                }
                "none" => {
                    self.bump();
                    Ok(self.expr(ExprKind::NoneLit, start))
                }
                "vec" | "set" | "map" | "stack" | "queue" | "deque" | "heap" | "option" => {
                    let t = self.ty()?;
                    self.expect_p("(")?;
                    self.expect_p(")")?;
                    Ok(self.expr(ExprKind::Ctor(t), start))
                }
                "lnode" | "tnode" if *self.peek_at(1) == Tok::P("(") => {
                    self.bump();
                    self.bump();
                    let args = self.args(")")?;
                    Ok(self.expr(ExprKind::Call(w, args), start))
                }
                _ if is_reserved(&w) => Err(Diag::at(
                    sp,
                    "parse.reserved",
                    format!("I did not expect '{w}' here; it is a reserved word."),
                )),
                _ => {
                    self.bump();
                    if self.eat_p("(") {
                        let args = self.args(")")?;
                        Ok(self.expr(ExprKind::Call(w, args), start))
                    } else {
                        Ok(self.expr(ExprKind::Var(w), start))
                    }
                }
            },
            _ => Err(Diag::at(sp, "parse.expected", format!("I expected a value here, but found {}.", self.describe()))),
        }
    }
}

/// Parse program text into a [`Program`].
pub fn parse(src: &str) -> Result<Program, Diag> {
    if src.len() > MAX_SOURCE {
        return Err(Diag::plain("parse.long", "That page is too long for my desk."));
    }
    let toks = lex(src)?;
    let mut p = Parser { toks, pos: 0, next_id: 0, spans: Vec::new(), depth: 0, heights: Vec::new(), too_tall: false };
    let fns = p.program()?;
    if p.too_tall {
        return Err(Diag::plain("parse.tall", "That sentence is far too long-winded for me. Break it into smaller steps with `let`."));
    }
    Ok(Program { fns, next_id: p.next_id, spans: p.spans })
}

/// Parse a single type such as `map<string, vec<int>>` (level signatures, tests).
pub fn parse_type(src: &str) -> Result<Ty, Diag> {
    let toks = lex(src)?;
    let mut p = Parser { toks, pos: 0, next_id: 0, spans: Vec::new(), depth: 0, heights: Vec::new(), too_tall: false };
    let t = p.ty()?;
    if *p.peek() != Tok::Eof {
        return Err(Diag::at(p.span(), "parse.expected", "Extra text after the type."));
    }
    Ok(t)
}

/// Parse one expression (used for level examples and the autocomplete preview).
pub fn parse_expr(src: &str) -> Result<Expr, Diag> {
    let toks = lex(src)?;
    let mut p = Parser { toks, pos: 0, next_id: 0, spans: Vec::new(), depth: 0, heights: Vec::new(), too_tall: false };
    let e = p.expr_top()?;
    if *p.peek() != Tok::Eof {
        return Err(Diag::at(p.span(), "parse.expected", "Extra text after the expression."));
    }
    if p.too_tall {
        return Err(Diag::plain("parse.tall", "That sentence is far too long-winded for me."));
    }
    Ok(e)
}

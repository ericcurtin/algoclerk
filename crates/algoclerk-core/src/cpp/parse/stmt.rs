// SPDX-License-Identifier: Apache-2.0
//! Statements: declarations (with structured bindings and C arrays), control flow, blocks, and the
//! attachment of comments to statements.
//!
//! Comment rules: full-line comments (and, leniently, any comment not otherwise claimed) before a
//! statement become its `comments`; a comment on the same line after a simple statement is its
//! `trailing`, and for a statement with a braced body the comment after the opening `{` is. Comments
//! before a closing `}` are the block's `tail_comments`. A `switch (init; cond)` is desugared to
//! `{ init; switch (cond) ... }` because the syntax tree has no init slot for it.

use super::*;

/// Keywords that start a statement the subset does not stock, with what to tell the player.
fn unsupported_stmt_kw(k: &str) -> Option<&'static str> {
    Some(match k {
        "try" | "catch" => "`try` and `catch` (exceptions)",
        "goto" => "`goto`",
        "class" => "classes (write a `struct`)",
        "enum" => "`enum`",
        "union" => "`union`",
        "template" => "templates",
        "namespace" => "namespaces",
        "using" => "`using` inside a function (put it at the top of the file)",
        "typedef" => "`typedef` inside a function (put it at the top of the file)",
        "static_assert" => "`static_assert`",
        "asm" => "inline assembly",
        "extern" => "`extern`",
        "struct" => "structs declared inside a function (declare them at the top of the file)",
        "thread_local" | "register" | "volatile" | "mutable" | "virtual" | "friend" => {
            "that storage specifier"
        }
        _ => return None,
    })
}

/// Clean one comment token into lines of text for `Stmt::comments` (`//` text, leading space kept).
/// Trailing blanks and backslashes are dropped so that printing and re-parsing is stable.
pub(crate) fn comment_lines(text: &str, block: bool) -> Vec<String> {
    let mut lines: Vec<String> = text
        .split('\n')
        .map(|l| {
            let l = l.trim_end_matches(['\r', ' ', '\t', '\\']);
            l.to_string()
        })
        .collect();
    if block && lines.len() > 1 {
        // `/*\n * a\n */` keeps its text lines only
        if lines.first().is_some_and(|l| l.trim().is_empty()) {
            lines.remove(0);
        }
        if lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

impl Parser<'_> {
    // ---- comments -----------------------------------------------------------------------------

    /// Take every comment that starts before byte `pos` and was not taken yet.
    pub(crate) fn take_comments_before(&mut self, pos: u32) -> Vec<String> {
        let mut out = Vec::new();
        while let Some(c) = self.comments.get(self.cidx) {
            if c.span.start >= pos {
                break;
            }
            if let Tok::Comment { text, block } = &c.kind {
                out.extend(comment_lines(text, *block));
            }
            self.cidx += 1;
        }
        out
    }

    /// Drop every comment that starts before byte `pos`.
    pub(crate) fn skip_comments_before(&mut self, pos: u32) {
        while self.comments.get(self.cidx).is_some_and(|c| c.span.start < pos) {
            self.cidx += 1;
        }
    }

    /// A comment on the same line right after the last consumed token, if there is one.
    pub(crate) fn take_trailing(&mut self) -> Option<String> {
        let c = self.comments.get(self.cidx)?;
        let Tok::Comment { text, block } = &c.kind else {
            return None;
        };
        let next_start = self.tok().map_or(u32::MAX, |t| t.span.start);
        if c.span.start < self.last_end || c.nl_before || c.span.end > next_start {
            return None;
        }
        if text.contains('\n') {
            return None;
        }
        let line = comment_lines(text, *block).pop().unwrap_or_default();
        self.cidx += 1;
        Some(line)
    }

    // ---- blocks and statements ----------------------------------------------------------------

    /// Parse `{ ... }`. The comment after the `{` is left in `block_open` for the owner statement.
    pub(crate) fn parse_block(&mut self) -> PResult<Block> {
        if !self.at_punct("{") {
            return Err(self.expected("a '{'"));
        }
        let open_span = self.cur_span();
        self.bump();
        self.enter()?;
        let open = self.take_trailing();
        let mut stmts = Vec::new();
        while !self.at_punct("}") {
            if self.at_eof() {
                return Err(Diag::at(
                    open_span,
                    "parse.expected",
                    "I expected a '}' to close this '{'",
                ));
            }
            stmts.push(self.parse_stmt()?);
        }
        let tail_comments = self.take_comments_before(self.cur_start());
        self.bump();
        self.leave();
        self.block_open = open;
        Ok(Block {
            stmts,
            tail_comments,
        })
    }

    /// The body of `if` / `for` / `while` / `do` / `else`: a block (with the comment after its `{`)
    /// or a single statement.
    fn parse_body(&mut self) -> PResult<(Stmt, Option<String>)> {
        if self.at_punct("{") {
            let start = self.cur_start();
            let block = self.parse_block()?;
            let open = self.block_open.take();
            let id = self.new_id(self.span_from(start));
            Ok((Stmt::new(id, StmtKind::Block(block)), open))
        } else {
            Ok((self.parse_stmt()?, None))
        }
    }

    /// One statement with its comments.
    pub(crate) fn parse_stmt(&mut self) -> PResult<Stmt> {
        let comments = self.take_comments_before(self.cur_start());
        self.enter()?;
        let start = self.cur_start();
        let r = self.stmt_inner();
        self.leave();
        let (kind, open) = r?;
        let compound = matches!(
            kind,
            StmtKind::If { .. }
                | StmtKind::Switch { .. }
                | StmtKind::For { .. }
                | StmtKind::RangeFor { .. }
                | StmtKind::While { .. }
                | StmtKind::DoWhile { .. }
                | StmtKind::Block(_)
        );
        let trailing = if compound {
            open
        } else {
            self.take_trailing()
        };
        let id = self.new_id(self.span_from(start));
        Ok(Stmt {
            id,
            kind,
            comments,
            trailing,
        })
    }

    /// A statement without comments (init statements in headers).
    fn mk_stmt(&mut self, start: u32, kind: StmtKind) -> Stmt {
        let id = self.new_id(self.span_from(start));
        Stmt::new(id, kind)
    }

    /// The statement kind and, for statements with a braced body, the comment after its `{`.
    fn stmt_inner(&mut self) -> PResult<(StmtKind, Option<String>)> {
        let span = self.cur_span();
        let Some(kind) = self.tok().map(|t| t.kind.clone()) else {
            return Err(self.expected("a statement"));
        };
        match kind {
            Tok::Punct("{") => {
                let b = self.parse_block()?;
                let open = self.block_open.take();
                Ok((StmtKind::Block(b), open))
            }
            Tok::Punct(";") => {
                self.bump();
                Ok((StmtKind::Empty, None))
            }
            Tok::Preproc(text) => Err(preproc_diag(&text, span)),
            Tok::Kw("if") => self.parse_if(),
            Tok::Kw("switch") => self.parse_switch(),
            Tok::Kw("for") => self.parse_for(),
            Tok::Kw("while") => self.parse_while(),
            Tok::Kw("do") => self.parse_do(),
            Tok::Kw("case") => self.parse_case(),
            Tok::Kw("default") => {
                self.bump();
                self.case_check(span)?;
                self.expect_punct(":")?;
                Ok((StmtKind::Case(None), None))
            }
            Tok::Kw("return") => {
                self.bump();
                let e = if self.at_punct(";") {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.expect_punct(";")?;
                Ok((StmtKind::Return(e), None))
            }
            Tok::Kw(k @ ("break" | "continue")) => {
                self.bump();
                self.expect_punct(";")?;
                let s = if k == "break" {
                    StmtKind::Break
                } else {
                    StmtKind::Continue
                };
                Ok((s, None))
            }
            Tok::Kw("else") => Err(Diag::at(
                span,
                "parse.unexpected",
                "this `else` has no `if` before it",
            )),
            Tok::Kw(k) if unsupported_stmt_kw(k).is_some() => {
                Err(self.unsupported(span, unsupported_stmt_kw(k).unwrap_or("that")))
            }
            _ => {
                if self.at_decl() {
                    let kind = self.parse_decl_kind()?;
                    self.expect_punct(";")?;
                    return Ok((kind, None));
                }
                let e = self.parse_expr()?;
                self.expect_punct(";")?;
                Ok((StmtKind::Expr(e), None))
            }
        }
    }

    /// `case` and `default` only make sense directly inside a `switch`.
    fn case_check(&self, span: Span) -> PResult<()> {
        if self.switch_depth == 0 {
            return Err(Diag::at(
                span,
                "parse.unexpected",
                "a `case` or `default` label only makes sense inside a `switch`",
            ));
        }
        Ok(())
    }

    fn parse_case(&mut self) -> PResult<(StmtKind, Option<String>)> {
        let span = self.cur_span();
        self.bump();
        self.case_check(span)?;
        let e = self.parse_assign()?;
        self.expect_punct(":")?;
        Ok((StmtKind::Case(Some(e)), None))
    }

    // ---- control flow -------------------------------------------------------------------------

    /// The `(init; cond)` of `if` and `switch`: an optional init statement and the condition.
    fn parse_cond_head(&mut self) -> PResult<(Option<Box<Stmt>>, Expr)> {
        self.expect_punct("(")?;
        let start = self.cur_start();
        let mut init = None;
        let cond;
        if self.at_decl() {
            let kind = self.parse_decl_kind()?;
            if self.at_punct(")") {
                return Err(self.unsupported(
                    self.span_from(start),
                    "a declaration as the condition (write `if (init; cond)` instead)",
                ));
            }
            self.expect_punct(";")?;
            init = Some(Box::new(self.mk_stmt(start, kind)));
            cond = self.parse_expr()?;
        } else {
            let e = self.parse_expr()?;
            if self.eat_punct(";") {
                init = Some(Box::new(self.mk_stmt(start, StmtKind::Expr(e))));
                cond = self.parse_expr()?;
            } else {
                cond = e;
            }
        }
        self.expect_punct(")")?;
        Ok((init, cond))
    }

    fn parse_if(&mut self) -> PResult<(StmtKind, Option<String>)> {
        self.bump();
        let (init, cond) = self.parse_cond_head()?;
        let (then, open) = self.parse_body()?;
        let mut els = None;
        if self.eat_kw("else") {
            if self.at_kw("if") {
                let start = self.cur_start();
                self.enter()?;
                let r = self.parse_if();
                self.leave();
                let (kind, _) = r?;
                els = Some(Box::new(self.mk_stmt(start, kind)));
            } else {
                els = Some(Box::new(self.parse_body()?.0));
            }
        }
        Ok((
            StmtKind::If {
                init,
                cond,
                then: Box::new(then),
                els,
            },
            open,
        ))
    }

    fn parse_switch(&mut self) -> PResult<(StmtKind, Option<String>)> {
        let start = self.cur_start();
        self.bump();
        let (init, cond) = self.parse_cond_head()?;
        if !self.at_punct("{") {
            return Err(self.expected("the '{' of the switch body"));
        }
        self.switch_depth += 1;
        let body = self.parse_block();
        self.switch_depth = self.switch_depth.saturating_sub(1);
        let body = body?;
        let open = self.block_open.take();
        let switch = StmtKind::Switch { cond, body };
        let Some(init) = init else {
            return Ok((switch, open));
        };
        // `switch (init; cond)` becomes `{ init; switch (cond) ... }`
        let mut inner = self.mk_stmt(start, switch);
        inner.trailing = open;
        Ok((
            StmtKind::Block(Block {
                stmts: vec![*init, inner],
                tail_comments: Vec::new(),
            }),
            None,
        ))
    }

    fn parse_while(&mut self) -> PResult<(StmtKind, Option<String>)> {
        self.bump();
        self.expect_punct("(")?;
        if self.at_decl() {
            return Err(self.unsupported(
                self.cur_span(),
                "a declaration as the condition of a `while`",
            ));
        }
        let cond = self.parse_expr()?;
        self.expect_punct(")")?;
        let (body, open) = self.parse_body()?;
        Ok((
            StmtKind::While {
                cond,
                body: Box::new(body),
            },
            open,
        ))
    }

    fn parse_do(&mut self) -> PResult<(StmtKind, Option<String>)> {
        self.bump();
        let (body, open) = self.parse_body()?;
        self.take_comments_before(self.cur_start());
        if !self.eat_kw("while") {
            return Err(self.expected("the `while` of this `do`"));
        }
        self.expect_punct("(")?;
        let cond = self.parse_expr()?;
        self.expect_punct(")")?;
        self.expect_punct(";")?;
        Ok((
            StmtKind::DoWhile {
                body: Box::new(body),
                cond,
            },
            open,
        ))
    }

    fn parse_for(&mut self) -> PResult<(StmtKind, Option<String>)> {
        self.bump();
        self.expect_punct("(")?;
        let start = self.cur_start();
        let mut init = None;
        if self.eat_punct(";") {
            // no init statement
        } else if self.at_decl() {
            let spec = self.parse_local_spec()?;
            if let Some(names) = self.range_for_head(&spec)? {
                let range = self.parse_expr()?;
                self.expect_punct(")")?;
                let (body, open) = self.parse_body()?;
                return Ok((
                    StmtKind::RangeFor {
                        spec,
                        structured: names.1,
                        names: names.0,
                        range,
                        body: Box::new(body),
                    },
                    open,
                ));
            }
            let kind = self.parse_decl_rest(spec)?;
            self.expect_punct(";")?;
            init = Some(Box::new(self.mk_stmt(start, kind)));
        } else {
            let e = self.parse_expr()?;
            self.expect_punct(";")?;
            init = Some(Box::new(self.mk_stmt(start, StmtKind::Expr(e))));
        }
        let cond = if self.at_punct(";") {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect_punct(";")?;
        let incr = if self.at_punct(")") {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect_punct(")")?;
        let (body, open) = self.parse_body()?;
        Ok((
            StmtKind::For {
                init,
                cond,
                incr,
                body: Box::new(body),
            },
            open,
        ))
    }

    /// After the type of a `for (` header: `name :` or `[a, b] :` makes it a range-for. Consumes the
    /// names and the colon on success; leaves the cursor alone otherwise.
    fn range_for_head(&mut self, spec: &TypeSpec) -> PResult<Option<(Vec<String>, bool)>> {
        if self.ident().is_some() && self.at_punct_at(1, ":") {
            let name = self.expect_ident("a name")?;
            self.bump();
            return Ok(Some((vec![name], false)));
        }
        if spec.ty == Ty::Auto && self.at_punct("[") {
            let m = self.mark();
            if let Some(names) = self.try_binding_names()
                && self.at_punct(":")
            {
                self.bump();
                return Ok(Some((names, true)));
            }
            self.restore(m);
        }
        Ok(None)
    }

    /// `[a, b, c]` of a structured binding; `None` when it is not one.
    fn try_binding_names(&mut self) -> Option<Vec<String>> {
        self.bump();
        let mut names = Vec::new();
        loop {
            let n = self.ident()?.to_string();
            self.bump();
            names.push(n);
            if !self.eat_punct(",") {
                break;
            }
        }
        if self.eat_punct("]") { Some(names) } else { None }
    }

    // ---- declarations -------------------------------------------------------------------------

    /// Does a declaration start here? (`int(x);` and `unsigned{}` are expressions, not declarations.)
    pub(crate) fn at_decl(&mut self) -> bool {
        if self.at_builtin_type()
            && !self.at_kw("auto")
            && !self.at_kw("decltype")
            && (self.at_punct_at(1, "(") || self.at_punct_at(1, "{"))
        {
            return false;
        }
        self.looks_like_decl()
    }

    /// The type of a local declaration: `constexpr` is a `const`, `static` is not in the subset.
    pub(crate) fn parse_local_spec(&mut self) -> PResult<TypeSpec> {
        if self.at_kw("static") {
            return Err(self.unsupported(
                self.cur_span(),
                "`static` variables inside functions (use a parameter or a local)",
            ));
        }
        let constexpr = self.eat_kw("constexpr");
        let mut spec = self.parse_type_spec()?;
        if constexpr {
            spec.konst = true;
        }
        Ok(spec)
    }

    /// A declaration without its `;`: `int a = 1, b;`, `auto [k, v] = e`.
    pub(crate) fn parse_decl_kind(&mut self) -> PResult<StmtKind> {
        let spec = self.parse_local_spec()?;
        self.parse_decl_rest(spec)
    }

    /// The part of a declaration after its type.
    pub(crate) fn parse_decl_rest(&mut self, spec: TypeSpec) -> PResult<StmtKind> {
        if spec.ty == Ty::Auto && self.at_punct("[") {
            let Some(names) = self.try_binding_names() else {
                return Err(self.expected("the names of the structured binding, like `[a, b]`"));
            };
            if !self.at_punct("=") {
                return Err(self.expected("an '=' after the structured binding"));
            }
            self.bump();
            let init = self.parse_assign()?;
            return Ok(StmtKind::Structured { spec, names, init });
        }
        let vars = self.parse_declarators(&spec)?;
        self.register_consts(&spec, &vars);
        Ok(StmtKind::Decl { spec, vars })
    }

    /// `a = 1, b(2), c[3] = {}` after a type. After a pointer type every name repeats the stars:
    /// `ListNode *slow = head, *fast = head`.
    pub(crate) fn parse_declarators(&mut self, spec: &TypeSpec) -> PResult<Vec<Declarator>> {
        let mut vars = Vec::new();
        loop {
            vars.push(self.parse_declarator()?);
            if !self.at_punct(",") {
                break;
            }
            if spec.is_ref || (spec.konst && matches!(spec.ty, Ty::Ptr(_))) {
                return Err(self.unsupported(
                    self.cur_span(),
                    "several names after a reference or `const` pointer type (declare one per line)",
                ));
            }
            self.bump();
            self.eat_stars(&spec.ty)?;
        }
        Ok(vars)
    }

    /// The `*`s a further name needs after a pointer type (none for other types).
    pub(crate) fn eat_stars(&mut self, ty: &Ty) -> PResult<()> {
        let mut depth = 0;
        let mut t = ty;
        while let Ty::Ptr(inner) = t {
            depth += 1;
            t = inner;
        }
        for _ in 0..depth {
            if !self.eat_punct("*") {
                return Err(self.unsupported(
                    self.cur_span(),
                    "a name without `*` after a pointer type (declare one per line)",
                ));
            }
        }
        if depth == 0 && (self.at_punct("*") || self.at_punct("&")) {
            return Err(self.unsupported(
                self.cur_span(),
                "a pointer or reference name after a plain type (declare one per line)",
            ));
        }
        Ok(())
    }

    fn parse_declarator(&mut self) -> PResult<Declarator> {
        let start = self.cur_start();
        let name = self.expect_ident("a variable name")?;
        let mut array = None;
        let mut unsized_span = None;
        if self.at_punct("[") {
            let open = self.cur_span();
            self.bump();
            if self.at_punct("]") {
                unsized_span = Some(open);
            } else {
                array = Some(self.parse_expr()?);
            }
            self.expect_punct("]")?;
            if self.at_punct("[") {
                return Err(self.unsupported(
                    self.cur_span(),
                    "multi-dimensional C arrays (use `std::vector<std::vector<T>>`)",
                ));
            }
        }
        let init = if self.eat_punct("=") {
            if self.at_punct("{") {
                Init::Brace {
                    elems: self.parse_brace_elems()?,
                    with_eq: true,
                }
            } else {
                Init::Copy(self.parse_assign()?)
            }
        } else if self.at_punct("{") {
            Init::Brace {
                elems: self.parse_brace_elems()?,
                with_eq: false,
            }
        } else if self.at_punct("(") {
            let at = self.cur_span();
            let args = self.parse_call_args()?;
            if args.is_empty() {
                return Err(Diag::at(
                    at,
                    "parse.vexing",
                    format!("`... {name}();` declares a function in C++; write `... {name};` instead"),
                ));
            }
            Init::Direct(args)
        } else {
            Init::None
        };
        if let Some(open) = unsized_span {
            // `int a[] = {1, 2}`: the size is the number of elements
            let Init::Brace { elems, .. } = &init else {
                return Err(Diag::at(
                    open,
                    "parse.expected",
                    "I expected a size between the '[' and ']' of this array",
                ));
            };
            let n = elems.len() as u64;
            let e = Expr {
                id: self.new_id(open),
                kind: ExprKind::Int {
                    value: n,
                    suffix: IntSuffix::None,
                    base: IntBase::Dec,
                },
            };
            array = Some(e);
        }
        let id = self.new_id(self.span_from(start));
        Ok(Declarator {
            id,
            name,
            array,
            init,
        })
    }

    /// Remember `const int N = 5;` so that `std::array<int, N>` can be understood.
    pub(crate) fn register_consts(&mut self, spec: &TypeSpec, vars: &[Declarator]) {
        if !spec.konst || !spec.ty.is_integral() || spec.is_ref {
            return;
        }
        for v in vars {
            if v.array.is_none()
                && let Init::Copy(e) = &v.init
                && let Some(n) = self.const_eval(e, 0)
            {
                self.declare_const(&v.name, n);
            }
        }
    }

    /// Value of a small constant expression (integer literals, known constants, `+ - * / % <<`).
    pub(crate) fn const_eval(&self, e: &Expr, depth: u32) -> Option<i64> {
        if depth > 32 {
            return None;
        }
        match &e.kind {
            ExprKind::Int { value, .. } => i64::try_from(*value).ok(),
            ExprKind::Var(n) => self.consts.get(n).copied(),
            ExprKind::Unary(UnOp::Neg, a) => self.const_eval(a, depth + 1)?.checked_neg(),
            ExprKind::Unary(UnOp::Plus, a) => self.const_eval(a, depth + 1),
            ExprKind::Binary(op, a, b) => {
                let (a, b) = (
                    self.const_eval(a, depth + 1)?,
                    self.const_eval(b, depth + 1)?,
                );
                match op {
                    BinOp::Add => a.checked_add(b),
                    BinOp::Sub => a.checked_sub(b),
                    BinOp::Mul => a.checked_mul(b),
                    BinOp::Div => a.checked_div(b),
                    BinOp::Rem => a.checked_rem(b),
                    BinOp::Shl => u32::try_from(b).ok().and_then(|s| a.checked_shl(s)),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// The diagnostic for a preprocessor line where a statement should be.
pub(crate) fn preproc_diag(text: &str, span: Span) -> Diag {
    let word: String = text
        .trim_start_matches('#')
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    match word.as_str() {
        "define" => Diag::at(
            span,
            "parse.unsupported",
            "this office does not stock `#define` (macros) yet; use `const` or `constexpr` instead",
        ),
        "include" => Diag::at(
            span,
            "parse.unexpected",
            "an `#include` belongs at the top of the file, outside the functions",
        ),
        _ => Diag::at(
            span,
            "parse.unsupported",
            format!("this office does not stock the preprocessor line `#{word}` yet"),
        ),
    }
}

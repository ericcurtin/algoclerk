// SPDX-License-Identifier: Apache-2.0
//! Parser: a hand-written recursive-descent parser with precedence climbing for expressions.
//!
//! This file holds the parser state ([`Parser`]: token cursor, node ids, spans, depth limit, comment
//! side table, known type names) and the public entry points. The grammar lives in submodules:
//!
//! * `types`: type specifiers (builtin types, `std::` templates with `>>` splitting, aliases, structs,
//!   `const`, `&`, `*`) and parameters;
//! * `expr`: the complete expression grammar (precedence climbing, assignment, ternary, casts, postfix
//!   operators, `new` / `delete`, lambdas, brace lists, constructor calls and template-id calls);
//! * `stmt`: declarations, structured bindings, control flow, blocks, comment attachment;
//! * `item`: includes, `using`, `typedef`, free functions, structs, global constants.
//!
//! Errors are [`Diag`]s worded as a clerk would say them; the parser stops at the first one.

use super::ast::*;
use super::diag::Diag;
use super::lexer::{Tok, Token, lex, split_comments};
use std::collections::{HashMap, HashSet};

mod expr;
mod item;
mod stmt;
#[cfg(test)]
mod stmt_tests;
#[cfg(test)]
mod tests;
mod types;

/// Result of every parsing function: the first problem found stops the parse.
pub(crate) type PResult<T> = Result<T, Diag>;

/// Nesting limit for expressions and statements (a friendly `parse.depth` error beyond it).
pub(crate) const MAX_DEPTH: u32 = 200;

/// Options for [`parse`].
#[derive(Clone, Debug, Default)]
pub struct ParseOpts {
    /// First node id to hand out (so prelude and player ids do not collide).
    pub first_id: NodeId,
    /// Struct and alias names defined by the prelude.
    pub known_types: Vec<String>,
}

/// A saved cursor for backtracking ([`Parser::mark`] / [`Parser::restore`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Mark {
    pos: usize,
    splits: usize,
    depth: u32,
    cidx: usize,
    switch_depth: u32,
}

/// Parser state.
#[derive(Debug)]
pub(crate) struct Parser<'a> {
    /// The source text (spans index into it).
    pub(crate) src: &'a str,
    /// Tokens without comments (preprocessor lines are tokens).
    pub(crate) toks: Vec<Token>,
    /// Index of the current token.
    pub(crate) pos: usize,
    /// End offset of the last consumed token.
    pub(crate) last_end: u32,
    /// Next node id to hand out.
    pub(crate) next_id: NodeId,
    /// Spans by node id (grown by [`Parser::new_id`]).
    pub(crate) spans: Vec<Span>,
    /// Current nesting depth ([`Parser::enter`] / [`Parser::leave`]).
    pub(crate) depth: u32,
    /// Comment tokens in source order (the statement half attaches them to statements and items).
    pub(crate) comments: Vec<Token>,
    /// Struct and alias names usable as types.
    pub(crate) known: HashSet<String>,
    /// What each alias stands for (aliases known only by name, from the prelude, are absent).
    pub(crate) aliases: HashMap<String, Ty>,
    /// `const int N = 5;` globals usable as array sizes in `std::array<int, N>`.
    pub(crate) consts: HashMap<String, i64>,
    /// `using namespace std;` was seen: unqualified std names are types and functions.
    pub(crate) using_std: bool,
    /// Index of the first comment not yet attached to a statement or item.
    pub(crate) cidx: usize,
    /// How many `switch` bodies enclose the statement being parsed (`case` needs one).
    pub(crate) switch_depth: u32,
    /// The comment found right after the `{` of the block parsed last (it becomes the `trailing`
    /// comment of the statement that owns the block).
    pub(crate) block_open: Option<String>,
    /// Tokens `>>` that were cut in two (index, original) so that [`Parser::restore`] can undo it.
    splits: Vec<(usize, Token)>,
}

impl<'a> Parser<'a> {
    /// Tokenize `src` and set up the state.
    pub(crate) fn new(src: &'a str, opts: &ParseOpts) -> PResult<Parser<'a>> {
        let all = lex(src)?;
        let (toks, comments) = split_comments(all);
        Ok(Parser {
            src,
            toks,
            pos: 0,
            last_end: 0,
            next_id: opts.first_id,
            spans: Vec::new(),
            depth: 0,
            comments,
            known: opts.known_types.iter().cloned().collect(),
            aliases: HashMap::new(),
            consts: HashMap::new(),
            using_std: false,
            cidx: 0,
            switch_depth: 0,
            block_open: None,
            splits: Vec::new(),
        })
    }

    // ---- node ids and spans -------------------------------------------------------------------

    /// Hand out a node id and record its span.
    pub(crate) fn new_id(&mut self, span: Span) -> NodeId {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let i = id as usize;
        if self.spans.len() <= i {
            self.spans.resize(i + 1, Span::default());
        }
        self.spans[i] = span;
        id
    }

    /// The span from `start` to the end of the last consumed token.
    pub(crate) fn span_from(&self, start: u32) -> Span {
        Span {
            start,
            end: self.last_end.max(start),
        }
    }

    /// Span of a node already created.
    pub(crate) fn span_of(&self, id: NodeId) -> Span {
        self.spans.get(id as usize).copied().unwrap_or_default()
    }

    /// Build an expression node spanning `start` to the last consumed token.
    pub(crate) fn mk_expr(&mut self, start: u32, kind: ExprKind) -> Expr {
        let span = self.span_from(start);
        Expr {
            id: self.new_id(span),
            kind,
        }
    }

    /// Start offset of an expression.
    pub(crate) fn start_of(&self, e: &Expr) -> u32 {
        self.span_of(e.id).start
    }

    // ---- depth limit --------------------------------------------------------------------------

    /// Go one level deeper; fails with a friendly error beyond [`MAX_DEPTH`].
    pub(crate) fn enter(&mut self) -> PResult<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Diag::at(
                self.cur_span(),
                "parse.depth",
                "this is nested too deeply for me to follow; could you split it up?",
            ));
        }
        Ok(())
    }

    /// Undo one [`Parser::enter`].
    pub(crate) fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    // ---- token cursor -------------------------------------------------------------------------

    /// The current token.
    pub(crate) fn tok(&self) -> Option<&Token> {
        self.toks.get(self.pos)
    }

    /// The token `n` places after the current one.
    pub(crate) fn tok_at(&self, n: usize) -> Option<&Token> {
        self.toks.get(self.pos.saturating_add(n))
    }

    /// Is the current token the punctuation `p`?
    pub(crate) fn at_punct(&self, p: &str) -> bool {
        self.tok().is_some_and(|t| t.is_punct(p))
    }

    /// Is the token `n` places ahead the punctuation `p`?
    pub(crate) fn at_punct_at(&self, n: usize, p: &str) -> bool {
        self.tok_at(n).is_some_and(|t| t.is_punct(p))
    }

    /// Is the current token the keyword `k`?
    pub(crate) fn at_kw(&self, k: &str) -> bool {
        self.tok().is_some_and(|t| t.is_kw(k))
    }

    /// The identifier at the current token.
    pub(crate) fn ident(&self) -> Option<&str> {
        self.ident_at(0)
    }

    /// The identifier `n` places ahead.
    pub(crate) fn ident_at(&self, n: usize) -> Option<&str> {
        match self.tok_at(n) {
            Some(Token {
                kind: Tok::Ident(s),
                ..
            }) => Some(s),
            _ => None,
        }
    }

    /// All tokens consumed.
    pub(crate) fn at_eof(&self) -> bool {
        self.pos >= self.toks.len()
    }

    /// Start offset of the current token (the end of the text at the end).
    pub(crate) fn cur_start(&self) -> u32 {
        self.cur_span().start
    }

    /// Span of the current token; at the end of the text, a one byte span on the last byte.
    pub(crate) fn cur_span(&self) -> Span {
        match self.tok() {
            Some(t) => t.span,
            None => {
                let n = self.src.len() as u32;
                Span {
                    start: n.saturating_sub(1),
                    end: n,
                }
            }
        }
    }

    /// Consume the current token.
    pub(crate) fn bump(&mut self) {
        if let Some(t) = self.toks.get(self.pos) {
            self.last_end = t.span.end;
            self.pos += 1;
        }
    }

    /// Consume the punctuation `p` if it is next.
    pub(crate) fn eat_punct(&mut self, p: &str) -> bool {
        let yes = self.at_punct(p);
        if yes {
            self.bump();
        }
        yes
    }

    /// Consume the keyword `k` if it is next.
    pub(crate) fn eat_kw(&mut self, k: &str) -> bool {
        let yes = self.at_kw(k);
        if yes {
            self.bump();
        }
        yes
    }

    /// Consume the punctuation `p` or say that it is missing.
    pub(crate) fn expect_punct(&mut self, p: &str) -> PResult<()> {
        if self.eat_punct(p) {
            Ok(())
        } else {
            Err(self.expected(&format!("a '{p}'")))
        }
    }

    /// Consume an identifier or say that `what` is missing.
    pub(crate) fn expect_ident(&mut self, what: &str) -> PResult<String> {
        match self.ident().map(str::to_string) {
            Some(s) => {
                self.bump();
                Ok(s)
            }
            None => Err(self.expected(what)),
        }
    }

    // ---- diagnostics --------------------------------------------------------------------------

    /// "I expected `what` here" at the current token.
    pub(crate) fn expected(&self, what: &str) -> Diag {
        Diag::at(
            self.cur_span(),
            "parse.expected",
            format!("I expected {what} here"),
        )
    }

    /// A construct outside the subset, in the clerk's words.
    pub(crate) fn unsupported(&self, span: Span, what: &str) -> Diag {
        Diag::at(
            span,
            "parse.unsupported",
            format!("this office does not stock {what} yet"),
        )
    }

    // ---- backtracking -------------------------------------------------------------------------

    /// Remember the cursor.
    pub(crate) fn mark(&self) -> Mark {
        Mark {
            pos: self.pos,
            splits: self.splits.len(),
            depth: self.depth,
            cidx: self.cidx,
            switch_depth: self.switch_depth,
        }
    }

    /// Go back to a remembered cursor (undoing `>>` splits made since).
    pub(crate) fn restore(&mut self, m: Mark) {
        while self.splits.len() > m.splits {
            if let Some((i, t)) = self.splits.pop() {
                self.toks[i] = t;
            }
        }
        self.pos = m.pos;
        self.depth = m.depth;
        self.cidx = m.cidx;
        self.switch_depth = m.switch_depth;
        self.last_end = if m.pos > 0 {
            self.toks[m.pos - 1].span.end
        } else {
            0
        };
    }

    /// Consume one `>` closing a template argument list; a `>>` token is cut in two so that
    /// `vector<vector<int>>` works.
    pub(crate) fn close_angle(&mut self) -> PResult<()> {
        if self.at_punct(">") {
            self.bump();
            return Ok(());
        }
        if self.at_punct(">>") {
            let orig = self.toks[self.pos].clone();
            let start = orig.span.start + 1;
            self.last_end = start;
            self.toks[self.pos] = Token {
                kind: Tok::Punct(">"),
                span: Span {
                    start,
                    end: orig.span.end,
                },
                nl_before: false,
            };
            self.splits.push((self.pos, orig));
            return Ok(());
        }
        Err(self.expected("a '>'"))
    }
}

#[cfg(test)]
/// Parse a single expression (the whole text) with no `using namespace std` and no known types.
pub(crate) fn parse_expr_str(src: &str) -> Result<Expr, Vec<Diag>> {
    parse_expr_str_with(src, &ParseOpts::default(), false)
}

#[cfg(test)]
/// Parse a single expression with the given known types and `using namespace std` state.
pub(crate) fn parse_expr_str_with(
    src: &str,
    opts: &ParseOpts,
    using_std: bool,
) -> Result<Expr, Vec<Diag>> {
    let run = || -> PResult<Expr> {
        let mut p = Parser::new(src, opts)?;
        p.using_std = using_std;
        let e = p.parse_expr()?;
        if !p.at_eof() {
            return Err(Diag::at(
                p.cur_span(),
                "parse.unexpected",
                "I did not expect this here; the expression looked finished",
            ));
        }
        Ok(e)
    };
    run().map_err(|d| vec![d])
}

/// Parse a source text: includes, `using`, aliases, structs, global constants and functions. The
/// result has `prelude_items = 0` and a span for every node. The first problem found is returned as a
/// single friendly diagnostic.
pub fn parse(src: &str, opts: &ParseOpts) -> Result<Program, Vec<Diag>> {
    let run = || -> PResult<Program> {
        let mut p = Parser::new(src, opts)?;
        let items = p.parse_items()?;
        Ok(Program {
            items,
            prelude_items: 0,
            next_id: p.next_id,
            spans: p.spans,
        })
    };
    run().map_err(|d| vec![d])
}

/// Names of the structs and aliases a program defines.
pub fn known_types(p: &Program) -> Vec<String> {
    p.items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Struct(s) => Some(s.name.clone()),
            ItemKind::Alias { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

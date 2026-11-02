// SPDX-License-Identifier: Apache-2.0
//! Convenience entry points: compile prelude + player text, run an entry function on literal arguments,
//! render the result.

use super::ast::{ExprKind, ItemKind, Program, Ty, walk_expr, walk_stmts};
use super::check::{CheckOpts, TypeInfo, check};
use super::diag::Diag;
use super::interp::{Limits, RunReport, run};
use super::parse::{ParseOpts, known_types, parse};
use super::value::{Heap, Plain, parse_value, render};

/// A parsed and checked program (level prelude followed by the player's code).
#[derive(Clone, Debug)]
pub struct Compiled {
    /// Syntax tree; `prelude_items` marks the read-only part.
    pub program: Program,
    /// Type information.
    pub info: TypeInfo,
}

/// Parse the prelude, parse the player's text (knowing the prelude's structs and aliases), merge both into
/// one [`Program`] and check it.
///
/// Diagnostics of the prelude (a level bug) carry no span; those of the player's text carry spans into
/// `player`.
pub fn compile(prelude: &str, player: &str, opts: &CheckOpts) -> Result<Compiled, Vec<Diag>> {
    let mut pre = parse(prelude, &ParseOpts::default()).map_err(|ds| {
        ds.into_iter()
            .map(|mut d| {
                d.span = None;
                d.msg = format!("level prelude: {}", d.msg);
                d
            })
            .collect::<Vec<_>>()
    })?;
    let mut pl = parse(
        player,
        &ParseOpts {
            first_id: pre.next_id,
            known_types: known_types(&pre),
        },
    )?;
    let prelude_items = pre.items.len();
    let mut spans = std::mem::take(&mut pl.spans);
    let blank = pre.next_id as usize;
    if spans.len() < blank {
        spans.resize(blank, Default::default());
    }
    for s in spans.iter_mut().take(blank) {
        *s = Default::default();
    }
    pre.items.append(&mut pl.items);
    let program = Program {
        items: pre.items,
        prelude_items,
        next_id: pl.next_id.max(pre.next_id),
        spans,
    };
    let info = check(&program, opts)?;
    Ok(Compiled { program, info })
}

/// The result of running a compiled function on literal text arguments.
#[derive(Clone, Debug)]
pub struct Called {
    /// The raw report.
    pub report: RunReport,
    /// Rendered return value (when the run succeeded), in the brace notation of [`super::value`].
    pub output: Option<String>,
    /// Return type.
    pub ret: Ty,
}

impl Compiled {
    /// Parse the argument literals for `entry` using its declared parameter types.
    pub fn parse_args(&self, entry: &str, args: &[String]) -> Result<(Vec<Plain>, Heap), String> {
        let f = self
            .program
            .find(entry)
            .ok_or_else(|| format!("no function named {entry}"))?;
        if f.params.len() != args.len() {
            return Err(format!(
                "{entry} takes {} arguments, got {}",
                f.params.len(),
                args.len()
            ));
        }
        let mut heap = Heap::default();
        let mut vals = Vec::new();
        for (p, a) in f.params.iter().zip(args) {
            vals.push(
                parse_value(a, &p.spec.ty, &mut heap)
                    .map_err(|e| format!("argument {}: {e}", p.name))?,
            );
        }
        Ok((vals, heap))
    }

    /// Run `entry` on literal text arguments.
    pub fn call(&self, entry: &str, args: &[String], limits: &Limits) -> Result<Called, String> {
        let (vals, heap) = self.parse_args(entry, args)?;
        let ret = self
            .program
            .find(entry)
            .map_or(Ty::Void, |f| f.ret.ty.clone());
        let report = run(&self.program, entry, vals, heap, limits);
        let output = report
            .result
            .as_ref()
            .ok()
            .map(|v| render(v, &ret, &report.heap));
        Ok(Called {
            report,
            output,
            ret,
        })
    }

    /// The program size used by the size medal: statements plus expression nodes of the player's own
    /// functions and globals (the prelude and signatures do not count).
    pub fn size(&self) -> u32 {
        program_size(&self.program)
    }
}

/// Number of statement and expression nodes in the player's part of a program.
pub fn program_size(p: &Program) -> u32 {
    let mut n = 0u32;
    fn count_block(b: &super::ast::Block, n: &mut u32) {
        walk_stmts(b, &mut |s| {
            *n += 1;
            super::ast::stmt_exprs(s, &mut |e| count_expr(e, n));
        });
    }
    fn count_expr(e: &super::ast::Expr, n: &mut u32) {
        let mut lambdas = Vec::new();
        walk_expr(e, &mut |x| {
            *n += 1;
            if let ExprKind::Lambda(l) = &x.kind {
                lambdas.push(&l.body);
            }
        });
        // `walk_expr` already visited the expressions inside lambda bodies; add their statements.
        for b in lambdas {
            walk_stmts(b, &mut |_| *n += 1);
        }
    }
    for item in p.player_items() {
        match &item.kind {
            ItemKind::Func(f) => {
                if let Some(b) = &f.body {
                    count_block(b, &mut n);
                }
            }
            ItemKind::Global { vars, .. } => {
                for v in vars {
                    n += 1;
                    if let super::ast::Init::Copy(e) = &v.init {
                        count_expr(e, &mut n);
                    }
                }
            }
            ItemKind::Struct(_) => n += 1,
            _ => {}
        }
    }
    n
}

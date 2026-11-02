// SPDX-License-Identifier: Apache-2.0
//! Canonical text form of a program (the inverse of the parser; comments are not kept).

use super::ast::*;

const P_NOT: u8 = 3;
const P_UNARY: u8 = 11;

fn esc(c: u8, quote: u8) -> String {
    match c {
        b'\n' => "\\n".into(),
        b'\t' => "\\t".into(),
        b'\r' => "\\r".into(),
        0 => "\\0".into(),
        b'\\' => "\\\\".into(),
        c if c == quote => format!("\\{}", c as char),
        c => (c as char).to_string(),
    }
}

/// Text of a pattern.
pub fn pat_text(p: &Pat) -> String {
    match p {
        Pat::Var(v) => v.clone(),
        Pat::Wild => "_".into(),
        Pat::Tuple(ps) => format!("({})", ps.iter().map(pat_text).collect::<Vec<_>>().join(", ")),
    }
}

fn prec_of(e: &Expr) -> u8 {
    match &e.kind {
        ExprKind::Binary(op, ..) => op.prec(),
        ExprKind::Unary(UnOp::Not, _) => P_NOT,
        ExprKind::Unary(..) => P_UNARY,
        ExprKind::Int(n) if *n < 0 => P_UNARY,
        _ => 20,
    }
}

/// Text of an expression.
pub fn expr_text(e: &Expr) -> String {
    let mut s = String::new();
    expr(e, &mut s);
    s
}

fn wrap(e: &Expr, min: u8, out: &mut String) {
    if prec_of(e) < min {
        out.push('(');
        expr(e, out);
        out.push(')');
    } else {
        expr(e, out);
    }
}

fn list(es: &[Expr], out: &mut String) {
    for (i, a) in es.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        expr(a, out);
    }
}

fn expr(e: &Expr, out: &mut String) {
    match &e.kind {
        ExprKind::Int(n) => out.push_str(&n.to_string()),
        ExprKind::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        ExprKind::Char(c) => {
            out.push('\'');
            out.push_str(&esc(*c, b'\''));
            out.push('\'');
        }
        ExprKind::Str(s) => {
            out.push('"');
            for b in s.bytes() {
                out.push_str(&esc(b, b'"'));
            }
            out.push('"');
        }
        ExprKind::Null => out.push_str("null"),
        ExprKind::NoneLit => out.push_str("none"),
        ExprKind::Var(v) => out.push_str(v),
        ExprKind::Unary(op, a) => match op {
            UnOp::Neg => {
                out.push('-');
                if matches!(a.kind, ExprKind::Int(_)) {
                    // `-(5)`: a bare `-5` would re-parse as a literal
                    out.push('(');
                    expr(a, out);
                    out.push(')');
                } else {
                    wrap(a, P_UNARY + 1, out);
                }
            }
            UnOp::BitNot => {
                out.push('~');
                wrap(a, P_UNARY + 1, out);
            }
            UnOp::Not => {
                out.push_str("not ");
                wrap(a, P_NOT, out);
            }
        },
        ExprKind::Binary(op, a, b) => {
            wrap(a, op.prec(), out);
            out.push(' ');
            out.push_str(op.text());
            out.push(' ');
            wrap(b, op.prec() + 1, out);
        }
        ExprKind::Index(a, i) => {
            wrap(a, 20, out);
            out.push('[');
            expr(i, out);
            out.push(']');
        }
        ExprKind::Slice(a, lo, hi) => {
            wrap(a, 20, out);
            out.push('[');
            if let Some(l) = lo {
                expr(l, out);
            }
            out.push_str("..");
            if let Some(h) = hi {
                expr(h, out);
            }
            out.push(']');
        }
        ExprKind::Field(a, f) => {
            wrap(a, 20, out);
            out.push('.');
            out.push_str(f);
        }
        ExprKind::TupleIdx(a, n) => {
            wrap(a, 20, out);
            out.push('.');
            out.push_str(&n.to_string());
        }
        ExprKind::Call(n, args) => {
            out.push_str(n);
            out.push('(');
            list(args, out);
            out.push(')');
        }
        ExprKind::Method(r, n, args) => {
            wrap(r, 20, out);
            out.push('.');
            out.push_str(n);
            out.push('(');
            list(args, out);
            out.push(')');
        }
        ExprKind::VecLit(items) => {
            out.push('[');
            list(items, out);
            out.push(']');
        }
        ExprKind::VecRepeat(x, n) => {
            out.push('[');
            expr(x, out);
            out.push_str("; ");
            expr(n, out);
            out.push(']');
        }
        ExprKind::TupleLit(items) => {
            out.push('(');
            list(items, out);
            out.push(')');
        }
        ExprKind::Ctor(t) => {
            out.push_str(&t.to_string());
            out.push_str("()");
        }
    }
}

fn indent(n: usize, out: &mut String) {
    for _ in 0..n {
        out.push_str("    ");
    }
}

fn block(b: &Block, depth: usize, out: &mut String) {
    for s in &b.stmts {
        stmt(s, depth, out);
    }
}

fn stmt(s: &Stmt, d: usize, out: &mut String) {
    indent(d, out);
    match &s.kind {
        StmtKind::Let { pat, ty, init } => {
            out.push_str("let ");
            out.push_str(&pat_text(pat));
            if let Some(t) = ty {
                out.push_str(": ");
                out.push_str(&t.to_string());
            }
            out.push_str(" = ");
            expr(init, out);
            out.push_str(";\n");
        }
        StmtKind::Assign { target, op, value } => {
            expr(target, out);
            match op {
                Some(o) => {
                    out.push(' ');
                    out.push_str(o.text());
                    out.push_str("= ");
                }
                None => out.push_str(" = "),
            }
            expr(value, out);
            out.push_str(";\n");
        }
        StmtKind::If { .. } => {
            if_chain(s, d, out);
        }
        StmtKind::While { cond, body } => {
            out.push_str("while ");
            expr(cond, out);
            out.push_str(" {\n");
            block(body, d + 1, out);
            indent(d, out);
            out.push_str("}\n");
        }
        StmtKind::ForRange { var, start, end, step, body } => {
            out.push_str("for ");
            out.push_str(var);
            out.push_str(" in range(");
            match start {
                Some(a) => {
                    expr(a, out);
                    out.push_str(", ");
                }
                // range(n, step) would be ambiguous; spell out the start when there is a step.
                None if step.is_some() => out.push_str("0, "),
                None => {}
            }
            expr(end, out);
            if let Some(st) = step {
                out.push_str(", ");
                expr(st, out);
            }
            out.push_str(") {\n");
            block(body, d + 1, out);
            indent(d, out);
            out.push_str("}\n");
        }
        StmtKind::ForEach { pat, iter, body } => {
            out.push_str("for ");
            out.push_str(&pat_text(pat));
            out.push_str(" in ");
            expr(iter, out);
            out.push_str(" {\n");
            block(body, d + 1, out);
            indent(d, out);
            out.push_str("}\n");
        }
        StmtKind::Break => out.push_str("break;\n"),
        StmtKind::Continue => out.push_str("continue;\n"),
        StmtKind::Return(e) => {
            out.push_str("return");
            if let Some(e) = e {
                out.push(' ');
                expr(e, out);
            }
            out.push_str(";\n");
        }
        StmtKind::Expr(e) => {
            expr(e, out);
            out.push_str(";\n");
        }
    }
}

fn if_chain(s: &Stmt, d: usize, out: &mut String) {
    let StmtKind::If { cond, then, els } = &s.kind else { return };
    out.push_str("if ");
    expr(cond, out);
    out.push_str(" {\n");
    block(then, d + 1, out);
    indent(d, out);
    out.push('}');
    match els {
        None => out.push('\n'),
        Some(b) => {
            if let [only] = b.stmts.as_slice()
                && matches!(only.kind, StmtKind::If { .. })
            {
                out.push_str(" else ");
                if_chain(only, d, out);
            } else {
                out.push_str(" else {\n");
                block(b, d + 1, out);
                indent(d, out);
                out.push_str("}\n");
            }
        }
    }
}

/// Signature line text such as `fn f(a: vec<int>) -> int`.
pub fn signature_text(f: &FnDecl) -> String {
    let ps: Vec<String> = f
        .params
        .iter()
        .map(|p| format!("{}: {}{}", p.name, if p.by_ref { "&mut " } else { "" }, p.ty))
        .collect();
    let mut s = format!("fn {}({})", f.name, ps.join(", "));
    if f.ret != Ty::Unit {
        s.push_str(&format!(" -> {}", f.ret));
    }
    s
}

/// Print one function.
pub fn fn_text(f: &FnDecl) -> String {
    let mut out = signature_text(f);
    out.push_str(" {\n");
    block(&f.body, 1, &mut out);
    out.push_str("}\n");
    out
}

/// Print a whole program (functions separated by a blank line).
pub fn print(p: &Program) -> String {
    p.fns.iter().map(fn_text).collect::<Vec<_>>().join("\n")
}

/// Print one statement (and its body) at the given indent.
pub fn stmt_text(s: &Stmt, depth: usize) -> String {
    let mut out = String::new();
    stmt(s, depth, &mut out);
    out
}

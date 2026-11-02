// SPDX-License-Identifier: Apache-2.0
//! Pretty printer: canonical 4-space K&R formatting with the minimal parentheses that keep the
//! meaning, `else if` chains, comments reprinted, and the exact type spelling of the player.
//!
//! Printing a parsed program and parsing the text again gives a structurally equal program (after
//! [`normalize`]). Known, deliberate normalisations: `/* */` comments become `//` lines, `inline`
//! becomes `static`, an unsized C array gets its counted size, `new T` becomes `new T()`, a
//! brace-initialised member initialiser becomes a parenthesised one, and `case` labels sit one level
//! inside their `switch`.

use super::ast::*;

const PAD: &str = "    ";

fn pad(n: usize) -> String {
    PAD.repeat(n)
}

// ---------------------------------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------------------------------

/// A declared type with `const` and `&`, spelled the way the player wrote it (`TypeSpec::spelled`).
pub fn print_type(t: &TypeSpec) -> String {
    let base = t.spelled.clone().unwrap_or_else(|| t.ty.to_string());
    let mut s = if !t.konst {
        base
    } else if matches!(t.ty, Ty::Ptr(_)) {
        format!("{base} const")
    } else {
        format!("const {base}")
    };
    if t.is_ref {
        s.push('&');
    }
    s
}

fn print_param(p: &Param) -> String {
    if p.name.is_empty() {
        print_type(&p.spec)
    } else {
        format!("{} {}", print_type(&p.spec), p.name)
    }
}

fn params_text(ps: &[Param]) -> String {
    ps.iter().map(print_param).collect::<Vec<_>>().join(", ")
}

// ---------------------------------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------------------------------

/// Binding strength of an expression's outermost operator: comma 1 ... multiplicative 13, assignment 2,
/// `?:` 3, prefix 16, postfix and primary 17 and 18.
fn prec_of(e: &Expr) -> u8 {
    match &e.kind {
        ExprKind::Binary(op, ..) => op.prec(),
        ExprKind::Assign { .. } => 2,
        ExprKind::Ternary(..) => 3,
        ExprKind::Unary(UnOp::PostInc | UnOp::PostDec, _) => 17,
        ExprKind::Unary(..)
        | ExprKind::Delete(_)
        | ExprKind::New { .. }
        | ExprKind::Cast {
            style: CastStyle::CStyle,
            ..
        } => 16,
        _ => 18,
    }
}

const POSTFIX: u8 = 17;
const UNARY: u8 = 16;
const ASSIGN: u8 = 2;

fn quote_char(c: u8, quote: u8) -> String {
    match c {
        b'\n' => "\\n".into(),
        b'\t' => "\\t".into(),
        b'\r' => "\\r".into(),
        b'\\' => "\\\\".into(),
        0 => "\\0".into(),
        c if c == quote => format!("\\{}", c as char),
        0x20..=0x7e => (c as char).to_string(),
        c => format!("\\{c:03o}"),
    }
}

fn int_text(value: u64, suffix: IntSuffix, base: IntBase) -> String {
    let digits = match base {
        IntBase::Dec => value.to_string(),
        IntBase::Hex => format!("0x{value:X}"),
        IntBase::Bin => format!("0b{value:b}"),
        IntBase::Oct if value == 0 => "00".into(),
        IntBase::Oct => format!("0{value:o}"),
    };
    let suf = match suffix {
        IntSuffix::None => "",
        IntSuffix::U => "u",
        IntSuffix::LL => "LL",
        IntSuffix::ULL => "ULL",
    };
    format!("{digits}{suf}")
}

fn un_text(op: UnOp) -> &'static str {
    match op {
        UnOp::Neg => "-",
        UnOp::Plus => "+",
        UnOp::Not => "!",
        UnOp::BitNot => "~",
        UnOp::Deref => "*",
        UnOp::AddrOf => "&",
        UnOp::PreInc | UnOp::PostInc => "++",
        UnOp::PreDec | UnOp::PostDec => "--",
    }
}

fn targs_text(a: &[TArg]) -> String {
    let parts: Vec<String> = a
        .iter()
        .map(|t| match t {
            TArg::Ty(t) => t.to_string(),
            TArg::Int(i) => i.to_string(),
        })
        .collect();
    format!("<{}>", parts.join(", "))
}

/// `e`, in parentheses when its operator binds looser than `min`.
fn ex(e: &Expr, min: u8, ind: usize) -> String {
    let s = expr_raw(e, ind);
    if prec_of(e) < min {
        format!("({s})")
    } else {
        s
    }
}

fn args_text(args: &[Expr], ind: usize) -> String {
    args.iter()
        .map(|a| ex(a, ASSIGN, ind))
        .collect::<Vec<_>>()
        .join(", ")
}

fn expr_raw(e: &Expr, ind: usize) -> String {
    match &e.kind {
        ExprKind::Int {
            value,
            suffix,
            base,
        } => int_text(*value, *suffix, *base),
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::Char(c) => format!("'{}'", quote_char(*c, b'\'')),
        ExprKind::Str(s) => {
            let b = s.as_bytes();
            let mut body = String::new();
            for (i, &c) in b.iter().enumerate() {
                // `\0` followed by a digit would read as a longer octal escape
                if c == 0 && b.get(i + 1).is_some_and(|d| (b'0'..=b'7').contains(d)) {
                    body.push_str("\\000");
                } else {
                    body.push_str(&quote_char(c, b'"'));
                }
            }
            format!("\"{body}\"")
        }
        ExprKind::NullPtr => "nullptr".into(),
        ExprKind::Var(n) => n.clone(),
        ExprKind::Path(segs) => segs
            .iter()
            .map(|s| match &s.targs {
                None => s.name.clone(),
                Some(a) => format!("{}{}", s.name, targs_text(a)),
            })
            .collect::<Vec<_>>()
            .join("::"),
        ExprKind::Unary(op @ (UnOp::PostInc | UnOp::PostDec), a) => {
            format!("{}{}", ex(a, POSTFIX, ind), un_text(*op))
        }
        ExprKind::Unary(op, a) => {
            let t = un_text(*op);
            let inner = ex(a, UNARY, ind);
            // `- -x` must not become `--x`, nor `& &x` become `&&x`
            let clash = matches!(
                (t.chars().last(), inner.chars().next()),
                (Some('-'), Some('-')) | (Some('+'), Some('+')) | (Some('&'), Some('&'))
            );
            if clash {
                format!("{t}({inner})")
            } else {
                format!("{t}{inner}")
            }
        }
        ExprKind::Binary(op, a, b) => format!(
            "{}{} {}",
            ex(a, op.prec(), ind),
            if *op == BinOp::Comma {
                ",".to_string()
            } else {
                format!(" {}", op.text())
            },
            ex(b, op.prec() + 1, ind)
        ),
        ExprKind::Assign { op, lhs, rhs } => format!(
            "{} {}= {}",
            ex(lhs, 4, ind),
            op.map_or("", |o| o.text()),
            ex(rhs, ASSIGN, ind)
        ),
        ExprKind::Ternary(c, a, b) => format!(
            "{} ? {} : {}",
            ex(c, 4, ind),
            ex(a, 1, ind),
            ex(b, ASSIGN, ind)
        ),
        ExprKind::Index(a, i) => format!("{}[{}]", ex(a, POSTFIX, ind), ex(i, 1, ind)),
        ExprKind::Call { callee, args } => {
            format!("{}({})", ex(callee, POSTFIX, ind), args_text(args, ind))
        }
        ExprKind::Member { obj, name, arrow } => {
            // `1.x` would be read as a decimal number
            let o = if matches!(obj.kind, ExprKind::Int { .. }) {
                format!("({})", expr_raw(obj, ind))
            } else {
                ex(obj, POSTFIX, ind)
            };
            format!("{o}{}{name}", if *arrow { "->" } else { "." })
        }
        ExprKind::Cast { style, ty, e } => match style {
            CastStyle::Static => {
                format!("static_cast<{}>({})", print_type(ty), ex(e, 1, ind))
            }
            CastStyle::CStyle => format!("({}){}", print_type(ty), ex(e, UNARY, ind)),
            CastStyle::Functional => format!("{}({})", print_type(ty), ex(e, ASSIGN, ind)),
        },
        ExprKind::Lambda(l) => lambda_text(l, ind),
        ExprKind::InitList(a) => format!("{{{}}}", args_text(a, ind)),
        ExprKind::Construct { ty, args, brace } => {
            let (o, c) = if *brace { ('{', '}') } else { ('(', ')') };
            format!("{}{o}{}{c}", print_type(ty), args_text(args, ind))
        }
        ExprKind::New { ty, args, brace } => {
            let (o, c) = if *brace { ('{', '}') } else { ('(', ')') };
            format!("new {}{o}{}{c}", print_type(ty), args_text(args, ind))
        }
        ExprKind::Delete(a) => format!("delete {}", ex(a, UNARY, ind)),
    }
}

fn lambda_text(l: &Lambda, ind: usize) -> String {
    let mut caps: Vec<String> = Vec::new();
    match l.captures.default {
        CaptureDefault::None => {}
        CaptureDefault::Ref => caps.push("&".into()),
        CaptureDefault::Copy => caps.push("=".into()),
    }
    for (n, r) in &l.captures.items {
        caps.push(format!("{}{n}", if *r { "&" } else { "" }));
    }
    let mut s = format!("[{}]({})", caps.join(", "), params_text(&l.params));
    if l.mutable {
        s.push_str(" mutable");
    }
    if let Some(r) = &l.ret {
        s.push_str(" -> ");
        s.push_str(&print_type(r));
    }
    s.push(' ');
    s.push_str(&inline_block(&l.body, ind));
    s
}

/// `{ return a < b; }` on one line when the block is one short simple statement, else a normal block.
/// Every statement is printed once, however deeply lambdas nest.
fn inline_block(b: &Block, ind: usize) -> String {
    if b.stmts.is_empty() && b.tail_comments.is_empty() {
        return "{}".into();
    }
    if let [s] = b.stmts.as_slice()
        && b.tail_comments.is_empty()
        && s.comments.is_empty()
        && s.trailing.is_none()
        && let Some(t) = simple_text(s, ind + 1)
    {
        if !t.contains('\n') && t.len() <= 60 {
            return format!("{{ {t} }}");
        }
        return format!("{{\n{}{t}\n{}}}", pad(ind + 1), pad(ind));
    }
    let mut out = String::from("{\n");
    write_block_inner(&mut out, b, ind + 1);
    out.push_str(&pad(ind));
    out.push('}');
    out
}

/// Print one expression.
pub fn print_expr(e: &Expr) -> String {
    expr_raw(e, 0)
}

// ---------------------------------------------------------------------------------------------------
// Statements
// ---------------------------------------------------------------------------------------------------

fn declarator_text(d: &Declarator, ind: usize) -> String {
    let mut s = d.name.clone();
    if let Some(a) = &d.array {
        s.push_str(&format!("[{}]", ex(a, 1, ind)));
    }
    match &d.init {
        Init::None => {}
        Init::Copy(e) => s.push_str(&format!(" = {}", ex(e, ASSIGN, ind))),
        Init::Direct(a) => s.push_str(&format!("({})", args_text(a, ind))),
        Init::Brace { elems, with_eq } => s.push_str(&format!(
            "{}{{{}}}",
            if *with_eq { " = " } else { "" },
            args_text(elems, ind)
        )),
    }
    s
}

fn decls_text(vars: &[Declarator], ind: usize) -> String {
    vars.iter()
        .map(|d| declarator_text(d, ind))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The one-line text (with its `;`) of a statement that has no body, without comments.
fn simple_text(s: &Stmt, ind: usize) -> Option<String> {
    Some(match &s.kind {
        StmtKind::Decl { spec, vars }
            if vars.len() > 1 && matches!(spec.ty, Ty::Ptr(_)) && !spec.konst && !spec.is_ref =>
        {
            // `ListNode *a = x, *b = y;`: every name carries the stars
            let full = print_type(spec);
            let base = full.trim_end_matches('*');
            let stars = "*".repeat(full.len() - base.len());
            let names: Vec<String> = vars
                .iter()
                .map(|d| format!("{stars}{}", declarator_text(d, ind)))
                .collect();
            format!("{base} {};", names.join(", "))
        }
        StmtKind::Decl { spec, vars } => {
            format!("{} {};", print_type(spec), decls_text(vars, ind))
        }
        StmtKind::Structured { spec, names, init } => format!(
            "{} [{}] = {};",
            print_type(spec),
            names.join(", "),
            ex(init, ASSIGN, ind)
        ),
        StmtKind::Expr(e) => {
            let t = ex(e, 1, ind);
            // a statement cannot start with `{`: that is a block
            if t.starts_with('{') {
                format!("({t});")
            } else {
                format!("{t};")
            }
        }
        StmtKind::Return(None) => "return;".into(),
        StmtKind::Return(Some(e)) => format!("return {};", ex(e, 1, ind)),
        StmtKind::Break => "break;".into(),
        StmtKind::Continue => "continue;".into(),
        StmtKind::Empty => ";".into(),
        _ => return None,
    })
}

/// The text of a statement in a `for` / `if` header (with its `;`).
fn header_stmt(s: &Stmt, ind: usize) -> String {
    simple_text(s, ind).unwrap_or_else(|| print_stmt(s, 0).trim().to_string())
}

fn trailing_text(s: &Stmt) -> String {
    s.trailing
        .as_ref()
        .map_or(String::new(), |t| format!(" //{t}"))
}

fn write_block_inner(out: &mut String, b: &Block, ind: usize) {
    for s in &b.stmts {
        write_stmt(out, s, ind);
    }
    for t in &b.tail_comments {
        out.push_str(&pad(ind));
        out.push_str("//");
        out.push_str(t);
        out.push('\n');
    }
}

fn is_empty_block(b: &Block) -> bool {
    b.stmts.is_empty() && b.tail_comments.is_empty()
}

/// Is the statement an `if` without `else` at its end (so that a following `else` would attach to it)?
fn ends_open_if(s: &Stmt) -> bool {
    match &s.kind {
        StmtKind::If { els: None, .. } => true,
        StmtKind::If { els: Some(e), .. } => ends_open_if(e),
        StmtKind::For { body, .. }
        | StmtKind::RangeFor { body, .. }
        | StmtKind::While { body, .. } => ends_open_if(body),
        _ => false,
    }
}

/// Write `header`, then the body: a block (`{` hugging the header, the closing `}` without a newline)
/// or a single statement on the next line. Returns whether the body was braced. `force` braces a single
/// statement (dangling `else` protection).
fn write_body(
    out: &mut String,
    header: &str,
    body: &Stmt,
    ind: usize,
    trailing: &str,
    force: bool,
) -> bool {
    out.push_str(header);
    match &body.kind {
        StmtKind::Block(b) => {
            if is_empty_block(b) && trailing.is_empty() {
                out.push_str(" {}");
            } else {
                out.push_str(" {");
                out.push_str(trailing);
                out.push('\n');
                write_block_inner(out, b, ind + 1);
                out.push_str(&pad(ind));
                out.push('}');
            }
            true
        }
        _ if force => {
            out.push_str(" {");
            out.push_str(trailing);
            out.push('\n');
            write_stmt(out, body, ind + 1);
            out.push_str(&pad(ind));
            out.push('}');
            true
        }
        _ => {
            out.push_str(trailing);
            out.push('\n');
            write_stmt(out, body, ind + 1);
            false
        }
    }
}

fn if_head(init: &Option<Box<Stmt>>, cond: &Expr, ind: usize) -> String {
    match init {
        Some(i) => format!("{} {}", header_stmt(i, ind), ex(cond, 1, ind)),
        None => ex(cond, 1, ind),
    }
}

fn write_if(out: &mut String, s: &Stmt, ind: usize) {
    let StmtKind::If {
        init,
        cond,
        then,
        els,
    } = &s.kind
    else {
        return;
    };
    let mut header = format!("{}if ({})", pad(ind), if_head(init, cond, ind));
    let (mut then, mut els) = (then, els);
    let mut trailing = trailing_text(s);
    loop {
        let force = els.is_some() && ends_open_if(then);
        let braced = write_body(out, &header, then, ind, &trailing, force);
        let Some(e) = els else {
            if braced {
                out.push('\n');
            }
            return;
        };
        if braced {
            out.push_str(" else");
        } else {
            out.push_str(&format!("{}else", pad(ind)));
        }
        match &e.kind {
            StmtKind::If {
                init,
                cond,
                then: t2,
                els: e2,
            } if e.comments.is_empty() && e.trailing.is_none() => {
                header = format!(" if ({})", if_head(init, cond, ind));
                then = t2;
                els = e2;
                trailing = String::new();
            }
            _ => {
                let braced = write_body(out, "", e, ind, "", false);
                if braced {
                    out.push('\n');
                }
                return;
            }
        }
    }
}

fn case_text(e: &Option<Expr>, ind: usize) -> String {
    match e {
        Some(e) => format!("case {}:", ex(e, ASSIGN, ind)),
        None => "default:".into(),
    }
}

fn write_switch_body(out: &mut String, b: &Block, ind: usize) {
    let mut in_case = false;
    let mut i = 0;
    while i < b.stmts.len() {
        let s = &b.stmts[i];
        i += 1;
        let StmtKind::Case(e) = &s.kind else {
            write_stmt(out, s, if in_case { ind + 1 } else { ind });
            continue;
        };
        in_case = true;
        for c in &s.comments {
            out.push_str(&format!("{}//{c}\n", pad(ind)));
        }
        let label = format!("{}{}", pad(ind), case_text(e, ind));
        // `case 1: {` hugs its block
        if let Some(Stmt {
            kind: StmtKind::Block(inner),
            comments,
            trailing: None,
            ..
        }) = b.stmts.get(i)
            && comments.is_empty()
            && s.trailing.is_none()
        {
            i += 1;
            if is_empty_block(inner) {
                out.push_str(&format!("{label} {{}}\n"));
            } else {
                out.push_str(&format!("{label} {{\n"));
                write_block_inner(out, inner, ind + 1);
                out.push_str(&format!("{}}}\n", pad(ind)));
            }
            continue;
        }
        out.push_str(&label);
        out.push_str(&trailing_text(s));
        out.push('\n');
    }
    for t in &b.tail_comments {
        out.push_str(&format!("{}//{t}\n", pad(ind + usize::from(in_case))));
    }
}

fn write_stmt(out: &mut String, s: &Stmt, ind: usize) {
    for c in &s.comments {
        out.push_str(&format!("{}//{c}\n", pad(ind)));
    }
    if let Some(t) = simple_text(s, ind) {
        out.push_str(&format!("{}{t}{}\n", pad(ind), trailing_text(s)));
        return;
    }
    match &s.kind {
        StmtKind::Case(e) => {
            out.push_str(&format!(
                "{}{}{}\n",
                pad(ind),
                case_text(e, ind),
                trailing_text(s)
            ));
        }
        StmtKind::Block(b) => {
            if is_empty_block(b) && s.trailing.is_none() {
                out.push_str(&format!("{}{{}}\n", pad(ind)));
            } else {
                out.push_str(&format!("{}{{{}\n", pad(ind), trailing_text(s)));
                write_block_inner(out, b, ind + 1);
                out.push_str(&format!("{}}}\n", pad(ind)));
            }
        }
        StmtKind::If { .. } => write_if(out, s, ind),
        StmtKind::Switch { cond, body } => {
            out.push_str(&format!(
                "{}switch ({}) {{{}\n",
                pad(ind),
                ex(cond, 1, ind),
                trailing_text(s)
            ));
            write_switch_body(out, body, ind + 1);
            out.push_str(&format!("{}}}\n", pad(ind)));
        }
        StmtKind::For {
            init,
            cond,
            incr,
            body,
        } => {
            let init_t = init
                .as_ref()
                .map_or(";".to_string(), |i| header_stmt(i, ind));
            let mut head = init_t;
            match (cond, incr) {
                (Some(c), _) => head.push_str(&format!(" {};", ex(c, 1, ind))),
                (None, Some(_)) => head.push_str(" ;"),
                (None, None) if init.is_some() => head.push_str(" ;"),
                (None, None) => head.push(';'),
            }
            if let Some(i) = incr {
                head.push_str(&format!(" {}", ex(i, 1, ind)));
            }
            let header = format!("{}for ({head})", pad(ind));
            write_body_nl(out, &header, body, ind, &trailing_text(s));
        }
        StmtKind::RangeFor {
            spec,
            names,
            structured,
            range,
            body,
        } => {
            let n = if *structured {
                format!("[{}]", names.join(", "))
            } else {
                names.join(", ")
            };
            let header = format!(
                "{}for ({} {n} : {})",
                pad(ind),
                print_type(spec),
                ex(range, 1, ind)
            );
            write_body_nl(out, &header, body, ind, &trailing_text(s));
        }
        StmtKind::While { cond, body } => {
            let header = format!("{}while ({})", pad(ind), ex(cond, 1, ind));
            write_body_nl(out, &header, body, ind, &trailing_text(s));
        }
        StmtKind::DoWhile { body, cond } => {
            let braced = write_body(
                out,
                &format!("{}do", pad(ind)),
                body,
                ind,
                &trailing_text(s),
                false,
            );
            let lead = if braced { " ".to_string() } else { pad(ind) };
            out.push_str(&format!("{lead}while ({});\n", ex(cond, 1, ind)));
        }
        // simple statements were written above
        _ => {}
    }
}

/// [`write_body`] followed by the newline a braced body still lacks.
fn write_body_nl(out: &mut String, header: &str, body: &Stmt, ind: usize, trailing: &str) {
    if write_body(out, header, body, ind, trailing, false) {
        out.push('\n');
    }
}

/// Print one statement (with its comments) at an indentation level of 4 spaces each. Every line,
/// including the last, ends with a newline.
pub fn print_stmt(s: &Stmt, indent: usize) -> String {
    let mut out = String::new();
    write_stmt(&mut out, s, indent);
    out
}

// ---------------------------------------------------------------------------------------------------
// Items and programs
// ---------------------------------------------------------------------------------------------------

fn write_comments(out: &mut String, comments: &[String], ind: usize) {
    for c in comments {
        out.push_str(&format!("{}//{c}\n", pad(ind)));
    }
}

/// `{ ... }` of a function or constructor body, followed by a newline.
fn write_fn_body(out: &mut String, b: &Block, ind: usize) {
    if is_empty_block(b) {
        out.push_str(" {}\n");
    } else {
        out.push_str(" {\n");
        write_block_inner(out, b, ind + 1);
        out.push_str(&format!("{}}}\n", pad(ind)));
    }
}

fn write_struct(out: &mut String, s: &StructDecl) {
    out.push_str(&format!("struct {} {{\n", s.name));
    for f in &s.fields {
        let mut line = format!("{}{} {}", PAD, print_type(&f.spec), f.name);
        if let Some(d) = &f.default {
            line.push_str(&format!(" = {}", ex(d, ASSIGN, 1)));
        }
        out.push_str(&line);
        out.push_str(";\n");
    }
    if !s.fields.is_empty() && !s.ctors.is_empty() {
        out.push('\n');
    }
    for c in &s.ctors {
        out.push_str(&format!("{PAD}{}({})", s.name, params_text(&c.params)));
        if !c.inits.is_empty() {
            let inits: Vec<String> = c
                .inits
                .iter()
                .map(|(n, a)| format!("{n}({})", args_text(a, 1)))
                .collect();
            out.push_str(&format!(" : {}", inits.join(", ")));
        }
        write_fn_body(out, &c.body, 1);
    }
    out.push_str("};\n");
}

fn write_item(out: &mut String, it: &Item) {
    match &it.kind {
        ItemKind::Func(f) if it.comments.is_empty() => write_comments(out, &f.comments, 0),
        _ => write_comments(out, &it.comments, 0),
    }
    match &it.kind {
        ItemKind::Include { path, angle } => {
            let (o, c) = if *angle { ('<', '>') } else { ('"', '"') };
            out.push_str(&format!("#include {o}{path}{c}\n"));
        }
        ItemKind::UsingNamespaceStd => out.push_str("using namespace std;\n"),
        ItemKind::Alias { name, ty, typedef } => {
            if *typedef {
                out.push_str(&format!("typedef {ty} {name};\n"));
            } else {
                out.push_str(&format!("using {name} = {ty};\n"));
            }
        }
        ItemKind::Struct(s) => write_struct(out, s),
        ItemKind::Global {
            spec,
            constexpr_,
            vars,
        } => {
            let ty = if *constexpr_ {
                let mut s = spec.clone();
                s.konst = false;
                format!("constexpr {}", print_type(&s))
            } else {
                print_type(spec)
            };
            out.push_str(&format!("{ty} {};\n", decls_text(vars, 0)));
        }
        ItemKind::Func(f) => {
            out.push_str(&format!(
                "{}{} {}({})",
                if f.is_static { "static " } else { "" },
                print_type(&f.ret),
                f.name,
                params_text(&f.params)
            ));
            match &f.body {
                None => out.push_str(";\n"),
                Some(b) => write_fn_body(out, b, 0),
            }
        }
    }
}

/// Items of the same class sit together; includes, aliases and globals, structs and functions are
/// separated by a blank line (consecutive forward declarations are not).
fn item_class(it: &Item) -> u8 {
    match &it.kind {
        ItemKind::Include { .. } => 0,
        ItemKind::UsingNamespaceStd | ItemKind::Alias { .. } | ItemKind::Global { .. } => 1,
        ItemKind::Func(f) if f.body.is_none() => 4,
        ItemKind::Func(_) => 2,
        ItemKind::Struct(_) => 3,
    }
}

/// Formatted code: 4 spaces, K&R braces. `player_only` skips the prelude items.
pub fn print_program(p: &Program, player_only: bool) -> String {
    let items: &[Item] = if player_only {
        p.player_items()
    } else {
        &p.items
    };
    let mut out = String::new();
    let mut prev: Option<u8> = None;
    for it in items {
        let class = item_class(it);
        if let Some(pc) = prev
            && (pc != class || class == 2 || class == 3)
        {
            out.push('\n');
        }
        write_item(&mut out, it);
        prev = Some(class);
    }
    out
}

// ---------------------------------------------------------------------------------------------------
// normalize
// ---------------------------------------------------------------------------------------------------

fn norm_exprs(es: &mut [Expr]) {
    es.iter_mut().for_each(norm_expr);
}

fn norm_params(ps: &mut [Param]) {
    for p in ps {
        p.id = 0;
    }
}

fn norm_block(b: &mut Block) {
    b.stmts.iter_mut().for_each(norm_stmt);
}

fn norm_vars(vars: &mut [Declarator]) {
    for v in vars {
        v.id = 0;
        if let Some(a) = &mut v.array {
            norm_expr(a);
        }
        match &mut v.init {
            Init::None => {}
            Init::Copy(e) => norm_expr(e),
            Init::Direct(a) | Init::Brace { elems: a, .. } => norm_exprs(a),
        }
    }
}

fn norm_expr(e: &mut Expr) {
    e.id = 0;
    match &mut e.kind {
        ExprKind::Unary(_, a) | ExprKind::Delete(a) => norm_expr(a),
        ExprKind::Cast { e: a, .. } => norm_expr(a),
        ExprKind::Member { obj, .. } => norm_expr(obj),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) => {
            norm_expr(a);
            norm_expr(b);
        }
        ExprKind::Assign { lhs, rhs, .. } => {
            norm_expr(lhs);
            norm_expr(rhs);
        }
        ExprKind::Ternary(a, b, c) => {
            norm_expr(a);
            norm_expr(b);
            norm_expr(c);
        }
        ExprKind::Call { callee, args } => {
            norm_expr(callee);
            norm_exprs(args);
        }
        ExprKind::InitList(a)
        | ExprKind::Construct { args: a, .. }
        | ExprKind::New { args: a, .. } => norm_exprs(a),
        ExprKind::Lambda(l) => {
            norm_params(&mut l.params);
            norm_block(&mut l.body);
        }
        _ => {}
    }
}

fn norm_stmt(s: &mut Stmt) {
    s.id = 0;
    match &mut s.kind {
        StmtKind::Decl { vars, .. } => norm_vars(vars),
        StmtKind::Structured { init, .. } => norm_expr(init),
        StmtKind::Expr(e) | StmtKind::Return(Some(e)) | StmtKind::Case(Some(e)) => norm_expr(e),
        StmtKind::If {
            init,
            cond,
            then,
            els,
        } => {
            if let Some(i) = init {
                norm_stmt(i);
            }
            norm_expr(cond);
            norm_stmt(then);
            if let Some(e) = els {
                norm_stmt(e);
            }
        }
        StmtKind::Switch { cond, body } => {
            norm_expr(cond);
            norm_block(body);
        }
        StmtKind::For {
            init,
            cond,
            incr,
            body,
        } => {
            if let Some(i) = init {
                norm_stmt(i);
            }
            if let Some(c) = cond {
                norm_expr(c);
            }
            if let Some(i) = incr {
                norm_expr(i);
            }
            norm_stmt(body);
        }
        StmtKind::RangeFor { range, body, .. } => {
            norm_expr(range);
            norm_stmt(body);
        }
        StmtKind::While { cond, body } | StmtKind::DoWhile { body, cond } => {
            norm_expr(cond);
            norm_stmt(body);
        }
        StmtKind::Block(b) => norm_block(b),
        StmtKind::Return(None)
        | StmtKind::Case(None)
        | StmtKind::Break
        | StmtKind::Continue
        | StmtKind::Empty => {}
    }
}

/// Zero every id and span (structural comparison in tests).
pub fn normalize(p: &mut Program) {
    p.next_id = 0;
    p.spans.clear();
    for it in &mut p.items {
        it.id = 0;
        match &mut it.kind {
            ItemKind::Func(f) => {
                f.id = 0;
                norm_params(&mut f.params);
                if let Some(b) = &mut f.body {
                    norm_block(b);
                }
            }
            ItemKind::Struct(s) => {
                s.id = 0;
                for f in &mut s.fields {
                    if let Some(d) = &mut f.default {
                        norm_expr(d);
                    }
                }
                for c in &mut s.ctors {
                    c.id = 0;
                    norm_params(&mut c.params);
                    for (_, a) in &mut c.inits {
                        norm_exprs(a);
                    }
                    norm_block(&mut c.body);
                }
            }
            ItemKind::Global { vars, .. } => norm_vars(vars),
            ItemKind::Include { .. } | ItemKind::UsingNamespaceStd | ItemKind::Alias { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpp::parse::{ParseOpts, parse, parse_expr_str};

    fn show(src: &str) -> String {
        print_expr(&parse_expr_str(src).expect("parses"))
    }

    fn same_tree(src: &str) {
        let printed = show(src);
        let (mut a, mut b) = (wrap(src), wrap(&printed));
        normalize(&mut a);
        normalize(&mut b);
        assert_eq!(a, b, "{src} printed as {printed}");
    }

    fn wrap(e: &str) -> Program {
        parse(&format!("int f() {{ return {e}; }}"), &ParseOpts::default()).expect("parses")
    }

    #[test]
    fn minimal_parentheses() {
        let cases = [
            ("a+b*c", "a + b * c"),
            ("(a+b)*c", "(a + b) * c"),
            ("(a*b)+c", "a * b + c"),
            ("a-(b-c)", "a - (b - c)"),
            ("(a-b)-c", "a - b - c"),
            ("a/(b*c)", "a / (b * c)"),
            ("(a/b)*c", "a / b * c"),
            ("a<b==c<d", "a < b == c < d"),
            ("(a==b)==c", "a == b == c"),
            ("a==(b==c)", "a == (b == c)"),
            ("(a&&b)||c", "a && b || c"),
            ("a&&(b||c)", "a && (b || c)"),
            ("!(a&&b)", "!(a && b)"),
            ("!a&&b", "!a && b"),
            ("-(-x)", "-(-x)"),
            ("-(--x)", "-(--x)"),
            ("- - -x", "-(-(-x))"),
            ("&(&x)", "&(&x)"),
            ("a- -b", "a - -b"),
            ("a-(-b)", "a - -b"),
            ("*p++", "*p++"),
            ("(*p)++", "(*p)++"),
            ("*(p+1)", "*(p + 1)"),
            ("(a+b)[0]", "(a + b)[0]"),
            ("a[(b,c)]", "a[b, c]"),
            ("f((a,b),c)", "f((a, b), c)"),
            ("a=b=c", "a = b = c"),
            ("(a=b)=c", "(a = b) = c"),
            ("a=b?c:d", "a = b ? c : d"),
            ("a?b:c?d:e", "a ? b : c ? d : e"),
            ("(a?b:c)?d:e", "(a ? b : c) ? d : e"),
            ("a?(b,c):d", "a ? b, c : d"),
            ("(a,b),c", "a, b, c"),
            ("a,(b,c)", "a, (b, c)"),
            ("(int)-x", "(int)-x"),
            ("(int)(a+b)", "(int)(a + b)"),
            ("(int)a+b", "(int)a + b"),
            (
                "static_cast<long long>(a)*b",
                "static_cast<long long>(a) * b",
            ),
            ("(new int(1))[0]", "(new int(1))[0]"),
            ("(1).x", "(1).x"),
            ("a<<b+c", "a << b + c"),
            ("(a<<b)+c", "(a << b) + c"),
            ("a?b:c", "a ? b : c"),
            ("x+=y-=z", "x += y -= z"),
            ("'\\n'+'\\''", "'\\n' + '\\''"),
            ("\"a\\\"b\\\\\"", "\"a\\\"b\\\\\""),
            ("0xFFu|0b101|010|5ull", "0xFFu | 0b101 | 010 | 5ULL"),
        ];
        for (src, want) in cases {
            assert_eq!(show(src), want, "{src}");
            same_tree(src);
        }
    }

    #[test]
    fn types_honour_spelling() {
        let p = parse(
            "using namespace std;\nvoid f(const vector<int>& a, const std::vector<int>& b, ListNode* c, unsigned int d, size_t e, int* const g) {}",
            &ParseOpts {
                first_id: 0,
                known_types: vec!["ListNode".into()],
            },
        )
        .unwrap();
        let ItemKind::Func(f) = &p.items[1].kind else {
            panic!()
        };
        let got: Vec<String> = f.params.iter().map(print_param).collect();
        assert_eq!(
            got,
            [
                "const vector<int>& a",
                "const std::vector<int>& b",
                "ListNode* c",
                "unsigned int d",
                "size_t e",
                "int* const g"
            ]
        );
    }

    #[test]
    fn program_layout() {
        let src = "#include <vector>\n#include <string>\nusing namespace std;\nconst int N = 3;\nstruct S { int a; S(int x) : a(x) {} };\nint g(int);\nint h(int);\nint g(int x) { if (x) { return 1; } else if (x > 2) return 2; else { return 3; } }\nint h(int x) { for (;;) { break; } return x; }";
        let p = parse(src, &ParseOpts::default()).unwrap();
        let want = "#include <vector>\n#include <string>\n\nusing namespace std;\nconst int N = 3;\n\nstruct S {\n    int a;\n\n    S(int x) : a(x) {}\n};\n\nint g(int);\nint h(int);\n\nint g(int x) {\n    if (x) {\n        return 1;\n    } else if (x > 2)\n        return 2;\n    else {\n        return 3;\n    }\n}\n\nint h(int x) {\n    for (;;) {\n        break;\n    }\n    return x;\n}\n";
        assert_eq!(print_program(&p, false), want);
        let mut p2 = p.clone();
        p2.prelude_items = 5;
        let player = print_program(&p2, true);
        assert!(player.starts_with("int g(int);"), "{player}");
        assert!(!player.contains("#include"));
    }

    #[test]
    fn dangling_else_is_protected() {
        // `if (a) { if (b) x; } else y;` printed from an AST whose inner `if` lost its braces
        let p = parse(
            "void f() { if (a) { if (b) x(); } else y(); }",
            &ParseOpts::default(),
        )
        .unwrap();
        let text = print_program(&p, false);
        assert!(text.contains("if (a) {"), "{text}");
        let again = parse(&text, &ParseOpts::default()).unwrap();
        let (mut a, mut b) = (p, again);
        normalize(&mut a);
        normalize(&mut b);
        assert_eq!(a, b);
        // an AST built by hand: the then-branch is an unbraced if-without-else
        let mut hand = parse("void f() { if (a) x(); else y(); }", &ParseOpts::default()).unwrap();
        let ItemKind::Func(f) = &mut hand.items[0].kind else {
            panic!()
        };
        let body = f.body.as_mut().unwrap();
        let StmtKind::If { then, .. } = &mut body.stmts[0].kind else {
            panic!()
        };
        let inner = Stmt::new(
            0,
            StmtKind::If {
                init: None,
                cond: Expr {
                    id: 0,
                    kind: ExprKind::Var("b".into()),
                },
                then: then.clone(),
                els: None,
            },
        );
        **then = inner;
        let text = print_program(&hand, false);
        let re = parse(&text, &ParseOpts::default()).unwrap();
        let ItemKind::Func(f2) = &re.items[0].kind else {
            panic!()
        };
        let StmtKind::If { els, .. } = &f2.body.as_ref().unwrap().stmts[0].kind else {
            panic!()
        };
        assert!(els.is_some(), "the else must stay on the outer if:\n{text}");
    }

    #[test]
    fn deep_programs_print_without_overflowing_the_stack() {
        let src = format!(
            "int f() {{ {}return {}1{};{} }}",
            "if (x) { ".repeat(80),
            "(".repeat(80),
            ")".repeat(80),
            " }".repeat(80)
        );
        let p = parse(&src, &ParseOpts::default()).unwrap();
        let text = print_program(&p, false);
        assert!(text.len() > 1000);
        let _ = parse(&text, &ParseOpts::default()).unwrap();
    }
}

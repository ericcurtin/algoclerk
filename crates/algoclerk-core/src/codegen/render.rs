// SPDX-License-Identifier: Apache-2.0
//! Rendering of the structured tree (or the fallback form) into C, C++ and Rust text.

use super::structure::{Cond, Stmt, structure};
use super::{Generated, Lang, Options, Shape, meta};
use crate::asm::Program;
use crate::isa::{Addr, Instr};
use crate::level::{CodeMeta, Level, ParamKind, ParamSpec, RetType, ReturnKind};
use std::collections::HashSet;

const KEYWORDS: &[&str] = &[
    "alignas", "alignof", "and", "as", "asm", "async", "auto", "await", "bool", "break", "case", "catch",
    "char", "class", "const", "constexpr", "continue", "crate", "default", "delete", "do", "double", "dyn",
    "else", "enum", "extern", "false", "final", "float", "fn", "for", "friend", "goto", "if", "impl", "in",
    "inline", "int", "let", "long", "loop", "match", "mod", "move", "mut", "namespace", "new", "not", "or",
    "override", "pub", "ref", "register", "restrict", "return", "self", "short", "signed", "sizeof",
    "static", "struct", "super", "switch", "template", "this", "throw", "trait", "true", "try", "type",
    "typedef", "typename", "union", "unsafe", "unsigned", "use", "using", "virtual", "void", "volatile",
    "where", "while", "yield", "main", "std", "size_t", "i32", "usize", "u32", "u8", "i64", "u64",
];

const RESERVED: &[&str] = &[
    "hand", "next", "inbox", "inbox_len", "outbox", "out_len", "floor", "floor_start", "pc", "i",
];

fn snake(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("v_{out}").trim_end_matches('_').to_string()
    } else {
        out
    }
}

struct Namer {
    used: HashSet<String>,
}

impl Namer {
    fn new(extra: &[String]) -> Namer {
        let mut used: HashSet<String> = RESERVED.iter().map(|s| (*s).to_string()).collect();
        used.extend(extra.iter().cloned());
        Namer { used }
    }

    fn unique(&mut self, base: &str) -> String {
        let mut name = snake(base);
        if KEYWORDS.contains(&name.as_str()) {
            name.push('_');
        }
        let mut candidate = name.clone();
        let mut n = 2;
        while self.used.contains(&candidate) {
            candidate = format!("{name}_{n}");
            n += 1;
        }
        self.used.insert(candidate.clone());
        candidate
    }
}

enum Ret {
    List,
    Scalar { boolean: bool },
}

struct Gen<'a> {
    lang: Lang,
    prog: &'a Program,
    array: bool,
    floor_len: usize,
    slot_names: Vec<Option<String>>,
    ret: Ret,
    lines: Vec<String>,
    indent: usize,
}

fn cond_text(lang: Lang, c: Cond) -> String {
    let body = match c {
        Cond::Zero => "hand == 0",
        Cond::NotZero => "hand != 0",
        Cond::Neg => "hand < 0",
        Cond::NotNeg => "hand >= 0",
    };
    match lang {
        Lang::Rust => body.to_string(),
        _ => format!("({body})"),
    }
}

impl Gen<'_> {
    fn line(&mut self, text: impl AsRef<str>) {
        let t = text.as_ref();
        if t.is_empty() {
            self.lines.push(String::new());
        } else {
            self.lines.push(format!("{}{}", "    ".repeat(self.indent), t));
        }
    }

    fn line_c(&mut self, text: impl AsRef<str>, comment: &str) {
        let t = text.as_ref();
        if comment.is_empty() {
            self.line(t);
        } else {
            self.line(format!("{t}  // {comment}"));
        }
    }

    fn slot(&self, a: Addr) -> String {
        match a {
            Addr::Direct(n) => {
                if self.array {
                    format!("floor[{n}]")
                } else {
                    self.slot_names
                        .get(n)
                        .cloned()
                        .flatten()
                        .unwrap_or_else(|| format!("tile_{n}"))
                }
            }
            Addr::Indirect(n) => match self.lang {
                Lang::Rust => format!("floor[floor[{n}] as usize]"),
                _ => format!("floor[floor[{n}]]"),
            },
        }
    }

    fn exit_text(&self) -> String {
        match (&self.ret, self.lang) {
            (Ret::List, Lang::C) => "return out_len;".into(),
            (Ret::List, _) => "return outbox;".into(),
            (Ret::Scalar { boolean: true }, _) => "return false;".into(),
            (Ret::Scalar { boolean: false }, _) => "return 0;".into(),
        }
    }

    fn target_name(&self, t: usize) -> String {
        if t >= self.prog.instrs.len() {
            "end".into()
        } else {
            self.prog
                .labels
                .get(t)
                .and_then(|v| v.first().cloned())
                .unwrap_or_else(|| format!("L{t}"))
        }
    }

    fn instr_comment(&self, i: usize) -> String {
        self.prog.instrs[i].map_target(|t| self.target_name(*t)).to_string()
    }

    /// Statements implementing a non-jump instruction.
    fn ins_stmts(&self, i: usize) -> Vec<String> {
        let rust = self.lang == Lang::Rust;
        let semi = ";";
        match &self.prog.instrs[i] {
            Instr::Inbox => {
                let len = match self.lang {
                    Lang::C => "inbox_len",
                    Lang::Cpp => "inbox.size()",
                    Lang::Rust => "inbox.len()",
                };
                let exit = self.exit_text();
                if rust {
                    vec![
                        format!("if next == {len} {{ {exit} }}"),
                        "hand = inbox[next];".into(),
                        "next += 1;".into(),
                    ]
                } else {
                    vec![
                        format!("if (next == {len}) {{ {exit} }}"),
                        "hand = inbox[next++];".into(),
                    ]
                }
            }
            Instr::Outbox => match &self.ret {
                Ret::Scalar { boolean: true } => vec!["return hand != 0;".into()],
                Ret::Scalar { boolean: false } => vec!["return hand;".into()],
                Ret::List => match self.lang {
                    Lang::C => vec!["outbox[out_len++] = hand;".into()],
                    Lang::Cpp => vec!["outbox.push_back(hand);".into()],
                    Lang::Rust => vec!["outbox.push(hand);".into()],
                },
            },
            Instr::CopyTo(a) => vec![format!("{} = hand{semi}", self.slot(*a))],
            Instr::CopyFrom(a) => vec![format!("hand = {}{semi}", self.slot(*a))],
            Instr::Add(a) => vec![format!("hand += {}{semi}", self.slot(*a))],
            Instr::Sub(a) => vec![format!("hand -= {}{semi}", self.slot(*a))],
            Instr::Mul(a) => vec![format!("hand *= {}{semi}", self.slot(*a))],
            Instr::Div(a) => vec![format!("hand /= {}{semi}", self.slot(*a))],
            Instr::Mod(a) => vec![format!("hand %= {}{semi}", self.slot(*a))],
            Instr::BumpUp(a) | Instr::BumpDn(a) => {
                let up = matches!(self.prog.instrs[i], Instr::BumpUp(_));
                let s = self.slot(*a);
                if rust {
                    vec![
                        format!("{s} {}= 1;", if up { '+' } else { '-' }),
                        format!("hand = {s};"),
                    ]
                } else {
                    vec![format!("hand = {}{s};", if up { "++" } else { "--" })]
                }
            }
            Instr::Jump(_) | Instr::JumpZ(_) | Instr::JumpN(_) => Vec::new(),
        }
    }

    fn emit_ins(&mut self, i: usize) {
        let stmts = self.ins_stmts(i);
        let comment = self.instr_comment(i);
        for (k, s) in stmts.iter().enumerate() {
            self.line_c(s, if k == 0 { &comment } else { "" });
        }
    }

    fn open_if(&mut self, cond: Cond, comment: &str) {
        let c = cond_text(self.lang, cond);
        let text = match self.lang {
            Lang::Rust => format!("if {c} {{"),
            _ => format!("if {c} {{"),
        };
        self.line_c(text, comment);
    }

    fn stmts(&mut self, v: &[Stmt]) {
        for s in v {
            self.stmt(s);
        }
    }

    fn head_comment(&self, head: usize) -> String {
        match self.prog.labels.get(head).and_then(|l| l.first()) {
            Some(l) => format!("{l}:"),
            None => String::new(),
        }
    }

    fn block(&mut self, v: &[Stmt]) {
        self.indent += 1;
        self.stmts(v);
        self.indent -= 1;
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Ins(i) => self.emit_ins(*i),
            Stmt::If {
                cond,
                then,
                els,
                at,
            } => {
                let comment = self.instr_comment(*at);
                self.open_if(*cond, &comment);
                self.block(then);
                if !els.is_empty() {
                    self.line("} else {");
                    self.block(els);
                }
                self.line("}");
            }
            Stmt::Loop { body, head } => {
                let open = match self.lang {
                    Lang::C => "for (;;) {",
                    Lang::Cpp => "while (true) {",
                    Lang::Rust => "loop {",
                };
                let c = self.head_comment(*head);
                self.line_c(open, &c);
                self.block(body);
                self.line("}");
            }
            Stmt::While { cond, body, head } => {
                let c = cond_text(self.lang, *cond);
                let open = format!("while {c} {{");
                let cm = self.head_comment(*head);
                self.line_c(open, &cm);
                self.block(body);
                self.line("}");
            }
            Stmt::DoWhile {
                body,
                cond,
                head,
                at,
            } => {
                let cm = self.head_comment(*head);
                let end_comment = self.instr_comment(*at);
                if self.lang == Lang::Rust {
                    self.line_c("loop {", &cm);
                    self.block(body);
                    self.indent += 1;
                    let c = cond_text(Lang::Rust, cond.negate());
                    self.line_c(format!("if {c} {{ break; }}"), &end_comment);
                    self.indent -= 1;
                    self.line("}");
                } else {
                    self.line_c("do {", &cm);
                    self.block(body);
                    let c = cond_text(self.lang, *cond);
                    self.line_c(format!("}} while {c};"), &end_comment);
                }
            }
            Stmt::Break(at) => {
                let c = self.instr_comment(*at);
                self.line_c("break;", &c);
            }
            Stmt::Continue(at) => {
                let c = self.instr_comment(*at);
                self.line_c("continue;", &c);
            }
            Stmt::Exit(at) => {
                let c = self.instr_comment(*at);
                let e = self.exit_text();
                self.line_c(e, &c);
            }
        }
    }

    // ---- fallback forms -------------------------------------------------------------------

    fn label_for(&self, t: usize) -> String {
        match self.prog.labels.get(t).and_then(|l| l.first()) {
            Some(l) => format!("l_{}", snake(l)),
            None => format!("l_{t}"),
        }
    }

    fn goto_form(&mut self) {
        let n = self.prog.instrs.len();
        let mut targeted = vec![false; n + 1];
        for ins in &self.prog.instrs {
            if let Some(t) = ins.target() {
                targeted[*t] = true;
            }
        }
        self.line("// The jumps in this program do not fit loops and branches,");
        self.line("// so the clerk's own labels are kept and the code uses goto.");
        for i in 0..n {
            if targeted[i] {
                let l = self.label_for(i);
                self.indent = 0;
                self.line(format!("{l}:"));
                self.indent = 1;
            }
            let comment = self.instr_comment(i);
            let ins = self.prog.instrs[i].clone();
            let goto = |g: &Self, t: usize| {
                if t >= n {
                    g.exit_text()
                } else {
                    format!("goto {};", g.label_for(t))
                }
            };
            match ins {
                Instr::Jump(t) => {
                    let g = goto(self, t);
                    self.line_c(g, &comment);
                }
                Instr::JumpZ(t) | Instr::JumpN(t) => {
                    let c = if matches!(ins, Instr::JumpZ(_)) { Cond::Zero } else { Cond::Neg };
                    let g = goto(self, t);
                    let ct = cond_text(self.lang, c);
                    self.line_c(format!("if {ct} {{ {g} }}"), &comment);
                }
                _ => self.emit_ins(i),
            }
        }
        let e = self.exit_text();
        self.line(e);
    }

    fn state_machine_form(&mut self) {
        let n = self.prog.instrs.len();
        let mut leader = vec![false; n + 1];
        leader[0] = true;
        for (i, ins) in self.prog.instrs.iter().enumerate() {
            if let Some(t) = ins.target() {
                leader[*t] = true;
                leader[i + 1] = true;
            }
        }
        self.line("// The jumps in this program do not fit loops and branches, so `pc` walks");
        self.line("// through the blocks of the program like the clerk's finger does.");
        self.line("let mut pc: usize = 0;");
        self.line("loop {");
        self.indent += 1;
        self.line("match pc {");
        self.indent += 1;
        let starts: Vec<usize> = (0..n).filter(|i| leader[*i]).collect();
        for (bi, &s) in starts.iter().enumerate() {
            let end = starts.get(bi + 1).copied().unwrap_or(n);
            let label = self.head_comment(s);
            self.line_c(format!("{s} => {{"), &label);
            self.indent += 1;
            let mut ended = false;
            for i in s..end {
                let comment = self.instr_comment(i);
                let ins = self.prog.instrs[i].clone();
                match ins {
                    Instr::Jump(t) => {
                        self.line_c(format!("pc = {t};"), &comment);
                        ended = true;
                    }
                    Instr::JumpZ(t) | Instr::JumpN(t) => {
                        let c = if matches!(ins, Instr::JumpZ(_)) { Cond::Zero } else { Cond::Neg };
                        let ct = cond_text(Lang::Rust, c);
                        self.line_c(
                            format!("pc = if {ct} {{ {t} }} else {{ {} }};", i + 1),
                            &comment,
                        );
                        ended = true;
                    }
                    _ => self.emit_ins(i),
                }
            }
            if !ended {
                self.line(format!("pc = {end};"));
            }
            self.indent -= 1;
            self.line("}");
        }
        let e = self.exit_text();
        self.line(format!("_ => {{ {} }}", e.trim_end_matches(';')));
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }
}

fn used_direct_slots(p: &Program) -> Vec<usize> {
    let mut v: Vec<usize> = p
        .instrs
        .iter()
        .filter_map(|i| i.addr())
        .map(|a| a.slot())
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

fn c_param(p: &ParamSpec) -> String {
    match p.kind {
        ParamKind::Scalar | ParamKind::Length => format!("int {}", p.name),
        _ => format!("const int *{n}, size_t {n}_len", n = p.name),
    }
}

fn cpp_param(p: &ParamSpec) -> String {
    match p.kind {
        ParamKind::Scalar | ParamKind::Length => format!("int {}", p.name),
        _ => format!("const std::vector<int>& {}", p.name),
    }
}

fn rust_param(p: &ParamSpec) -> String {
    match p.kind {
        ParamKind::Scalar | ParamKind::Length => format!("{}: i32", p.name),
        _ => format!("{}: &[i32]", p.name),
    }
}

pub(super) fn generate(program: &Program, lang: Lang, level: Option<&Level>, opts: &Options) -> Generated {
    let n = program.instrs.len();
    let max_slot = program.instrs.iter().filter_map(|i| i.addr()).map(|a| a.slot()).max();
    let referenced = max_slot.map_or(1, |m| m + 1);
    let floor_len = match level {
        Some(l) => l.machine.floor.max(1),
        None => opts.floor_len.unwrap_or(0).max(referenced),
    };
    let template: Vec<Option<i32>> = level
        .and_then(|l| l.floor_template().ok())
        .unwrap_or_else(|| vec![None; floor_len]);
    let meta: Option<&CodeMeta> = level.and_then(|l| l.code.as_ref()).filter(|_| !opts.force_generic);
    let floor_input = meta.is_none() && level.is_some_and(meta::level_has_floor_input);
    let meta_floor = meta.is_some_and(|m| m.params.iter().any(|p| p.at.is_some()));
    let array = program.uses_indirect() || floor_input || meta_floor;

    let function = opts
        .function_name
        .clone()
        .or_else(|| meta.map(|m| m.function.clone()))
        .unwrap_or_else(|| "clerk_run".to_string());
    let mut reserved: Vec<String> = vec![function.clone()];
    if let Some(m) = meta {
        for p in &m.params {
            reserved.push(p.name.clone());
            reserved.push(format!("{}_len", p.name));
        }
    }
    let mut namer = Namer::new(&reserved);
    let used = used_direct_slots(program);
    let mut slot_names: Vec<Option<String>> = vec![None; floor_len.max(referenced)];
    for &s in &used {
        let base = level
            .and_then(|l| l.slot_name(s))
            .map_or_else(|| format!("tile_{s}"), ToString::to_string);
        slot_names[s] = Some(namer.unique(&base));
    }

    let ret = match meta {
        Some(m) if m.returns == ReturnKind::Scalar => Ret::Scalar {
            boolean: m.return_type == RetType::Bool,
        },
        _ => Ret::List,
    };

    let mut g = Gen {
        lang,
        prog: program,
        array,
        floor_len,
        slot_names,
        ret,
        lines: Vec::new(),
        indent: 0,
    };

    // Header and includes.
    let title = level.map(|l| l.title.as_str());
    match title {
        Some(t) => g.line(format!("// Generated by AlgoClerk from your solution to \"{t}\".")),
        None => g.line("// Generated by AlgoClerk from a clerk program."),
    }
    if let Some(s) = &opts.stats {
        g.line(format!(
            "// Size: {} instructions. Speed: {:.1} steps on average, {} at worst.",
            s.size, s.avg_steps, s.worst_steps
        ));
    }
    if let Some(e) = level.and_then(|l| l.explain.as_ref()) {
        g.line(format!("// Idea: {}.", e.algorithm));
    }
    let boolean = matches!(g.ret, Ret::Scalar { boolean: true });
    match lang {
        Lang::C => {
            g.line("#include <stddef.h>");
            if boolean {
                g.line("#include <stdbool.h>");
            }
        }
        Lang::Cpp => {
            g.line("#include <array>");
            g.line("#include <cstddef>");
            g.line("#include <vector>");
        }
        Lang::Rust => {}
    }
    g.line("");

    // Signature.
    let doc = meta
        .map(|m| m.doc.clone())
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "Runs the clerk's program: reads the inbox, returns the outbox.".to_string());
    g.line(format!("// {doc}"));
    let params: Vec<String> = match (meta, lang) {
        (Some(m), Lang::C) => m.params.iter().filter(|p| p.kind != ParamKind::Length).map(c_param).collect(),
        (Some(m), Lang::Cpp) => m.params.iter().filter(|p| p.kind != ParamKind::Length).map(cpp_param).collect(),
        (Some(m), Lang::Rust) => m.params.iter().filter(|p| p.kind != ParamKind::Length).map(rust_param).collect(),
        (None, Lang::C) => {
            let mut v = vec!["const int *inbox".to_string(), "size_t inbox_len".to_string()];
            if floor_input {
                v.push("const int *floor_start".into());
            }
            v
        }
        (None, Lang::Cpp) => {
            let mut v = vec!["const std::vector<int>& inbox".to_string()];
            if floor_input {
                v.push("const std::vector<int>& floor_start".into());
            }
            v
        }
        (None, Lang::Rust) => {
            let mut v = vec!["inbox: &[i32]".to_string()];
            if floor_input {
                v.push("floor_start: &[i32]".into());
            }
            v
        }
    };
    let mut params = params;
    let list_ret = matches!(g.ret, Ret::List);
    if lang == Lang::C && list_ret {
        params.push("int *outbox".into());
        g.line("// Writes the clerk's output to `outbox` and returns how many values it wrote.");
    }
    let plist = params.join(", ");
    match lang {
        Lang::C => {
            let rt = match (&g.ret, boolean) {
                (Ret::List, _) => "size_t",
                (_, true) => "bool",
                _ => "int",
            };
            g.line(format!("{rt} {function}({plist}) {{"));
        }
        Lang::Cpp => {
            let rt = match (&g.ret, boolean) {
                (Ret::List, _) => "std::vector<int>",
                (_, true) => "bool",
                _ => "int",
            };
            g.line(format!("{rt} {function}({plist}) {{"));
        }
        Lang::Rust => {
            let rt = match (&g.ret, boolean) {
                (Ret::List, _) => "Vec<i32>",
                (_, true) => "bool",
                _ => "i32",
            };
            g.line("#[allow(unused_assignments, unused_mut, unreachable_code)]");
            g.line(format!("pub fn {function}({plist}) -> {rt} {{"));
        }
    }
    g.indent = 1;

    // Prologue: inbox.
    let has_inbox = program.instrs.iter().any(|i| matches!(i, Instr::Inbox));
    if let Some(m) = meta {
        let inbox_params: Vec<&ParamSpec> = m.params.iter().filter(|p| p.at.is_none()).collect();
        let single_list = inbox_params.len() == 1 && inbox_params[0].kind == ParamKind::List;
        if has_inbox {
            prologue_inbox(&mut g, &inbox_params, single_list);
        }
    }
    // Locals.
    let touches_hand = program.instrs.iter().any(|i| !matches!(i, Instr::Jump(_)));
    if has_inbox {
        match lang {
            Lang::C => g.line("size_t next = 0;  // how many inbox values the clerk has taken"),
            Lang::Cpp => g.line("std::size_t next = 0;  // how many inbox values the clerk has taken"),
            Lang::Rust => g.line("let mut next = 0;  // how many inbox values the clerk has taken"),
        }
    }
    if list_ret {
        match lang {
            Lang::C => {
                if program.instrs.iter().any(|i| matches!(i, Instr::Outbox)) {
                    g.line("size_t out_len = 0;");
                }
            }
            Lang::Cpp => g.line("std::vector<int> outbox;"),
            Lang::Rust => g.line("let mut outbox = Vec::new();"),
        }
    }
    if touches_hand {
        match lang {
            Lang::Rust => g.line("let mut hand: i32 = 0;  // the clerk's hand"),
            _ => g.line("int hand = 0;  // the clerk's hand"),
        }
    }
    declare_floor(&mut g, level, &template, &used, floor_input, meta);

    // Body.
    let tree = if opts.force_fallback { None } else { structure(program) };
    let structured = tree.is_some();
    if n > 0 {
        match tree {
            Some(t) => {
                g.stmts(&t);
                let e = g.exit_text();
                g.line(e);
            }
            None => match lang {
                Lang::Rust => {
                    g.state_machine_form();
                }
                _ => g.goto_form(),
            },
        }
    } else {
        let e = g.exit_text();
        g.line(e);
    }
    g.indent = 0;
    g.line("}");

    Generated {
        lang,
        code: g.lines.join("\n") + "\n",
        function,
        structured,
        shape: match meta {
            Some(m) => Shape::Meta(m.clone()),
            None => Shape::Generic { floor_param: floor_input },
        },
        floor_len,
        explain: level.and_then(|l| l.explain.clone()),
    }
}

fn prologue_inbox(g: &mut Gen<'_>, ps: &[&ParamSpec], single_list: bool) {
    let lang = g.lang;
    if single_list {
        let n = &ps[0].name;
        match lang {
            Lang::C => {
                g.line(format!("const int *inbox = {n};"));
                g.line(format!("size_t inbox_len = {n}_len;"));
            }
            Lang::Cpp => g.line(format!("const std::vector<int>& inbox = {n};")),
            Lang::Rust => g.line(format!("let inbox = {n};")),
        }
        return;
    }
    match lang {
        Lang::C => {
            let mut terms: Vec<String> = Vec::new();
            for p in ps {
                match p.kind {
                    ParamKind::Scalar | ParamKind::Length => terms.push("1".into()),
                    ParamKind::List => terms.push(format!("{}_len", p.name)),
                    ParamKind::Counted | ParamKind::Terminated => terms.push(format!("{}_len + 1", p.name)),
                }
            }
            if terms.is_empty() {
                terms.push("0".into());
            }
            g.line(format!("size_t inbox_len = {};", terms.join(" + ")));
            g.line("int inbox[inbox_len > 0 ? inbox_len : 1];  // the values the clerk will find in the inbox");
            g.line("size_t filled = 0;");
            for p in ps {
                let n = &p.name;
                match p.kind {
                    ParamKind::Scalar => g.line(format!("inbox[filled++] = {n};")),
                    ParamKind::Length => {
                        let of = p.of.clone().unwrap_or_default();
                        g.line(format!("inbox[filled++] = (int){of}_len;"));
                    }
                    ParamKind::Counted => {
                        g.line(format!("inbox[filled++] = (int){n}_len;"));
                        g.line(format!("for (size_t i = 0; i < {n}_len; i++) inbox[filled++] = {n}[i];"));
                    }
                    ParamKind::List => {
                        g.line(format!("for (size_t i = 0; i < {n}_len; i++) inbox[filled++] = {n}[i];"));
                    }
                    ParamKind::Terminated => {
                        g.line(format!("for (size_t i = 0; i < {n}_len; i++) inbox[filled++] = {n}[i];"));
                        g.line(format!("inbox[filled++] = {};", p.terminator));
                    }
                }
            }
        }
        Lang::Cpp => {
            g.line("std::vector<int> inbox;  // the values the clerk will find in the inbox");
            for p in ps {
                let n = &p.name;
                match p.kind {
                    ParamKind::Scalar => g.line(format!("inbox.push_back({n});")),
                    ParamKind::Length => {
                        let of = p.of.clone().unwrap_or_default();
                        g.line(format!("inbox.push_back(static_cast<int>({of}.size()));"));
                    }
                    ParamKind::Counted => {
                        g.line(format!("inbox.push_back(static_cast<int>({n}.size()));"));
                        g.line(format!("inbox.insert(inbox.end(), {n}.begin(), {n}.end());"));
                    }
                    ParamKind::List => g.line(format!("inbox.insert(inbox.end(), {n}.begin(), {n}.end());")),
                    ParamKind::Terminated => {
                        g.line(format!("inbox.insert(inbox.end(), {n}.begin(), {n}.end());"));
                        g.line(format!("inbox.push_back({});", p.terminator));
                    }
                }
            }
        }
        Lang::Rust => {
            g.line("let mut inbox: Vec<i32> = Vec::new();  // the values the clerk will find in the inbox");
            for p in ps {
                let n = &p.name;
                match p.kind {
                    ParamKind::Scalar => g.line(format!("inbox.push({n});")),
                    ParamKind::Length => {
                        let of = p.of.clone().unwrap_or_default();
                        g.line(format!("inbox.push({of}.len() as i32);"));
                    }
                    ParamKind::Counted => {
                        g.line(format!("inbox.push({n}.len() as i32);"));
                        g.line(format!("inbox.extend_from_slice({n});"));
                    }
                    ParamKind::List => g.line(format!("inbox.extend_from_slice({n});")),
                    ParamKind::Terminated => {
                        g.line(format!("inbox.extend_from_slice({n});"));
                        g.line(format!("inbox.push({});", p.terminator));
                    }
                }
            }
        }
    }
}

fn declare_floor(
    g: &mut Gen<'_>,
    level: Option<&Level>,
    template: &[Option<i32>],
    used: &[usize],
    floor_input: bool,
    meta: Option<&CodeMeta>,
) {
    let lang = g.lang;
    let init = |s: usize| template.get(s).copied().flatten().unwrap_or(0);
    if !g.array {
        for &s in used {
            let name = g.slot_names[s].clone().unwrap_or_default();
            let text = match lang {
                Lang::Rust => format!("let mut {name}: i32 = {};", init(s)),
                _ => format!("int {name} = {};", init(s)),
            };
            g.line_c(text, &format!("tile {s}"));
        }
        return;
    }
    let n = g.floor_len;
    let names: Vec<String> = (0..n)
        .filter_map(|s| level.and_then(|l| l.slot_name(s)).map(|nm| format!("{s} = {nm}")))
        .collect();
    if !names.is_empty() {
        g.line(format!("// Floor tiles: {}.", names.join(", ")));
    }
    let values: Vec<i32> = (0..n).map(init).collect();
    let all_zero = values.iter().all(|v| *v == 0);
    let list = values.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
    if floor_input {
        match lang {
            Lang::C => {
                g.line(format!("int floor[{n}];  // starts as `floor_start`"));
                g.line(format!("for (size_t i = 0; i < {n}; i++) floor[i] = floor_start[i];"));
            }
            Lang::Cpp => {
                g.line(format!("std::array<int, {n}> floor{{}};  // starts as `floor_start`"));
                g.line(format!("for (std::size_t i = 0; i < {n}; i++) floor[i] = floor_start[i];"));
            }
            Lang::Rust => {
                g.line(format!("let mut floor = [0i32; {n}];  // starts as `floor_start`"));
                g.line(format!("floor.copy_from_slice(&floor_start[..{n}]);"));
            }
        }
        return;
    }
    match lang {
        Lang::C => {
            let v = if all_zero { "0".to_string() } else { list };
            g.line(format!("int floor[{n}] = {{{v}}};"));
        }
        Lang::Cpp => {
            let v = if all_zero { String::new() } else { list };
            g.line(format!("std::array<int, {n}> floor{{{v}}};"));
        }
        Lang::Rust => {
            if all_zero {
                g.line(format!("let mut floor = [0i32; {n}];"));
            } else {
                g.line(format!("let mut floor: [i32; {n}] = [{list}];"));
            }
        }
    }
    if let Some(m) = meta {
        for p in m.params.iter().filter(|p| p.at.is_some()) {
            let at = p.at.unwrap_or(0);
            let nm = &p.name;
            match (p.kind, lang) {
                (ParamKind::Scalar, _) => g.line(format!("floor[{at}] = {nm};")),
                (_, Lang::C) => g.line(format!(
                    "for (size_t i = 0; i < {nm}_len; i++) floor[{at} + i] = {nm}[i];"
                )),
                (_, Lang::Cpp) => g.line(format!(
                    "for (std::size_t i = 0; i < {nm}.size(); i++) floor[{at} + i] = {nm}[i];"
                )),
                (_, Lang::Rust) => g.line(format!(
                    "floor[{at}..{at} + {nm}.len()].copy_from_slice({nm});"
                )),
            }
        }
    }
}

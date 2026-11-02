// SPDX-License-Identifier: Apache-2.0
//! Assembler: text <-> [`Listing`] (editor model) -> [`Program`] (executable form).
//!
//! The text syntax is documented in `docs/ISA.md`. Parsing never panics and reports
//! every problem it finds, with one-based line numbers.

use crate::i18n::{Diagnostic, english};
use crate::isa::{Addr, Allowed, Instr, MAX_SLOT, Opcode, OperandKind};
use std::collections::HashMap;
use std::fmt;

/// What a listing line contains.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// `name:`
    Label(String),
    /// An instruction.
    Instr(Instr<String>),
    /// Nothing (a comment-only line when `Line::comment` is set).
    Blank,
}

/// One editor line: a label, an instruction or a comment, with an optional trailing comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// The content.
    pub kind: LineKind,
    /// Comment text (without the `--` marker).
    pub comment: Option<String>,
    /// One-based line in the text this was parsed from (0 when built in code).
    pub src: usize,
}

impl Line {
    /// An instruction line.
    pub fn instr(i: Instr<String>) -> Line {
        Line {
            kind: LineKind::Instr(i),
            comment: None,
            src: 0,
        }
    }

    /// A label line.
    pub fn label(name: impl Into<String>) -> Line {
        Line {
            kind: LineKind::Label(name.into()),
            comment: None,
            src: 0,
        }
    }

    /// A comment-only line.
    pub fn comment(text: impl Into<String>) -> Line {
        Line {
            kind: LineKind::Blank,
            comment: Some(text.into()),
            src: 0,
        }
    }
}

/// The editable program: an ordered list of lines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Listing {
    /// Lines in order.
    pub lines: Vec<Line>,
}

/// Problems found while parsing text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// Mnemonic not recognised.
    UnknownInstruction(String),
    /// The instruction needs an operand.
    MissingOperand(Opcode),
    /// The instruction takes no operand.
    UnexpectedOperand(Opcode),
    /// More than one operand.
    TooManyOperands(Opcode),
    /// Not a valid floor address.
    BadAddress(String),
    /// A jump target is not a valid label name.
    BadLabel(String),
    /// A label name is empty or malformed.
    BadLabelDefinition(String),
}

/// A parse error with its line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// One-based line number.
    pub line: usize,
    /// What went wrong.
    pub kind: ParseErrorKind,
}

impl Diagnostic for ParseError {
    fn key(&self) -> &'static str {
        match self.kind {
            ParseErrorKind::UnknownInstruction(_) => "parse.unknown_instruction",
            ParseErrorKind::MissingOperand(_) => "parse.missing_operand",
            ParseErrorKind::UnexpectedOperand(_) => "parse.unexpected_operand",
            ParseErrorKind::TooManyOperands(_) => "parse.too_many_operands",
            ParseErrorKind::BadAddress(_) => "parse.bad_address",
            ParseErrorKind::BadLabel(_) => "parse.bad_label",
            ParseErrorKind::BadLabelDefinition(_) => "parse.bad_label_definition",
        }
    }

    fn args(&self) -> Vec<(&'static str, String)> {
        let mut v = vec![("line", self.line.to_string())];
        match &self.kind {
            ParseErrorKind::UnknownInstruction(s)
            | ParseErrorKind::BadAddress(s)
            | ParseErrorKind::BadLabel(s)
            | ParseErrorKind::BadLabelDefinition(s) => v.push(("text", s.clone())),
            ParseErrorKind::MissingOperand(o)
            | ParseErrorKind::UnexpectedOperand(o)
            | ParseErrorKind::TooManyOperands(o) => v.push(("op", o.mnemonic().to_string())),
        }
        v
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message(english()))
    }
}

impl std::error::Error for ParseError {}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_') && s.len() <= 48
}

fn split_comment(line: &str) -> (&str, Option<String>) {
    let mut cut: Option<usize> = None;
    for pat in ["--", "//", ";", "#"] {
        if let Some(i) = line.find(pat) {
            cut = Some(cut.map_or(i, |c| c.min(i)));
        }
    }
    match cut {
        None => (line, None),
        Some(i) => {
            let rest = &line[i..];
            let skip = if rest.starts_with("--") || rest.starts_with("//") {
                2
            } else {
                1
            };
            (&line[..i], Some(rest[skip..].trim().to_string()))
        }
    }
}

fn parse_number(s: &str) -> Option<usize> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) || s.len() > 6 {
        return None;
    }
    let n: usize = s.parse().ok()?;
    (n <= MAX_SLOT).then_some(n)
}

fn parse_addr(s: &str) -> Option<Addr> {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        parse_number(inner.trim()).map(Addr::Indirect)
    } else {
        parse_number(s).map(Addr::Direct)
    }
}

/// Parse assembly text. Returns every error found, or the listing.
pub fn parse(text: &str) -> Result<Listing, Vec<ParseError>> {
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    for (idx, raw) in text.lines().enumerate() {
        let src = idx + 1;
        let (code, comment) = split_comment(raw);
        let mut rest = code.trim();
        let mut pending_comment = comment;
        let mut emitted = false;
        // Leading labels: `name:`
        while let Some(colon) = rest.find(':') {
            let name = rest[..colon].trim();
            if name.contains(char::is_whitespace) {
                break;
            }
            if !is_ident(name) {
                errors.push(ParseError {
                    line: src,
                    kind: ParseErrorKind::BadLabelDefinition(name.to_string()),
                });
                rest = "";
                emitted = true;
                break;
            }
            lines.push(Line {
                kind: LineKind::Label(name.to_string()),
                comment: None,
                src,
            });
            emitted = true;
            rest = rest[colon + 1..].trim();
        }
        if rest.is_empty() {
            if let Some(c) = pending_comment.take() {
                if let Some(last) = lines.last_mut().filter(|l| emitted && l.src == src) {
                    last.comment = Some(c);
                } else {
                    lines.push(Line {
                        kind: LineKind::Blank,
                        comment: Some(c),
                        src,
                    });
                }
            }
            continue;
        }
        let mut parts = rest.split_whitespace();
        let word = parts.next().unwrap_or_default();
        let args: Vec<&str> = parts.collect();
        let Some(op) = Opcode::from_mnemonic(word) else {
            errors.push(ParseError {
                line: src,
                kind: ParseErrorKind::UnknownInstruction(word.chars().take(24).collect()),
            });
            continue;
        };
        let operand_text = rest[word.len()..].trim();
        let err = |kind| ParseError { line: src, kind };
        let instr: Instr<String> = match op.operand_kind() {
            OperandKind::None => {
                if !args.is_empty() {
                    errors.push(err(ParseErrorKind::UnexpectedOperand(op)));
                    continue;
                }
                Instr::with_addr(op, Addr::Direct(0), String::new())
            }
            OperandKind::Address => {
                if args.is_empty() {
                    errors.push(err(ParseErrorKind::MissingOperand(op)));
                    continue;
                }
                match parse_addr(operand_text) {
                    Some(a) => Instr::with_addr(op, a, String::new()),
                    None => {
                        let looks_multi = args.len() > 1 && !operand_text.starts_with('[');
                        errors.push(err(if looks_multi {
                            ParseErrorKind::TooManyOperands(op)
                        } else {
                            ParseErrorKind::BadAddress(operand_text.chars().take(24).collect())
                        }));
                        continue;
                    }
                }
            }
            OperandKind::Label => {
                if args.is_empty() {
                    errors.push(err(ParseErrorKind::MissingOperand(op)));
                    continue;
                }
                if args.len() > 1 {
                    errors.push(err(ParseErrorKind::TooManyOperands(op)));
                    continue;
                }
                if !is_ident(args[0]) {
                    errors.push(err(ParseErrorKind::BadLabel(args[0].chars().take(24).collect())));
                    continue;
                }
                Instr::with_addr(op, Addr::Direct(0), args[0].to_string())
            }
        };
        lines.push(Line {
            kind: LineKind::Instr(instr),
            comment: pending_comment.take(),
            src,
        });
    }
    if errors.is_empty() {
        Ok(Listing { lines })
    } else {
        Err(errors)
    }
}

impl Listing {
    /// Canonical text form. Re-parsing the output yields an equal listing (modulo `src`).
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for l in &self.lines {
            match &l.kind {
                LineKind::Label(n) => {
                    out.push_str(n);
                    out.push(':');
                    if let Some(c) = &l.comment {
                        out.push_str(" -- ");
                        out.push_str(c);
                    }
                }
                LineKind::Instr(i) => {
                    let s = i.to_string();
                    out.push_str("    ");
                    out.push_str(&s);
                    if let Some(c) = &l.comment {
                        for _ in s.len()..16 {
                            out.push(' ');
                        }
                        out.push_str(" -- ");
                        out.push_str(c);
                    }
                }
                LineKind::Blank => {
                    if let Some(c) = &l.comment {
                        out.push_str("-- ");
                        out.push_str(c);
                    }
                }
            }
            out.push('\n');
        }
        out
    }

    /// Number of instruction lines (the "size" score).
    pub fn instruction_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| matches!(l.kind, LineKind::Instr(_)))
            .count()
    }

    /// Resolve labels and produce an executable program.
    pub fn compile(&self) -> Result<Program, Vec<CompileError>> {
        let mut errors = Vec::new();
        let mut label_at: HashMap<&str, usize> = HashMap::new();
        let mut labels_by_index: Vec<Vec<String>> = vec![Vec::new()];
        let mut count = 0usize;
        for l in &self.lines {
            match &l.kind {
                LineKind::Label(n) => {
                    if label_at.insert(n.as_str(), count).is_some() {
                        errors.push(CompileError {
                            line: l.src,
                            kind: CompileErrorKind::DuplicateLabel(n.clone()),
                        });
                    }
                    labels_by_index[count].push(n.clone());
                }
                LineKind::Instr(_) => {
                    count += 1;
                    labels_by_index.push(Vec::new());
                }
                LineKind::Blank => {}
            }
        }
        let mut instrs = Vec::with_capacity(count);
        let mut listing_idx = Vec::with_capacity(count);
        let mut text_line = Vec::with_capacity(count);
        for (li, l) in self.lines.iter().enumerate() {
            if let LineKind::Instr(i) = &l.kind {
                let mapped = match i.target() {
                    Some(name) => match label_at.get(name.as_str()) {
                        Some(&t) => i.map_target(|_| t),
                        None => {
                            errors.push(CompileError {
                                line: l.src,
                                kind: CompileErrorKind::UnknownLabel(name.clone()),
                            });
                            i.map_target(|_| 0)
                        }
                    },
                    None => i.map_target(|_| 0),
                };
                instrs.push(mapped);
                listing_idx.push(li);
                text_line.push(l.src);
            }
        }
        if errors.is_empty() {
            Ok(Program {
                instrs,
                labels: labels_by_index,
                listing_idx,
                text_line,
            })
        } else {
            Err(errors)
        }
    }
}

/// Problems found while resolving labels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileErrorKind {
    /// The label is defined twice.
    DuplicateLabel(String),
    /// A jump refers to a label that does not exist.
    UnknownLabel(String),
}

/// A label problem with its line (0 if the listing was built in code).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileError {
    /// One-based text line or 0.
    pub line: usize,
    /// What went wrong.
    pub kind: CompileErrorKind,
}

impl Diagnostic for CompileError {
    fn key(&self) -> &'static str {
        match self.kind {
            CompileErrorKind::DuplicateLabel(_) => "compile.duplicate_label",
            CompileErrorKind::UnknownLabel(_) => "compile.unknown_label",
        }
    }
    fn args(&self) -> Vec<(&'static str, String)> {
        let (CompileErrorKind::DuplicateLabel(n) | CompileErrorKind::UnknownLabel(n)) = &self.kind;
        vec![("line", self.line.to_string()), ("text", n.clone())]
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message(english()))
    }
}

impl std::error::Error for CompileError {}

/// An executable program: instructions with resolved jump targets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    /// Instructions; jump targets are instruction indexes (`len()` means "end of program").
    pub instrs: Vec<Instr<usize>>,
    /// Label names attached to each instruction index (length `instrs.len() + 1`).
    pub labels: Vec<Vec<String>>,
    /// For each instruction, its index in the source [`Listing`].
    pub listing_idx: Vec<usize>,
    /// For each instruction, its one-based text line (0 if unknown).
    pub text_line: Vec<usize>,
}

/// A rule violation of a program against a level's machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProblemKind {
    /// The level does not offer this instruction.
    NotAllowed(Opcode),
    /// The level does not offer indirect addressing.
    IndirectNotAllowed,
    /// The slot does not exist on this floor.
    NoSuchTile(usize),
}

/// A problem tied to an instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// Instruction index.
    pub instr: usize,
    /// One-based text line (0 if unknown).
    pub line: usize,
    /// What is wrong.
    pub kind: ProblemKind,
}

impl Diagnostic for Problem {
    fn key(&self) -> &'static str {
        match self.kind {
            ProblemKind::NotAllowed(_) => "check.not_allowed",
            ProblemKind::IndirectNotAllowed => "check.indirect_not_allowed",
            ProblemKind::NoSuchTile(_) => "check.no_such_tile",
        }
    }
    fn args(&self) -> Vec<(&'static str, String)> {
        let mut v = vec![
            ("line", self.line.to_string()),
            ("instr", (self.instr + 1).to_string()),
        ];
        match &self.kind {
            ProblemKind::NotAllowed(o) => v.push(("op", o.mnemonic().to_string())),
            ProblemKind::IndirectNotAllowed => {}
            ProblemKind::NoSuchTile(n) => v.push(("slot", n.to_string())),
        }
        v
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message(english()))
    }
}

impl Program {
    /// Build a program straight from instructions (labels are invented as `L<index>`).
    /// Returns `None` if a jump target is out of range.
    pub fn from_instrs(instrs: Vec<Instr<usize>>) -> Option<Program> {
        let n = instrs.len();
        if instrs.iter().any(|i| i.target().is_some_and(|t| *t > n)) {
            return None;
        }
        let mut labels = vec![Vec::new(); n + 1];
        for t in instrs.iter().filter_map(|i| i.target().copied()) {
            if labels[t].is_empty() {
                labels[t].push(format!("L{t}"));
            }
        }
        Some(Program {
            listing_idx: (0..n).collect(),
            text_line: vec![0; n],
            instrs,
            labels,
        })
    }

    /// Number of instructions (the "size" score).
    pub fn size(&self) -> usize {
        self.instrs.len()
    }

    /// Whether the program uses any indirect addressing.
    pub fn uses_indirect(&self) -> bool {
        self.instrs
            .iter()
            .any(|i| i.addr().is_some_and(Addr::is_indirect))
    }

    /// Check the program against a level's instruction set and floor size.
    pub fn check(&self, allowed: &Allowed, floor_size: usize) -> Vec<Problem> {
        let mut out = Vec::new();
        for (idx, i) in self.instrs.iter().enumerate() {
            let line = self.text_line[idx];
            let mut push = |kind| {
                out.push(Problem {
                    instr: idx,
                    line,
                    kind,
                })
            };
            if !allowed.allows(i.opcode()) {
                push(ProblemKind::NotAllowed(i.opcode()));
            }
            if let Some(a) = i.addr() {
                if a.is_indirect() && !allowed.allows_indirect() {
                    push(ProblemKind::IndirectNotAllowed);
                }
                if a.slot() >= floor_size {
                    push(ProblemKind::NoSuchTile(a.slot()));
                }
            }
        }
        out
    }

    /// Rebuild an editable listing (labels first, canonical).
    pub fn to_listing(&self) -> Listing {
        let mut lines = Vec::new();
        for (idx, i) in self.instrs.iter().enumerate() {
            for l in &self.labels[idx] {
                lines.push(Line::label(l.clone()));
            }
            let named = i.map_target(|t| {
                self.labels
                    .get(*t)
                    .and_then(|v| v.first().cloned())
                    .unwrap_or_else(|| format!("L{t}"))
            });
            lines.push(Line::instr(named));
        }
        for l in self.labels.last().into_iter().flatten() {
            lines.push(Line::label(l.clone()));
        }
        Listing { lines }
    }

    /// Canonical assembly text.
    pub fn to_text(&self) -> String {
        self.to_listing().to_text()
    }
}

/// Parse and compile in one go; the error is a printable list of messages.
pub fn assemble(text: &str) -> Result<Program, Vec<String>> {
    let listing = parse(text).map_err(|e| e.iter().map(ToString::to_string).collect::<Vec<_>>())?;
    listing
        .compile()
        .map_err(|e| e.iter().map(ToString::to_string).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_program() {
        let l = parse("start:\n  INBOX -- take\n  outbox\n  JUMP start\n").unwrap();
        assert_eq!(l.instruction_count(), 3);
        let p = l.compile().unwrap();
        assert_eq!(p.instrs[2], Instr::Jump(0));
        assert_eq!(p.labels[0], vec!["start".to_string()]);
    }

    #[test]
    fn label_and_instr_same_line() {
        let l = parse("a: b: INBOX").unwrap();
        assert_eq!(l.lines.len(), 3);
        let p = l.compile().unwrap();
        assert_eq!(p.labels[0].len(), 2);
    }

    #[test]
    fn indirect_and_comments() {
        let l = parse("COPYTO [ 3 ] # c\nADD 12 ; d\n// e\n").unwrap();
        assert_eq!(
            l.lines[0].kind,
            LineKind::Instr(Instr::CopyTo(Addr::Indirect(3)))
        );
        assert_eq!(l.lines[2].comment.as_deref(), Some("e"));
    }

    #[test]
    fn reports_all_errors() {
        let e = parse("FOO\nADD\nADD x\nINBOX 3\nJUMP\nJUMP 9\nADD 1 2\n1bad:\n").unwrap_err();
        assert_eq!(e.len(), 8);
        assert_eq!(e[0].line, 1);
        assert!(e[0].to_string().contains("FOO"));
    }

    #[test]
    fn compile_errors() {
        let e = parse("a:\na:\nJUMP nowhere").unwrap().compile().unwrap_err();
        assert_eq!(e.len(), 2);
    }

    #[test]
    fn text_roundtrip() {
        let src = "-- hello\nloop:\n    INBOX -- in\n    JUMPZ end\n    COPYTO [2]\n    JUMP loop\nend:\n";
        let l = parse(src).unwrap();
        let l2 = parse(&l.to_text()).unwrap();
        let strip = |l: &Listing| -> Vec<(LineKind, Option<String>)> {
            l.lines
                .iter()
                .map(|x| (x.kind.clone(), x.comment.clone()))
                .collect()
        };
        assert_eq!(strip(&l), strip(&l2));
    }

    #[test]
    fn program_listing_roundtrip() {
        let p = assemble("a:\nINBOX\nJUMPZ b\nJUMP a\nb:\nOUTBOX").unwrap();
        let p2 = p.to_listing().compile().unwrap();
        assert_eq!(p.instrs, p2.instrs);
        assert_eq!(p.labels, p2.labels);
    }

    #[test]
    fn check_rules() {
        let p = assemble("INBOX\nMUL 9\nCOPYTO [1]").unwrap();
        let probs = p.check(&Allowed::basic(), 4);
        assert_eq!(probs.len(), 3);
    }

    #[test]
    fn from_instrs_rejects_bad_target() {
        assert!(Program::from_instrs(vec![Instr::Jump(5)]).is_none());
        assert!(Program::from_instrs(vec![Instr::Jump(1)]).is_some());
    }
}

// SPDX-License-Identifier: Apache-2.0
//! One play session: the program (text is the source of truth), undo history, the compiled form with
//! diagnostics, the recorded run that the office scene replays, and the verdict.

use crate::blocks;
use crate::textedit::TextEdit;
use algoclerk_core::judge::{Judge, Verdict};
use algoclerk_core::lang::ast::{Program, Ty};
use algoclerk_core::lang::check::{Allowed, CheckOpts};
use algoclerk_core::lang::diag::Diag;
use algoclerk_core::lang::interp::{Limits, RtError};
use algoclerk_core::lang::run::{Compiled, compile};
use algoclerk_core::lang::value::{NodeData, Value};
use algoclerk_core::level::Level;
use std::sync::Arc;

/// One recorded moment of a run.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Statement that just ran (0 for the very first frame).
    pub node: u32,
    /// Cumulative abstract cost.
    pub cost: u64,
    /// Name, type and value of every declared local.
    pub vars: Vec<(String, Ty, Value)>,
    /// The value being returned.
    pub returned: Option<Value>,
}

/// A recorded run on one test case.
#[derive(Debug, Clone)]
pub struct RunView {
    /// Which case of the suite.
    pub case: usize,
    /// Moments, first to last.
    pub frames: Vec<Frame>,
    /// Current position.
    pub pos: usize,
    /// Return value text or the runtime error.
    pub result: Result<String, RtError>,
    /// Total cost.
    pub cost: u64,
    /// Node arena for list and tree widgets.
    pub nodes: Vec<NodeData>,
    /// Expected output text.
    pub expected: String,
    /// More events happened than were recorded.
    pub truncated: bool,
    /// Return type.
    pub ret: Ty,
}

/// The program being written.
pub struct Session {
    /// The level (None for the sandbox).
    pub level: Option<Level>,
    /// The judge for the level.
    pub judge: Option<Arc<Judge>>,
    /// Text editor state (source of truth).
    pub edit: TextEdit,
    last_text: String,
    undo: Vec<String>,
    redo: Vec<String>,
    /// Latest compile result.
    pub compiled: Result<Compiled, Vec<Diag>>,
    /// The last program that compiled (the block view keeps showing it while the text is broken).
    pub good: Option<Program>,
    /// Current run.
    pub run: Option<RunView>,
    /// Latest verdict from Check.
    pub verdict: Option<Verdict>,
    /// Chosen case for visualisation.
    pub case: usize,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("level", &self.level.as_ref().map(|l| &l.id))
            .finish_non_exhaustive()
    }
}

const SANDBOX: &str = "fn main(nums: vec<int>) -> int {\n    let total = 0;\n    for x in nums {\n        total += x;\n    }\n    return total;\n}\n";

impl Session {
    /// A session for a level, starting from a saved draft or the starter program.
    pub fn for_level(level: &Level, draft: &str) -> Result<Session, String> {
        let judge = Judge::new(level)?;
        let text = if draft.trim().is_empty() {
            level.starter()
        } else {
            draft.to_string()
        };
        let mut s = Session::raw(Some(level.clone()), Some(Arc::new(judge)), &text);
        s.recompile();
        Ok(s)
    }

    /// The sandbox.
    pub fn sandbox(saved: &str) -> Session {
        let mut s = Session::raw(
            None,
            None,
            if saved.trim().is_empty() {
                SANDBOX
            } else {
                saved
            },
        );
        s.recompile();
        s
    }

    fn raw(level: Option<Level>, judge: Option<Arc<Judge>>, text: &str) -> Session {
        Session {
            level,
            judge,
            edit: TextEdit::new(text, true),
            last_text: text.to_string(),
            undo: Vec::new(),
            redo: Vec::new(),
            compiled: Err(Vec::new()),
            good: None,
            run: None,
            verdict: None,
            case: 0,
        }
    }

    /// The program text.
    pub fn text(&self) -> String {
        self.edit.text()
    }

    fn opts(&self) -> CheckOpts {
        match &self.level {
            Some(l) => l.check_opts(),
            None => CheckOpts {
                allowed: Allowed::all(),
                entry: None,
            },
        }
    }

    /// Re-check after the text changed.
    pub fn recompile(&mut self) {
        let text = self.edit.text();
        self.compiled = compile(&text, &self.opts());
        if let Ok(c) = &self.compiled {
            self.good = Some(c.program.clone());
        } else if let Ok(p) = algoclerk_core::lang::parse::parse(&text) {
            self.good = Some(p);
        }
        self.last_text = text;
    }

    /// Call every tick: notices edits made through the text editor.
    pub fn sync(&mut self) -> bool {
        let now = self.edit.text();
        if now != self.last_text {
            self.undo.push(std::mem::take(&mut self.last_text));
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.recompile();
            self.run = None;
            self.verdict = None;
            return true;
        }
        false
    }

    /// Replace the whole text (block edits, undo, reset).
    pub fn set_text(&mut self, text: &str) {
        self.edit.set_text(text);
        self.sync();
    }

    /// Undo the last change.
    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.edit.text());
        self.edit.set_text(&prev);
        self.last_text = prev;
        self.recompile();
        self.run = None;
        true
    }

    /// Redo.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.edit.text());
        self.edit.set_text(&next);
        self.last_text = next;
        self.recompile();
        self.run = None;
        true
    }

    /// Can undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Can redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Apply a block edit that yields new text.
    pub fn apply(&mut self, text: Option<String>) -> bool {
        match text {
            Some(t) => {
                self.set_text(&t);
                true
            }
            None => false,
        }
    }

    /// Number of cases that can be visualised (examples).
    pub fn case_count(&self) -> usize {
        self.judge
            .as_ref()
            .map_or(1, |j| j.suite.iter().filter(|c| c.example).count().max(1))
    }

    /// The entry function name.
    pub fn entry_name(&self) -> String {
        match &self.level {
            Some(l) => l.signature.name.clone(),
            None => self
                .good
                .as_ref()
                .and_then(|p| p.fns.last().map(|f| f.name.clone()))
                .unwrap_or_default(),
        }
    }

    /// Argument literals for the chosen case.
    pub fn case_args(&self) -> Vec<String> {
        match &self.judge {
            Some(j) => j
                .suite
                .iter()
                .filter(|c| c.example)
                .nth(self.case)
                .map(|c| c.args.clone())
                .unwrap_or_default(),
            None => vec!["[3, 1, 4, 1, 5]".to_string()],
        }
    }

    /// Record a run of the chosen case. Returns an error message when the program does not compile.
    pub fn start_run(&mut self) -> Result<(), String> {
        let c = match &self.compiled {
            Ok(c) => c,
            Err(d) => {
                return Err(d
                    .first()
                    .map(|d| d.msg.clone())
                    .unwrap_or_else(|| "The page is empty.".into()));
            }
        };
        let entry = self.entry_name();
        let args = self.case_args();
        let limits = Limits {
            steps: 200_000,
            trace_events: 4000,
            ..Limits::default()
        };
        let called = c.call(&entry, &args, &limits)?;
        let (vals, nodes0) = c.parse_args(&entry, &args)?;
        let f = c.program.find(&entry).ok_or("missing function")?;
        let fidx = c
            .program
            .fns
            .iter()
            .position(|x| x.name == entry)
            .unwrap_or(0);
        let mut frames = Vec::new();
        // first frame: the arguments sitting on the inbox
        frames.push(Frame {
            node: 0,
            cost: 0,
            vars: f
                .params
                .iter()
                .zip(vals.iter())
                .map(|(p, v)| (p.name.clone(), p.ty.clone(), v.clone()))
                .collect(),
            returned: None,
        });
        let mut truncated = false;
        if let Some(tr) = &called.report.trace {
            truncated = tr.truncated;
            for e in &tr.events {
                let locals = &c.info.fns[e.func as usize].locals;
                let vars = locals
                    .iter()
                    .zip(e.vars.iter())
                    .filter(|(_, v)| **v != Value::Unit)
                    .map(|((n, t), v)| (n.clone(), t.clone(), v.clone()))
                    .collect();
                frames.push(Frame {
                    node: e.node,
                    cost: e.cost,
                    vars,
                    returned: e.returned.clone(),
                });
            }
        }
        let _ = (fidx, nodes0);
        let expected = self
            .judge
            .as_ref()
            .and_then(|j| {
                j.suite
                    .iter()
                    .filter(|c| c.example)
                    .nth(self.case)
                    .map(|c| c.expect.clone())
            })
            .unwrap_or_default();
        self.run = Some(RunView {
            case: self.case,
            frames,
            pos: 0,
            result: called
                .report
                .result
                .map(|_| called.output.unwrap_or_default()),
            cost: called.report.cost,
            nodes: called.report.nodes,
            expected,
            truncated,
            ret: f.ret.clone(),
        });
        Ok(())
    }

    /// Run the judge on the current program (the Check button).
    pub fn check(&mut self) -> Option<&Verdict> {
        let j = self.judge.clone()?;
        let text = self.edit.text();
        self.verdict = Some(j.judge(&text));
        self.verdict.as_ref()
    }

    /// Statement being executed in the current frame.
    pub fn current_node(&self) -> Option<u32> {
        self.run
            .as_ref()
            .and_then(|r| r.frames.get(r.pos))
            .map(|f| f.node)
            .filter(|n| *n != 0)
    }

    /// Block rows of the best known program.
    pub fn rows(&self) -> Vec<blocks::Row> {
        self.good.as_ref().map(blocks::rows).unwrap_or_default()
    }

    /// One-based line and column range of a diagnostic in the text.
    pub fn diag_span(&self, d: &Diag) -> Option<(usize, usize, usize)> {
        let sp = d.span.or_else(|| {
            d.node
                .and_then(|n| self.compiled_program().and_then(|p| p.span(n)))
        })?;
        let text = self.edit.text();
        let (l, c) = algoclerk_core::lang::diag::line_col(&text, sp.start);
        let (l2, c2) = algoclerk_core::lang::diag::line_col(&text, sp.end);
        Some((l, c - 1, if l2 == l { c2 - 1 } else { c + 3 }))
    }

    fn compiled_program(&self) -> Option<&Program> {
        match &self.compiled {
            Ok(c) => Some(&c.program),
            Err(_) => None,
        }
    }

    /// The first problem with the program, if any.
    pub fn problem(&self) -> Option<Diag> {
        match &self.compiled {
            Ok(_) => None,
            Err(d) => d.first().cloned(),
        }
    }

    /// Span of the problem, resolving node ids through a fresh parse when needed.
    pub fn problem_span(&self) -> Option<(usize, usize, usize)> {
        let d = self.problem()?;
        if d.span.is_some() {
            return self.diag_span(&d);
        }
        let p = algoclerk_core::lang::parse::parse(&self.edit.text()).ok()?;
        let sp = d.node.and_then(|n| p.span(n))?;
        let text = self.edit.text();
        let (l, c) = algoclerk_core::lang::diag::line_col(&text, sp.start);
        let (l2, c2) = algoclerk_core::lang::diag::line_col(&text, sp.end);
        Some((l, c - 1, if l2 == l { c2 - 1 } else { c + 3 }))
    }
}

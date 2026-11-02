// SPDX-License-Identifier: Apache-2.0
//! One play session: the program (text is the source of truth), undo history, the compiled form with
//! diagnostics, the recorded run that the office scene replays, and the verdict.
//!
//! The text held by the session is the player's part only (own includes, helpers and the function). The
//! level's prelude (includes and structs) is read-only and is prepended whenever the text is compiled.

use crate::blocks;
use crate::textedit::TextEdit;
use algoclerk_core::cpp::ast::{ItemKind, Program, Ty};
use algoclerk_core::cpp::check::CheckOpts;
use algoclerk_core::cpp::diag::{Diag, line_col};
use algoclerk_core::cpp::interp::{Limits, RtError};
use algoclerk_core::cpp::parse::{ParseOpts, known_types, parse};
use algoclerk_core::cpp::run::{Compiled, compile};
use algoclerk_core::cpp::value::{Heap, Plain};
use algoclerk_core::judge::{Judge, Verdict};
use algoclerk_core::level::Level;
use std::sync::Arc;

/// One recorded moment of a run.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Statement that just ran (0 for the very first frame).
    pub node: u32,
    /// Cumulative abstract cost.
    pub cost: u64,
    /// Call depth of the running function (0 for the entry function).
    pub depth: u32,
    /// Name, C++ type and value of every visible local.
    pub vars: Vec<(String, Ty, Plain)>,
    /// The value being returned.
    pub returned: Option<Plain>,
    /// Struct instances the pointers of this frame refer to (list and tree nodes).
    pub heap: Heap,
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
    /// The last program that parsed (the block view keeps showing it while the text is broken).
    pub good: Option<Program>,
    /// `good` was parsed from the current text (so its node ids and spans match the editor).
    pub good_fresh: bool,
    /// The last program that compiled without problems (type information for autocomplete).
    pub last_ok: Option<Compiled>,
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

const SANDBOX: &str = "#include <vector>\n\nint sumAll(const std::vector<int>& nums) {\n    int total = 0;\n    for (int x : nums) {\n        total += x;\n    }\n    return total;\n}\n";

impl Session {
    /// A session for a level, starting from a saved draft or the starter program.
    pub fn for_level(level: &Level, draft: &str) -> Result<Session, String> {
        let judge = Judge::new(level)?;
        let text = if draft.trim().is_empty() {
            level.starter_code()
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
            good_fresh: false,
            last_ok: None,
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
            None => CheckOpts::all(),
        }
    }

    /// The read-only prelude text of the level (empty in the sandbox).
    pub fn prelude(&self) -> &str {
        self.level.as_ref().map_or("", |l| l.prelude.as_str())
    }

    /// Re-check after the text changed.
    pub fn recompile(&mut self) {
        let text = self.edit.text();
        self.compiled = compile(self.prelude(), &text, &self.opts());
        self.good_fresh = true;
        if let Ok(c) = &self.compiled {
            self.good = Some(c.program.clone());
            self.last_ok = Some(c.clone());
        } else if let Ok(p) = parse_with_prelude(self.prelude(), &text) {
            self.good = Some(p);
        } else {
            self.good_fresh = false;
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
                .and_then(|p| {
                    p.player_items().iter().rev().find_map(|i| match &i.kind {
                        ItemKind::Func(f) if f.body.is_some() => Some(f.name.clone()),
                        _ => None,
                    })
                })
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
            None => vec!["{3, 1, 4, 1, 5}".to_string()],
        }
    }

    /// Record a run of the chosen case. Returns an error message when the program does not compile.
    ///
    /// # Errors
    /// The first problem with the program, or why the test arguments cannot be passed.
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
        let (vals, heap0) = c.parse_args(&entry, &args)?;
        let f = c.program.find(&entry).ok_or("missing function")?;
        let mut frames = Vec::new();
        // first frame: the arguments sitting on the inbox
        frames.push(Frame {
            node: 0,
            cost: 0,
            depth: 0,
            vars: f
                .params
                .iter()
                .zip(vals.iter())
                .map(|(p, v)| (p.name.clone(), p.spec.ty.clone(), v.clone()))
                .collect(),
            returned: None,
            heap: heap0,
        });
        let mut truncated = false;
        let report = called.report;
        if let Some(tr) = report.trace {
            truncated = tr.truncated;
            for e in tr.events {
                frames.push(Frame {
                    node: e.node,
                    cost: e.cost,
                    depth: e.depth,
                    vars: e
                        .vars
                        .into_iter()
                        .map(|v| (v.name, v.ty, v.value))
                        .collect(),
                    returned: e.returned,
                    heap: e.heap,
                });
            }
        }
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
        let ret = f.ret.ty.clone();
        self.run = Some(RunView {
            case: self.case,
            frames,
            pos: 0,
            result: report.result.map(|_| called.output.unwrap_or_default()),
            cost: report.cost,
            expected,
            truncated,
            ret,
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

    /// One-based line and zero-based column range of a byte span in the text.
    fn line_range(&self, start: u32, end: u32) -> (usize, usize, usize) {
        let text = self.edit.text();
        let (l, c) = line_col(&text, start);
        let (l2, c2) = line_col(&text, end);
        let to = if l2 == l {
            c2 - 1
        } else {
            text.split('\n')
                .nth(l - 1)
                .map_or(c + 3, |t| t.chars().count())
        };
        (l, c - 1, to.max(c))
    }

    /// The text span of a node of the current program, as `(line, col_from, col_to)`.
    pub fn node_span(&self, node: u32) -> Option<(usize, usize, usize)> {
        let p = match &self.compiled {
            Ok(c) => &c.program,
            Err(_) if self.good_fresh => self.good.as_ref()?,
            Err(_) => return None,
        };
        let sp = p.span(node)?;
        Some(self.line_range(sp.start, sp.end))
    }

    /// One-based line of a statement node.
    pub fn node_line(&self, node: u32) -> Option<usize> {
        self.node_span(node).map(|s| s.0)
    }

    /// One-based line and column range of a diagnostic in the text.
    pub fn diag_span(&self, d: &Diag) -> Option<(usize, usize, usize)> {
        match (d.span, d.node) {
            (Some(sp), _) => Some(self.line_range(sp.start, sp.end)),
            (None, Some(n)) => self.node_span(n),
            _ => None,
        }
    }

    /// The first problem with the program, if any.
    pub fn problem(&self) -> Option<Diag> {
        match &self.compiled {
            Ok(_) => None,
            Err(d) => d.first().cloned(),
        }
    }

    /// Span of the problem, resolving node ids through the freshly parsed program when needed.
    pub fn problem_span(&self) -> Option<(usize, usize, usize)> {
        self.diag_span(&self.problem()?)
    }

    /// Span of the statement the recorded run stopped at (a runtime error), if the run failed.
    pub fn run_error_span(&self) -> Option<(usize, usize, usize)> {
        let e = self.run.as_ref()?.result.as_ref().err()?;
        self.node_span(e.node)
    }

    /// Text of the first problem, in the clerk's words (a compile problem or a runtime error of the run).
    pub fn problem_message(&self) -> Option<String> {
        self.problem().map(|d| d.msg).or_else(|| {
            self.run
                .as_ref()?
                .result
                .as_ref()
                .err()
                .map(|e| e.msg.clone())
        })
    }
}

/// Parse the player's text knowing the prelude's structs and aliases, without checking it. The result
/// holds the prelude items first (`prelude_items`) and has spans for the player's nodes only.
///
/// # Errors
/// The first parse problem, with a span into `player`.
pub fn parse_with_prelude(prelude: &str, player: &str) -> Result<Program, Vec<Diag>> {
    let mut pre = parse(prelude, &ParseOpts::default()).unwrap_or_default();
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
    for sp in spans.iter_mut().take(blank) {
        *sp = Default::default();
    }
    pre.items.append(&mut pl.items);
    Ok(Program {
        items: pre.items,
        prelude_items,
        next_id: pl.next_id.max(pre.next_id),
        spans,
    })
}

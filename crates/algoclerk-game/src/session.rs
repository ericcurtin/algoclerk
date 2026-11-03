// SPDX-License-Identifier: Apache-2.0
//! One play session: the program (text is the source of truth), undo history, the compiled form with
//! diagnostics, the recorded run that the office scene replays, and the verdict.
//!
//! The text held by the session is the player's part only (own includes, helpers and the function). The
//! level's prelude (includes and structs) is read-only and is prepended whenever the text is compiled.

use crate::blocks;
use crate::textedit::TextEdit;
use algoclerk_core::cpp::ast::{ItemKind, Program, Ty, walk_stmts};
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
    /// Run or Check was pressed since the text last changed (an untouched starter shows its problem only then).
    pub pressed: bool,
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
        let fresh = draft.trim().is_empty();
        let text = if fresh {
            // the cursor waits on an indented empty line between the braces
            level.starter_code().replacen("{\n}", "{\n    \n}", 1)
        } else {
            draft.to_string()
        };
        let mut s = Session::raw(Some(level.clone()), Some(Arc::new(judge)), &text);
        if fresh && let Some(at) = text.find("{\n    \n}") {
            s.edit.cursor = text[..at].chars().count() + 6;
        }
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
        let mut edit = TextEdit::new(text, true);
        edit.smart = true;
        Session {
            level,
            judge,
            edit,
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
            pressed: false,
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
            self.pressed = false;
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
        self.pressed = true;
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
                    // the core reports the local that a `return` hands out as a `void` variable: skip those
                    vars: e
                        .vars
                        .into_iter()
                        .filter(|v| !(v.ty == Ty::Void && v.value == Plain::Void))
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
        self.pressed = true;
        let text = self.edit.text();
        self.verdict = Some(j.judge(&text));
        self.verdict.as_ref()
    }

    /// Statement being executed in the current frame. At the end of a run that stopped with an error it is
    /// the statement holding the failing expression.
    pub fn current_node(&self) -> Option<u32> {
        let r = self.run.as_ref()?;
        if r.pos + 1 >= r.frames.len()
            && let Err(e) = &r.result
        {
            return self.statement_of(e.node);
        }
        r.frames.get(r.pos).map(|f| f.node).filter(|n| *n != 0)
    }

    /// The innermost statement of the compiled program whose text contains the node (a statement is its
    /// own statement).
    pub fn statement_of(&self, node: u32) -> Option<u32> {
        let c = self.compiled.as_ref().ok()?;
        let target = c.program.span(node)?;
        let mut best: Option<(u32, u32)> = None;
        for it in c.program.player_items() {
            let ItemKind::Func(f) = &it.kind else {
                continue;
            };
            let Some(body) = &f.body else { continue };
            walk_stmts(body, &mut |st| {
                if let Some(sp) = c.program.span(st.id)
                    && sp.start <= target.start
                    && sp.end >= target.end
                    && best.is_none_or(|(_, len)| sp.end - sp.start < len)
                {
                    best = Some((st.id, sp.end - sp.start));
                }
            });
        }
        best.map(|(id, _)| id)
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

    /// The entry function has no statements yet and the only thing wrong is the missing `return`: an
    /// untouched (or emptied) starter. Nothing is reported as an error until Run or Check is pressed.
    pub fn nudge(&self) -> bool {
        if self.pressed {
            return false;
        }
        let Err(d) = &self.compiled else {
            return false;
        };
        if d.first().map(|d| d.code) != Some("flow.missing-return") {
            return false;
        }
        let entry = self.entry_name();
        self.good
            .as_ref()
            .and_then(|p| p.find(&entry))
            .and_then(|f| f.body.as_ref())
            .is_some_and(|b| b.stmts.is_empty())
    }

    /// The only problem is the missing `return` and Run was not pressed yet: the player is probably still
    /// building the answer, so it is shown as a friendly heads-up and not as a red error.
    pub fn soft_problem(&self) -> bool {
        !self.pressed
            && !self.nudge()
            && self
                .raw_problem()
                .is_some_and(|d| d.code == "flow.missing-return")
    }

    /// The first problem with the program, if any (none while the starter is merely untouched).
    pub fn problem(&self) -> Option<Diag> {
        if self.nudge() {
            return None;
        }
        self.raw_problem()
    }

    /// The first compile problem, even for an untouched starter.
    pub fn raw_problem(&self) -> Option<Diag> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use algoclerk_core::cpp::value::render;
    use algoclerk_levels::Campaign;

    fn level(id: &str) -> Level {
        Campaign::shipped().find(id).expect("level").clone()
    }

    #[test]
    fn the_starter_is_the_signature_with_an_empty_body() {
        let l = level("two-sum");
        let s = Session::for_level(&l, "").unwrap();
        assert_eq!(
            s.text().trim_end(),
            l.starter_code()
                .replacen("{\n}", "{\n    \n}", 1)
                .trim_end()
        );
        assert_eq!(
            s.edit.line_col(s.edit.cursor),
            (1, 4),
            "the cursor waits inside the braces"
        );
        // the only thing wrong with the empty body is the missing `return`, which is not shown yet
        assert_eq!(s.raw_problem().map(|d| d.code), Some("flow.missing-return"));
        assert!(s.nudge() && s.problem().is_none());
        assert_eq!(s.prelude(), l.prelude);
        assert_eq!(s.entry_name(), "twoSum");
    }

    #[test]
    fn a_run_records_frames_with_cpp_types_and_values() {
        let l = level("two-sum");
        let mut s = Session::for_level(&l, &l.reference).unwrap();
        s.start_run().unwrap();
        let r = s.run.as_ref().unwrap();
        assert_eq!(r.result.as_deref().ok(), Some("{0, 1}"));
        assert_eq!(r.expected, "{0, 1}");
        // first frame: the arguments, with their C++ types
        let first = &r.frames[0];
        assert_eq!(first.vars[0].0, "nums");
        assert_eq!(first.vars[0].1.to_string(), "std::vector<int>");
        assert_eq!(
            render(&first.vars[0].2, &first.vars[0].1, &first.heap),
            "{2, 7, 11, 15}"
        );
        // later frames show the map with its C++ type
        let last = r.frames.last().unwrap();
        assert!(last.returned.is_some());
        assert!(
            last.vars
                .iter()
                .any(|(n, t, _)| n == "seen" && t.to_string() == "std::unordered_map<int, int>")
        );
        assert!(r.frames.iter().skip(1).all(|f| f.node != 0));
    }

    #[test]
    fn list_arguments_travel_with_their_heap() {
        let l = level("reverse-list");
        let mut s = Session::for_level(&l, &l.reference).unwrap();
        s.start_run().unwrap();
        let r = s.run.as_ref().unwrap();
        let (name, ty, v) = &r.frames[0].vars[0];
        assert_eq!(name, "head");
        assert_eq!(ty.to_string(), "ListNode*");
        assert_eq!(render(v, ty, &r.frames[0].heap), "{1, 2, 3}");
    }

    #[test]
    fn compile_problems_point_at_the_line_and_columns() {
        let l = level("sum-the-bills");
        let text = "int sumTheBills(const std::vector<int>& bills) {\n    int total = 0;\n    for (int b : bills) {\n        total += \"1\";\n    }\n    return total;\n}\n";
        let s = Session::for_level(&l, text).unwrap();
        let d = s.problem().expect("a type problem");
        assert!(d.msg.contains("`+=`"), "{}", d.msg);
        let (line, from, to) = s.problem_span().expect("span");
        assert_eq!(line, 4);
        assert!(from >= 8 && to > from && to <= 24, "{from}..{to}");
        // a parse problem carries its own span
        let s = Session::for_level(
            &l,
            "int sumTheBills(const std::vector<int>& bills) {\n    int total = ;\n}\n",
        )
        .unwrap();
        assert_eq!(s.problem_span().map(|x| x.0), Some(2));
    }

    #[test]
    fn a_runtime_error_highlights_its_statement() {
        let l = level("sum-the-bills");
        let text = "int sumTheBills(const std::vector<int>& bills) {\n    int total = 0;\n    for (std::size_t i = 0; i <= bills.size(); ++i) {\n        total += bills[i];\n    }\n    return total;\n}\n";
        let mut s = Session::for_level(&l, text).unwrap();
        assert!(s.problem().is_none());
        s.start_run().unwrap();
        let r = s.run.as_mut().unwrap();
        assert!(r.result.is_err());
        r.pos = r.frames.len() - 1;
        assert_eq!(s.run_error_span().map(|x| x.0), Some(4));
        let node = s.current_node().expect("the failing statement");
        assert_eq!(s.node_line(node), Some(4));
        assert!(s.problem_message().unwrap().contains("out of range"));
    }

    #[test]
    fn the_sandbox_runs_its_own_function() {
        let mut s = Session::sandbox("");
        assert_eq!(s.entry_name(), "sumAll");
        s.start_run().unwrap();
        assert_eq!(s.run.as_ref().unwrap().result.as_deref().ok(), Some("14"));
    }

    #[test]
    fn undo_and_redo_follow_the_text() {
        let l = level("two-sum");
        let mut s = Session::for_level(&l, "").unwrap();
        let start = s.text();
        s.set_text("int x;\n");
        assert!(s.can_undo() && s.undo());
        assert_eq!(s.text(), start);
        assert!(s.redo());
        assert_eq!(s.text(), "int x;\n");
    }

    #[test]
    fn an_untouched_starter_is_a_nudge_not_an_error_until_run_is_pressed() {
        let l = level("stamp-duty");
        let mut s = Session::for_level(&l, "").unwrap();
        assert!(s.nudge());
        assert!(
            s.problem().is_none() && s.problem_span().is_none() && s.problem_message().is_none()
        );
        // the checker still knows about the missing return
        assert_eq!(s.raw_problem().map(|d| d.code), Some("flow.missing-return"));
        // pressing Run reports it in the clerk's words
        let msg = s.start_run().unwrap_err();
        assert!(msg.contains("must give back"), "{msg}");
        assert!(!s.nudge());
        assert_eq!(s.problem().map(|d| d.msg), Some(msg));
        // Check works on the empty body too, and says the same thing
        let mut s = Session::for_level(&l, "").unwrap();
        let v = s.check().unwrap().clone();
        assert!(
            !v.passed
                && v.diags
                    .first()
                    .is_some_and(|d| d.msg.contains("must give back"))
        );
        assert!(s.problem().is_some());
        // typing anything (even a comment-only edit) re-arms nothing: a real statement shows real errors
        let t = format!(
            "{}\n",
            l.starter_code().replace("{\n", "{\n    int x = 0;\n")
        );
        s.set_text(&t);
        assert!(!s.nudge() && s.problem().is_some());
        // emptying the body again brings the nudge back
        s.set_text(&l.starter_code());
        assert!(s.nudge() && s.problem().is_none());
    }

    #[test]
    fn a_wrong_but_not_empty_body_is_reported_at_once() {
        let l = level("stamp-duty");
        let text = l.starter_code().replacen("{", "{\n    int x = 1;", 1);
        let s = Session::for_level(&l, &text).unwrap();
        assert!(!s.nudge());
        assert_eq!(s.problem().map(|d| d.code), Some("flow.missing-return"));
    }
}

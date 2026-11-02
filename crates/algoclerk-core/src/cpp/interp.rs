// SPDX-License-Identifier: Apache-2.0
//! Interpreter with the cost model (STUB: replaced by the INTERP work package).

use super::ast::*;
use super::value::{Heap, Plain};

/// Iteration order of unordered containers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnorderedOrder {
    /// Insertion order.
    Insertion,
    /// Reverse insertion order.
    Reverse,
}

/// Resource limits for one run.
#[derive(Clone, Debug)]
pub struct Limits {
    /// Abstract cost budget.
    pub steps: u64,
    /// Maximum call depth.
    pub depth: u32,
    /// Maximum live elements.
    pub cells: i64,
    /// Sample memory for the space estimate.
    pub measure: bool,
    /// Record up to this many trace events.
    pub trace_events: usize,
    /// Order of unordered containers.
    pub unordered: UnorderedOrder,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            steps: 5_000_000,
            depth: 200,
            cells: 4_000_000,
            measure: false,
            trace_events: 0,
            unordered: UnorderedOrder::Insertion,
        }
    }
}

/// What stopped a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RtKind {
    /// Cost budget exhausted.
    Time,
    /// Too many live elements.
    Memory,
    /// Too many nested calls.
    Depth,
    /// Division by zero.
    DivZero,
    /// Signed overflow.
    Overflow,
    /// Index out of range.
    Index,
    /// Bad shift.
    Shift,
    /// Null dereference.
    Null,
    /// Dangling pointer or use after delete.
    Dangling,
    /// Uninitialised read.
    Uninit,
    /// Iterator or reference invalidated.
    Invalidated,
    /// Fell off the end of a non-void function.
    MissingReturn,
    /// Empty container access.
    Empty,
    /// Invalid comparator.
    BadCompare,
    /// Other invalid argument.
    BadArg,
    /// Interpreter invariant broken.
    Internal,
}

/// A runtime error worded for the speech bubble.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RtError {
    /// Category.
    pub kind: RtKind,
    /// Statement or expression being executed.
    pub node: NodeId,
    /// Message.
    pub msg: String,
}

/// A variable in a trace snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct VarSnap {
    /// Name.
    pub name: String,
    /// Declared type.
    pub ty: Ty,
    /// Value.
    pub value: Plain,
}

/// One recorded step.
#[derive(Clone, Debug)]
pub struct Event {
    /// Statement that ran.
    pub node: NodeId,
    /// Function index.
    pub func: u32,
    /// Call depth.
    pub depth: u32,
    /// Cumulative cost.
    pub cost: u64,
    /// Visible variables.
    pub vars: Vec<VarSnap>,
    /// Returned value for `return`.
    pub returned: Option<Plain>,
    /// Struct instances reachable from the variables (for drawing lists and trees).
    pub heap: Heap,
}

/// A recorded run.
#[derive(Clone, Debug, Default)]
pub struct Trace {
    /// Events.
    pub events: Vec<Event>,
    /// More happened than was recorded.
    pub truncated: bool,
}

/// Everything a run produced.
#[derive(Clone, Debug)]
pub struct RunReport {
    /// Result or error.
    pub result: Result<Plain, RtError>,
    /// Abstract cost.
    pub cost: u64,
    /// Peak live cells.
    pub peak_cells: u64,
    /// Cells of the inputs.
    pub input_cells: u64,
    /// Trace if requested.
    pub trace: Option<Trace>,
    /// Heap the result's pointers refer to.
    pub heap: Heap,
}

/// Run `entry` with the given arguments.
pub fn run(
    _p: &Program,
    _entry: &str,
    _args: Vec<Plain>,
    _heap: Heap,
    _limits: &Limits,
) -> RunReport {
    RunReport {
        result: Err(RtError {
            kind: RtKind::Internal,
            node: 0,
            msg: "interpreter not written yet".into(),
        }),
        cost: 0,
        peak_cells: 0,
        input_cells: 0,
        trace: None,
        heap: Heap::default(),
    }
}

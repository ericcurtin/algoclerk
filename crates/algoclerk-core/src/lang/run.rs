// SPDX-License-Identifier: Apache-2.0
//! Convenience entry points: compile text, run it on literal arguments, render the result.

use super::ast::{Program, Ty};
use super::check::{CheckOpts, TypeInfo, check};
use super::diag::Diag;
use super::interp::{Limits, RunReport, run};
use super::parse::parse;
use super::value::{NodeData, Value, parse_value, render};

/// A parsed and checked program.
#[derive(Clone, Debug)]
pub struct Compiled {
    /// Syntax tree.
    pub program: Program,
    /// Type information.
    pub info: TypeInfo,
}

/// Parse and check program text.
pub fn compile(src: &str, opts: &CheckOpts) -> Result<Compiled, Vec<Diag>> {
    let program = parse(src).map_err(|d| vec![d])?;
    let info = check(&program, opts)?;
    Ok(Compiled { program, info })
}

/// The result of running a compiled function on literal text arguments.
#[derive(Clone, Debug)]
pub struct Called {
    /// The raw report.
    pub report: RunReport,
    /// Rendered return value (when it succeeded).
    pub output: Option<String>,
    /// Return type.
    pub ret: Ty,
}

impl Compiled {
    /// Parse the argument literals for `entry` using its declared parameter types.
    pub fn parse_args(
        &self,
        entry: &str,
        args: &[String],
    ) -> Result<(Vec<Value>, Vec<NodeData>), String> {
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
        let mut nodes = vec![NodeData::default()];
        let mut vals = Vec::new();
        for (p, a) in f.params.iter().zip(args) {
            vals.push(
                parse_value(a, &p.ty, &mut nodes)
                    .map_err(|e| format!("argument {}: {e}", p.name))?,
            );
        }
        Ok((vals, nodes))
    }

    /// Run `entry` on literal text arguments.
    pub fn call(&self, entry: &str, args: &[String], limits: &Limits) -> Result<Called, String> {
        let (vals, nodes) = self.parse_args(entry, args)?;
        let ret = self
            .program
            .find(entry)
            .map(|f| f.ret.clone())
            .unwrap_or(Ty::Unit);
        let report = run(&self.program, &self.info, entry, vals, nodes, limits);
        let output = report
            .result
            .as_ref()
            .ok()
            .map(|v| render(v, &ret, &report.nodes));
        Ok(Called {
            report,
            output,
            ret,
        })
    }
}

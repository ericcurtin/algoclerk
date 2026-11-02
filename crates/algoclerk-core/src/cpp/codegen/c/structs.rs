// SPDX-License-Identifier: Apache-2.0
//! Structs (`ListNode`, `TreeNode`, player structs): plain C structs with `make` (by value) and `new`
//! (on the heap) constructors translated from the C++ constructor.

use super::{Cx, Rk};
use crate::cpp::ast::*;
use crate::cpp::codegen::common::*;
use std::fmt::Write as _;

impl Cx<'_> {
    /// Remember that a struct is used (its definition goes after the container types).
    pub(super) fn use_struct(&mut self, name: &str) -> R<()> {
        if !self.used_structs.iter().any(|s| s == name) {
            let Some(s) = self.prog.find_struct(name) else {
                return no("C", format!("the unknown type {name}"));
            };
            if s.ctors.len() > 1 {
                return no("C", format!("{name} with several constructors"));
            }
            self.used_structs.push(name.to_string());
        }
        Ok(())
    }

    pub(super) fn struct_text(&mut self, name: &str) -> R<String> {
        let Some(s) = self.prog.find_struct(name).cloned() else {
            return no("C", format!("the unknown type {name}"));
        };
        let mut o = format!("\nstruct {name} {{\n");
        for f in &s.fields {
            let t = self.ctype(&f.spec.ty)?;
            let sp = if t.ends_with('*') { "" } else { " " };
            let _ = writeln!(o, "    {t}{sp}{};", f.name);
        }
        o.push_str("};\n\n");
        let mut params = Vec::new();
        let mut inits = Vec::new();
        self.push_scope();
        if let Some(c) = s.ctors.first() {
            if !c.body.stmts.is_empty() {
                return no("C", format!("the constructor body of {name}"));
            }
            for p in &c.params {
                let (d, rk) = self.param(&p.spec, &p.name)?;
                params.push(d);
                self.declare(&p.name, p.spec.ty.clone(), rk);
            }
            for f in &s.fields {
                let given = c.inits.iter().find(|(n, _)| n == &f.name);
                let v = match (given, &f.default) {
                    (Some((_, args)), _) if args.len() == 1 => {
                        self.conv_val(&args[0], &f.spec.ty)?.s
                    }
                    (Some(_), _) => return no("C", "a field initialised with several values"),
                    (None, Some(d)) => self.conv_val(d, &f.spec.ty)?.s,
                    (None, None) => self.zero_of(&f.spec.ty)?,
                };
                inits.push((f.name.clone(), v));
            }
        } else {
            for f in &s.fields {
                let t = self.ctype(&f.spec.ty)?;
                params.push(format!("{t} {}", f.name));
                inits.push((f.name.clone(), f.name.clone()));
            }
        }
        self.pop_scope();
        let ps = if params.is_empty() {
            "void".to_string()
        } else {
            params.join(", ")
        };
        let args: Vec<String> = s.ctors.first().map_or_else(
            || s.fields.iter().map(|f| f.name.clone()).collect(),
            |c| c.params.iter().map(|p| self.name(&p.name)).collect(),
        );
        let _ = writeln!(o, "static {name} {name}_make({ps}) {{\n    {name} s;");
        for (f, v) in &inits {
            let _ = writeln!(o, "    s.{f} = {v};");
        }
        o.push_str("    return s;\n}\n\n");
        let _ = writeln!(
            o,
            "static {name} *{name}_new({ps}) {{\n    {name} *p = malloc(sizeof *p);\n    *p = {name}_make({});\n    return p;\n}}",
            args.join(", ")
        );
        Ok(o)
    }

    /// The zero value of a type (what a default-constructed object holds).
    pub(super) fn zero_of(&mut self, t: &Ty) -> R<String> {
        Ok(match t {
            Ty::Ptr(_) => "NULL".into(),
            t if t.is_integral() => "0".into(),
            t => format!("{}_zero()", self.name_of(t)?),
        })
    }

    pub(super) fn struct_params(&self, name: &str) -> Option<Vec<Ty>> {
        let s = self.prog.find_struct(name)?;
        Some(match s.ctors.first() {
            Some(c) => c.params.iter().map(|p| p.spec.ty.clone()).collect(),
            None => s.fields.iter().map(|f| f.spec.ty.clone()).collect(),
        })
    }

    #[allow(dead_code)]
    pub(super) fn is_val(&self, n: &str) -> bool {
        self.lookup(n).is_some_and(|v| v.rk == Rk::Val)
    }
}

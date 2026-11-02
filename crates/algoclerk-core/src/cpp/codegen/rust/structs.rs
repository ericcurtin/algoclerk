// SPDX-License-Identifier: Apache-2.0
//! Structs with pointers (`ListNode`, `TreeNode`, player structs of `Copy` fields): a small arena and a
//! `Copy` handle with accessors, written into the prelude. Object identity and aliasing are preserved
//! (two handles to the same node see each other's writes), as with C++ pointers.

use super::{Rk, Rs};
use crate::cpp::ast::*;
use crate::cpp::codegen::common::*;
use std::fmt::Write as _;

impl Rs<'_> {
    /// Remember that a struct is used (its arena is written into the prelude).
    pub(super) fn use_struct(&mut self, name: &str) -> R<()> {
        if !self.used_structs.iter().any(|s| s == name) {
            let Some(s) = self.prog.find_struct(name) else {
                return no("Rust", format!("the unknown type {name}"));
            };
            if s.ctors.len() > 1 {
                return no("Rust", format!("{name} with several constructors"));
            }
            self.used_structs.push(name.to_string());
        }
        Ok(())
    }

    /// Prelude text of one struct: handle type, arena, constructor and field accessors.
    pub(super) fn struct_text(&mut self, name: &str) -> R<String> {
        let Some(s) = self.prog.find_struct(name).cloned() else {
            return no("Rust", format!("the unknown type {name}"));
        };
        let up = name.to_uppercase();
        let mut fields = Vec::new();
        for f in &s.fields {
            if !self.is_copy(&f.spec.ty) || f.spec.ty == Ty::Str {
                return no("Rust", format!("the field `{}` of {name}", f.name));
            }
            fields.push((f.name.clone(), self.rty(&f.spec.ty)?, f.spec.ty.clone()));
        }
        let mut o = String::new();
        let _ = writeln!(
            o,
            "// `{name}*` is a handle to a node in a small arena, so nodes can alias and form cycles."
        );
        let _ = writeln!(
            o,
            "#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]\nstruct {name}(Option<usize>);\n"
        );
        let _ = writeln!(o, "struct {name}Data {{");
        for (n, t, _) in &fields {
            let _ = writeln!(o, "    {n}: {t},");
        }
        let _ = writeln!(o, "}}\n");
        let _ = writeln!(
            o,
            "thread_local! {{\n    static {up}_ARENA: RefCell<Vec<{name}Data>> = RefCell::new(Vec::new());\n}}\n"
        );
        let _ = writeln!(
            o,
            "impl {name} {{\n    const NULL: {name} = {name}(None);\n"
        );
        let (params, vals) = self.ctor_parts(&s)?;
        let _ = writeln!(o, "    fn new({params}) -> {name} {{");
        let _ = writeln!(o, "        {up}_ARENA.with(|a| {{");
        let _ = writeln!(o, "            let mut a = a.borrow_mut();");
        let _ = writeln!(o, "            a.push({name}Data {{ {vals} }});");
        let _ = writeln!(
            o,
            "            {name}(Some(a.len() - 1))\n        }})\n    }}"
        );
        for (n, t, _) in &fields {
            let _ = writeln!(
                o,
                "\n    fn {n}(self) -> {t} {{\n        {up}_ARENA.with(|a| a.borrow()[self.0.unwrap()].{n})\n    }}"
            );
            let _ = writeln!(
                o,
                "\n    fn set_{n}(self, x: {t}) {{\n        {up}_ARENA.with(|a| a.borrow_mut()[self.0.unwrap()].{n} = x);\n    }}"
            );
        }
        o.push_str("}\n");
        Ok(o)
    }

    /// Parameter list and field initialisers of the constructor.
    fn ctor_parts(&mut self, s: &StructDecl) -> R<(String, String)> {
        let mut params = Vec::new();
        let mut vals = Vec::new();
        self.push_scope();
        if let Some(c) = s.ctors.first() {
            if !c.body.stmts.is_empty() {
                return no("Rust", format!("the constructor body of {}", s.name));
            }
            for p in &c.params {
                let t = self.rty(&p.spec.ty)?;
                params.push(format!("{}: {t}", self.name(&p.name)));
                self.declare(&p.name, p.spec.ty.clone(), Rk::Val);
            }
            for f in &s.fields {
                let given = c.inits.iter().find(|(n, _)| n == &f.name);
                let v = match (given, &f.default) {
                    (Some((_, args)), _) if args.len() == 1 => {
                        self.conv_val(&args[0], &f.spec.ty)?.s
                    }
                    (Some(_), _) => return no("Rust", "a field initialised with several values"),
                    (None, Some(d)) => self.conv_val(d, &f.spec.ty)?.s,
                    (None, None) => self.default_val(&f.spec.ty)?,
                };
                vals.push(format!("{}: {v}", f.name));
            }
        } else {
            for f in &s.fields {
                let t = self.rty(&f.spec.ty)?;
                params.push(format!("{}: {t}", f.name));
                vals.push(f.name.clone());
            }
        }
        self.pop_scope();
        Ok((params.join(", "), vals.join(", ")))
    }
}

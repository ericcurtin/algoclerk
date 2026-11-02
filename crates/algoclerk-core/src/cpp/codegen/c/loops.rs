// SPDX-License-Identifier: Apache-2.0
//! C translation of `for`, range-`for` and `switch`.

use super::stmt::pad;
use super::{Cx, Rk};
use crate::cpp::ast::*;
use crate::cpp::codegen::common::*;

impl Cx<'_> {
    pub(super) fn for_loop(
        &mut self,
        init: Option<&Stmt>,
        cond: Option<&Expr>,
        incr: Option<&Expr>,
        body: &Stmt,
        ind: usize,
        out: &mut String,
    ) -> R<()> {
        let p = pad(ind);
        self.push_scope();
        let mut inline = String::new();
        let mut pre = false;
        match init.map(|s| &s.kind) {
            None => {}
            Some(StmtKind::Expr(e)) => inline = self.ex(e, true)?.s,
            Some(StmtKind::Decl { spec, vars })
                if !spec.is_ref
                    && vars.iter().all(|v| {
                        let t = self.info.decl_ty(v.id);
                        v.array.is_none() && (t.is_integral() || matches!(t, Ty::Ptr(_)))
                    }) =>
            {
                let mut parts = Vec::new();
                for (i, v) in vars.iter().enumerate() {
                    let d = self.decl(spec, v)?;
                    let d = d.trim_end_matches(';').to_string();
                    if i == 0 {
                        parts.push(d);
                    } else {
                        let init = d.split_once(" = ").map(|x| x.1.to_string());
                        parts.push(match init {
                            Some(i) => format!("{} = {i}", self.name(&v.name)),
                            None => self.name(&v.name),
                        });
                    }
                }
                inline = parts.join(", ");
            }
            Some(_) => pre = true,
        }
        if pre {
            out.push_str(&format!("{p}{{\n"));
            if let Some(i) = init {
                self.stmt(i, ind + 1, out)?;
            }
        }
        let q = if pre { pad(ind + 1) } else { p.clone() };
        let c = match cond {
            Some(c) => self.cond(c)?.s,
            None => String::new(),
        };
        let inc = match incr {
            Some(e) => self.ex(e, true)?.s,
            None => String::new(),
        };
        let b = self.body(body, ind + if pre { 2 } else { 1 })?;
        out.push_str(&format!("{q}for ({inline}; {c}; {inc}) {{\n{b}{q}}}\n"));
        if pre {
            out.push_str(&format!("{p}}}\n"));
        }
        self.pop_scope();
        Ok(())
    }

    /// The declaration binding `name` to an element reachable as `place` (an lvalue text).
    fn bind(&mut self, name: &str, ty: &Ty, place: &str, spec: &TypeSpec) -> R<(String, Rk)> {
        let ct = self.ctype(ty)?;
        let sp = if ct.ends_with('*') { "" } else { " " };
        let n = self.name(name);
        if spec.is_ref && !(spec.konst && self.is_plain(ty)) {
            return Ok((format!("{ct}{sp}*{n} = &{place};"), Rk::Ref));
        }
        if self.is_plain(ty) {
            return Ok((format!("{ct}{sp}{n} = {place};"), Rk::Val));
        }
        let t = self.name_of(ty)?;
        Ok((format!("{ct}{sp}{n} = {t}_copy({place});"), Rk::Val))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn range_for(
        &mut self,
        id: NodeId,
        spec: &TypeSpec,
        names: &[String],
        structured: bool,
        range: &Expr,
        body: &Stmt,
        ind: usize,
        out: &mut String,
    ) -> R<()> {
        let p = pad(ind);
        let q = pad(ind + 1);
        let cty = self.ty_of(range);
        let binds: Vec<(String, Ty)> = self.info.bindings(id).to_vec();
        let mutable = spec.is_ref && !spec.konst;
        let x = self.ex(range, mutable)?;
        let i = self.fresh("i");
        let head;
        let mut decls: Vec<String> = Vec::new();
        let mut declared: Vec<(String, Ty, Rk)> = Vec::new();
        let mut guard = String::new();
        match &cty {
            Ty::Str | Ty::StrView => {
                if mutable || structured {
                    return no("C", "this loop over a string");
                }
                head = format!(
                    "for (size_t {i} = 0; {i} < {}; ++{i}) {{",
                    self.dot(&x, "len")
                );
                let place = format!("{}[{i}]", self.dot(&x, "data"));
                let (d, rk) = self.bind(&names[0], &Ty::Char, &place, spec)?;
                decls.push(d);
                declared.push((names[0].clone(), Ty::Char, rk));
            }
            Ty::Vector(el)
            | Ty::Array(el, _)
            | Ty::Stack(el)
            | Ty::Deque(el)
            | Ty::Queue(el)
            | Ty::List(el) => {
                let deq = matches!(cty, Ty::Deque(_) | Ty::Queue(_) | Ty::List(_));
                head = format!(
                    "for (size_t {i} = 0; {i} < {}; ++{i}) {{",
                    self.dot(&x, "len")
                );
                let place = if deq {
                    let n = self.name_of(&cty)?;
                    let ptr = self.addr(x.clone(), &cty)?;
                    format!("(*{n}_at({ptr}, {i}))")
                } else {
                    format!("{}[{i}]", self.dot(&x, "data"))
                };
                self.elem_binds(
                    structured,
                    names,
                    &binds,
                    el,
                    &place,
                    spec,
                    &mut decls,
                    &mut declared,
                )?;
            }
            Ty::Map(..) | Ty::UnorderedMap(..) | Ty::Set(_) | Ty::UnorderedSet(_) => {
                let (k, v) = match &cty {
                    Ty::Map(k, v) | Ty::UnorderedMap(k, v) => (k.clone(), v.clone()),
                    Ty::Set(k) | Ty::UnorderedSet(k) => (k.clone(), k.clone()),
                    _ => return no("C", "this loop"),
                };
                let ordered = matches!(cty, Ty::Map(..) | Ty::Set(_));
                let is_map = matches!(cty, Ty::Map(..) | Ty::UnorderedMap(..));
                if ordered {
                    head = format!(
                        "for (size_t {i} = 0; {i} < {}; ++{i}) {{",
                        self.dot(&x, "len")
                    );
                } else {
                    head = format!(
                        "for (size_t {i} = 0; {i} < {}; ++{i}) {{",
                        self.dot(&x, "cap")
                    );
                    guard = format!(
                        "{}if ({}[{i}] != 1) {{\n{}continue;\n{}}}\n",
                        pad(ind + 1),
                        self.dot(&x, "st"),
                        pad(ind + 2),
                        pad(ind + 1)
                    );
                }
                let kp = format!("{}[{i}]", self.dot(&x, "keys"));
                let vp = format!("{}[{i}]", self.dot(&x, "vals"));
                let kspec = TypeSpec {
                    konst: true,
                    ..spec.clone()
                };
                if !is_map {
                    let (d, rk) = self.bind(&names[0], &k, &kp, &kspec)?;
                    decls.push(d);
                    declared.push((names[0].clone(), (*k).clone(), rk));
                } else if structured {
                    let (d, rk) = self.bind(&names[0], &k, &kp, &kspec)?;
                    decls.push(d);
                    declared.push((names[0].clone(), (*k).clone(), rk));
                    let (d, rk) = self.bind(&names[1], &v, &vp, spec)?;
                    decls.push(d);
                    declared.push((names[1].clone(), (*v).clone(), rk));
                } else {
                    if mutable {
                        return no("C", "a mutable reference to a map entry");
                    }
                    let pt = Ty::Pair(k, v);
                    let pn = self.name_of(&pt)?;
                    let n = self.name(&names[0]);
                    decls.push(format!("{pn} {n} = {{{kp}, {vp}}};"));
                    declared.push((names[0].clone(), pt, Rk::Val));
                }
            }
            t => return no("C", format!("a range-for over {t}")),
        }
        self.push_scope();
        for (n, t, rk) in declared {
            self.declare(&n, t, rk);
        }
        let b = self.body(body, ind + 1)?;
        self.pop_scope();
        let ds: String = decls
            .iter()
            .map(|d| format!("{}{d}\n", pad(ind + 1)))
            .collect();
        let _ = &q;
        out.push_str(&format!("{p}{head}\n{guard}{ds}{b}{p}}}\n"));
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn elem_binds(
        &mut self,
        structured: bool,
        names: &[String],
        binds: &[(String, Ty)],
        el: &Ty,
        place: &str,
        spec: &TypeSpec,
        decls: &mut Vec<String>,
        declared: &mut Vec<(String, Ty, Rk)>,
    ) -> R<()> {
        if !structured {
            let (d, rk) = self.bind(&names[0], el, place, spec)?;
            decls.push(d);
            declared.push((names[0].clone(), el.clone(), rk));
            return Ok(());
        }
        let fields: Vec<String> = match el {
            Ty::Pair(..) => vec!["first".into(), "second".into()],
            Ty::Tuple(ts) => (0..ts.len()).map(|i| format!("f{i}")).collect(),
            t => return no("C", format!("a structured binding of {t}")),
        };
        for ((nm, (_, t)), f) in names.iter().zip(binds).zip(&fields) {
            let (d, rk) = self.bind(nm, t, &format!("{place}.{f}"), spec)?;
            decls.push(d);
            declared.push((nm.clone(), t.clone(), rk));
        }
        Ok(())
    }

    pub(super) fn switch(
        &mut self,
        cond: &Expr,
        body: &Block,
        ind: usize,
        out: &mut String,
    ) -> R<()> {
        let p = pad(ind);
        let ct = self.ty_of(cond);
        let c = self.ex(cond, false)?;
        let c = self.own(c, &ct)?;
        out.push_str(&format!("{p}switch ({}) {{\n", c.s));
        self.push_scope();
        let stmts = &body.stmts;
        let mut i = 0;
        while i < stmts.len() {
            let mut labels = String::new();
            while i < stmts.len() {
                match &stmts[i].kind {
                    StmtKind::Case(None) => labels.push_str(&format!("{}default:\n", pad(ind + 1))),
                    StmtKind::Case(Some(e)) => {
                        let v = self.conv_val(e, &ct)?;
                        labels.push_str(&format!("{}case {}:\n", pad(ind + 1), v.s));
                    }
                    _ => break,
                }
                i += 1;
            }
            let start = i;
            while i < stmts.len() && !matches!(stmts[i].kind, StmtKind::Case(_)) {
                i += 1;
            }
            let group = &stmts[start..i];
            out.push_str(&labels);
            let braces = group
                .iter()
                .any(|s| matches!(s.kind, StmtKind::Decl { .. } | StmtKind::Structured { .. }));
            if braces {
                out.push_str(&format!("{}{{\n", pad(ind + 2)));
            }
            self.stmts(group, ind + 2 + usize::from(braces), out)?;
            if braces {
                out.push_str(&format!("{}}}\n", pad(ind + 2)));
            }
        }
        self.pop_scope();
        out.push_str(&format!("{p}}}\n"));
        Ok(())
    }
}

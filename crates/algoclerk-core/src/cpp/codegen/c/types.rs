// SPDX-License-Identifier: Apache-2.0
//! C types: every container, pair, tuple and optional the program uses is instantiated once from the
//! templates of the prelude, in dependency order, with a readable name (`Vec_int`, `Hmap_Str_Vec_int`).

use super::Cx;
use super::tmpl_base::*;
use super::tmpl_more::*;
use crate::cpp::ast::*;
use crate::cpp::codegen::common::*;
use std::fmt::Write as _;

impl Cx<'_> {
    /// The mangled name of a type, usable as an identifier fragment; registers it (and what it contains).
    pub fn name_of(&mut self, t: &Ty) -> R<String> {
        let n = match t {
            Ty::Bool => "bool".into(),
            Ty::Char => "char".into(),
            Ty::Int => "int".into(),
            Ty::UInt => "uint".into(),
            Ty::LongLong => "ll".into(),
            Ty::ULongLong => "ull".into(),
            Ty::Size => "size_t".into(),
            Ty::Str | Ty::StrView => "Str".into(),
            Ty::Vector(e) | Ty::Stack(e) | Ty::Array(e, _) => format!("Vec_{}", self.nested(e)?),
            Ty::Deque(e) | Ty::Queue(e) | Ty::List(e) => format!("Deq_{}", self.nested(e)?),
            Ty::PriorityQueue(e, cmp) => {
                let dir = match &**cmp {
                    Ty::Functor(Functor::Less, _) => "max",
                    Ty::Functor(Functor::Greater, _) => "min",
                    _ => return no("C", "a priority_queue ordered by a lambda"),
                };
                format!("Heap_{}_{dir}", self.nested(e)?)
            }
            Ty::UnorderedMap(k, v) => format!("Hmap_{}_{}", self.nested(k)?, self.nested(v)?),
            Ty::Map(k, v) => format!("Smap_{}_{}", self.nested(k)?, self.nested(v)?),
            Ty::UnorderedSet(e) => {
                self.nested(&Ty::Char)?;
                format!("Hset_{}", self.nested(e)?)
            }
            Ty::Set(e) => {
                self.nested(&Ty::Char)?;
                format!("Sset_{}", self.nested(e)?)
            }
            Ty::Pair(a, b) => format!("Pair_{}_{}", self.nested(a)?, self.nested(b)?),
            Ty::Tuple(ts) => {
                let parts: R<Vec<String>> = ts.iter().map(|t| self.nested(t)).collect();
                format!("Tup_{}", parts?.join("_"))
            }
            Ty::Optional(e) => format!("Opt_{}", self.nested(e)?),
            Ty::Ptr(inner) => match &**inner {
                Ty::Struct(s) => {
                    self.use_struct(s)?;
                    format!("{s}p")
                }
                _ => return no("C", "a pointer to a non-struct type"),
            },
            Ty::Struct(s) => {
                self.use_struct(s)?;
                return Ok(s.clone());
            }
            t => return no("C", format!("the type {t}")),
        };
        if !self.types.iter().any(|(_, m)| m == &n) {
            self.types.push((t.clone(), n.clone()));
        }
        Ok(n)
    }

    /// The name of a type stored inside another (a struct by value cannot be: it has no helper functions).
    fn nested(&mut self, t: &Ty) -> R<String> {
        if matches!(t, Ty::Struct(_)) {
            return no(
                "C",
                "a struct stored by value in a container, pair or optional",
            );
        }
        self.name_of(t)
    }

    /// The C spelling of a type in declarations.
    pub fn ctype(&mut self, t: &Ty) -> R<String> {
        match t {
            Ty::Void => Ok("void".into()),
            Ty::Ptr(inner) => match &**inner {
                Ty::Struct(s) => {
                    self.name_of(t)?;
                    Ok(format!("{s} *"))
                }
                _ => no("C", "a pointer to a non-struct type"),
            },
            Ty::NullPtr => Ok("void *".into()),
            t => self.name_of(t),
        }
    }

    /// Is the value a plain C scalar that is copied by assignment?
    pub fn is_plain(&self, t: &Ty) -> bool {
        t.is_integral() || matches!(t, Ty::Ptr(_) | Ty::NullPtr | Ty::StrView | Ty::Void)
    }

    /// Prelude text: headers, templates, every instantiated type, struct definitions.
    pub fn prelude_text(&mut self) -> R<String> {
        let mut structs = String::new();
        for s in self.used_structs.clone() {
            structs.push_str(&self.struct_text(&s)?);
        }
        let reg = self.types.clone();
        let mut inst = String::new();
        let mut used: Vec<&str> = Vec::new();
        for (t, n) in &reg {
            let (text, tmpl) = self.instantiate(t, n)?;
            if let Some(m) = tmpl
                && !used.contains(&m)
            {
                used.push(m);
            }
            inst.push_str(&text);
        }
        let mut o = String::from(HEADER);
        for s in &self.used_structs {
            let _ = writeln!(o, "\ntypedef struct {s} {s};");
        }
        if reg.iter().any(|(t, _)| matches!(t, Ty::Str | Ty::StrView)) {
            o.push_str(STR);
        }
        for m in ["VEC", "DEQ", "HEAP", "HMAP", "SMAP"] {
            if used.contains(&m) {
                o.push_str(match m {
                    "VEC" => VEC,
                    "DEQ" => DEQ,
                    "HEAP" => HEAP,
                    "HMAP" => HMAP,
                    _ => SMAP,
                });
            }
        }
        o.push_str("\n/* Types used by the program. */\n");
        o.push_str(&inst);
        o.push_str(&structs);
        Ok(o)
    }

    /// The instantiation text of one registered type and the template macro it needs.
    fn instantiate(&mut self, t: &Ty, n: &str) -> R<(String, Option<&'static str>)> {
        let nm = |this: &mut Self, t: &Ty| this.nested(t);
        Ok(match t {
            Ty::Bool | Ty::Char | Ty::Int | Ty::UInt | Ty::LongLong | Ty::ULongLong | Ty::Size => {
                (format!("SCALAR({n})\n"), None)
            }
            Ty::Str | Ty::StrView => (String::new(), None),
            Ty::Ptr(inner) => {
                let Ty::Struct(s) = &**inner else {
                    return no("C", "this pointer");
                };
                (format!("typedef {s} *{n};\nSCALAR({n})\n"), None)
            }
            Ty::Vector(e) | Ty::Stack(e) | Ty::Array(e, _) => {
                let e = nm(self, e)?;
                (format!("VEC({n}, {e}, {e})\n"), Some("VEC"))
            }
            Ty::Deque(e) | Ty::Queue(e) | Ty::List(e) => {
                let e = nm(self, e)?;
                (format!("DEQ({n}, {e}, {e})\n"), Some("DEQ"))
            }
            Ty::PriorityQueue(e, cmp) => {
                let e = nm(self, e)?;
                let first = if matches!(&**cmp, Ty::Functor(Functor::Greater, _)) {
                    "MIN_FIRST"
                } else {
                    "MAX_FIRST"
                };
                (format!("HEAP({n}, {e}, {e}, {first})\n"), Some("HEAP"))
            }
            Ty::UnorderedMap(k, v) | Ty::Map(k, v) => {
                let (k, v) = (nm(self, k)?, nm(self, v)?);
                let m = if matches!(t, Ty::Map(..)) {
                    "SMAP"
                } else {
                    "HMAP"
                };
                (
                    format!("{m}({n}, {k}, {k}, {v}, {v})\n"),
                    Some(if m == "SMAP" { "SMAP" } else { "HMAP" }),
                )
            }
            Ty::UnorderedSet(e) | Ty::Set(e) => {
                let e = nm(self, e)?;
                let m = if matches!(t, Ty::Set(_)) {
                    "SMAP"
                } else {
                    "HMAP"
                };
                (
                    format!("{m}({n}, {e}, {e}, char, char)\n"),
                    Some(if m == "SMAP" { "SMAP" } else { "HMAP" }),
                )
            }
            Ty::Pair(a, b) => {
                let (a, b) = (nm(self, a)?, nm(self, b)?);
                (glue(n, &[("first", &a), ("second", &b)]), None)
            }
            Ty::Tuple(ts) => {
                let mut fields = Vec::new();
                for (i, t) in ts.iter().enumerate() {
                    fields.push((format!("f{i}"), nm(self, t)?));
                }
                let refs: Vec<(&str, &str)> = fields
                    .iter()
                    .map(|(f, t)| (f.as_str(), t.as_str()))
                    .collect();
                (glue(n, &refs), None)
            }
            Ty::Optional(e) => {
                let e = nm(self, e)?;
                (opt_glue(n, &e), None)
            }
            Ty::Struct(_) => (String::new(), None),
            t => return no("C", format!("the type {t}")),
        })
    }
}

/// Struct with the helper functions of a pair or tuple (fields compared in order).
fn glue(n: &str, fields: &[(&str, &str)]) -> String {
    let decl: Vec<String> = fields.iter().map(|(f, t)| format!("{t} {f};")).collect();
    let mut o = format!("typedef struct {{ {} }} {n};\n", decl.join(" "));
    let each = |f: &dyn Fn(&str, &str) -> String, sep: &str| {
        fields
            .iter()
            .map(|(a, b)| f(a, b))
            .collect::<Vec<_>>()
            .join(sep)
    };
    let _ = writeln!(
        o,
        "static {n} {n}_copy({n} p) {{ {n} r = {{{}}}; return r; }}",
        each(&|f, t| format!("{t}_copy(p.{f})"), ", ")
    );
    let _ = writeln!(
        o,
        "static {n} {n}_zero(void) {{ {n} r = {{{}}}; return r; }}",
        each(&|_, t| format!("{t}_zero()"), ", ")
    );
    let _ = writeln!(
        o,
        "static int {n}_eq({n} a, {n} b) {{ return {}; }}",
        each(&|f, t| format!("{t}_eq(a.{f}, b.{f})"), " && ")
    );
    let _ = writeln!(o, "static int {n}_lt({n} a, {n} b) {{");
    for (f, t) in fields {
        let _ = writeln!(
            o,
            "    if ({t}_lt(a.{f}, b.{f})) return 1;\n    if ({t}_lt(b.{f}, a.{f})) return 0;"
        );
    }
    o.push_str("    return 0;\n}\n");
    let _ = writeln!(
        o,
        "static size_t {n}_hash({n} p) {{ return {}; }}",
        each(&|f, t| format!("{t}_hash(p.{f})"), " * 31 + ")
    );
    o
}

fn opt_glue(n: &str, e: &str) -> String {
    let mut o = format!("typedef struct {{ bool has; {e} v; }} {n};\n");
    let _ = writeln!(
        o,
        "static {n} {n}_zero(void) {{ {n} r = {{false, {e}_zero()}}; return r; }}"
    );
    let _ = writeln!(
        o,
        "static {n} {n}_copy({n} o) {{ {n} r = {{o.has, o.has ? {e}_copy(o.v) : o.v}}; return r; }}"
    );
    let _ = writeln!(
        o,
        "static int {n}_eq({n} a, {n} b) {{ return a.has == b.has && (!a.has || {e}_eq(a.v, b.v)); }}"
    );
    let _ = writeln!(
        o,
        "static int {n}_lt({n} a, {n} b) {{ return !a.has ? b.has : (b.has && {e}_lt(a.v, b.v)); }}"
    );
    let _ = writeln!(
        o,
        "static size_t {n}_hash({n} o) {{ return o.has ? {e}_hash(o.v) + 1 : 0; }}"
    );
    o
}

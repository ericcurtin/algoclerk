// SPDX-License-Identifier: Apache-2.0
//! The C half of the compile-and-compare harness: builds the arguments of every test case, calls the
//! translated entry function and prints each result in the notation of [`crate::cpp::value`]. The `put`
//! functions are generated for exactly the types of the entry's signature (and what they contain).

use super::Generated;
use super::c::Cx;
use super::common::*;
use crate::cpp::ast::*;
use crate::cpp::run::Compiled;
use crate::cpp::value::{Heap, Plain, parse_value};
use std::fmt::Write as _;

fn c_text(s: &[u8]) -> String {
    let mut o = String::from("\"");
    for &c in s {
        match c {
            b'"' => o.push_str("\\\""),
            b'\\' => o.push_str("\\\\"),
            b'?' => o.push_str("\\?"),
            0x20..=0x7e => o.push(c as char),
            c => {
                let _ = write!(o, "\\{c:03o}");
            }
        }
    }
    o.push('"');
    o
}

fn list(xs: &[Plain], t: &Ty, h: &Heap, cx: &mut Cx<'_>) -> R<String> {
    let parts: R<Vec<String>> = xs.iter().map(|x| value(x, t, h, cx)).collect();
    Ok(parts?.join(", "))
}

/// A C expression of type `ty` holding `v`.
fn value(v: &Plain, ty: &Ty, h: &Heap, cx: &mut Cx<'_>) -> R<String> {
    let n = cx.name_of(ty)?;
    Ok(match (v, ty) {
        (Plain::Int(i), Ty::Bool) => (if *i != 0 { "true" } else { "false" }).to_string(),
        (Plain::Int(i), Ty::Char) => format!("(char){}", *i as i8),
        (Plain::Int(i), Ty::Int) if *i == i128::from(i32::MIN) => "(-2147483647 - 1)".into(),
        (Plain::Int(i), Ty::Int) => format!("{i}"),
        (Plain::Int(i), Ty::UInt) => format!("{i}u"),
        (Plain::Int(i), Ty::LongLong) if *i == i128::from(i64::MIN) => {
            "(-9223372036854775807LL - 1)".into()
        }
        (Plain::Int(i), Ty::LongLong) => format!("{i}LL"),
        (Plain::Int(i), Ty::ULongLong | Ty::Size) => format!("{i}ULL"),
        (Plain::Str(s), Ty::Str | Ty::StrView) => format!("Str_n({}, {})", c_text(s), s.len()),
        (Plain::Opt(None), Ty::Optional(_)) => format!("({n}){{false}}"),
        (Plain::Opt(Some(x)), Ty::Optional(t)) => format!("({n}){{true, {}}}", value(x, t, h, cx)?),
        (Plain::Seq(xs), Ty::Pair(a, b)) if xs.len() == 2 => format!(
            "({n}){{{}, {}}}",
            value(&xs[0], a, h, cx)?,
            value(&xs[1], b, h, cx)?
        ),
        (Plain::Seq(xs), Ty::Tuple(ts)) if xs.len() == ts.len() => {
            let parts: R<Vec<String>> =
                xs.iter().zip(ts).map(|(x, t)| value(x, t, h, cx)).collect();
            format!("({n}){{{}}}", parts?.join(", "))
        }
        (Plain::Seq(xs), Ty::Vector(t) | Ty::Stack(t) | Ty::Array(t, _)) => {
            if xs.is_empty() {
                format!("{n}_zero()")
            } else {
                let en = cx.name_of(t)?;
                format!(
                    "{n}_from(({en}[]){{{}}}, {})",
                    list(xs, t, h, cx)?,
                    xs.len()
                )
            }
        }
        (Plain::Ptr(0), Ty::Ptr(_)) => "NULL".into(),
        (Plain::Ptr(p), Ty::Ptr(inner)) => match &**inner {
            Ty::Struct(s) if s == "ListNode" => list_value(*p, h)?,
            Ty::Struct(s) if s == "TreeNode" => tree_value(*p, h)?,
            _ => return no("C", "this test value"),
        },
        _ => return no("C", format!("a test argument of type {ty}")),
    })
}

fn node_val(h: &Heap, n: u32) -> R<i128> {
    h.nodes
        .get(n as usize)
        .and_then(|f| f.first())
        .and_then(Plain::as_int)
        .ok_or_else(|| "bad node".to_string())
}

fn list_value(p: u32, h: &Heap) -> R<String> {
    let mut order: Vec<u32> = Vec::new();
    let mut cur = p;
    let mut cycle = -1i64;
    while cur != 0 && order.len() < 5_000_000 {
        if let Some(i) = order.iter().position(|&x| x == cur) {
            cycle = i as i64;
            break;
        }
        order.push(cur);
        cur = match h.nodes.get(cur as usize).and_then(|f| f.get(1)) {
            Some(Plain::Ptr(n)) => *n,
            _ => 0,
        };
    }
    let vals: R<Vec<String>> = order
        .iter()
        .map(|n| node_val(h, *n).map(|v| v.to_string()))
        .collect();
    Ok(format!(
        "ach_make_list((int[]){{{}}}, {}, {cycle})",
        vals?.join(", "),
        order.len()
    ))
}

fn tree_value(p: u32, h: &Heap) -> R<String> {
    let mut items: Vec<Option<i128>> = Vec::new();
    let mut q = std::collections::VecDeque::from([p]);
    while let Some(n) = q.pop_front() {
        if n == 0 {
            items.push(None);
            continue;
        }
        if items.len() > 5_000_000 {
            return Err("tree too large".into());
        }
        items.push(Some(node_val(h, n)?));
        for i in [1usize, 2] {
            q.push_back(match h.nodes.get(n as usize).and_then(|f| f.get(i)) {
                Some(Plain::Ptr(c)) => *c,
                _ => 0,
            });
        }
    }
    while items.last() == Some(&None) {
        items.pop();
    }
    if items.is_empty() {
        return Ok("NULL".into());
    }
    let vals: Vec<String> = items.iter().map(|i| i.unwrap_or(0).to_string()).collect();
    let present: Vec<String> = items
        .iter()
        .map(|i| if i.is_some() { "1" } else { "0" }.to_string())
        .collect();
    Ok(format!(
        "ach_make_tree((int[]){{{}}}, (unsigned char[]){{{}}}, {})",
        vals.join(", "),
        present.join(", "),
        items.len()
    ))
}

/// The `put` function of one registered type.
fn put_fn(ty: &Ty, n: &str, cx: &mut Cx<'_>) -> R<String> {
    let scalar = |fmt: &str| {
        format!(
            "static void {n}_put(Str *o, {n} v) {{ char b[40]; snprintf(b, sizeof b, \"{fmt}\", v); put_lit(o, b); }}\n"
        )
    };
    Ok(match ty {
        Ty::Bool => "static void bool_put(Str *o, bool v) { put_lit(o, v ? \"true\" : \"false\"); }\n".to_string(),
        Ty::Char => "static void char_put(Str *o, char v) { put_lit(o, \"'\"); put_esc(o, (unsigned char)v, '\\''); put_lit(o, \"'\"); }\n".to_string(),
        Ty::Int => scalar("%d"),
        Ty::UInt => scalar("%u"),
        Ty::LongLong => scalar("%lld"),
        Ty::ULongLong => scalar("%llu"),
        Ty::Size => "static void size_t_put(Str *o, size_t v) { char b[40]; snprintf(b, sizeof b, \"%zu\", v); put_lit(o, b); }\n".to_string(),
        Ty::Str | Ty::StrView => "static void Str_put(Str *o, Str v) { put_lit(o, \"\\\"\"); for (size_t i = 0; i < v.len; ++i) put_esc(o, (unsigned char)v.data[i], '\"'); put_lit(o, \"\\\"\"); }\n".to_string(),
        Ty::Vector(e) | Ty::Stack(e) | Ty::Array(e, _) => {
            let en = cx.name_of(e)?;
            format!("static void {n}_put(Str *o, {n} v) {{\n    put_lit(o, \"{{\");\n    for (size_t i = 0; i < v.len; ++i) {{\n        if (i) put_lit(o, \", \");\n        {en}_put(o, v.data[i]);\n    }}\n    put_lit(o, \"}}\");\n}}\n")
        }
        Ty::Deque(e) | Ty::Queue(e) | Ty::List(e) => {
            let en = cx.name_of(e)?;
            format!("static void {n}_put(Str *o, {n} v) {{\n    put_lit(o, \"{{\");\n    for (size_t i = 0; i < v.len; ++i) {{\n        if (i) put_lit(o, \", \");\n        {en}_put(o, *{n}_at(&v, i));\n    }}\n    put_lit(o, \"}}\");\n}}\n")
        }
        Ty::Map(k, v) | Ty::UnorderedMap(k, v) => {
            let (kn, vn) = (cx.name_of(k)?, cx.name_of(v)?);
            let hashed = matches!(ty, Ty::UnorderedMap(..));
            map_put(n, &kn, Some(&vn), hashed)
        }
        Ty::Set(k) | Ty::UnorderedSet(k) => {
            let kn = cx.name_of(k)?;
            map_put(n, &kn, None, matches!(ty, Ty::UnorderedSet(_)))
        }
        Ty::Pair(a, b) => {
            let (an, bn) = (cx.name_of(a)?, cx.name_of(b)?);
            format!("static void {n}_put(Str *o, {n} v) {{ put_lit(o, \"{{\"); {an}_put(o, v.first); put_lit(o, \", \"); {bn}_put(o, v.second); put_lit(o, \"}}\"); }}\n")
        }
        Ty::Tuple(ts) => {
            let mut body = String::new();
            for (i, t) in ts.iter().enumerate() {
                let tn = cx.name_of(t)?;
                let _ = write!(body, "{}{tn}_put(o, v.f{i}); ", if i > 0 { "put_lit(o, \", \"); " } else { "" });
            }
            format!("static void {n}_put(Str *o, {n} v) {{ put_lit(o, \"{{\"); {body}put_lit(o, \"}}\"); }}\n")
        }
        Ty::Optional(e) => {
            let en = cx.name_of(e)?;
            format!("static void {n}_put(Str *o, {n} v) {{ if (!v.has) {{ put_lit(o, \"nullopt\"); return; }} put_lit(o, \"{{\"); {en}_put(o, v.v); put_lit(o, \"}}\"); }}\n")
        }
        Ty::Ptr(inner) => match &**inner {
            Ty::Struct(s) if s == "ListNode" => LIST_PUT.to_string(),
            Ty::Struct(s) if s == "TreeNode" => TREE_PUT.to_string(),
            _ => return no("C", "printing this pointer"),
        },
        Ty::Struct(_) => String::new(),
        t => return no("C", format!("printing {t}")),
    })
}

/// `put` of a map or set: entries sorted by key (the order of unordered containers is unspecified).
fn map_put(n: &str, kn: &str, vn: Option<&str>, hashed: bool) -> String {
    let (limit, live) = if hashed {
        ("v.cap", "v.st[i] == 1")
    } else {
        ("v.len", "1")
    };
    let entry = match vn {
        Some(vn) => format!(
            "put_lit(o, \"{{\"); {kn}_put(o, v.keys[k]); put_lit(o, \", \"); {vn}_put(o, v.vals[k]); put_lit(o, \"}}\");"
        ),
        None => format!("{kn}_put(o, v.keys[k]);"),
    };
    format!(
        "static void {n}_put(Str *o, {n} v) {{
    size_t *idx = malloc((v.len + 1) * sizeof(size_t)), cnt = 0;
    for (size_t i = 0; i < {limit}; ++i) if ({live}) idx[cnt++] = i;
    for (size_t a = 1; a < cnt; ++a) {{
        size_t x = idx[a], b = a;
        while (b > 0 && {kn}_lt(v.keys[x], v.keys[idx[b - 1]])) {{ idx[b] = idx[b - 1]; --b; }}
        idx[b] = x;
    }}
    put_lit(o, \"{{\");
    for (size_t j = 0; j < cnt; ++j) {{
        size_t k = idx[j];
        if (j) put_lit(o, \", \");
        {entry}
    }}
    put_lit(o, \"}}\");
    free(idx);
}}
"
    )
}

/// A complete C program: the generated code, the support code and a `main` running every case.
pub fn program(c: &Compiled, entry: &str, g: &Generated, cases: &[Vec<String>]) -> R<String> {
    let Some(sig) = c.program.find(entry) else {
        return Err(format!("no function named {entry}"));
    };
    let mut cx = Cx::new(&c.program, &c.info);
    for t in &sig_types(sig) {
        cx.name_of(t)?;
    }
    let reg = cx.registry();
    let mut out = g.full();
    out.push_str("\n/* ---- generated by the AlgoClerk harness ---- */\n");
    if !g.prelude.contains("} Str;") {
        out.push_str(super::c::STR_TEMPLATE);
    }
    out.push_str(SUPPORT);
    if uses_struct(&sig_types(sig), "ListNode") {
        out.push_str(LIST_SUPPORT);
    }
    if uses_struct(&sig_types(sig), "TreeNode") {
        out.push_str(TREE_SUPPORT);
    }
    for (t, n) in &reg {
        out.push_str(&put_fn(t, n, &mut cx)?);
    }
    let ret_ct = cx.ctype(&sig.ret.ty)?;
    let ret_n = cx.name_of(&sig.ret.ty)?;
    for (ci, args) in cases.iter().enumerate() {
        if args.len() != sig.params.len() {
            return Err(format!(
                "{entry} takes {} arguments, the test has {}",
                sig.params.len(),
                args.len()
            ));
        }
        let mut heap = Heap::default();
        let mut body = String::new();
        let mut names = Vec::new();
        for (i, (p, lit)) in sig.params.iter().zip(args).enumerate() {
            let v = parse_value(lit, &p.spec.ty, &mut heap)
                .map_err(|e| format!("argument {}: {e}", p.name))?;
            let ct = cx.ctype(&p.spec.ty)?;
            let sp = if ct.ends_with('*') { "" } else { " " };
            let _ = writeln!(
                body,
                "    {ct}{sp}a{i} = {};",
                value(&v, &p.spec.ty, &heap, &mut cx)?
            );
            let plain = p.spec.ty.is_integral() || matches!(p.spec.ty, Ty::Ptr(_) | Ty::StrView);
            let by_ptr = p.spec.is_ref && !(p.spec.konst && plain);
            names.push(if by_ptr {
                format!("&a{i}")
            } else {
                format!("a{i}")
            });
        }
        let sp = if ret_ct.ends_with('*') { "" } else { " " };
        let _ = write!(
            out,
            "static void ach_case_{ci}(void) {{\n{body}    {ret_ct}{sp}r = {}({});\n    Str o = Str_from(\"\");\n    {ret_n}_put(&o, r);\n    puts(o.data);\n}}\n\n",
            super::c::fn_name(entry),
            names.join(", ")
        );
    }
    out.push_str("int main(void) {\n");
    for ci in 0..cases.len() {
        let _ = writeln!(out, "    ach_case_{ci}();");
    }
    out.push_str("    return 0;\n}\n");
    Ok(out)
}

const SUPPORT: &str = r#"
static void put_lit(Str *o, const char *s) { Str_cat_n(o, s, strlen(s)); }
static void put_esc(Str *o, unsigned char c, char quote) {
    char b[8];
    switch (c) {
    case '\n': put_lit(o, "\\n"); return;
    case '\t': put_lit(o, "\\t"); return;
    case '\r': put_lit(o, "\\r"); return;
    case 0: put_lit(o, "\\0"); return;
    case '\\': put_lit(o, "\\\\"); return;
    default: break;
    }
    if (c == (unsigned char)quote) {
        Str_push(o, '\\');
        Str_push(o, (char)c);
    } else if (c >= 0x20 && c < 0x7f) {
        Str_push(o, (char)c);
    } else {
        snprintf(b, sizeof b, "\\x%02x", c);
        put_lit(o, b);
    }
}
"#;

const LIST_SUPPORT: &str = r#"
static ListNode *ach_make_list(const int *vals, size_t n, long cycle) {
    ListNode *head = NULL, *tail = NULL, **nodes = malloc((n + 1) * sizeof *nodes);
    for (size_t i = 0; i < n; ++i) {
        ListNode *p = ListNode_new(vals[i]);
        p->next = NULL;
        if (tail) tail->next = p; else head = p;
        tail = p;
        nodes[i] = p;
    }
    if (cycle >= 0 && tail && (size_t)cycle < n) tail->next = nodes[cycle];
    free(nodes);
    return head;
}
"#;

const LIST_PUT: &str = r#"static void ListNodep_put(Str *o, ListNodep p) {
    if (!p) { put_lit(o, "nullptr"); return; }
    ListNode **seen = malloc(1000001 * sizeof *seen);
    size_t n = 0;
    long cycle = -1;
    while (p && n < 1000000) {
        for (size_t i = 0; i < n && n < 5000; ++i) if (seen[i] == p) cycle = (long)i;
        if (cycle >= 0) break;
        seen[n++] = p;
        p = p->next;
    }
    put_lit(o, "{");
    for (size_t i = 0; i < n; ++i) {
        char b[24];
        snprintf(b, sizeof b, "%d", seen[i]->val);
        if (i) put_lit(o, ", ");
        put_lit(o, b);
    }
    put_lit(o, "}");
    if (cycle >= 0) { char b[32]; snprintf(b, sizeof b, " @%ld", cycle); put_lit(o, b); }
    free(seen);
}
"#;

const TREE_SUPPORT: &str = r#"
static TreeNode *ach_make_tree(const int *vals, const unsigned char *present, size_t n) {
    if (n == 0 || !present[0]) return NULL;
    TreeNode **ids = malloc(n * sizeof *ids);
    for (size_t i = 0; i < n; ++i) {
        ids[i] = NULL;
        if (present[i]) {
            ids[i] = TreeNode_new(vals[i]);
            ids[i]->left = NULL;
            ids[i]->right = NULL;
        }
    }
    size_t next = 1;
    for (size_t i = 0; i < n; ++i) {
        if (!ids[i]) continue;
        if (next < n) ids[i]->left = ids[next++];
        if (next < n) ids[i]->right = ids[next++];
    }
    TreeNode *root = ids[0];
    free(ids);
    return root;
}
"#;

const TREE_PUT: &str = r#"static void TreeNodep_put(Str *o, TreeNodep root) {
    if (!root) { put_lit(o, "nullptr"); return; }
    size_t cap = 1024, n = 0, head = 0;
    TreeNode **q = malloc(cap * sizeof *q);
    q[n++] = root;
    while (head < n && n < 1000000) {
        TreeNode *x = q[head++];
        if (!x) continue;
        if (n + 2 > cap) { cap *= 2; q = realloc(q, cap * sizeof *q); }
        q[n++] = x->left;
        q[n++] = x->right;
    }
    while (n > 0 && !q[n - 1]) n--;
    put_lit(o, "{");
    for (size_t i = 0; i < n; ++i) {
        char b[24];
        if (i) put_lit(o, ", ");
        if (q[i]) { snprintf(b, sizeof b, "%d", q[i]->val); put_lit(o, b); } else put_lit(o, "nullptr");
    }
    put_lit(o, "}");
    free(q);
}
"#;

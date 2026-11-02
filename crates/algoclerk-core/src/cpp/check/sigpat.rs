// SPDX-License-Identifier: Apache-2.0
//! Parsing of the display signatures of [`crate::cpp::stdlib::MEMBERS`] into typing patterns.

use crate::cpp::ast::Ty;

/// A type pattern of a library signature.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum PT {
    Void,
    /// A concrete type (`int`, `std::size_t`, `std::string`).
    Ty(Ty),
    /// The receiver's own type (`vector`, `pair`, `optional` without arguments).
    SelfTy,
    /// A template parameter (`T`, `K`, `V`, `A`, `B`, `U`, `R`).
    Tpl(String),
    /// An iterator template parameter (`InputIt`, `RandomIt`, ...), named.
    Iter(String),
    /// A callable template parameter (`Compare`, `Pred`, `Fn`, `BinaryOp`, `Cmp`), named.
    Callable(String),
    Pair(Box<PT>, Box<PT>),
    /// `tuple<T...>`: the pack is the variadic parameter.
    Tuple,
    IList(Box<PT>),
    Ptr(Box<PT>),
    /// `iterator`, `reverse_iterator`, `const_iterator` of the receiver.
    RecvIter {
        reverse: bool,
        konst: bool,
    },
    /// `difference_type`.
    Diff,
    /// `nullopt_t`.
    Nullopt,
    /// A parameter pack (`Args...`, `T...`, `A...`).
    Pack(String),
    /// Anything the checker does not model: matches every argument.
    Any,
}

/// One parameter of a signature.
#[derive(Clone, Debug)]
pub(super) struct PParam {
    pub pt: PT,
    pub konst: bool,
    pub is_ref: bool,
    pub has_default: bool,
}

/// A parsed signature.
#[derive(Clone, Debug)]
pub(super) struct Sig {
    pub ret: PT,
    pub ret_ref: bool,
    pub ret_konst: bool,
    pub params: Vec<PParam>,
    /// Declared `const` after the parameter list.
    pub konst_method: bool,
    /// No parameter list: a data member or constant.
    pub is_data: bool,
}

/// Split at top-level commas (outside `<>` and `()`).
fn split_top(s: &str) -> Vec<&str> {
    let (mut depth, mut start) = (0i32, 0usize);
    let mut out = Vec::new();
    for (i, c) in s.char_indices() {
        match c {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = s[start..].trim();
    if !last.is_empty() {
        out.push(last);
    }
    out
}

/// Index of the last space outside `<>`.
fn last_space(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut at = None;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ' ' if depth == 0 => at = Some(i),
            _ => {}
        }
    }
    at
}

/// Parse one type text (qualifiers already removed).
fn parse_pt(t: &str) -> PT {
    let t = t.trim().trim_start_matches("std::");
    if let Some(inner) = t.strip_suffix("...") {
        return PT::Pack(inner.trim_end_matches('&').to_string());
    }
    if let Some(inner) = t.strip_suffix('*') {
        return PT::Ptr(Box::new(parse_pt(inner)));
    }
    if let Some((name, rest)) = t.split_once('<') {
        let args = split_top(rest.trim_end_matches('>'));
        return match name {
            "pair" if args.len() == 2 => {
                PT::Pair(Box::new(parse_pt(args[0])), Box::new(parse_pt(args[1])))
            }
            "tuple" => PT::Tuple,
            "initializer_list" if args.len() == 1 => PT::IList(Box::new(parse_pt(args[0]))),
            _ => PT::Any,
        };
    }
    match t {
        "void" => PT::Void,
        "int" => PT::Ty(Ty::Int),
        "bool" => PT::Ty(Ty::Bool),
        "char" => PT::Ty(Ty::Char),
        "unsigned" => PT::Ty(Ty::UInt),
        "long long" => PT::Ty(Ty::LongLong),
        "unsigned long long" => PT::Ty(Ty::ULongLong),
        "size_t" => PT::Ty(Ty::Size),
        "string" => PT::Ty(Ty::Str),
        "string_view" => PT::Ty(Ty::StrView),
        "vector" | "pair" | "tuple" | "optional" | "array" | "deque" | "list" => PT::SelfTy,
        "iterator" => PT::RecvIter {
            reverse: false,
            konst: false,
        },
        "reverse_iterator" => PT::RecvIter {
            reverse: true,
            konst: false,
        },
        "const_iterator" => PT::RecvIter {
            reverse: false,
            konst: true,
        },
        "difference_type" | "ptrdiff_t" => PT::Diff,
        "nullopt_t" => PT::Nullopt,
        "T" | "K" | "V" | "A" | "B" | "U" | "R" => PT::Tpl(t.to_string()),
        n if ["InputIt", "ForwardIt", "RandomIt", "BidirIt", "OutputIt"]
            .iter()
            .any(|p| n.starts_with(p)) =>
        {
            PT::Iter(n.to_string())
        }
        n if ["Compare", "Pred", "Fn", "BinaryOp", "Cmp"].contains(&n) => {
            PT::Callable(n.to_string())
        }
        _ => PT::Any,
    }
}

/// Peel `const`, `&`, `constexpr`, `static` off a type text.
fn qualifiers(t: &str) -> (String, bool, bool) {
    let mut s = t.trim().to_string();
    let (mut konst, mut is_ref) = (false, false);
    loop {
        let before = s.clone();
        for w in ["constexpr ", "static ", "const "] {
            if let Some(r) = s.strip_prefix(w) {
                konst |= w == "const ";
                s = r.trim().to_string();
            }
        }
        if let Some(r) = s.strip_suffix('&') {
            is_ref = true;
            s = r.trim().to_string();
        }
        if s == before {
            break;
        }
    }
    (s, konst, is_ref)
}

/// Parse a display signature such as `std::size_t find(char c, std::size_t pos = 0) const`.
pub(super) fn parse_sig(sig: &str) -> Sig {
    let sig = sig.replace("operator()", "operator_call");
    let Some(open) = sig.find('(') else {
        let head = sig.trim();
        let ty = last_space(head).map_or(head, |i| &head[..i]);
        let (t, konst, is_ref) = qualifiers(ty);
        return Sig {
            ret: parse_pt(&t),
            ret_ref: is_ref,
            ret_konst: konst,
            params: Vec::new(),
            konst_method: false,
            is_data: true,
        };
    };
    let close = sig.rfind(')').unwrap_or(sig.len());
    let head = sig[..open].trim();
    let ret_text = last_space(head).map_or("void", |i| &head[..i]);
    let (rt, ret_konst, ret_ref) = qualifiers(ret_text);
    let params = split_top(&sig[open + 1..close])
        .into_iter()
        .map(|p| {
            let (p, has_default) = match p.split_once(" = ") {
                Some((a, _)) => (a, true),
                None => (p, false),
            };
            let ty = last_space(p).map_or(p, |i| &p[..i]);
            let (t, konst, is_ref) = qualifiers(ty);
            PParam {
                pt: parse_pt(&t),
                konst,
                is_ref,
                has_default,
            }
        })
        .collect();
    Sig {
        ret: parse_pt(&rt),
        ret_ref,
        ret_konst,
        params,
        konst_method: sig[close..].contains("const"),
        is_data: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_shapes() {
        let s = parse_sig("std::size_t find(char c, std::size_t pos = 0) const");
        assert_eq!(s.ret, PT::Ty(Ty::Size));
        assert!(s.konst_method && s.params.len() == 2 && s.params[1].has_default);
        let s = parse_sig("std::pair<iterator, bool> insert(const std::pair<K, V>& kv)");
        assert!(matches!(s.ret, PT::Pair(..)));
        assert!(s.params[0].konst && s.params[0].is_ref);
        let s = parse_sig("T& get<I>(pair<A, B>& p)");
        assert!(s.ret_ref && matches!(s.ret, PT::Tpl(_)));
        let s = parse_sig("R operator()(A... args) const");
        assert!(matches!(s.params[0].pt, PT::Pack(_)));
        let s = parse_sig("A first");
        assert!(s.is_data && s.ret == PT::Tpl("A".into()));
        let s = parse_sig("ForwardIt next(ForwardIt it, difference_type n = 1)");
        assert!(matches!(s.ret, PT::Iter(_)) && matches!(s.params[1].pt, PT::Diff));
    }
}

// SPDX-License-Identifier: Apache-2.0
//! A small, dedicated parser for the one-line function signature of a level
//! (`std::vector<int> twoSum(const std::vector<int>& nums, int target)`).
//!
//! The grammar of a signature is only the *type* sub-language of the subset plus a name and a parameter
//! list, so levels do not need the full parser to load. `tests` in this module check that it agrees with
//! [`crate::cpp::parse::parse`] whenever that parser is available.

use super::ast::{Functor, NodeId, Param, Ty, TypeSpec};
use super::check::EntrySig;
use std::collections::BTreeMap;

/// Type aliases (`using Grid = std::vector<std::vector<char>>;`) found in a level prelude.
pub type Aliases = BTreeMap<String, Ty>;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Num(u64),
    Punct(char),
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let st = i;
            loop {
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                if b.get(i) == Some(&b':')
                    && b.get(i + 1) == Some(&b':')
                    && b.get(i + 2)
                        .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
                {
                    i += 2;
                } else {
                    break;
                }
            }
            out.push(Tok::Ident(src[st..i].to_string()));
        } else if c.is_ascii_digit() {
            let st = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let n = src[st..i]
                .parse::<u64>()
                .map_err(|_| "number too large".to_string())?;
            out.push(Tok::Num(n));
        } else if "<>,()*&;".contains(c as char) {
            out.push(Tok::Punct(c as char));
            i += 1;
        } else {
            return Err(format!("unexpected character '{}'", c as char));
        }
    }
    Ok(out)
}

struct P<'a> {
    t: Vec<Tok>,
    i: usize,
    aliases: &'a Aliases,
}

impl P<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn eat_p(&mut self, c: char) -> bool {
        if self.peek() == Some(&Tok::Punct(c)) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expect_p(&mut self, c: char) -> Result<(), String> {
        if self.eat_p(c) {
            Ok(())
        } else {
            Err(format!("I expected '{c}' here"))
        }
    }
    fn eat_kw(&mut self, w: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Ident(x)) if x == w) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn spec(&mut self) -> Result<TypeSpec, String> {
        let mut konst = self.eat_kw("const");
        let ty = self.base()?;
        if self.eat_kw("const") {
            konst = true;
        }
        let mut ty = ty;
        while self.eat_p('*') {
            ty = Ty::Ptr(Box::new(ty));
            if self.eat_kw("const") {
                konst = true;
            }
        }
        let is_ref = self.eat_p('&');
        Ok(TypeSpec {
            ty,
            konst,
            is_ref,
            spelled: None,
        })
    }

    fn ty(&mut self) -> Result<Ty, String> {
        let s = self.spec()?;
        if s.konst || s.is_ref {
            return Err("const and & are not allowed inside a template argument".into());
        }
        Ok(s.ty)
    }

    fn targs(&mut self) -> Result<Vec<Result<Ty, u64>>, String> {
        self.expect_p('<')?;
        let mut v = Vec::new();
        loop {
            if let Some(Tok::Num(n)) = self.peek() {
                v.push(Err(*n));
                self.i += 1;
            } else {
                v.push(Ok(self.function_or_ty()?));
            }
            if self.eat_p(',') {
                continue;
            }
            self.expect_p('>')?;
            return Ok(v);
        }
    }

    /// A type, or `R(A, B)` (the argument of `std::function`).
    fn function_or_ty(&mut self) -> Result<Ty, String> {
        let t = self.ty()?;
        if self.eat_p('(') {
            let mut args = Vec::new();
            if !self.eat_p(')') {
                loop {
                    args.push(self.ty()?);
                    if self.eat_p(',') {
                        continue;
                    }
                    self.expect_p(')')?;
                    break;
                }
            }
            return Ok(Ty::Function(args, Box::new(t)));
        }
        Ok(t)
    }

    fn base(&mut self) -> Result<Ty, String> {
        let Some(Tok::Ident(name)) = self.peek().cloned() else {
            return Err("I expected a type here".into());
        };
        self.i += 1;
        let bare = name.strip_prefix("std::").unwrap_or(&name).to_string();
        let one = |a: Vec<Result<Ty, u64>>| -> Result<Ty, String> {
            match a.into_iter().next() {
                Some(Ok(t)) => Ok(t),
                _ => Err(format!("{bare} needs a type argument")),
            }
        };
        let tys = |a: Vec<Result<Ty, u64>>| -> Result<Vec<Ty>, String> {
            a.into_iter()
                .map(|x| x.map_err(|_| format!("{bare} takes types, not numbers")))
                .collect()
        };
        Ok(match bare.as_str() {
            "void" => Ty::Void,
            "bool" => Ty::Bool,
            "char" => Ty::Char,
            "int" => Ty::Int,
            "size_t" => Ty::Size,
            "signed" => {
                self.eat_kw("int");
                Ty::Int
            }
            "unsigned" => {
                if self.eat_kw("long") {
                    if !self.eat_kw("long") {
                        return Err("the supply cupboard does not stock `long`".into());
                    }
                    self.eat_kw("int");
                    Ty::ULongLong
                } else {
                    self.eat_kw("int");
                    Ty::UInt
                }
            }
            "long" => {
                if self.eat_kw("long") {
                    self.eat_kw("int");
                    Ty::LongLong
                } else {
                    return Err(
                        "the supply cupboard does not stock `long`: write `long long`".into(),
                    );
                }
            }
            "short" | "float" | "double" => {
                return Err(format!("the supply cupboard does not stock `{bare}`"));
            }
            "string" => Ty::Str,
            "string_view" => Ty::StrView,
            "vector" => Ty::Vector(Box::new(one(self.targs()?)?)),
            "deque" => Ty::Deque(Box::new(one(self.targs()?)?)),
            "list" => Ty::List(Box::new(one(self.targs()?)?)),
            "stack" => Ty::Stack(Box::new(one(self.targs()?)?)),
            "queue" => Ty::Queue(Box::new(one(self.targs()?)?)),
            "set" => Ty::Set(Box::new(one(self.targs()?)?)),
            "unordered_set" => Ty::UnorderedSet(Box::new(one(self.targs()?)?)),
            "optional" => Ty::Optional(Box::new(one(self.targs()?)?)),
            "array" => {
                let mut a = self.targs()?.into_iter();
                match (a.next(), a.next()) {
                    (Some(Ok(t)), Some(Err(n))) => Ty::Array(Box::new(t), n as usize),
                    _ => return Err("std::array needs a type and a size".into()),
                }
            }
            "pair" => {
                let mut a = tys(self.targs()?)?.into_iter();
                match (a.next(), a.next(), a.next()) {
                    (Some(x), Some(y), None) => Ty::Pair(Box::new(x), Box::new(y)),
                    _ => return Err("std::pair needs two types".into()),
                }
            }
            "tuple" => Ty::Tuple(tys(self.targs()?)?),
            "map" | "unordered_map" => {
                let mut a = tys(self.targs()?)?.into_iter();
                match (a.next(), a.next(), a.next()) {
                    (Some(k), Some(v), None) => {
                        if bare == "map" {
                            Ty::Map(Box::new(k), Box::new(v))
                        } else {
                            Ty::UnorderedMap(Box::new(k), Box::new(v))
                        }
                    }
                    _ => return Err(format!("std::{bare} needs a key and a value type")),
                }
            }
            "priority_queue" => {
                let a = tys(self.targs()?)?;
                let t = a
                    .first()
                    .cloned()
                    .ok_or("std::priority_queue needs a type")?;
                let cmp = match a.get(2) {
                    Some(c) => c.clone(),
                    None => Ty::Functor(Functor::Less, Box::new(t.clone())),
                };
                Ty::PriorityQueue(Box::new(t), Box::new(cmp))
            }
            "less" | "greater" => {
                let t = one(self.targs()?)?;
                let f = if bare == "less" {
                    Functor::Less
                } else {
                    Functor::Greater
                };
                Ty::Functor(f, Box::new(t))
            }
            "function" => {
                let t = one(self.targs()?)?;
                if let Ty::Function(..) = t {
                    t
                } else {
                    return Err("std::function needs a signature like R(A, B)".into());
                }
            }
            _ => {
                if let Some(t) = self.aliases.get(&name) {
                    t.clone()
                } else if name.contains("::") {
                    return Err(format!("the supply cupboard does not stock `{name}`"));
                } else {
                    Ty::Struct(name)
                }
            }
        })
    }
}

/// Parse the type of a declaration (`std::vector<int>`, `const Grid&`, `ListNode*`).
pub fn parse_type(text: &str, aliases: &Aliases) -> Result<TypeSpec, String> {
    let mut p = P {
        t: lex(text)?,
        i: 0,
        aliases,
    };
    let s = p.spec()?;
    if p.i != p.t.len() {
        return Err("unexpected text after the type".into());
    }
    Ok(s)
}

/// Parse `ret name(type a, type b)` into an [`EntrySig`]. Parameter ids are 1.. in order.
pub fn parse_signature(text: &str, aliases: &Aliases) -> Result<EntrySig, String> {
    let mut p = P {
        t: lex(text.trim().trim_end_matches(';'))?,
        i: 0,
        aliases,
    };
    p.eat_kw("static");
    p.eat_kw("inline");
    let ret = p.spec()?;
    let Some(Tok::Ident(name)) = p.peek().cloned() else {
        return Err("I expected the function name here".into());
    };
    p.i += 1;
    p.expect_p('(')?;
    let mut params: Vec<Param> = Vec::new();
    if !p.eat_p(')') {
        loop {
            let spec = p.spec()?;
            if spec.ty == Ty::Void {
                return Err("a parameter cannot be void".into());
            }
            let pname = match p.peek() {
                Some(Tok::Ident(n)) => {
                    let n = n.clone();
                    p.i += 1;
                    n
                }
                _ => return Err("every parameter of a signature needs a name".into()),
            };
            params.push(Param {
                id: params.len() as NodeId + 1,
                name: pname,
                spec,
            });
            if p.eat_p(',') {
                continue;
            }
            p.expect_p(')')?;
            break;
        }
    }
    if p.i != p.t.len() {
        return Err("unexpected text after the signature".into());
    }
    Ok(EntrySig { name, params, ret })
}

/// Find `using Name = Type;` and `typedef Type Name;` lines in a prelude (one per line).
pub fn prelude_aliases(prelude: &str) -> Aliases {
    let mut out = Aliases::new();
    for line in prelude.lines() {
        let l = line.trim();
        let parsed = if let Some(rest) = l.strip_prefix("using ") {
            rest.trim_end_matches(';')
                .split_once('=')
                .map(|(n, t)| (n.trim().to_string(), t.trim().to_string()))
        } else if let Some(rest) = l.strip_prefix("typedef ") {
            let rest = rest.trim_end_matches(';').trim();
            rest.rsplit_once(char::is_whitespace)
                .map(|(t, n)| (n.trim().to_string(), t.trim().to_string()))
        } else {
            None
        };
        if let Some((name, ty)) = parsed
            && let Ok(spec) = parse_type(&ty, &out)
        {
            out.insert(name, spec.ty);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typical_signatures() {
        let a = Aliases::new();
        let s = parse_signature(
            "std::vector<int> twoSum(const std::vector<int>& nums, int target)",
            &a,
        )
        .unwrap();
        assert_eq!(s.name, "twoSum");
        assert_eq!(s.params.len(), 2);
        assert_eq!(s.params[0].spec.ty, Ty::vector(Ty::Int));
        assert!(s.params[0].spec.konst && s.params[0].spec.is_ref);
        assert_eq!(s.ret.ty, Ty::vector(Ty::Int));
        let s = parse_signature("ListNode* reverseList(ListNode* head)", &a).unwrap();
        assert_eq!(s.ret.ty, Ty::Ptr(Box::new(Ty::Struct("ListNode".into()))));
        let s = parse_signature(
            "long long f(std::unordered_map<std::string, std::vector<long long>> m, unsigned long long u, size_t n)",
            &a,
        )
        .unwrap();
        assert_eq!(s.params[1].spec.ty, Ty::ULongLong);
        assert_eq!(s.params[2].spec.ty, Ty::Size);
        assert_eq!(s.ret.ty, Ty::LongLong);
        let s = parse_signature(
            "int g(std::priority_queue<int, std::vector<int>, std::greater<int>>& q, std::array<int, 3> a)",
            &a,
        )
        .unwrap();
        assert_eq!(
            s.params[0].spec.ty,
            Ty::PriorityQueue(
                Box::new(Ty::Int),
                Box::new(Ty::Functor(Functor::Greater, Box::new(Ty::Int)))
            )
        );
        assert_eq!(s.params[1].spec.ty, Ty::Array(Box::new(Ty::Int), 3));
    }

    #[test]
    fn rejects_unsupported_and_malformed() {
        let a = Aliases::new();
        assert!(parse_signature("long f(int a)", &a).is_err());
        assert!(parse_signature("double f(int a)", &a).is_err());
        assert!(parse_signature("int f(int)", &a).is_err());
        assert!(parse_signature("int f(int a", &a).is_err());
        assert!(parse_signature("int f(int a) extra", &a).is_err());
        assert!(parse_signature("", &a).is_err());
    }

    #[test]
    fn aliases_from_the_prelude() {
        let a =
            prelude_aliases("#include <vector>\nusing Grid = std::vector<std::vector<char>>;\n");
        let s = parse_signature("int f(const Grid& g)", &a).unwrap();
        assert_eq!(s.params[0].spec.ty, Ty::vector(Ty::vector(Ty::Char)));
    }
}

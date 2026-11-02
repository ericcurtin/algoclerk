// SPDX-License-Identifier: Apache-2.0
//! Type specifiers: builtin types, `std::` templates, aliases, structs, `const`, `&`, `*`, parameters.
//!
//! Where the player's spelling differs from the canonical form (`Ty`'s `Display`), the exact text is
//! kept in [`TypeSpec::spelled`]. It covers the type without its leading `const` and trailing `&`
//! (for `const T*` the pointee `const` is part of the text, since [`TypeSpec`] cannot say it).
//! `decltype(name)` (a lambda comparator) is `Ty::Struct("decltype(name)")` and an alias known only by
//! name (declared by the prelude) is `Ty::Struct(alias)`.

use super::*;

/// Library type names (after `std::`, or unqualified under `using namespace std`).
pub(crate) const STD_TYPE_NAMES: &[&str] = &[
    "string",
    "string_view",
    "vector",
    "array",
    "pair",
    "tuple",
    "optional",
    "unordered_map",
    "map",
    "unordered_set",
    "set",
    "stack",
    "queue",
    "deque",
    "list",
    "priority_queue",
    "function",
    "less",
    "greater",
    "size_t",
    "nullptr_t",
];

/// Library types outside the subset: (name, what to tell the player).
const UNSUPPORTED_STD_TYPES: &[(&str, &str)] = &[
    ("multiset", "`std::multiset`"),
    ("multimap", "`std::multimap`"),
    ("unordered_multiset", "`std::unordered_multiset`"),
    ("unordered_multimap", "`std::unordered_multimap`"),
    ("bitset", "`std::bitset`"),
    ("shared_ptr", "`std::shared_ptr`"),
    ("unique_ptr", "`std::unique_ptr`"),
    ("weak_ptr", "`std::weak_ptr`"),
    ("ptrdiff_t", "`ptrdiff_t`"),
    ("int8_t", "`int8_t` (write `int` or `long long`)"),
    ("int16_t", "`int16_t` (write `int` or `long long`)"),
    ("int32_t", "`int32_t` (write `int`)"),
    (
        "int64_t",
        "`int64_t` (write `long long`, which is the same size everywhere)",
    ),
    (
        "uint8_t",
        "`uint8_t` (write `unsigned` or `unsigned long long`)",
    ),
    (
        "uint16_t",
        "`uint16_t` (write `unsigned` or `unsigned long long`)",
    ),
    ("uint32_t", "`uint32_t` (write `unsigned`)"),
    ("uint64_t", "`uint64_t` (write `unsigned long long`)"),
    ("span", "`std::span`"),
    ("variant", "`std::variant`"),
    ("any", "`std::any`"),
    ("complex", "`std::complex`"),
];

/// Keywords that start a builtin type.
const BUILTIN_KWS: &[&str] = &[
    "void", "bool", "char", "int", "unsigned", "signed", "long", "short", "float", "double",
    "wchar_t", "char16_t", "char32_t", "auto", "decltype",
];

/// Is `n` a library type name this parser knows (supported or not)?
pub(crate) fn is_std_type_name(n: &str) -> bool {
    STD_TYPE_NAMES.contains(&n) || UNSUPPORTED_STD_TYPES.iter().any(|(k, _)| *k == n)
}

/// Remove the whitespace of a type text except where two words meet; a comma is followed by a space.
fn normalise_type_text(s: &str) -> String {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = String::new();
    let mut pending = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        // comments inside the type text count as blanks
        if c == '/' && chars.peek() == Some(&'*') {
            let mut prev = ' ';
            for d in chars.by_ref() {
                if prev == '*' && d == '/' {
                    break;
                }
                prev = d;
            }
            pending = true;
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            for d in chars.by_ref() {
                if d == '\n' {
                    break;
                }
            }
            pending = true;
            continue;
        }
        if c.is_whitespace() {
            pending = true;
            continue;
        }
        let last = out.chars().last();
        if last == Some(',') || (pending && last.is_some_and(word) && word(c)) {
            out.push(' ');
        }
        pending = false;
        out.push(c);
    }
    out
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

impl Parser<'_> {
    /// Does a builtin type keyword (`int`, `unsigned`, `auto`, `decltype` ...) start here?
    pub(crate) fn at_builtin_type(&self) -> bool {
        self.tok()
            .is_some_and(|t| matches!(&t.kind, Tok::Kw(k) if BUILTIN_KWS.contains(k)))
    }

    /// Is `name` a struct or alias that was declared?
    pub(crate) fn is_known_type(&self, name: &str) -> bool {
        self.known.contains(name)
    }

    /// Record a struct name (usable as a type from now on).
    pub(crate) fn declare_struct(&mut self, name: &str) {
        self.known.insert(name.to_string());
    }

    /// Record an alias and what it stands for.
    pub(crate) fn declare_alias(&mut self, name: &str, ty: Ty) {
        self.known.insert(name.to_string());
        self.aliases.insert(name.to_string(), ty);
    }

    /// Record `const int N = 5;` so `std::array<int, N>` can be understood.
    pub(crate) fn declare_const(&mut self, name: &str, value: i64) {
        self.consts.insert(name.to_string(), value);
    }

    /// The spelling to keep when the text `start..end` differs from the canonical form of `ty`.
    pub(crate) fn spelled_for(&self, ty: &Ty, start: u32, end: u32) -> Option<String> {
        let text = self.src.get(start as usize..end as usize)?;
        let text = normalise_type_text(text);
        if squash(&text) == squash(&ty.to_string()) {
            None
        } else {
            Some(text)
        }
    }

    /// A type name without `const`, `&` or `*` (constructor calls, `new`): `std::vector<int>`.
    pub(crate) fn parse_simple_type(&mut self) -> PResult<TypeSpec> {
        self.enter()?;
        let start = self.cur_start();
        let r = self.parse_ty_core();
        self.leave();
        let ty = r?;
        let spelled = self.spelled_for(&ty, start, self.last_end);
        Ok(TypeSpec {
            ty,
            konst: false,
            is_ref: false,
            spelled,
        })
    }

    /// A type with `const`, `*` and `&`: `const std::vector<int>&`, `ListNode*`, `auto&`.
    pub(crate) fn parse_type_spec(&mut self) -> PResult<TypeSpec> {
        self.enter()?;
        let r = self.type_spec_inner();
        self.leave();
        r
    }

    fn type_spec_inner(&mut self) -> PResult<TypeSpec> {
        let start = self.cur_start();
        let mut inner_const = self.eat_kw("const");
        let base_start = self.cur_start();
        let mut ty = self.parse_ty_core()?;
        let mut text_end = self.last_end;
        let mut ptrs = 0usize;
        let mut ptr_const = false;
        let mut is_ref = false;
        loop {
            if self.at_kw("const") {
                let at = self.cur_span();
                self.bump();
                if ptrs == 0 {
                    inner_const = true;
                } else if inner_const {
                    return Err(self.unsupported(at, "a const pointer to a const value"));
                } else {
                    ptr_const = true;
                }
            } else if !ptr_const && self.at_punct("*") {
                self.bump();
                ptrs += 1;
                ty = Ty::Ptr(Box::new(ty));
                text_end = self.last_end;
            } else if self.at_punct("&&") {
                return Err(self.unsupported(self.cur_span(), "rvalue references (`&&`)"));
            } else if self.at_punct("&") {
                self.bump();
                is_ref = true;
                break;
            } else {
                break;
            }
        }
        let (konst, spelled) = if ptrs > 0 && inner_const {
            // `const T*`: the spec has no place for "pointee const", keep the text
            let text = self
                .src
                .get(start as usize..text_end as usize)
                .unwrap_or("");
            (false, Some(normalise_type_text(text)))
        } else {
            let konst = if ptrs > 0 { ptr_const } else { inner_const };
            (konst, self.spelled_for(&ty, base_start, text_end))
        };
        Ok(TypeSpec {
            ty,
            konst,
            is_ref,
            spelled,
        })
    }

    /// A type name and its template arguments, no `const` / `*` / `&`.
    fn parse_ty_core(&mut self) -> PResult<Ty> {
        let span = self.cur_span();
        let kind = self.tok().map(|t| t.kind.clone());
        let ty = match kind {
            Some(Tok::Kw(k)) => self.parse_kw_type(k, span)?,
            Some(Tok::Ident(name)) => self.parse_named_type(&name, span)?,
            _ => return Err(self.expected("a type")),
        };
        self.type_member_suffix(ty)
    }

    /// `::iterator` and friends after a container type.
    fn type_member_suffix(&mut self, ty: Ty) -> PResult<Ty> {
        if !self.at_punct("::") || ty.header().is_none() {
            return Ok(ty);
        }
        let reverse = match self.ident_at(1) {
            Some("iterator" | "const_iterator") => false,
            Some("reverse_iterator" | "const_reverse_iterator") => true,
            _ => return Ok(ty),
        };
        self.bump();
        self.bump();
        Ok(Ty::Iter {
            container: Box::new(ty),
            reverse,
        })
    }

    fn unsupported_since(&self, start: Span, what: &str) -> Diag {
        self.unsupported(
            Span {
                start: start.start,
                end: self.last_end.max(start.end),
            },
            what,
        )
    }

    /// A type that starts with a keyword (cursor on the keyword).
    fn parse_kw_type(&mut self, k: &'static str, span: Span) -> PResult<Ty> {
        self.bump();
        match k {
            "auto" => Ok(Ty::Auto),
            "void" => Ok(Ty::Void),
            "bool" => Ok(Ty::Bool),
            "char" => Ok(Ty::Char),
            "int" => Ok(Ty::Int),
            "decltype" => {
                self.expect_punct("(")?;
                let name = match self.ident().map(str::to_string) {
                    Some(n) if self.at_punct_at(1, ")") => n,
                    _ => {
                        return Err(self.unsupported(
                            self.cur_span(),
                            "`decltype` of anything but a plain name",
                        ));
                    }
                };
                self.bump();
                self.bump();
                Ok(Ty::Struct(format!("decltype({name})")))
            }
            "unsigned" => {
                if self.at_kw("char") || self.at_kw("short") {
                    return Err(self.unsupported(span, "`unsigned char` and `unsigned short`"));
                }
                if self.eat_kw("long") {
                    if !self.eat_kw("long") {
                        return Err(self.unsupported_since(
                            span,
                            "`unsigned long` (write `unsigned long long`)",
                        ));
                    }
                    self.eat_kw("int");
                    return Ok(Ty::ULongLong);
                }
                self.eat_kw("int");
                Ok(Ty::UInt)
            }
            "signed" => {
                if self.at_kw("char") || self.at_kw("short") || self.at_kw("long") {
                    return Err(self.unsupported(span, "`signed char`, `short` and `long`"));
                }
                self.eat_kw("int");
                Ok(Ty::Int)
            }
            "long" => {
                if self.eat_kw("long") {
                    self.eat_kw("int");
                    return Ok(Ty::LongLong);
                }
                Err(self.unsupported_since(
                    span,
                    "`long` (write `long long`, which is the same size everywhere)",
                ))
            }
            "short" => Err(self.unsupported(span, "`short`")),
            "float" | "double" => {
                Err(self.unsupported(span, "decimal numbers (`float` and `double`) at all"))
            }
            _ => Err(self.unsupported(span, &format!("the type `{k}`"))),
        }
    }

    /// A type that starts with an identifier: `std::...`, an alias, a struct, `size_t`, or an unqualified
    /// std name under `using namespace std`.
    fn parse_named_type(&mut self, name: &str, span: Span) -> PResult<Ty> {
        if name == "std" && self.at_punct_at(1, "::") {
            let Some(n) = self.ident_at(2).map(str::to_string) else {
                self.bump();
                self.bump();
                return Err(self.expected("a type name after `std::`"));
            };
            self.bump();
            self.bump();
            self.bump();
            return self.resolve_std(&n, span);
        }
        if self.is_known_type(name) {
            self.bump();
            return Ok(self
                .aliases
                .get(name)
                .cloned()
                .unwrap_or_else(|| Ty::Struct(name.to_string())));
        }
        if name == "size_t" || (self.using_std && is_std_type_name(name)) {
            self.bump();
            return self.resolve_std(name, span);
        }
        Err(Diag::at(
            span,
            "parse.expected",
            format!("I expected a type here, and `{name}` is not one I know"),
        ))
    }

    /// The library type `std::name` (cursor after the name).
    fn resolve_std(&mut self, name: &str, span: Span) -> PResult<Ty> {
        match name {
            "size_t" => return Ok(Ty::Size),
            "nullptr_t" => return Ok(Ty::NullPtr),
            "string" | "string_view" => {
                return Ok(if name == "string" {
                    Ty::Str
                } else {
                    Ty::StrView
                });
            }
            "function" => return self.parse_function_type(),
            _ => {}
        }
        if let Some((_, what)) = UNSUPPORTED_STD_TYPES.iter().find(|(k, _)| *k == name) {
            return Err(self.unsupported_since(span, what));
        }
        if !STD_TYPE_NAMES.contains(&name) {
            return Err(Diag::at(
                span,
                "parse.expected",
                format!("I expected a type here, and `std::{name}` is not one I know"),
            ));
        }
        if !self.at_punct("<") {
            return Err(Diag::at(
                self.cur_span(),
                "parse.expected",
                format!(
                    "I expected the template arguments of `std::{name}` here, like `std::{name}<int>`"
                ),
            ));
        }
        let targs = self.parse_template_args()?;
        self.build_std_ty(name, targs, span)
    }

    /// `<R(A, B)>` after `std::function`.
    fn parse_function_type(&mut self) -> PResult<Ty> {
        self.expect_punct("<")?;
        let ret = self.parse_type_spec()?.ty;
        self.expect_punct("(")?;
        let mut params = Vec::new();
        if !self.at_punct(")") {
            loop {
                params.push(self.parse_type_spec()?.ty);
                if !self.eat_punct(",") {
                    break;
                }
            }
        }
        self.expect_punct(")")?;
        self.close_angle()?;
        Ok(Ty::Function(params, Box::new(ret)))
    }

    /// `<int, std::string, 5>` (cursor on `<`). A `>>` closing two lists is split.
    pub(crate) fn parse_template_args(&mut self) -> PResult<Vec<TArg>> {
        self.expect_punct("<")?;
        let mut args = Vec::new();
        if self.at_punct(">") || self.at_punct(">>") {
            self.close_angle()?;
            return Ok(args);
        }
        loop {
            let numeric = match self.tok().map(|t| &t.kind) {
                Some(Tok::Int { .. } | Tok::Punct("(" | "-")) => true,
                Some(Tok::Ident(n)) => {
                    self.consts.contains_key(n)
                        && !self.is_known_type(n)
                        && !self.at_punct_at(1, "::")
                }
                _ => false,
            };
            if numeric {
                // `5`, `N`, `N + 1`, `2 * N`: a small constant expression
                let at = self.cur_span();
                let e = self.parse_bin(BinOp::Add.prec())?;
                match self.const_eval(&e, 0) {
                    Some(v) if v >= 0 => args.push(TArg::Int(v)),
                    _ => {
                        return Err(Diag::at(
                            self.span_from(at.start),
                            "parse.expected",
                            "I expected a plain number here (a size must be a number or a `const` declared above)",
                        ));
                    }
                }
            } else if matches!(self.tok().map(|t| &t.kind), Some(Tok::Ident(n))
                if !self.is_known_type(n) && n != "std" && !is_std_type_name(n))
            {
                return Err(Diag::at(
                    self.cur_span(),
                    "parse.expected",
                    "I expected a type or a plain number here (a size must be a number or a `const` declared above)",
                ));
            } else {
                args.push(TArg::Ty(self.parse_type_spec()?.ty));
            }
            if !self.eat_punct(",") {
                break;
            }
        }
        self.close_angle()?;
        Ok(args)
    }

    /// `std::name<targs>` as a [`Ty`], with friendly errors for wrong argument lists.
    fn build_std_ty(&self, name: &str, targs: Vec<TArg>, span: Span) -> PResult<Ty> {
        let bad = |want: &str| {
            Diag::at(
                Span {
                    start: span.start,
                    end: self.last_end.max(span.end),
                },
                "parse.expected",
                format!("`std::{name}` needs {want} between the `<` and `>`"),
            )
        };
        let extra = || {
            self.unsupported_since(
                span,
                &format!("custom comparators, hashes or allocators for `std::{name}`"),
            )
        };
        let mut tys = Vec::new();
        for (i, a) in targs.iter().enumerate() {
            match a {
                TArg::Ty(t) => tys.push(t.clone()),
                TArg::Int(_) if name == "array" && i == 1 => {}
                TArg::Int(_) => return Err(bad("types, not numbers,")),
            }
        }
        let b = |t: &Ty| Box::new(t.clone());
        match name {
            "vector" | "optional" | "unordered_set" | "set" | "stack" | "queue" | "deque"
            | "list" | "less" | "greater" => {
                if tys.len() > 1 {
                    return Err(extra());
                }
                let Some(t) = tys.first() else {
                    return Err(bad("one type"));
                };
                Ok(match name {
                    "vector" => Ty::Vector(b(t)),
                    "optional" => Ty::Optional(b(t)),
                    "unordered_set" => Ty::UnorderedSet(b(t)),
                    "set" => Ty::Set(b(t)),
                    "stack" => Ty::Stack(b(t)),
                    "queue" => Ty::Queue(b(t)),
                    "deque" => Ty::Deque(b(t)),
                    "list" => Ty::List(b(t)),
                    "less" => Ty::Functor(Functor::Less, b(t)),
                    _ => Ty::Functor(Functor::Greater, b(t)),
                })
            }
            "pair" | "unordered_map" | "map" => {
                if tys.len() > 2 {
                    return Err(extra());
                }
                let [k, v] = tys.as_slice() else {
                    return Err(bad("two types"));
                };
                Ok(match name {
                    "pair" => Ty::Pair(b(k), b(v)),
                    "unordered_map" => Ty::UnorderedMap(b(k), b(v)),
                    _ => Ty::Map(b(k), b(v)),
                })
            }
            "array" => match (targs.first(), targs.get(1), targs.len()) {
                (Some(TArg::Ty(t)), Some(TArg::Int(n)), 2) => Ok(Ty::Array(b(t), *n as usize)),
                _ => Err(bad("a type and a size")),
            },
            "tuple" => {
                if tys.is_empty() {
                    return Err(bad("at least one type"));
                }
                Ok(Ty::Tuple(tys))
            }
            "priority_queue" => {
                let Some(t) = tys.first() else {
                    return Err(bad("an element type"));
                };
                match tys.len() {
                    1 => Ok(Ty::PriorityQueue(
                        b(t),
                        Box::new(Ty::Functor(Functor::Less, b(t))),
                    )),
                    2 | 3 if tys[1] != Ty::Vector(b(t)) => Err(self.unsupported_since(
                        span,
                        "a `std::priority_queue` over anything but a `std::vector`",
                    )),
                    2 => Ok(Ty::PriorityQueue(
                        b(t),
                        Box::new(Ty::Functor(Functor::Less, b(t))),
                    )),
                    3 => Ok(Ty::PriorityQueue(b(t), Box::new(tys[2].clone()))),
                    _ => Err(extra()),
                }
            }
            _ => Err(bad("template arguments")),
        }
    }

    /// A function or lambda parameter: type, optional name. Default arguments and array parameters are
    /// not in the subset.
    pub(crate) fn parse_param(&mut self) -> PResult<Param> {
        let start = self.cur_start();
        let spec = self.parse_type_spec()?;
        let name = self.ident().map(str::to_string).unwrap_or_default();
        if !name.is_empty() {
            self.bump();
        }
        if self.at_punct("=") {
            return Err(self.unsupported(self.cur_span(), "default arguments"));
        }
        if self.at_punct("[") {
            return Err(self.unsupported(self.cur_span(), "array parameters (use `std::vector`)"));
        }
        let span = self.span_from(start);
        Ok(Param {
            id: self.new_id(span),
            name,
            spec,
        })
    }

    /// `(T a, U b)`: a parenthesised parameter list (cursor on `(`).
    pub(crate) fn parse_params(&mut self) -> PResult<Vec<Param>> {
        self.expect_punct("(")?;
        let mut params = Vec::new();
        // `(void)` is an empty list
        if self.at_kw("void") && self.at_punct_at(1, ")") {
            self.bump();
        } else if !self.at_punct(")") {
            loop {
                params.push(self.parse_param()?);
                if !self.eat_punct(",") {
                    break;
                }
            }
        }
        self.expect_punct(")")?;
        Ok(params)
    }

    /// Does a declaration start here? A statement is a declaration when it starts with a builtin type
    /// keyword, `auto`, `const`, `constexpr`, `static`, a library type name, or an alias / struct name
    /// followed by a declarator. The cursor does not move.
    pub(crate) fn looks_like_decl(&mut self) -> bool {
        if self.at_builtin_type()
            || self.at_kw("const")
            || self.at_kw("constexpr")
            || self.at_kw("static")
        {
            // `int(x);` and `unsigned(x);` are declarations in C++ too
            return true;
        }
        let Some(name) = self.ident().map(str::to_string) else {
            return false;
        };
        let std_start = name == "std" && self.at_punct_at(1, "::");
        let named = self.is_known_type(&name)
            || name == "size_t"
            || (self.using_std && is_std_type_name(&name));
        if !std_start && !named {
            return false;
        }
        let m = self.mark();
        let r = self.parse_type_spec();
        let yes = match r {
            Ok(_) => self.ident().is_some(),
            Err(d) => d.code == "parse.unsupported",
        };
        self.restore(m);
        yes
    }
}

// SPDX-License-Identifier: Apache-2.0
//! Top-level items: `#include`, `using namespace std;`, `using X = T;`, `typedef`, structs with
//! fields and constructors, global constants and free functions (including forward declarations).

use super::stmt::preproc_diag;
use super::*;

/// Keywords that start an item the subset does not stock, with what to tell the player.
fn unsupported_item_kw(k: &str) -> Option<&'static str> {
    Some(match k {
        "template" => "templates",
        "class" => "classes (write a `struct`)",
        "enum" => "`enum`",
        "union" => "`union`",
        "namespace" => "namespaces",
        "extern" => "`extern`",
        "static_assert" => "`static_assert`",
        "friend" | "virtual" | "explicit" | "operator" => "that kind of declaration",
        "try" | "goto" => "statements outside a function",
        "thread_local" | "volatile" | "register" | "mutable" => "that storage specifier",
        _ => return None,
    })
}

impl Parser<'_> {
    /// All items up to the end of the text, each with the comments written above it.
    pub(crate) fn parse_items(&mut self) -> PResult<Vec<Item>> {
        let mut items = Vec::new();
        loop {
            while self.eat_punct(";") {}
            if self.at_eof() {
                return Ok(items);
            }
            let comments = self.take_comments_before(self.cur_start());
            let start = self.cur_start();
            self.enter()?;
            let r = self.parse_item_kind();
            self.leave();
            let mut kind = r?;
            let span = self.span_from(start);
            if let ItemKind::Func(f) = &mut kind {
                f.comments = comments.clone();
                if let Some(slot) = self.spans.get_mut(f.id as usize) {
                    *slot = span;
                }
            }
            // comments inside the item that no statement claimed are dropped
            self.skip_comments_before(self.last_end);
            let id = self.new_id(span);
            items.push(Item { id, kind, comments });
        }
    }

    fn parse_item_kind(&mut self) -> PResult<ItemKind> {
        let span = self.cur_span();
        let Some(kind) = self.tok().map(|t| t.kind.clone()) else {
            return Err(self.expected("a declaration"));
        };
        match kind {
            Tok::Preproc(text) => {
                self.bump();
                self.parse_include(&text, span)
            }
            Tok::Kw("using") => self.parse_using(),
            Tok::Kw("typedef") => self.parse_typedef(),
            Tok::Kw("struct") => self.parse_struct(),
            Tok::Kw(k) if unsupported_item_kw(k).is_some() => Err(self.unsupported(
                span,
                unsupported_item_kw(k).unwrap_or("that"),
            )),
            Tok::Punct("{" | "}") => Err(self.expected("a declaration")),
            _ => self.parse_func_or_global(),
        }
    }

    /// `#include <vector>` / `#include "x.h"`; any other directive is not stocked.
    fn parse_include(&self, text: &str, span: Span) -> PResult<ItemKind> {
        let body = text.trim_start_matches('#').trim_start();
        let Some(rest) = body.strip_prefix("include") else {
            return Err(preproc_diag(text, span));
        };
        let rest = rest.trim();
        let bad = || {
            Diag::at(
                span,
                "parse.expected",
                "I expected <header> or \"header\" after `#include`",
            )
        };
        let (open, close, angle) = if rest.starts_with('<') {
            ('<', '>', true)
        } else if rest.starts_with('"') {
            ('"', '"', false)
        } else {
            return Err(bad());
        };
        let inner = &rest[1..];
        let Some(end) = inner.find(close) else {
            return Err(bad());
        };
        let tail = inner[end + 1..].trim();
        if !(tail.is_empty() || tail.starts_with("//") || tail.starts_with("/*")) {
            return Err(bad());
        }
        let path = &inner[..end];
        if path.is_empty() || path.contains(open) && angle {
            return Err(bad());
        }
        Ok(ItemKind::Include {
            path: path.to_string(),
            angle,
        })
    }

    /// `using namespace std;` and `using Name = Type;`.
    fn parse_using(&mut self) -> PResult<ItemKind> {
        let span = self.cur_span();
        self.bump();
        if self.eat_kw("namespace") {
            if self.ident() == Some("std") {
                self.bump();
                self.expect_punct(";")?;
                self.using_std = true;
                return Ok(ItemKind::UsingNamespaceStd);
            }
            return Err(self.unsupported(span, "namespaces other than `std`"));
        }
        if self.ident().is_some() && self.at_punct_at(1, "=") {
            let name = self.expect_ident("a name")?;
            self.bump();
            let ty = self.parse_alias_type()?;
            self.expect_punct(";")?;
            self.declare_alias(&name, ty.clone());
            return Ok(ItemKind::Alias {
                name,
                ty,
                typedef: false,
            });
        }
        Err(self.unsupported(
            span,
            "`using` declarations (write `std::name`, or `using namespace std;`)",
        ))
    }

    /// The aliased type of a `using` / `typedef`: no `const` or `&` (an alias holds a plain type).
    fn parse_alias_type(&mut self) -> PResult<Ty> {
        let start = self.cur_span();
        let spec = self.parse_type_spec()?;
        let const_ptr = matches!(spec.ty, Ty::Ptr(_))
            && spec.spelled.as_deref().is_some_and(|s| s.contains("const"));
        if spec.konst || spec.is_ref || const_ptr {
            return Err(self.unsupported(
                self.span_from(start.start),
                "an alias for a `const` or reference type",
            ));
        }
        let mut has_auto = false;
        spec.ty.walk(&mut |t| has_auto |= *t == Ty::Auto);
        if has_auto {
            return Err(Diag::at(
                self.span_from(start.start),
                "parse.expected",
                "I expected a real type here, not `auto`",
            ));
        }
        Ok(spec.ty)
    }

    /// `typedef Type Name;`
    fn parse_typedef(&mut self) -> PResult<ItemKind> {
        self.bump();
        if self.at_kw("struct") || self.at_kw("enum") || self.at_kw("union") || self.at_kw("class") {
            return Err(self.unsupported(
                self.cur_span(),
                "`typedef` of a type definition (declare the `struct` directly)",
            ));
        }
        let ty = self.parse_alias_type()?;
        let name = self.expect_ident("the new type name")?;
        if self.at_punct("[") || self.at_punct("(") {
            return Err(self.unsupported(
                self.cur_span(),
                "`typedef` of arrays and function types (use `using`)",
            ));
        }
        self.expect_punct(";")?;
        self.declare_alias(&name, ty.clone());
        Ok(ItemKind::Alias {
            name,
            ty,
            typedef: true,
        })
    }

    // ---- structs ------------------------------------------------------------------------------

    /// `struct Name { fields; constructors; };`
    fn parse_struct(&mut self) -> PResult<ItemKind> {
        let start = self.cur_start();
        self.bump();
        let name = self.expect_ident("the name of the struct")?;
        if self.at_punct(":") {
            return Err(self.unsupported(self.cur_span(), "inheritance"));
        }
        if !self.at_punct("{") {
            return Err(self.expected("the '{' of the struct"));
        }
        let open_span = self.cur_span();
        self.bump();
        self.declare_struct(&name);
        let mut fields = Vec::new();
        let mut ctors = Vec::new();
        while !self.at_punct("}") {
            if self.at_eof() {
                return Err(Diag::at(
                    open_span,
                    "parse.expected",
                    "I expected a '}' to close this struct",
                ));
            }
            self.parse_member(&name, &mut fields, &mut ctors)?;
        }
        self.bump();
        self.expect_punct(";")?;
        let id = self.new_id(self.span_from(start));
        Ok(ItemKind::Struct(StructDecl {
            id,
            name,
            fields,
            ctors,
        }))
    }

    /// One member of a struct: a field declaration, a constructor, or an access label.
    fn parse_member(
        &mut self,
        owner: &str,
        fields: &mut Vec<Field>,
        ctors: &mut Vec<Ctor>,
    ) -> PResult<()> {
        let span = self.cur_span();
        if self.eat_punct(";") {
            return Ok(());
        }
        if self.at_kw("public") && self.at_punct_at(1, ":") {
            self.bump();
            self.bump();
            return Ok(());
        }
        if self.at_kw("private") || self.at_kw("protected") {
            return Err(self.unsupported(span, "`private` and `protected` (structs are all public)"));
        }
        if self.at_punct("~") {
            return Err(self.unsupported(span, "destructors"));
        }
        if let Some(Tok::Kw(k)) = self.tok().map(|t| t.kind.clone())
            && !self.at_builtin_type()
            && k != "const"
            && k != "constexpr"
            && k != "unsigned"
        {
            let what = match k {
                "static" => "`static` members",
                "using" | "typedef" => "type aliases inside a struct (put them above it)",
                "struct" => "structs inside structs",
                "template" => "templates",
                "explicit" => "`explicit` constructors",
                _ => "that kind of member",
            };
            return Err(self.unsupported(span, what));
        }
        if self.ident() == Some(owner) && self.at_punct_at(1, "(") {
            let c = self.parse_ctor()?;
            ctors.push(c);
            return Ok(());
        }
        let spec = self.parse_type_spec()?;
        if self.at_kw("operator") {
            return Err(self.unsupported(self.cur_span(), "operator overloading"));
        }
        loop {
            let name = self.expect_ident("a field name")?;
            if self.at_punct("(") {
                return Err(self.unsupported(
                    self.cur_span(),
                    "member functions (a struct only holds data and constructors)",
                ));
            }
            if self.at_punct("[") {
                return Err(self.unsupported(
                    self.cur_span(),
                    "C arrays as fields (use `std::array` or `std::vector`)",
                ));
            }
            let default = if self.eat_punct("=") {
                Some(self.parse_assign()?)
            } else if self.at_punct("{") {
                let start = self.cur_start();
                let elems = self.parse_brace_elems()?;
                Some(self.mk_expr(start, ExprKind::InitList(elems)))
            } else {
                None
            };
            fields.push(Field {
                spec: spec.clone(),
                name,
                default,
            });
            if !self.at_punct(",") {
                break;
            }
            if spec.is_ref || (spec.konst && matches!(spec.ty, Ty::Ptr(_))) {
                return Err(self.unsupported(
                    self.cur_span(),
                    "several names after a reference or `const` pointer type (declare one per line)",
                ));
            }
            self.bump();
            self.eat_stars(&spec.ty)?;
        }
        self.expect_punct(";")
    }

    /// `Name(params) : a(x), b{y} { body }` (cursor on the name).
    fn parse_ctor(&mut self) -> PResult<Ctor> {
        let start = self.cur_start();
        self.bump();
        let params = self.parse_params()?;
        let mut inits = Vec::new();
        if self.eat_punct(":") {
            loop {
                let member = self.expect_ident("a member name")?;
                let args = if self.at_punct("{") {
                    self.parse_brace_elems()?
                } else if self.at_punct("(") {
                    self.parse_call_args()?
                } else {
                    return Err(self.expected("a '(' after the member name"));
                };
                inits.push((member, args));
                if !self.eat_punct(",") {
                    break;
                }
            }
        }
        if !self.at_punct("{") {
            return Err(self.unsupported(
                self.cur_span(),
                "constructors without a body (`= default`, `= delete`, declarations)",
            ));
        }
        let body = self.parse_block()?;
        let id = self.new_id(self.span_from(start));
        Ok(Ctor {
            id,
            params,
            inits,
            body,
        })
    }

    // ---- functions and global constants -------------------------------------------------------

    /// `[static] [inline] [constexpr] Type name(params) { body }`, a forward declaration, or a global
    /// constant `const int N = 5;`.
    fn parse_func_or_global(&mut self) -> PResult<ItemKind> {
        let start = self.cur_start();
        let mut is_static = false;
        let mut constexpr_ = false;
        loop {
            if self.at_kw("static") || self.at_kw("inline") {
                is_static = true;
                self.bump();
            } else if self.at_kw("constexpr") {
                constexpr_ = true;
                self.bump();
            } else {
                break;
            }
        }
        let mut spec = self.parse_type_spec()?;
        if self.ident().is_some() && self.at_punct_at(1, "(") {
            if constexpr_ {
                return Err(self.unsupported(self.span_from(start), "`constexpr` functions"));
            }
            return self.parse_function(start, spec, is_static);
        }
        if constexpr_ {
            spec.konst = true;
        }
        if !spec.konst {
            return Err(self.unsupported(
                self.span_from(start),
                "global variables that are not `const` (declare it inside a function and pass it along)",
            ));
        }
        let vars = self.parse_declarators(&spec)?;
        self.expect_punct(";")?;
        self.register_consts(&spec, &vars);
        Ok(ItemKind::Global {
            spec,
            constexpr_,
            vars,
        })
    }

    /// The rest of a function after its return type (cursor on the name).
    fn parse_function(&mut self, start: u32, ret: TypeSpec, is_static: bool) -> PResult<ItemKind> {
        let name = self.expect_ident("the function name")?;
        let params = self.parse_params()?;
        if self.at_kw("noexcept") || self.at_kw("const") {
            return Err(self.unsupported(self.cur_span(), "`noexcept` and `const` on functions"));
        }
        let body = if self.eat_punct(";") {
            None
        } else if self.at_punct("{") {
            let saved = std::mem::take(&mut self.switch_depth);
            let b = self.parse_block();
            self.switch_depth = saved;
            Some(b?)
        } else {
            return Err(self.expected("the '{' of the function body, or a ';'"));
        };
        let id = self.new_id(self.span_from(start));
        Ok(ItemKind::Func(FnDecl {
            id,
            name,
            params,
            ret,
            is_static,
            body,
            comments: Vec::new(),
        }))
    }
}

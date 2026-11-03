// SPDX-License-Identifier: Apache-2.0
//! Unit tests of the type and expression parser.

use super::*;

/// A compact s-expression for an expression, for readable assertions.
fn sx(e: &Expr) -> String {
    let list = |v: &[Expr]| v.iter().map(sx).collect::<Vec<_>>().join(" ");
    match &e.kind {
        ExprKind::Int { value, suffix, .. } => match suffix {
            IntSuffix::None => value.to_string(),
            s => format!("{value}{s:?}"),
        },
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::Char(c) => format!("'{}'", *c as char),
        ExprKind::Str(s) => format!("{s:?}"),
        ExprKind::NullPtr => "nullptr".into(),
        ExprKind::Var(n) => n.clone(),
        ExprKind::Path(segs) => segs
            .iter()
            .map(|s| match &s.targs {
                None => s.name.clone(),
                Some(a) => format!(
                    "{}<{}>",
                    s.name,
                    a.iter()
                        .map(|t| match t {
                            TArg::Ty(t) => t.to_string(),
                            TArg::Int(i) => i.to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            })
            .collect::<Vec<_>>()
            .join("::"),
        ExprKind::Unary(op, a) => format!("({op:?} {})", sx(a)),
        ExprKind::Binary(op, a, b) => format!("({} {} {})", op.text(), sx(a), sx(b)),
        ExprKind::Assign { op, lhs, rhs } => {
            format!("({}= {} {})", op.map_or("", |o| o.text()), sx(lhs), sx(rhs))
        }
        ExprKind::Ternary(a, b, c) => format!("(? {} {} {})", sx(a), sx(b), sx(c)),
        ExprKind::Index(a, b) => format!("(idx {} {})", sx(a), sx(b)),
        ExprKind::Call { callee, args } => {
            format!("(call {} {})", sx(callee), list(args)).replace(" )", ")")
        }
        ExprKind::Member { obj, name, arrow } => {
            format!("({} {} {name})", if *arrow { "->" } else { "." }, sx(obj))
        }
        ExprKind::Cast { style, ty, e } => format!("({style:?} {} {})", tx(ty), sx(e)),
        ExprKind::Lambda(l) => format!(
            "(lambda {:?}{} [{}] ({}){}{} {})",
            l.captures.default,
            l.captures
                .items
                .iter()
                .map(|(n, r)| format!(" {}{n}", if *r { "&" } else { "" }))
                .collect::<String>(),
            "",
            l.params
                .iter()
                .map(|p| format!("{} {}", tx(&p.spec), p.name))
                .collect::<Vec<_>>()
                .join(", "),
            if l.mutable { " mutable" } else { "" },
            l.ret
                .as_ref()
                .map_or(String::new(), |r| format!(" -> {}", tx(r))),
            l.body.stmts.len()
        ),
        ExprKind::InitList(a) => format!("{{{}}}", list(a)),
        ExprKind::Construct { ty, args, brace } => format!(
            "(new-{} {} {})",
            if *brace { "brace" } else { "paren" },
            tx(ty),
            list(args)
        )
        .replace(" )", ")"),
        ExprKind::New { ty, args, brace } => format!(
            "(new{} {} {})",
            if *brace { "{}" } else { "" },
            tx(ty),
            list(args)
        )
        .replace(" )", ")"),
        ExprKind::Delete(a) => format!("(delete {})", sx(a)),
    }
}

/// A type spec as `const X&` with the spelling in brackets when it was kept.
fn tx(t: &TypeSpec) -> String {
    format!(
        "{}{}{}{}",
        if t.konst { "const " } else { "" },
        t.ty,
        t.spelled
            .as_ref()
            .map_or(String::new(), |s| format!("[{s}]")),
        if t.is_ref { "&" } else { "" }
    )
}

fn ex(src: &str) -> String {
    match parse_expr_str(src) {
        Ok(e) => sx(&e),
        Err(d) => panic!("`{src}` failed: {:?}", d),
    }
}

fn exs(src: &str) -> String {
    match parse_expr_str_with(src, &ParseOpts::default(), true) {
        Ok(e) => sx(&e),
        Err(d) => panic!("`{src}` failed: {:?}", d),
    }
}

fn exk(src: &str) -> String {
    let opts = ParseOpts {
        first_id: 0,
        known_types: vec!["ListNode".into(), "TreeNode".into(), "ll".into()],
    };
    match parse_expr_str_with(src, &opts, false) {
        Ok(e) => sx(&e),
        Err(d) => panic!("`{src}` failed: {:?}", d),
    }
}

fn err(src: &str) -> Diag {
    match parse_expr_str(src) {
        Ok(e) => panic!("`{src}` should fail but gave {}", sx(&e)),
        Err(mut d) => d.remove(0),
    }
}

fn ty(src: &str, using_std: bool) -> Result<TypeSpec, Diag> {
    let opts = ParseOpts {
        first_id: 0,
        known_types: vec!["ListNode".into()],
    };
    let mut p = Parser::new(src, &opts)?;
    p.using_std = using_std;
    p.declare_alias("vi", Ty::vector(Ty::Int));
    let t = p.parse_type_spec()?;
    assert!(p.at_eof(), "`{src}`: left over tokens");
    Ok(t)
}

#[test]
fn binary_precedence_and_associativity() {
    assert_eq!(ex("1 + 2 * 3"), "(+ 1 (* 2 3))");
    assert_eq!(ex("a - b - c"), "(- (- a b) c)");
    assert_eq!(ex("a << 1 + 2"), "(<< a (+ 1 2))");
    assert_eq!(ex("a & b == c"), "(& a (== b c))");
    assert_eq!(ex("a || b && c"), "(|| a (&& b c))");
    assert_eq!(ex("a | b ^ c & d"), "(| a (^ b (& c d)))");
    assert_eq!(ex("a < b == c > d"), "(== (< a b) (> c d))");
    assert_eq!(ex("(a + b) * c"), "(* (+ a b) c)");
    assert_eq!(ex("a % b / c * d"), "(* (/ (% a b) c) d)");
    assert_eq!(ex("a >> 2 >> 1"), "(>> (>> a 2) 1)");
    assert_eq!(ex("a >= b && c <= d"), "(&& (>= a b) (<= c d))");
    assert_eq!(ex("a and b or not c"), "(|| (&& a b) (Not c))");
}

#[test]
fn assignment_ternary_comma() {
    assert_eq!(ex("a = b = 3"), "(= a (= b 3))");
    assert_eq!(ex("x += y * 2"), "(+= x (* y 2))");
    assert_eq!(ex("x <<= 2"), "(<<= x 2)");
    assert_eq!(ex("x >>= 1"), "(>>= x 1)");
    assert_eq!(ex("x ^= y"), "(^= x y)");
    assert_eq!(ex("a ? b : c ? d : e"), "(? a b (? c d e))");
    assert_eq!(ex("x = a ? 1 : 2"), "(= x (? a 1 2))");
    assert_eq!(ex("a ? b : c = d"), "(? a b (= c d))");
    assert_eq!(ex("a ? b, c : d"), "(? a (, b c) d)");
    assert_eq!(ex("a = 1, b = 2"), "(, (= a 1) (= b 2))");
    assert_eq!(ex("++i, j--"), "(, (PreInc i) (PostDec j))");
    assert_eq!(ex("a == b ? x + 1 : y"), "(? (== a b) (+ x 1) y)");
    assert_eq!(ex("v[i] += v[i - 1]"), "(+= (idx v i) (idx v (- i 1)))");
}

#[test]
fn unary_and_postfix() {
    assert_eq!(ex("-x"), "(Neg x)");
    assert_eq!(ex("!a && b"), "(&& (Not a) b)");
    assert_eq!(ex("*p++"), "(Deref (PostInc p))");
    assert_eq!(ex("&x"), "(AddrOf x)");
    assert_eq!(ex("- -x"), "(Neg (Neg x))");
    assert_eq!(ex("~a + +b"), "(+ (BitNot a) (Plus b))");
    assert_eq!(ex("a[i][j]"), "(idx (idx a i) j)");
    assert_eq!(ex("a.b(c).d"), "(. (call (. a b) c) d)");
    assert_eq!(ex("p->next->val"), "(-> (-> p next) val)");
    assert_eq!(ex("v.size()"), "(call (. v size))");
    assert_eq!(ex("f(1, g(2), {3, 4})"), "(call f 1 (call g 2) {3 4})");
    assert_eq!(ex("-a * b"), "(* (Neg a) b)");
    assert_eq!(ex("*it"), "(Deref it)");
    assert_eq!(ex("(*it).second"), "(. (Deref it) second)");
    assert_eq!(ex("it->second++"), "(PostInc (-> it second))");
    assert_eq!(ex("m[k]++"), "(PostInc (idx m k))");
    assert_eq!(ex("x = *--p"), "(= x (Deref (PreDec p)))");
}

#[test]
fn literals() {
    assert_eq!(ex("42"), "42");
    assert_eq!(ex("10ull"), "10ULL");
    assert_eq!(ex("0x1F"), "31");
    assert_eq!(ex("'a'"), "'a'");
    assert_eq!(ex("\"ab\" \"cd\""), "\"abcd\"");
    assert_eq!(ex("true && false"), "(&& true false)");
    assert_eq!(ex("nullptr"), "nullptr");
    assert_eq!(ex("p == nullptr"), "(== p nullptr)");
    assert_eq!(ex("1'000'000"), "1000000");
}

#[test]
fn casts() {
    assert_eq!(
        ex("static_cast<int>(v.size())"),
        "(Static int (call (. v size)))"
    );
    assert_eq!(
        ex("static_cast<long long>(a) * b"),
        "(* (Static long long a) b)"
    );
    assert_eq!(ex("(int)x + 1"), "(+ (CStyle int x) 1)");
    assert_eq!(ex("(long long)a * b"), "(* (CStyle long long a) b)");
    assert_eq!(ex("(unsigned)-1"), "(CStyle unsigned (Neg 1))");
    assert_eq!(ex("(char)(c + 1)"), "(CStyle char (+ c 1))");
    assert_eq!(ex("(void)x"), "(CStyle void x)");
    assert_eq!(ex("(unsigned int)x"), "(CStyle unsigned[unsigned int] x)");
    assert_eq!(ex("int(x)"), "(Functional int x)");
    assert_eq!(ex("char(c + 1)"), "(Functional char (+ c 1))");
    assert_eq!(ex("bool(x)"), "(Functional bool x)");
    assert_eq!(ex("unsigned(x)"), "(Functional unsigned x)");
    assert_eq!(ex("int{}"), "(new-brace int)");
    assert_eq!(
        ex("static_cast<std::vector<int>>(x)"),
        "(Static std::vector<int> x)"
    );
    assert_eq!(ex("static_cast<const int&>(x)"), "(Static const int& x)");
    // a parenthesised name is not a cast
    assert_eq!(ex("(x) - 1"), "(- x 1)");
    assert_eq!(ex("(x)(y)"), "(call x y)");
    assert_eq!(ex("(a + b)"), "(+ a b)");
    assert_eq!(exk("(ll)a * b"), "(* (CStyle ll a) b)");
    assert_eq!(exk("(ListNode*)p"), "(CStyle ListNode* p)");
    assert_eq!(exk("(ll) - b"), "(CStyle ll (Neg b))");
    assert_eq!(exs("(size_t)i"), "(CStyle size_t i)");
    // under `using namespace std` a variable that has a library type name stays a variable
    assert_eq!(exs("(list) - 1"), "(- list 1)");
}

#[test]
fn constructor_calls_and_template_ids() {
    assert_eq!(
        ex("std::vector<int>(n, 0)"),
        "(new-paren std::vector<int> n 0)"
    );
    assert_eq!(
        ex("std::vector<std::vector<int>>(n, std::vector<int>(m))"),
        "(new-paren std::vector<std::vector<int>> n (new-paren std::vector<int> m))"
    );
    assert_eq!(ex("std::string(5, 'a')"), "(new-paren std::string 5 'a')");
    assert_eq!(ex("std::string{\"a\"}"), "(new-brace std::string \"a\")");
    assert_eq!(
        ex("std::pair<int, int>{1, 2}"),
        "(new-brace std::pair<int, int> 1 2)"
    );
    assert_eq!(
        ex("std::pair<int,int>(a, b)"),
        "(new-paren std::pair<int, int> a b)"
    );
    assert_eq!(ex("std::greater<int>()"), "(new-paren std::greater<int>)");
    assert_eq!(ex("std::size_t(x)"), "(Functional size_t[std::size_t] x)");
    assert_eq!(
        ex("std::string(\"ab\").size()"),
        "(call (. (new-paren std::string \"ab\") size))"
    );
    assert_eq!(ex("std::get<0>(t)"), "(call std::get<0> t)");
    assert_eq!(ex("std::get<1>(t) + 1"), "(+ (call std::get<1> t) 1)");
    assert_eq!(
        ex("std::numeric_limits<int>::max()"),
        "(call std::numeric_limits<int>::max)"
    );
    assert_eq!(
        ex("std::numeric_limits<long long>::min() / 2"),
        "(/ (call std::numeric_limits<long long>::min) 2)"
    );
    assert_eq!(ex("std::string::npos"), "std::string::npos");
    assert_eq!(
        ex("s.find('a') == std::string::npos"),
        "(== (call (. s find) 'a') std::string::npos)"
    );
    assert_eq!(ex("std::max(a, b)"), "(call std::max a b)");
    assert_eq!(ex("std::max<int>(a, b)"), "(call std::max<int> a b)");
    assert_eq!(ex("std::make_pair(1, 2)"), "(call std::make_pair 1 2)");
    assert_eq!(
        ex("std::sort(v.begin(), v.end())"),
        "(call std::sort (call (. v begin)) (call (. v end)))"
    );
    assert_eq!(ex("std::nullopt"), "std::nullopt");
    // a comparison is not a template
    assert_eq!(ex("std::cout < x"), "(< std::cout x)");
}

#[test]
fn using_namespace_std_names() {
    assert_eq!(
        exs("vector<int>(n)"),
        "(new-paren std::vector<int>[vector<int>] n)"
    );
    assert_eq!(
        exs("vector<vector<int>>(n, vector<int>(m, 0))"),
        "(new-paren std::vector<std::vector<int>>[vector<vector<int>>] n \
         (new-paren std::vector<int>[vector<int>] m 0))"
    );
    assert_eq!(exs("max(a, b)"), "(call max a b)");
    assert_eq!(exs("max<int>(a, b)"), "(call max<int> a b)");
    assert_eq!(exs("get<0>(t)"), "(call get<0> t)");
    assert_eq!(exs("count < n"), "(< count n)");
    assert_eq!(exs("max < b"), "(< max b)");
    assert_eq!(
        exs("string(\"a\")"),
        "(new-paren std::string[string] \"a\")"
    );
    assert_eq!(exs("string::npos"), "string::npos");
    assert_eq!(
        exs("numeric_limits<int>::max()"),
        "(call numeric_limits<int>::max)"
    );
    assert_eq!(
        exs("map<int, int>{}"),
        "(new-brace std::map<int, int>[map<int, int>])"
    );
    assert_eq!(exs("set[x]"), "(idx set x)");
    assert_eq!(
        exs("pair<int, int>(1, 2)"),
        "(new-paren std::pair<int, int>[pair<int, int>] 1 2)"
    );
    // without the using directive the same names are plain variables
    assert_eq!(ex("max(a, b)"), "(call max a b)");
    assert_eq!(ex("vector < x"), "(< vector x)");
}

#[test]
fn known_types_new_delete() {
    assert_eq!(exk("ListNode(1)"), "(new-paren ListNode 1)");
    assert_eq!(
        exk("TreeNode{0, nullptr, nullptr}"),
        "(new-brace TreeNode 0 nullptr nullptr)"
    );
    assert_eq!(exk("new ListNode(x)"), "(new ListNode x)");
    assert_eq!(exk("new TreeNode"), "(new TreeNode)");
    assert_eq!(exk("new TreeNode{1}"), "(new{} TreeNode 1)");
    assert_eq!(
        exk("cur->next = new ListNode(a + b)"),
        "(= (-> cur next) (new ListNode (+ a b)))"
    );
    assert_eq!(exk("delete p"), "(delete p)");
    assert_eq!(exk("delete p->next"), "(delete (-> p next))");
    assert_eq!(ex("new int(5)"), "(new int 5)");
    assert_eq!(ex("new std::vector<int>(3)"), "(new std::vector<int> 3)");
    assert_eq!(exk("ll(x) * 2"), "(* (new-paren ll x) 2)");
    // an unknown name is a plain function call
    assert_eq!(ex("Foo(1)"), "(call Foo 1)");
    let d = err("new int[5]");
    assert_eq!(d.code, "parse.unsupported");
    assert_eq!(err("delete[] p").code, "parse.unsupported");
}

#[test]
fn lambdas_and_brace_lists() {
    assert_eq!(
        ex("[&](int a, int b) { return a < b; }"),
        "(lambda Ref [] (int a, int b) 1)"
    );
    assert_eq!(ex("[=]() { return x; }"), "(lambda Copy [] () 1)");
    assert_eq!(
        ex("[](auto a) { return a; }"),
        "(lambda None [] (auto a) 1)"
    );
    assert_eq!(ex("[&x, y] { return 1; }"), "(lambda None &x y [] () 1)");
    assert_eq!(
        ex("[&, x](const auto& v) { }"),
        "(lambda Ref x [] (const auto& v) 0)"
    );
    assert_eq!(
        ex("[&](const std::vector<int>& v) mutable -> int { return 1; }"),
        "(lambda Ref [] (const std::vector<int>& v) mutable -> int 1)"
    );
    assert_eq!(ex("[&](int, int) { }"), "(lambda Ref [] (int , int ) 0)");
    assert_eq!(
        ex("std::sort(v.begin(), v.end(), [](int a, int b) { return a > b; })"),
        "(call std::sort (call (. v begin)) (call (. v end)) (lambda None [] (int a, int b) 1))"
    );
    assert_eq!(ex("[&] { return 1; }()"), "(call (lambda Ref [] () 1))");
    assert_eq!(ex("{1, 2, 3}"), "{1 2 3}");
    assert_eq!(ex("{{1, 2}, {3, 4},}"), "{{1 2} {3 4}}");
    assert_eq!(ex("{}"), "{}");
    assert_eq!(ex("x = {1, 2}"), "(= x {1 2})");
    assert_eq!(err("[x = 1] { }").code, "parse.unsupported");
    assert_eq!(err("[this] { }").code, "parse.unsupported");
}

#[test]
fn friendly_errors() {
    let d = err("1 +");
    assert_eq!(d.code, "parse.expected");
    assert!(d.msg.contains("expression"), "{}", d.msg);
    assert!(d.span.is_some());
    assert_eq!(err("(1 + 2").msg, "I expected a ')' here");
    assert_eq!(err("a[1").msg, "I expected a ']' here");
    assert_eq!(err("f(1, 2").msg, "I expected a ')' here");
    assert_eq!(err("a ? b").msg, "I expected a ':' here");
    assert_eq!(err("a b").code, "parse.unexpected");
    assert_eq!(err("a.").code, "parse.expected");
    assert_eq!(err(")").code, "parse.expected");
    for (src, what) in [
        ("3.5", "decimal"),
        ("sizeof(int)", "sizeof"),
        ("throw 1", "throw"),
        ("(double)x", "decimal"),
        ("(long)x", "long"),
        ("long(x)", "long"),
        ("static_cast<short>(x)", "short"),
        ("reinterpret_cast<int*>(p)", "cast"),
        ("this", "this"),
        ("std::multiset<int>()", "multiset"),
        ("std::int64_t(x)", "int64_t"),
        ("[](int&& x) { }", "rvalue"),
    ] {
        let d = err(src);
        assert_eq!(d.code, "parse.unsupported", "{src}");
        assert!(d.msg.contains(what), "{src}: {}", d.msg);
        assert!(d.msg.starts_with("this office does not stock"), "{}", d.msg);
        assert!(d.span.is_some());
    }
}

#[test]
fn depth_limit_and_long_chains() {
    let deep = |n: usize| format!("{}x{}", "(".repeat(n), ")".repeat(n));
    // run on a thread with the default test stack: the limit must keep us well inside it
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(move || {
            assert!(parse_expr_str(&deep(150)).is_ok());
            assert_eq!(
                parse_expr_str(&deep(5000)).unwrap_err()[0].code,
                "parse.depth"
            );
            let neg = format!("{}x", "-".repeat(5000));
            assert_eq!(parse_expr_str(&neg).unwrap_err()[0].code, "parse.depth");
            let sum = vec!["1"; 5000].join(" + ");
            assert_eq!(parse_expr_str(&sum).unwrap_err()[0].code, "parse.depth");
            let calls = format!("x{}", ".f()".repeat(5000));
            assert_eq!(parse_expr_str(&calls).unwrap_err()[0].code, "parse.depth");
            let assign = format!("{}1", "a = ".repeat(5000));
            assert_eq!(parse_expr_str(&assign).unwrap_err()[0].code, "parse.depth");
            let nested = format!("{}1{}", "f(".repeat(5000), ")".repeat(5000));
            assert_eq!(parse_expr_str(&nested).unwrap_err()[0].code, "parse.depth");
            let braces = format!("{}1{}", "{".repeat(5000), "}".repeat(5000));
            assert_eq!(parse_expr_str(&braces).unwrap_err()[0].code, "parse.depth");
            let tern = format!("{}1", "a ? 1 : ".repeat(5000));
            assert_eq!(parse_expr_str(&tern).unwrap_err()[0].code, "parse.depth");
            let templ = format!("{}int{}", "std::vector<".repeat(5000), ">".repeat(5000));
            assert!(parse_expr_str(&templ).is_err());
            // a realistic long expression is fine
            let ok = vec!["a"; 100].join(" + ");
            assert!(parse_expr_str(&ok).is_ok());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn node_ids_and_spans() {
    let src = "(a + b) * f(c)[2]";
    let opts = ParseOpts {
        first_id: 100,
        known_types: vec![],
    };
    let mut p = Parser::new(src, &opts).unwrap();
    let e = p.parse_expr().unwrap();
    let mut ids = Vec::new();
    walk_expr(&e, &mut |x| ids.push(x.id));
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "ids are unique");
    assert!(ids.iter().all(|&i| i >= 100 && i < p.next_id));
    assert_eq!(p.next_id as usize, 100 + ids.len());
    assert_eq!(p.spans.len(), p.next_id as usize);
    let text = |id: NodeId| {
        let s = p.span_of(id);
        &src[s.start as usize..s.end as usize]
    };
    assert_eq!(text(e.id), src);
    let ExprKind::Binary(_, lhs, rhs) = &e.kind else {
        panic!("not a product")
    };
    assert_eq!(
        text(lhs.id),
        "(a + b)",
        "parentheses belong to the inner span"
    );
    assert_eq!(text(rhs.id), "f(c)[2]");
    walk_expr(&e, &mut |x| {
        assert!(p.span_of(x.id).end > p.span_of(x.id).start)
    });
}

#[test]
fn error_spans_point_at_the_problem() {
    let d = err("a + * ;");
    assert_eq!(d.span.unwrap().start, 6);
    let d = err("foo(1,");
    let s = d.span.unwrap();
    assert!(s.end as usize <= 6 && s.end > s.start);
}

// ---- types ---------------------------------------------------------------------------------------

#[test]
fn builtin_and_std_types() {
    let t = ty("const std::vector<int>&", false).unwrap();
    assert_eq!(
        (t.ty.clone(), t.konst, t.is_ref, t.spelled.clone()),
        (Ty::vector(Ty::Int), true, true, None)
    );
    assert_eq!(tx(&ty("int", false).unwrap()), "int");
    assert_eq!(tx(&ty("unsigned", false).unwrap()), "unsigned");
    assert_eq!(
        tx(&ty("unsigned int", false).unwrap()),
        "unsigned[unsigned int]"
    );
    assert_eq!(tx(&ty("long long", false).unwrap()), "long long");
    assert_eq!(
        tx(&ty("long long int", false).unwrap()),
        "long long[long long int]"
    );
    assert_eq!(
        tx(&ty("unsigned long long", false).unwrap()),
        "unsigned long long"
    );
    assert_eq!(tx(&ty("signed", false).unwrap()), "int[signed]");
    assert_eq!(tx(&ty("size_t", false).unwrap()), "size_t");
    assert_eq!(
        tx(&ty("std::size_t", false).unwrap()),
        "size_t[std::size_t]"
    );
    assert_eq!(tx(&ty("std::string", false).unwrap()), "std::string");
    assert_eq!(
        tx(&ty("const std::string&", false).unwrap()),
        "const std::string&"
    );
    assert_eq!(
        tx(&ty("std::string const&", false).unwrap()),
        "const std::string&"
    );
    assert_eq!(tx(&ty("auto", false).unwrap()), "auto");
    assert_eq!(tx(&ty("const auto&", false).unwrap()), "const auto&");
    assert_eq!(tx(&ty("auto&", false).unwrap()), "auto&");
    assert_eq!(tx(&ty("void", false).unwrap()), "void");
    assert_eq!(tx(&ty("bool", false).unwrap()), "bool");
    assert_eq!(
        tx(&ty("std::vector<std::vector<int> >", false).unwrap()),
        "std::vector<std::vector<int>>"
    );
    assert_eq!(
        tx(&ty("std::vector<std::vector<std::pair<int,int>>>&", false).unwrap()),
        "std::vector<std::vector<std::pair<int, int>>>&"
    );
    assert_eq!(
        tx(&ty("std::map<std::string,int>", false).unwrap()),
        "std::map<std::string, int>"
    );
    assert_eq!(
        tx(&ty("std::unordered_map<int, std::vector<int>>", false).unwrap()),
        "std::unordered_map<int, std::vector<int>>"
    );
    assert_eq!(
        tx(&ty("std::array<int, 5>", false).unwrap()),
        "std::array<int, 5>"
    );
    assert_eq!(
        tx(&ty("std::tuple<int, char, bool>", false).unwrap()),
        "std::tuple<int, char, bool>"
    );
    assert_eq!(
        tx(&ty("std::optional<std::string>", false).unwrap()),
        "std::optional<std::string>"
    );
    assert_eq!(
        tx(&ty("std::priority_queue<int>", false).unwrap()),
        "std::priority_queue<int>"
    );
    assert_eq!(
        tx(&ty(
            "std::priority_queue<int, std::vector<int>, std::greater<int>>",
            false
        )
        .unwrap()),
        "std::priority_queue<int, std::vector<int>, std::greater<int>>"
    );
    let t = ty(
        "std::priority_queue<int, std::vector<int>, decltype(cmp)>",
        false,
    )
    .unwrap();
    assert_eq!(
        t.ty,
        Ty::PriorityQueue(
            Box::new(Ty::Int),
            Box::new(Ty::Struct("decltype(cmp)".into()))
        )
    );
    assert_eq!(
        tx(&ty("std::function<bool(const int&, int)>", false).unwrap()),
        "std::function<bool(int, int)>[std::function<bool(const int&, int)>]"
    );
    assert_eq!(
        tx(&ty("std::function<void()>", false).unwrap()),
        "std::function<void()>"
    );
    assert_eq!(
        tx(&ty("std::stack<char>", false).unwrap()),
        "std::stack<char>"
    );
    assert_eq!(
        tx(&ty("std::set<long long>", false).unwrap()),
        "std::set<long long>"
    );
}

#[test]
fn pointers_aliases_and_iterators() {
    assert_eq!(tx(&ty("ListNode*", false).unwrap()), "ListNode*");
    assert_eq!(tx(&ty("ListNode *", false).unwrap()), "ListNode*");
    assert_eq!(tx(&ty("ListNode*&", false).unwrap()), "ListNode*&");
    assert_eq!(tx(&ty("ListNode**", false).unwrap()), "ListNode**");
    assert_eq!(
        tx(&ty("ListNode* const", false).unwrap()),
        "const ListNode*"
    );
    assert_eq!(
        tx(&ty("const ListNode*", false).unwrap()),
        "ListNode*[const ListNode*]"
    );
    assert_eq!(
        tx(&ty("ListNode const*", false).unwrap()),
        "ListNode*[ListNode const*]"
    );
    assert_eq!(tx(&ty("const int*&", false).unwrap()), "int*[const int*]&");
    assert_eq!(tx(&ty("vi", false).unwrap()), "std::vector<int>[vi]");
    assert_eq!(
        tx(&ty("const vi&", false).unwrap()),
        "const std::vector<int>[vi]&"
    );
    assert_eq!(
        tx(&ty("std::vector<vi>", false).unwrap()),
        "std::vector<std::vector<int>>[std::vector<vi>]"
    );
    assert_eq!(
        ty("std::vector<int>::iterator", false).unwrap().ty,
        Ty::Iter {
            container: Box::new(Ty::vector(Ty::Int)),
            reverse: false
        }
    );
    assert_eq!(
        ty("std::string::const_reverse_iterator", false).unwrap().ty,
        Ty::Iter {
            container: Box::new(Ty::Str),
            reverse: true
        }
    );
    assert_eq!(
        tx(&ty("vector<int>", true).unwrap()),
        "std::vector<int>[vector<int>]"
    );
    assert_eq!(
        tx(&ty("const string&", true).unwrap()),
        "const std::string[string]&"
    );
    assert_eq!(
        tx(&ty("vector<vector<int>>&", true).unwrap()),
        "std::vector<std::vector<int>>[vector<vector<int>>]&"
    );
    assert_eq!(
        tx(&ty("priority_queue<int, vector<int>, greater<int>>", true).unwrap()),
        "std::priority_queue<int, std::vector<int>, std::greater<int>>[priority_queue<int, vector<int>, greater<int>>]"
    );
    assert!(
        ty("vector<int>", false).is_err(),
        "unqualified std names need the using directive"
    );
    assert!(ty("Foo", false).is_err());
}

#[test]
fn unsupported_types() {
    for (src, what) in [
        ("long", "long"),
        ("unsigned long", "unsigned long"),
        ("short", "short"),
        ("double", "decimal"),
        ("float", "decimal"),
        ("unsigned short", "unsigned short"),
        ("int&&", "rvalue"),
        ("auto&&", "rvalue"),
        ("std::vector<int>&&", "rvalue"),
        ("std::multiset<int>", "multiset"),
        ("std::int64_t", "long long"),
        ("std::unique_ptr<int>", "unique_ptr"),
        ("std::map<int, int, std::greater<int>>", "comparators"),
        ("std::set<int, std::greater<int>>", "comparators"),
        ("std::stack<int, std::vector<int>>", "comparators"),
        (
            "std::priority_queue<int, std::deque<int>>",
            "priority_queue",
        ),
        ("const int* const", "const pointer"),
    ] {
        let d = ty(src, false).unwrap_err();
        assert_eq!(d.code, "parse.unsupported", "{src}: {}", d.msg);
        assert!(d.msg.contains(what), "{src}: {}", d.msg);
    }
    assert!(
        ty("std::vector", false)
            .unwrap_err()
            .msg
            .contains("std::vector<int>")
    );
    assert!(
        ty("std::vector<>", false)
            .unwrap_err()
            .msg
            .contains("one type")
    );
    assert!(
        ty("std::pair<int>", false)
            .unwrap_err()
            .msg
            .contains("two types")
    );
    assert!(
        ty("std::vector<int", false)
            .unwrap_err()
            .msg
            .contains("'>'")
    );
    assert!(
        ty("std::array<int, N>", false)
            .unwrap_err()
            .msg
            .contains("const")
    );
}

#[test]
fn array_sizes_from_declared_consts() {
    let mut p = Parser::new("std::array<int, N>", &ParseOpts::default()).unwrap();
    p.declare_const("N", 7);
    assert_eq!(
        p.parse_type_spec().unwrap().ty,
        Ty::Array(Box::new(Ty::Int), 7)
    );
}

#[test]
fn backtracking_undoes_shift_splits() {
    let mut p = Parser::new("vector<vector<int>> x", &ParseOpts::default()).unwrap();
    p.using_std = true;
    let before: Vec<_> = p.toks.iter().map(|t| (t.kind.clone(), t.span)).collect();
    let m = p.mark();
    assert!(p.parse_type_spec().is_ok());
    assert_eq!(p.ident(), Some("x"));
    p.restore(m);
    let after: Vec<_> = p.toks.iter().map(|t| (t.kind.clone(), t.span)).collect();
    assert_eq!(before, after);
    assert!(p.parse_type_spec().is_ok());
}

#[test]
fn declaration_lookahead() {
    let check = |src: &str, known: &[&str], using_std: bool| {
        let opts = ParseOpts {
            first_id: 0,
            known_types: known.iter().map(|s| s.to_string()).collect(),
        };
        let mut p = Parser::new(src, &opts).unwrap();
        p.using_std = using_std;
        let r = p.looks_like_decl();
        assert_eq!(p.pos, 0);
        r
    };
    assert!(check("int x = 0;", &[], false));
    assert!(check("auto it = m.begin();", &[], false));
    assert!(check("const int N = 5;", &[], false));
    assert!(check("static int memo[10];", &[], false));
    assert!(check("std::vector<int> v;", &[], false));
    assert!(check("std::string s;", &[], false));
    assert!(check("std::vector<std::vector<int>> g(n);", &[], false));
    assert!(check("vector<int> v;", &[], true));
    assert!(check("ListNode* p = head;", &["ListNode"], false));
    assert!(check("ll x;", &["ll"], false));
    assert!(check("size_t i = 0;", &[], false));
    assert!(check("std::multiset<int> s;", &[], false));
    assert!(!check("std::sort(v.begin(), v.end());", &[], false));
    assert!(!check("std::cout << x;", &[], false));
    assert!(!check("x = 5;", &[], false));
    assert!(!check("vector<int> v;", &[], false));
    assert!(!check("a * b;", &[], false));
    assert!(!check("list = 5;", &[], true));
    assert!(!check("ListNode(1);", &["ListNode"], false));
    assert!(!check("f(x);", &[], false));
}

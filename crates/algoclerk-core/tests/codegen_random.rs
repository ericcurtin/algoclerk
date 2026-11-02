// SPDX-License-Identifier: Apache-2.0
//! Random type-correct programs: the interpreter and the generated C, C++ and Rust (compiled with every
//! available compiler) must print identical results.

use algoclerk_core::codegen::compile::{available, compile_and_run};
use algoclerk_core::codegen::{Lang, generate, harness};
use algoclerk_core::judge::canonical;
use algoclerk_core::lang::ast::Ty;
use algoclerk_core::lang::check::{Allowed, CheckOpts, EntrySig};
use algoclerk_core::lang::interp::Limits;
use algoclerk_core::lang::parse::parse;
use algoclerk_core::lang::run::compile;
use algoclerk_core::level::Compare;
use algoclerk_core::testgen::Rng;

struct G<'a> {
    rng: &'a Rng,
    out: String,
    ind: usize,
    scopes: Vec<Vec<String>>,
    counter: usize,
}

impl G<'_> {
    fn pick<'b>(&self, xs: &[&'b str]) -> &'b str {
        xs[self.rng.range(0, xs.len() as i64 - 1) as usize]
    }
    fn int_vars(&self) -> Vec<String> {
        let mut v = vec!["x".to_string(), "y".to_string()];
        for s in &self.scopes {
            v.extend(s.iter().cloned());
        }
        v
    }
    fn int(&mut self, d: u32) -> String {
        if d == 0 || self.rng.chance(25) {
            return match self.rng.range(0, 3) {
                0 => self.rng.range(0, 9).to_string(),
                _ => {
                    let vars = self.int_vars();
                    vars[self.rng.range(0, vars.len() as i64 - 1) as usize].clone()
                }
            };
        }
        let (a, b) = (self.int(d - 1), self.int(d - 1));
        match self.rng.range(0, 21) {
            0 => format!("({a} + {b})"),
            1 => format!("({a} - {b})"),
            2 => format!("(({a} * {b}) % 1000)"),
            3 => format!("({a} / {})", self.rng.range(1, 5)),
            4 => format!("({a} % {})", self.rng.range(1, 7)),
            5 => format!("abs({a})"),
            6 => format!("max({a}, {b})"),
            7 => format!("min({a}, {b})"),
            8 => "len(v)".into(),
            9 => "len(s)".into(),
            10 => format!("v[abs({a}) % len(v)]"),
            11 => format!("m.get({a} % 20, 7)"),
            12 => format!("ord(s[abs({a}) % len(s)])"),
            13 => "sum(v)".into(),
            14 => format!("to_int(to_string({a} % 100))"),
            15 => format!("(-{a})"),
            16 => format!("({a} & {b})"),
            17 => format!("({a} | {b})"),
            18 => format!("({a} ^ {b})"),
            19 => format!("({a} << {}) % 1000", self.rng.range(0, 3)),
            20 => format!("s.find({})", self.str_lit()),
            _ => format!("t.{}", self.rng.range(0, 1)),
        }
    }
    fn str_lit(&mut self) -> String {
        format!("\"{}\"", self.pick(&["a", "b", "ab", "xyz", "", "Hi", "a b"]))
    }
    fn boolean(&mut self, d: u32) -> String {
        if d == 0 || self.rng.chance(20) {
            return self.pick(&["true", "false", "(x < y)", "v.is_empty()", "(len(s) > 2)"]).to_string();
        }
        let (a, b) = (self.int(d - 1), self.int(d - 1));
        match self.rng.range(0, 12) {
            0 => format!("({a} < {b})"),
            1 => format!("({a} <= {b})"),
            2 => format!("({a} == {b})"),
            3 => format!("({a} != {b})"),
            4 => format!("({a} >= {b})"),
            5 => format!("(not {})", self.boolean(d - 1)),
            6 => format!("({} and {})", self.boolean(d - 1), self.boolean(d - 1)),
            7 => format!("({} or {})", self.boolean(d - 1), self.boolean(d - 1)),
            8 => format!("v.contains({a} % 10)"),
            9 => format!("m.contains({a} % 20)"),
            10 => format!("st.contains({a} % 20)"),
            11 => format!("s.contains({})", self.str_lit()),
            _ => format!("({} == {})", self.string(d - 1), self.string(d - 1)),
        }
    }
    fn string(&mut self, d: u32) -> String {
        if d == 0 || self.rng.chance(30) {
            return if self.rng.chance(50) { "s".into() } else { self.str_lit() };
        }
        let a = self.int(d - 1);
        match self.rng.range(0, 9) {
            0 => format!("({} + {})", self.string(d - 1), self.string(d - 1)),
            1 => format!("to_string({a})"),
            2 => format!("{}.upper()", self.string(d - 1)),
            3 => format!("{}.lower()", self.string(d - 1)),
            4 => format!("sorted({})", self.string(d - 1)),
            5 => format!("reversed({})", self.string(d - 1)),
            6 => format!("join(sorted({}.split('a')), \"-\")", self.string(d - 1)),
            7 => format!("to_string(s[abs({a}) % len(s)])"),
            8 => format!("{}.trim()", self.string(d - 1)),
            _ => format!("({} + s[abs({a}) % len(s)])", self.string(d - 1)),
        }
    }
    fn vec(&mut self, d: u32) -> String {
        if d == 0 || self.rng.chance(30) {
            return "v".into();
        }
        let a = self.int(d - 1);
        match self.rng.range(0, 7) {
            0 => format!("sorted({})", self.vec(d - 1)),
            1 => format!("reversed({})", self.vec(d - 1)),
            2 => format!("({} + [{a} % 50, 1])", self.vec(d - 1)),
            3 => "m.keys()".into(),
            4 => "m.values()".into(),
            5 => "sorted(st)".into(),
            6 => format!("[{a} % 9; {}]", self.rng.range(0, 3)),
            _ => format!("v[0..min({}, len(v))]", self.rng.range(0, 4)),
        }
    }
    fn line(&mut self, s: &str) {
        self.out.push_str(&"    ".repeat(self.ind));
        self.out.push_str(s);
        self.out.push('\n');
    }
    fn stmts(&mut self, n: usize, depth: u32) {
        for _ in 0..n {
            self.stmt(depth);
        }
    }
    fn stmt(&mut self, depth: u32) {
        match self.rng.range(0, if depth == 0 { 8 } else { 14 }) {
            0 => {
                let e = self.int(3);
                self.line(&format!("x = ({e}) % 1000;"));
            }
            1 => {
                let e = self.int(3);
                self.line(&format!("y = ({e}) % 1000;"));
            }
            2 => {
                let e = self.int(2);
                self.line(&format!("v.push(({e}) % 100);"));
            }
            3 => {
                let (k, e) = (self.int(2), self.int(2));
                self.line(&format!("m[({k}) % 20] = ({e}) % 100;"));
            }
            4 => {
                let e = self.int(2);
                self.line(&format!("st.add(({e}) % 20);"));
            }
            5 => {
                let e = self.int(2);
                self.line(&format!("s += to_string(({e}) % 10);"));
            }
            6 => {
                let e = self.int(2);
                self.line(&format!("x += ({e}) % 7;"));
            }
            7 => {
                self.counter += 1;
                let name = format!("t{}", self.counter);
                let e = self.int(3);
                self.line(&format!("let {name} = ({e}) % 500;"));
                if let Some(sc) = self.scopes.last_mut() {
                    sc.push(name);
                }
            }
            8 => {
                let e = self.boolean(2);
                let a = self.int(2);
                self.line(&format!("if {e} {{ y = ({a}) % 1000; }}"));
            }
            9 => {
                let c = self.boolean(2);
                self.line(&format!("if {c} {{"));
                self.ind += 1;
                self.scopes.push(vec![]);
                self.stmts(2, depth - 1);
                self.scopes.pop();
                self.ind -= 1;
                self.line("} else {");
                self.ind += 1;
                self.scopes.push(vec![]);
                self.stmts(2, depth - 1);
                self.scopes.pop();
                self.ind -= 1;
                self.line("}");
            }
            10 => {
                self.counter += 1;
                let name = format!("i{}", self.counter);
                self.line(&format!("for {name} in range({}) {{", self.rng.range(1, 4)));
                self.ind += 1;
                self.scopes.push(vec![name]);
                self.stmts(2, depth - 1);
                self.scopes.pop();
                self.ind -= 1;
                self.line("}");
            }
            11 => {
                self.counter += 1;
                let name = format!("e{}", self.counter);
                let snapshot = self.vec(1);
                self.line(&format!("for {name} in {snapshot} {{"));
                self.ind += 1;
                self.scopes.push(vec![name]);
                // no mutation of v inside a loop over v is allowed, so only touch scalars here
                let a = self.int(2);
                self.line(&format!("x = (x + {a}) % 1000;"));
                self.scopes.pop();
                self.ind -= 1;
                self.line("}");
            }
            12 => {
                self.counter += 1;
                let name = format!("k{}", self.counter);
                self.line(&format!("let {name} = 0;"));
                self.line(&format!("while {name} < {} {{", self.rng.range(1, 4)));
                self.ind += 1;
                self.line(&format!("{name} += 1;"));
                self.scopes.push(vec![name.clone()]);
                let a = self.int(2);
                self.line(&format!("y = (y + {a}) % 1000;"));
                self.scopes.pop();
                self.ind -= 1;
                self.line("}");
            }
            13 => {
                self.counter += 1;
                let (i, e) = (format!("n{}", self.counter), format!("w{}", self.counter));
                self.line(&format!("for ({i}, {e}) in enumerate(sorted(st)) {{"));
                self.ind += 1;
                self.line(&format!("y = (y + {i} * {e}) % 1000;"));
                self.ind -= 1;
                self.line("}");
            }
            _ => {
                let a = self.int(2);
                self.line(&format!("t = ({a} % 5, {a} % 3);"));
            }
        }
    }
}

fn program(seed: u64, funcs: usize) -> String {
    let rng = Rng::new(seed);
    let mut src = String::new();
    for f in 0..funcs {
        let mut g = G { rng: &rng, out: String::new(), ind: 1, scopes: vec![vec![]], counter: 0 };
        g.line("let x = 3;");
        g.line("let y = 5;");
        g.line("let v: vec<int> = [1, 2, 3];");
        g.line("let s = \"ab\";");
        g.line("let m = map<int, int>();");
        g.line("let st = set<int>();");
        g.line("let t = (1, 2);");
        let n = rng.range(6, 12) as usize;
        g.stmts(n, 2);
        let tail = format!("    return (x + y + len(m) + len(st) + t.0 + t.1, {}, s);\n", g.vec(2));
        src.push_str(&format!("fn f{f}() -> (int, vec<int>, string) {{\n{}{tail}}}\n\n", g.out));
    }
    src.push_str("fn entry(k: int) -> (int, vec<int>, string) {\n");
    for f in 0..funcs {
        src.push_str(&format!("    if k == {f} {{ return f{f}(); }}\n"));
    }
    src.push_str("    return f0();\n}\n");
    src
}

fn run_batch(seed: u64) {
    let funcs = 24;
    let src = program(seed, funcs);
    let sig_ret = Ty::Tuple(vec![Ty::Int, Ty::vec(Ty::Int), Ty::Str]);
    let opts = CheckOpts { allowed: Allowed::all(), entry: None };
    let c = match compile(&src, &opts) {
        Ok(c) => c,
        Err(d) => panic!("seed {seed}: generated program does not type-check: {}\n{src}", d[0].msg),
    };
    let mut expected = Vec::new();
    let mut cases = Vec::new();
    for k in 0..funcs {
        let args = vec![k.to_string()];
        let r = c.call("entry", &args, &Limits::default()).unwrap();
        match &r.report.result {
            Ok(_) => expected.push(canonical(&r.output.unwrap(), &sig_ret, Compare::Exact)),
            Err(e) => panic!("seed {seed} fn f{k}: interpreter error: {}\n{src}", e.msg),
        }
        cases.push(args);
    }
    let sig = EntrySig { name: "entry".into(), params: c.program.find("entry").unwrap().params.clone(), ret: sig_ret.clone() };
    let dir = std::env::temp_dir().join(format!("algoclerk-rand-{}-{seed}", std::process::id()));
    let mut problems = Vec::new();
    for lang in Lang::ALL {
        let g = generate(&c, lang, "entry");
        let text = match harness::program(lang, &sig, &g, &cases) {
            Ok(t) => t,
            Err(e) => {
                problems.push(format!("{}: harness: {e}", lang.name()));
                continue;
            }
        };
        for comp in available(lang) {
            match compile_and_run(&comp, &text, &dir, &format!("r{seed}-{}", lang.id()), false) {
                Err(e) => problems.push(format!("[{}] {e}\n--- generated source:\n{text}", comp.exe)),
                Ok(out) => {
                    for (k, (line, want)) in out.lines().zip(expected.iter()).enumerate() {
                        let got = canonical(line, &sig_ret, Compare::Exact);
                        if &got != want {
                            problems.push(format!("[{}] f{k}: expected {want}, got {got}", comp.exe));
                        }
                    }
                    if out.lines().count() != expected.len() {
                        problems.push(format!("[{}] wrong number of output lines", comp.exe));
                    }
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(problems.is_empty(), "seed {seed}: {} problems\n{}\n=== program ===\n{src}", problems.len(), problems.join("\n"));
}

#[test]
fn random_programs_agree_across_all_languages() {
    let seeds: Vec<u64> = std::env::var("ALGOCLERK_RANDOM_SEEDS")
        .ok()
        .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_else(|| (1..=4).collect());
    std::thread::scope(|s| {
        let handles: Vec<_> = seeds.iter().map(|&seed| s.spawn(move || run_batch(seed))).collect();
        for h in handles {
            if let Err(e) = h.join() {
                std::panic::resume_unwind(e);
            }
        }
    });
}

#[test]
fn generated_programs_parse_check_and_print_stably() {
    for seed in 100..120 {
        let src = program(seed, 6);
        let p = parse(&src).unwrap();
        let printed = algoclerk_core::lang::print::print(&p);
        let again = algoclerk_core::lang::print::print(&parse(&printed).unwrap());
        assert_eq!(printed, again);
    }
}

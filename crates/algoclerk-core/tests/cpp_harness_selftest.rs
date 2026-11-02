// SPDX-License-Identifier: Apache-2.0
//! Self-test of the differential harness: an identity function of every supported type is built into a
//! translation unit, compiled with the host compilers, and the printed result must be the canonical text
//! of the literal that was fed in. This proves the C++ literal builders and the C++ printers agree with
//! `cpp::value` (the notation shared by level files, the judge and the interpreter).

use algoclerk_core::cpp::harness::{
    Compiler, available_compilers, compile, run_binary, translation_unit,
};
use algoclerk_core::judge::canonical;
use algoclerk_core::level::{Compare, Level};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

struct T {
    name: &'static str,
    includes: &'static str,
    ty: &'static str,
    cases: &'static [&'static str],
}

const LIST_NODE: &str = "struct ListNode {\n    int val;\n    ListNode* next;\n    ListNode(int v) : val(v), next(nullptr) {}\n};\n";
const TREE_NODE: &str = "struct TreeNode {\n    int val;\n    TreeNode* left;\n    TreeNode* right;\n    TreeNode(int v) : val(v), left(nullptr), right(nullptr) {}\n};\n";

const TYPES: &[T] = &[
    T {
        name: "bool",
        includes: "",
        ty: "bool",
        cases: &["true", "false"],
    },
    T {
        name: "char",
        includes: "",
        ty: "char",
        cases: &[
            "'a'", "'\\n'", "'\\''", "'\\\\'", "'\"'", "'\\x80'", "'\\x7f'", "'\\0'", "'?'", "' '",
        ],
    },
    T {
        name: "int",
        includes: "",
        ty: "int",
        cases: &["0", "-1", "2147483647", "-2147483648"],
    },
    T {
        name: "unsigned",
        includes: "",
        ty: "unsigned",
        cases: &["0", "4294967295"],
    },
    T {
        name: "ll",
        includes: "",
        ty: "long long",
        cases: &["9223372036854775807", "-9223372036854775808", "-5"],
    },
    T {
        name: "ull",
        includes: "",
        ty: "unsigned long long",
        cases: &["18446744073709551615", "0"],
    },
    T {
        name: "size",
        includes: "#include <vector>\n",
        ty: "std::size_t",
        cases: &["0", "18446744073709551615"],
    },
    T {
        name: "string",
        includes: "#include <string>\n",
        ty: "std::string",
        cases: &[
            "\"\"",
            "\"hello\"",
            "\"a\\\"b\\\\c\"",
            "\"tab\\there\\nnewline\"",
            "\"\\x01\\x31 \\x7f\\x80\\xff\"",
            "\"a\\0b\"",
            "\"what??!\"",
            "\"it's\"",
        ],
    },
    T {
        name: "string_view",
        includes: "#include <string_view>\n",
        ty: "std::string_view",
        cases: &["\"view\"", "\"\""],
    },
    T {
        name: "vec_int",
        includes: "#include <vector>\n",
        ty: "std::vector<int>",
        cases: &["{}", "{1}", "{1, 2, 3}", "{-5, 2147483647}"],
    },
    T {
        name: "vec_vec",
        includes: "#include <vector>\n",
        ty: "std::vector<std::vector<long long>>",
        cases: &["{}", "{{}}", "{{1, 2}, {}, {3}}"],
    },
    T {
        name: "vec_char",
        includes: "#include <vector>\n",
        ty: "std::vector<std::vector<char>>",
        cases: &["{{'1', '0'}, {'0', '\\n'}}"],
    },
    T {
        name: "vec_str",
        includes: "#include <vector>\n#include <string>\n",
        ty: "std::vector<std::string>",
        cases: &["{\"a\", \"b\"}", "{\"\"}", "{}"],
    },
    T {
        name: "array",
        includes: "#include <array>\n",
        ty: "std::array<int, 3>",
        cases: &["{1, 2, 3}", "{-1, 0, 7}"],
    },
    T {
        name: "deque",
        includes: "#include <deque>\n",
        ty: "std::deque<int>",
        cases: &["{3, 1, 2}", "{}"],
    },
    T {
        name: "list",
        includes: "#include <list>\n",
        ty: "std::list<unsigned>",
        cases: &["{3, 1, 2}", "{}"],
    },
    T {
        name: "set",
        includes: "#include <set>\n",
        ty: "std::set<int>",
        cases: &["{1, 2, 3}", "{}"],
    },
    T {
        name: "uset",
        includes: "#include <unordered_set>\n#include <string>\n",
        ty: "std::unordered_set<std::string>",
        cases: &["{\"b\", \"a\", \"c\"}", "{}", "{\"z\"}"],
    },
    T {
        name: "pair",
        includes: "#include <utility>\n#include <string>\n",
        ty: "std::pair<int, std::string>",
        cases: &["{1, \"x\"}", "{-1, \"\"}"],
    },
    T {
        name: "tuple",
        includes: "#include <tuple>\n",
        ty: "std::tuple<int, char, bool>",
        cases: &["{1, 'c', true}", "{-1, '\\n', false}"],
    },
    T {
        name: "optional",
        includes: "#include <optional>\n",
        ty: "std::optional<int>",
        cases: &["nullopt", "{5}", "{-5}"],
    },
    T {
        name: "map",
        includes: "#include <map>\n#include <string>\n",
        ty: "std::map<int, std::string>",
        cases: &["{{1, \"a\"}, {2, \"b\"}}", "{}"],
    },
    T {
        name: "umap",
        includes: "#include <unordered_map>\n#include <string>\n",
        ty: "std::unordered_map<std::string, int>",
        cases: &["{{\"z\", 1}, {\"a\", 2}, {\"m\", -3}}", "{}"],
    },
    T {
        name: "map_vec",
        includes: "#include <map>\n#include <string>\n#include <vector>\n",
        ty: "std::map<std::string, std::vector<int>>",
        cases: &["{{\"k\", {1, 2}}, {\"l\", {}}}"],
    },
    T {
        name: "vec_pair",
        includes: "#include <vector>\n#include <utility>\n",
        ty: "std::vector<std::pair<char, int>>",
        cases: &["{{'a', 1}, {'b', 2}}", "{}"],
    },
    T {
        name: "list_node",
        includes: "",
        ty: "ListNode*",
        cases: &[
            "{}",
            "{1}",
            "{1, 2, 3}",
            "{1, 2, 3} @0",
            "{1, 2, 3, 4} @2",
            "{5} @0",
            "nullptr",
        ],
    },
    T {
        name: "tree_node",
        includes: "",
        ty: "TreeNode*",
        cases: &[
            "{}",
            "nullptr",
            "{1}",
            "{1, 2, 3}",
            "{3, 9, 20, nullptr, nullptr, 15, 7}",
            "{1, nullptr, 2, nullptr, 3}",
        ],
    },
];

fn level_for(t: &T) -> Result<Level, String> {
    let structs = match t.ty {
        "ListNode*" => LIST_NODE,
        "TreeNode*" => TREE_NODE,
        _ => "",
    };
    let toml = format!(
        "title = \"id\"\nstatement = \"s\"\nprelude = '''\n{}{}'''\nsignature = \"{} identity({} x)\"\ntechnique = \"t\"\nbig_o = \"O(1) time, O(1) space\"\nreference = \"\"\"\n{} identity({} x) {{\n    return x;\n}}\n\"\"\"\n",
        t.includes, structs, t.ty, t.ty, t.ty, t.ty
    );
    let mut l = Level::from_toml(&toml, "selftest")?;
    l.id = t.name.to_string();
    Ok(l)
}

fn check(t: &T, compilers: &[Compiler], dir: &std::path::Path) -> Vec<String> {
    let level = match level_for(t) {
        Ok(l) => l,
        Err(e) => return vec![format!("{}: level: {e}", t.name)],
    };
    let args: Vec<Vec<String>> = t.cases.iter().map(|c| vec![(*c).to_string()]).collect();
    let tu = match translation_unit(&level, &level.reference, &args) {
        Ok(tu) => tu,
        Err(e) => return vec![format!("{}: harness: {e}", t.name)],
    };
    let ty = &level.signature.ret.ty;
    let mut problems = Vec::new();
    for cc in compilers {
        let name = format!("{}-{}", t.name, cc.exe.replace('+', "p"));
        let built = match compile(cc, &tu, dir, &name, false, 240) {
            Ok(b) => b,
            Err(e) => {
                problems.push(format!("{} [{}]: {e}", t.name, cc.exe));
                continue;
            }
        };
        if !built.diagnostics.trim().is_empty() {
            problems.push(format!(
                "{} [{}]: warnings:\n{}",
                t.name, cc.exe, built.diagnostics
            ));
        }
        match run_binary(&built.exe, 60, false) {
            Err(e) => problems.push(format!("{} [{}]: {e}", t.name, cc.exe)),
            Ok(lines) => {
                if lines.len() != t.cases.len() {
                    problems.push(format!("{} [{}]: {} lines", t.name, cc.exe, lines.len()));
                    continue;
                }
                for (line, lit) in lines.iter().zip(t.cases) {
                    let want = canonical(lit, ty, Compare::Exact);
                    let got = canonical(line, ty, Compare::Exact);
                    if got != want
                        || (got != *line
                            && !matches!(
                                ty,
                                algoclerk_core::cpp::ast::Ty::UnorderedSet(_)
                                    | algoclerk_core::cpp::ast::Ty::UnorderedMap(..)
                            ))
                    {
                        problems.push(format!(
                            "{} [{}]: fed {lit} but the program printed {line} (canonical {want})",
                            t.name, cc.exe
                        ));
                    }
                }
            }
        }
    }
    problems
}

#[test]
fn identity_functions_round_trip_through_real_compilers() {
    let compilers = available_compilers();
    if compilers.is_empty() {
        eprintln!("skipping: no C++ compiler installed");
        return;
    }
    let dir = std::env::temp_dir().join(format!("algoclerk-selftest-{}", std::process::id()));
    let next = AtomicUsize::new(0);
    let problems = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(t) = TYPES.get(i) else { break };
                    let p = check(t, &compilers, &dir);
                    if let Ok(mut all) = problems.lock() {
                        all.extend(p);
                    }
                }
            });
        }
    });
    let _ = std::fs::remove_dir_all(&dir);
    let problems = problems.into_inner().unwrap_or_default();
    assert!(problems.is_empty(), "{}", problems.join("\n---\n"));
}

// SPDX-License-Identifier: Apache-2.0
//! Hand-built syntax tree smoke tests for the interpreter (independent of the parser).

use algoclerk_core::cpp::ast::*;
use algoclerk_core::cpp::interp::{Limits, RtKind, run};
use algoclerk_core::cpp::value::{Heap, Plain};

struct B {
    next: NodeId,
}

impl B {
    fn id(&mut self) -> NodeId {
        self.next += 1;
        self.next
    }
    fn e(&mut self, kind: ExprKind) -> Expr {
        Expr {
            id: self.id(),
            kind,
        }
    }
    fn int(&mut self, v: u64) -> Expr {
        self.e(ExprKind::Int {
            value: v,
            suffix: IntSuffix::None,
            base: IntBase::Dec,
        })
    }
    fn var(&mut self, n: &str) -> Expr {
        self.e(ExprKind::Var(n.into()))
    }
    fn bin(&mut self, op: BinOp, a: Expr, b: Expr) -> Expr {
        self.e(ExprKind::Binary(op, Box::new(a), Box::new(b)))
    }
    fn s(&mut self, kind: StmtKind) -> Stmt {
        Stmt::new(self.id(), kind)
    }
    fn decl(&mut self, ty: Ty, name: &str, init: Expr) -> Stmt {
        let d = Declarator {
            id: self.id(),
            name: name.into(),
            array: None,
            init: Init::Copy(init),
        };
        self.s(StmtKind::Decl {
            spec: TypeSpec::plain(ty),
            vars: vec![d],
        })
    }
}

fn program(b: &mut B, f: FnDecl) -> Program {
    let id = b.id();
    Program {
        items: vec![Item {
            id,
            kind: ItemKind::Func(f),
            comments: vec![],
        }],
        prelude_items: 0,
        next_id: b.next + 1,
        spans: vec![],
    }
}

#[test]
fn sum_loop() {
    let mut b = B { next: 0 };
    // int f(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }
    let zero = b.int(0);
    let s_decl = b.decl(Ty::Int, "s", zero);
    let zero2 = b.int(0);
    let i_decl = b.decl(Ty::Int, "i", zero2);
    let (iv, nv) = (b.var("i"), b.var("n"));
    let cond = b.bin(BinOp::Lt, iv, nv);
    let iv2 = b.var("i");
    let incr = b.e(ExprKind::Unary(UnOp::PreInc, Box::new(iv2)));
    let (sv, iv3) = (b.var("s"), b.var("i"));
    let add = b.e(ExprKind::Assign {
        op: Some(BinOp::Add),
        lhs: Box::new(sv),
        rhs: Box::new(iv3),
    });
    let body = b.s(StmtKind::Expr(add));
    let for_ = b.s(StmtKind::For {
        init: Some(Box::new(i_decl)),
        cond: Some(cond),
        incr: Some(incr),
        body: Box::new(body),
    });
    let sv2 = b.var("s");
    let ret = b.s(StmtKind::Return(Some(sv2)));
    let pid = b.id();
    let f = FnDecl {
        id: b.id(),
        name: "f".into(),
        params: vec![Param {
            id: pid,
            name: "n".into(),
            spec: TypeSpec::plain(Ty::Int),
        }],
        ret: TypeSpec::plain(Ty::Int),
        is_static: false,
        body: Some(Block {
            stmts: vec![s_decl, for_, ret],
            tail_comments: vec![],
        }),
        comments: vec![],
    };
    let p = program(&mut b, f);
    let r = run(
        &p,
        "f",
        vec![Plain::int(100)],
        Heap::default(),
        &Limits::default(),
    );
    assert_eq!(r.result, Ok(Plain::int(4950)), "{:?}", r.result);
    assert!(r.cost > 100);
    let r = run(
        &p,
        "f",
        vec![Plain::int(i32::MAX as i64)],
        Heap::default(),
        &Limits::default(),
    );
    assert_eq!(r.result.unwrap_err().kind, RtKind::Time);
}

//! Constructors for serialized bootstrap expressions, not an execution engine.
use super::super::{
    term::{app, int, sym},
    Term,
};
pub(super) fn a(i: i128) -> Term {
    app("arg", vec![int(i)])
}
pub(super) fn l(t: Term) -> Term {
    app("literal", vec![t])
}
pub(super) fn n(i: i128) -> Term {
    l(int(i))
}
pub(super) fn s(t: &str) -> Term {
    l(sym(t))
}
pub(super) fn b(v: bool) -> Term {
    l(Term::Bool(v))
}
pub(super) fn get(t: Term, i: i128) -> Term {
    call(&format!("field{i}"), vec![t])
}
pub(super) fn at(t: Term, i: Term) -> Term {
    app("child", vec![t, i])
}
pub(super) fn unary(op: &str, t: Term) -> Term {
    app(op, vec![t])
}
pub(super) fn bin(op: &str, x: Term, y: Term) -> Term {
    app(op, vec![x, y])
}
pub(super) fn eq(x: Term, y: Term) -> Term {
    bin("equal", x, y)
}
pub(super) fn req(p: Term, t: Term) -> Term {
    app("require", vec![p, t])
}
pub(super) fn iff(p: Term, t: Term, f: Term) -> Term {
    app("if", vec![p, t, f])
}
pub(super) fn seq(v: Vec<Term>) -> Term {
    app("sequence", v)
}
pub(super) fn mk(op: &str, v: Vec<Term>) -> Term {
    make(s(op), seq(v))
}
pub(super) fn make(op: Term, v: Term) -> Term {
    app("make", vec![op, v])
}
pub(super) fn call(op: &str, v: Vec<Term>) -> Term {
    let mut c = vec![sym(op)];
    c.extend(v);
    app("helper", c)
}
pub(super) fn run(op: &str, v: Vec<Term>) -> Term {
    let mut c = vec![sym(op)];
    c.extend(v);
    app("op", c)
}
pub(super) fn children(t: Term) -> Term {
    run("children32", vec![t])
}
pub(super) fn cat(x: Term, y: Term) -> Term {
    bin("append", x, y)
}
pub(super) fn fold(items: Term, initial: Term, body: Term) -> Term {
    app("fold", vec![int(32), items, initial, body])
}
pub(super) fn dict(i: usize) -> Term {
    app("dictionary", vec![int(0), n(i as i128)])
}
pub(super) fn shape(t: Term, name: &str, count: i128, out: Term) -> Term {
    req(call("has-shape", vec![t, s(name), n(count)]), out)
}
pub(super) fn function(name: &str, arity: i128, body: Term) -> Term {
    app("function-v1", vec![sym(name), int(arity), body])
}
pub(super) fn operation(name: &str, arity: i128, body: Term) -> Term {
    app("operation-v1", vec![sym(name), int(arity), body])
}
pub(super) fn program(name: &str, body: Term) -> Term {
    app("program-v1", vec![sym(name), int(2), body])
}
pub(super) fn row(kind: Term, path: Term, meaning: Term) -> Term {
    mk("declaration-v1", vec![kind, path, meaning])
}
pub(super) fn pushed(context: Term, out: Term, item: Term) -> Term {
    seq(vec![context, cat(out, seq(vec![item]))])
}
pub(super) fn matched(acc: Term, item: Term) -> Term {
    seq(vec![
        get(acc.clone(), 0),
        bin("add", get(acc, 1), n(1)),
        item,
    ])
}

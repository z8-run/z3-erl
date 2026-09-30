//! A single datatype description consumed by every backend.
use crate::logic::{op, sort};

pub struct constructor {
    pub op: op,
    pub test: op,
    pub fields: &'static [(op, sort)],
}

pub const terms: &[constructor] = &[
    constructor {
        op: op::int,
        test: op::is_int,
        fields: &[(op::ival, sort::int)],
    },
    constructor {
        op: op::atom,
        test: op::is_atom,
        fields: &[(op::aval, sort::int)],
    },
    constructor {
        op: op::nil,
        test: op::is_nil,
        fields: &[],
    },
    constructor {
        op: op::cons,
        test: op::is_cons,
        fields: &[(op::head, sort::term), (op::tail, sort::term)],
    },
    constructor {
        op: op::tuple,
        test: op::is_tuple,
        fields: &[(op::items, sort::seq)],
    },
];

pub const sequences: &[constructor] = &[
    constructor {
        op: op::empty,
        test: op::is_empty,
        fields: &[],
    },
    constructor {
        op: op::push,
        test: op::is_push,
        fields: &[(op::first, sort::term), (op::rest, sort::seq)],
    },
];

pub const datatypes: &[(sort, &[constructor])] = &[(sort::term, terms), (sort::seq, sequences)];

/// These identities are proved and axiom-audited in kernel/lean/seq.lean.
/// Only theories mentioned by an obligation need their induction lemmas.
pub fn lemmas(v: &crate::vc::vc) -> Vec<crate::logic::expr> {
    use crate::logic::expr as e;
    let mut used = vec![];
    let mut quantified = false;
    for n in v.hypotheses.iter().chain([&v.goal]) {
        n.visit(&mut |n| match n {
            e::prim { op, .. } => used.push(*op),
            e::quant { .. } => quantified = true,
            _ => {}
        });
    }
    let mut out = vec![];
    let xs = e::var("xs", sort::seq);
    let x = e::var("x", sort::term);
    let len = e::one(op::len, xs.clone());
    if used.contains(&op::len) || used.contains(&op::nth) {
        out.push(e::quantify(
            true,
            vec![("xs".into(), sort::seq)],
            e::all([
                e::two(op::le, e::num(0), len.clone()),
                e::eq(e::eq(len.clone(), e::num(0)), e::eq(xs.clone(), e::empty())),
            ]),
        ));
    }
    if used.contains(&op::size) || used.contains(&op::mass) {
        out.push(e::quantify(
            true,
            vec![("x".into(), sort::term)],
            e::two(op::le, e::num(1), e::one(op::size, x)),
        ));
        out.push(e::quantify(
            true,
            vec![("xs".into(), sort::seq)],
            e::two(op::le, e::num(0), e::one(op::mass, xs.clone())),
        ));
        if used.contains(&op::nth) {
            let i = e::var("i", sort::int);
            out.push(e::quantify(
                true,
                vec![("xs".into(), sort::seq), ("i".into(), sort::int)],
                e::implies(
                    e::and(
                        e::two(op::le, e::num(0), i.clone()),
                        e::two(op::lt, i.clone(), len.clone()),
                    ),
                    e::two(
                        op::le,
                        e::one(op::size, e::two(op::nth, xs.clone(), i)),
                        e::one(op::mass, xs.clone()),
                    ),
                ),
            ));
        }
    }
    if quantified && used.contains(&op::nth) {
        let ys = e::var("ys", sort::seq);
        let i = e::var("i", sort::int);
        let fields = e::quantify(
            true,
            vec![("i".into(), sort::int)],
            e::implies(
                e::and(
                    e::two(op::le, e::num(0), i.clone()),
                    e::two(op::lt, i.clone(), len.clone()),
                ),
                e::eq(
                    e::two(op::nth, xs.clone(), i.clone()),
                    e::two(op::nth, ys.clone(), i),
                ),
            ),
        );
        out.push(e::quantify(
            true,
            vec![("xs".into(), sort::seq), ("ys".into(), sort::seq)],
            e::implies(
                e::and(e::eq(len, e::one(op::len, ys.clone())), fields),
                e::eq(xs, ys),
            ),
        ));
    }
    out
}

/// Recursive equations shared by the three proof targets.
pub struct definition {
    pub op: op,
    pub params: Vec<(String, sort)>,
    pub body: crate::logic::expr,
}

pub fn definitions() -> Vec<definition> {
    use crate::logic::expr as e;
    let x = e::var("x", sort::term);
    let xs = e::var("xs", sort::seq);
    let i = e::var("i", sort::int);
    let add = |a, b| e::two(op::add, a, b);
    vec![
        definition {
            op: op::len,
            params: vec![("xs".into(), sort::seq)],
            body: e::ite(
                e::one(op::is_push, xs.clone()),
                add(e::num(1), e::one(op::len, e::one(op::rest, xs.clone()))),
                e::num(0),
            ),
        },
        definition {
            op: op::nth,
            params: vec![("xs".into(), sort::seq), ("i".into(), sort::int)],
            body: e::ite(
                e::and(
                    e::one(op::is_push, xs.clone()),
                    e::two(op::le, e::num(0), i.clone()),
                ),
                e::ite(
                    e::eq(i.clone(), e::num(0)),
                    e::one(op::first, xs.clone()),
                    e::two(
                        op::nth,
                        e::one(op::rest, xs.clone()),
                        e::two(op::sub, i, e::num(1)),
                    ),
                ),
                e::nil(),
            ),
        },
        definition {
            op: op::size,
            params: vec![("x".into(), sort::term)],
            body: e::ite(
                e::one(op::is_cons, x.clone()),
                add(
                    e::num(1),
                    add(
                        e::one(op::size, e::one(op::head, x.clone())),
                        e::one(op::size, e::one(op::tail, x.clone())),
                    ),
                ),
                e::ite(
                    e::one(op::is_tuple, x.clone()),
                    add(e::num(1), e::one(op::mass, e::one(op::items, x))),
                    e::num(1),
                ),
            ),
        },
        definition {
            op: op::mass,
            params: vec![("xs".into(), sort::seq)],
            body: e::ite(
                e::one(op::is_push, xs.clone()),
                add(
                    e::one(op::size, e::one(op::first, xs.clone())),
                    e::one(op::mass, e::one(op::rest, xs)),
                ),
                e::num(0),
            ),
        },
    ]
}

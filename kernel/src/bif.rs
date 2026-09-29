//! Erlang operations expressed once in the shared logical algebra.
use crate::logic::{expr as e, op};
use anyhow::{Result, bail};

pub struct spec {
    pub value: e,
    pub safe: e,
}

pub fn apply(name: &str, args: &[e]) -> Result<spec> {
    let unary = |p: op, a: &e| spec {
        value: e::bool_term(e::one(p, a.clone())),
        safe: e::yes(),
    };
    match (name, args) {
        ("is_integer", [a]) => Ok(unary(op::is_int, a)),
        ("is_atom", [a]) => Ok(unary(op::is_atom, a)),
        ("is_tuple", [a]) => Ok(unary(op::is_tuple, a)),
        ("is_boolean", [a]) => Ok(spec {
            value: e::bool_term(e::is_bool(a.clone())),
            safe: e::yes(),
        }),
        ("is_nil", [a]) => Ok(spec {
            value: e::bool_term(e::eq(a.clone(), e::atom(2))),
            safe: e::yes(),
        }),
        ("not", [a]) => Ok(spec {
            value: e::bool_term(e::negate(e::true_term(a.clone()))),
            safe: e::is_bool(a.clone()),
        }),
        ("!", [a]) => Ok(spec {
            value: e::bool_term(e::negate(e::truthy(a.clone()))),
            safe: e::yes(),
        }),
        ("-", [a]) => arithmetic("-", &[e::int(0), a.clone()]),
        ("+", [a]) => Ok(spec {
            value: a.clone(),
            safe: e::one(op::is_int, a.clone()),
        }),
        ("+" | "-" | "*" | "div" | "rem" | "<" | "<=" | ">" | ">=", [_, _]) => {
            arithmetic(name, args)
        }
        ("===" | "==" | "!==" | "!=", [a, b]) => {
            let eq = e::eq(a.clone(), b.clone());
            Ok(spec {
                value: e::bool_term(if name.starts_with('!') {
                    e::negate(eq)
                } else {
                    eq
                }),
                safe: e::yes(),
            })
        }
        ("hd" | "tl", [a]) => Ok(spec {
            value: e::one(if name == "hd" { op::head } else { op::tail }, a.clone()),
            safe: e::one(op::is_cons, a.clone()),
        }),
        ("elem", [a, b]) => element(a, b),
        _ => bail!(
            "unsupported operation {name}/{} in {}",
            args.len(),
            crate::profile
        ),
    }
}

fn arithmetic(name: &str, args: &[e]) -> Result<spec> {
    let a = e::one(op::ival, args[0].clone());
    let b = e::one(op::ival, args[1].clone());
    let mut safe = e::all(args.iter().map(|a| e::one(op::is_int, a.clone())));
    let value = match name {
        "+" => e::one(op::int, e::two(op::add, a, b)),
        "-" => e::one(op::int, e::two(op::sub, a, b)),
        "*" => e::one(op::int, e::two(op::mul, a, b)),
        "div" | "rem" => {
            safe = e::and(safe, e::negate(e::eq(b.clone(), e::num(0))));
            let q = trunc(a.clone(), b.clone());
            e::one(
                op::int,
                if name == "div" {
                    q
                } else {
                    e::two(op::sub, a, e::two(op::mul, q, b))
                },
            )
        }
        "<" => e::bool_term(e::two(op::lt, a, b)),
        "<=" => e::bool_term(e::two(op::le, a, b)),
        ">" => e::bool_term(e::two(op::lt, b, a)),
        ">=" => e::bool_term(e::two(op::le, b, a)),
        _ => bail!("unsupported arithmetic {name}"),
    };
    Ok(spec { value, safe })
}

/// SMT's integer division is not Erlang's: divide magnitudes, then restore sign.
pub fn trunc(a: e, b: e) -> e {
    let neg_a = e::two(op::lt, a.clone(), e::num(0));
    let neg_b = e::two(op::lt, b.clone(), e::num(0));
    let abs_a = e::ite(neg_a.clone(), e::two(op::sub, e::num(0), a.clone()), a);
    let abs_b = e::ite(neg_b.clone(), e::two(op::sub, e::num(0), b.clone()), b);
    let q = e::two(op::div, abs_a, abs_b);
    e::ite(
        e::eq(neg_a, neg_b),
        q.clone(),
        e::two(op::sub, e::num(0), q),
    )
}

fn element(a: &e, b: &e) -> Result<spec> {
    let e::prim { op: op::int, args } = b else {
        bail!("elem requires a literal index");
    };
    let e::integer { value } = &args[0] else {
        bail!("elem requires a literal index");
    };
    let index: usize = value
        .parse()
        .map_err(|_| anyhow::anyhow!("elem index must be nonnegative"))?;
    if index > 255 {
        bail!("elem index exceeds profile limit 255");
    }
    let mut safe = e::one(op::is_tuple, a.clone());
    let mut xs = e::one(op::items, a.clone());
    for step in 0..=index {
        safe = e::and(safe, e::one(op::is_cons, xs.clone()));
        if step == index {
            break;
        }
        xs = e::one(op::tail, xs);
    }
    Ok(spec {
        value: e::one(op::head, xs),
        safe,
    })
}

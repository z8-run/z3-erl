use crate::{graph::world, spec::env};
use anyhow::{Result, bail};
use kernel::{
    ir::{kind, node},
    logic::{expr as e, op},
};
use std::collections::BTreeMap;

pub fn matches(w: &world, n: &node, value: e, vars: &mut env) -> Result<e> {
    let mut bound = BTreeMap::new();
    let test = pattern(w, n, value, &mut bound)?;
    vars.extend(bound);
    Ok(test)
}

/// Reconstruct a head using its fields so postconditions can name its result.
pub fn rebuild(w: &world, n: &node, value: e) -> e {
    match &n.kind {
        kind::int { value } => e::int(value),
        kind::atom { value } => e::atom(w.atoms[value]),
        kind::op { name, args } if name == "-" && args.len() == 1 => {
            if let kind::int { value } = &args[0].kind {
                e::one(op::int, e::two(op::sub, e::num(0), e::num(value)))
            } else {
                value
            }
        }
        kind::bind {
            pattern,
            value: other,
        } => {
            let shape = if matches!(pattern.kind, kind::var { .. }) {
                other
            } else {
                pattern
            };
            rebuild(w, shape, value)
        }
        kind::tuple { items } => {
            let mut xs = e::one(op::items, value);
            let mut fields = vec![];
            for p in items {
                fields.push(rebuild(w, p, e::one(op::first, xs.clone())));
                xs = e::one(op::rest, xs);
            }
            e::tuple(&fields)
        }
        kind::list { items, tail } => {
            let mut xs = value;
            let mut fields = vec![];
            for p in items {
                fields.push(rebuild(w, p, e::one(op::head, xs.clone())));
                xs = e::one(op::tail, xs);
            }
            e::list(
                &fields,
                tail.as_ref()
                    .map(|t| rebuild(w, t, xs))
                    .unwrap_or_else(e::nil),
            )
        }
        _ => value,
    }
}

fn pattern(w: &world, n: &node, value: e, bound: &mut env) -> Result<e> {
    match &n.kind {
        kind::var { name } if name == "_" => Ok(e::yes()),
        kind::var { name } => {
            if let Some(previous) = bound.get(name) {
                return Ok(e::eq(previous.clone(), value));
            }
            bound.insert(name.clone(), value);
            Ok(e::yes())
        }
        kind::int { value: n } => Ok(e::eq(value, e::int(n))),
        kind::op { name, args } if name == "-" && args.len() == 1 => {
            let kind::int { value: n } = &args[0].kind else {
                bail!("negative patterns require an integer literal");
            };
            Ok(e::eq(
                value,
                e::one(op::int, e::two(op::sub, e::num(0), e::num(n))),
            ))
        }
        kind::atom { value: n } => Ok(e::eq(value, e::atom(w.atoms[n]))),
        kind::tuple { items } => {
            let mut tests = vec![e::one(op::is_tuple, value.clone())];
            let mut xs = e::one(op::items, value);
            for n in items {
                tests.push(e::one(op::is_push, xs.clone()));
                tests.push(pattern(w, n, e::one(op::first, xs.clone()), bound)?);
                xs = e::one(op::rest, xs);
            }
            tests.push(e::one(op::is_empty, xs));
            Ok(e::all(tests))
        }
        kind::bind {
            pattern: left,
            value: right,
        } => Ok(e::and(
            pattern(w, left, value.clone(), bound)?,
            pattern(w, right, value, bound)?,
        )),
        kind::list { items, tail } => sequence(w, items, tail.as_deref(), value, bound),
        _ => bail!("unsupported pattern; use variables, literals, lists, or tuples"),
    }
}

fn sequence(
    w: &world,
    items: &[node],
    tail: Option<&node>,
    mut value: e,
    bound: &mut env,
) -> Result<e> {
    let mut tests = vec![];
    for n in items {
        tests.push(e::one(op::is_cons, value.clone()));
        tests.push(pattern(w, n, e::one(op::head, value.clone()), bound)?);
        value = e::one(op::tail, value);
    }
    tests.push(if let Some(n) = tail {
        pattern(w, n, value, bound)?
    } else {
        e::one(op::is_nil, value)
    });
    Ok(e::all(tests))
}

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
            let test = e::one(op::is_tuple, value.clone());
            Ok(e::and(
                test,
                sequence(w, items, None, e::one(op::items, value), bound)?,
            ))
        }
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

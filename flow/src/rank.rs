//! Well-founded measures for recursive components.
use crate::{
    graph::world,
    spec::{self, env},
};
use anyhow::{Context, Result, bail};
use kernel::{
    ir::fun,
    logic::{expr as e, op},
};
use std::collections::BTreeSet;

type measured = (Vec<e>, e, BTreeSet<String>);
pub(crate) fn measure(
    w: &world,
    f: &fun,
    c: &kernel::ir::clause,
    vars: &env,
) -> Result<Option<measured>> {
    let Some(nodes) = w.rank(f, c) else {
        return Ok(None);
    };
    if nodes.is_empty() {
        bail!("decreases requires at least one component");
    }
    let mut values = vec![];
    let mut safe = vec![];
    let mut deps = BTreeSet::new();
    for n in nodes {
        let v = spec::eval(w, f, vars, &n, None)?;
        let rank = e::ite(
            e::one(op::is_int, v.term.clone()),
            e::one(op::ival, v.term.clone()),
            e::one(op::size, v.term),
        );
        safe.extend([v.safe, e::two(op::le, e::num(0), rank.clone())]);
        values.push(rank);
        deps.extend(v.deps);
    }
    Ok(Some((values, e::all(safe), deps)))
}

pub(crate) fn decrease(
    w: &world,
    f: &fun,
    args: &[e],
    before: &[e],
) -> Result<(e, BTreeSet<String>)> {
    let mut goals = vec![];
    let mut dependencies = BTreeSet::new();
    for branch in spec::select(w, f, args)? {
        let (after, safe, deps) = measure(w, f, branch.clause, &branch.vars)?
            .context("every member of a measured recursive cycle needs a measure")?;
        if after.len() != before.len() {
            bail!("recursive measures need the same number of components");
        }
        dependencies.extend(deps);
        let mut prefix = e::yes();
        let mut less = e::no();
        for (a, b) in after.iter().zip(before) {
            less = e::or(
                less,
                e::and(prefix.clone(), e::two(op::lt, a.clone(), b.clone())),
            );
            prefix = e::and(prefix, e::eq(a.clone(), b.clone()));
        }
        goals.push(e::implies(branch.test, e::and(safe, less)));
    }
    Ok((e::all(goals), dependencies))
}

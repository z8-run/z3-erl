//! Ordered runtime dispatch and per-clause contracts.
use crate::{
    graph::world,
    spec::{bind, env, eval, eval_depth, result_at, value},
};
use anyhow::{Result, bail};
use kernel::{
    ir::{clause, fun, kind, node},
    logic::expr as e,
};
use std::collections::BTreeSet;

pub struct selection<'a> {
    pub clause: &'a clause,
    pub vars: env,
    pub test: e,
}

/// Runtime dispatch is independent of contract requirements.
pub fn select<'a>(w: &world, f: &'a fun, args: &[e]) -> Result<Vec<selection<'a>>> {
    let mut previous = e::no();
    let mut out = vec![];
    for c in &f.clauses {
        let mut vars = bind(f, args);
        let pattern = node {
            line: c.span.line,
            kind: kind::tuple {
                items: c.patterns.clone(),
            },
        };
        let test = crate::pattern::matches(w, &pattern, e::tuple(args), &mut vars)?;
        guard_syntax(&c.guard)?;
        let g = eval(w, f, &vars, &c.guard, None)?;
        let matched = e::all([test, g.safe, e::true_term(g.term)]);
        let test = e::and(e::negate(previous.clone()), matched.clone());
        previous = e::or(previous, matched);
        out.push(selection {
            clause: c,
            vars,
            test,
        });
    }
    Ok(out)
}

pub fn guard_syntax(n: &node) -> Result<()> {
    let mut valid = true;
    crate::graph::walk(n, &mut |n| {
        valid &= match &n.kind {
            kind::int { .. }
            | kind::atom { .. }
            | kind::var { .. }
            | kind::tuple { .. }
            | kind::list { .. } => true,
            kind::op { name, .. } => !matches!(name.as_str(), "&&" | "||" | "!" | "term_size"),
            _ => false,
        };
    });
    if !valid {
        bail!("line {}: expression is not an Elixir guard", n.line);
    }
    Ok(())
}

pub(crate) fn pre_depth(w: &world, f: &fun, args: &[e], depth: usize) -> Result<value> {
    let mut choices = vec![];
    let mut deps = BTreeSet::new();
    for branch in select(w, f, args)? {
        let mut parts = vec![branch.test];
        for n in &branch.clause.requires {
            let v = eval_depth(w, f, &branch.vars, n, None, depth + 1)?;
            parts.push(e::and(v.safe, e::true_term(v.term)));
            deps.extend(v.deps);
        }
        choices.push(e::all(parts));
    }
    Ok(value {
        term: e::bool_term(e::any(choices)),
        safe: e::yes(),
        deps,
    })
}

pub fn clause_post(
    w: &world,
    f: &fun,
    c: &clause,
    vars: &env,
    args: &[e],
    result: &e,
) -> Result<(e, BTreeSet<String>)> {
    let rebuilt = c
        .patterns
        .iter()
        .zip(args)
        .map(|(p, a)| crate::pattern::rebuild(w, p, a.clone()))
        .collect::<Vec<_>>();
    let at = result_at {
        owner: f,
        args,
        result,
        rebuilt: &rebuilt,
    };
    let mut parts = vec![];
    let mut deps = BTreeSet::new();
    for n in &c.ensures {
        let v = eval(w, f, vars, n, Some(&at))?;
        parts.push(e::and(v.safe, e::true_term(v.term)));
        deps.extend(v.deps);
    }
    Ok((e::all(parts), deps))
}

pub fn post(w: &world, f: &fun, args: &[e], result: &e) -> Result<(e, BTreeSet<String>)> {
    let mut parts = vec![];
    let mut deps = BTreeSet::new();
    for branch in select(w, f, args)? {
        let (post, ds) = clause_post(w, f, branch.clause, &branch.vars, args, result)?;
        parts.push(e::implies(branch.test, post));
        deps.extend(ds);
    }
    Ok((e::all(parts), deps))
}

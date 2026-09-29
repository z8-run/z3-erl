use crate::graph::world;
use anyhow::{Result, bail};
use kernel::{
    bif,
    ir::{fun, kind, mode, node},
    logic::{expr as e, op},
};
use std::collections::{BTreeMap, BTreeSet};

pub type env = BTreeMap<String, e>;

pub struct value {
    pub term: e,
    pub safe: e,
    pub deps: BTreeSet<String>,
}

impl value {
    fn plain(term: e) -> Self {
        Self {
            term,
            safe: e::yes(),
            deps: BTreeSet::new(),
        }
    }
}

pub struct result_at<'a> {
    pub owner: &'a fun,
    pub args: &'a [e],
    pub result: &'a e,
}

pub fn eval(w: &world, f: &fun, vars: &env, n: &node, at: Option<&result_at>) -> Result<value> {
    eval_depth(w, f, vars, n, at, 0)
}

fn eval_depth(
    w: &world,
    f: &fun,
    vars: &env,
    n: &node,
    at: Option<&result_at>,
    depth: usize,
) -> Result<value> {
    if depth > 96 {
        bail!("specification nesting or cyclic preconditions exceed 96 levels");
    }
    let sub = |n| eval_depth(w, f, vars, n, at, depth + 1);
    match &n.kind {
        kind::int { value } => {
            let digits = value.strip_prefix('-').unwrap_or(value);
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                bail!("invalid integer literal");
            }
            Ok(value::plain(e::int(value)))
        }
        kind::atom { value } => Ok(value::plain(e::atom(w.atoms[value]))),
        kind::var { name } => {
            Ok(value::plain(vars.get(name).cloned().ok_or_else(|| {
                anyhow::anyhow!("unbound variable {name}")
            })?))
        }
        kind::op { name, args } => {
            let values = args.iter().map(sub).collect::<Result<Vec<_>>>()?;
            operation(name, values)
        }
        kind::tuple { items } | kind::list { items, tail: None } => {
            let values = items.iter().map(sub).collect::<Result<Vec<_>>>()?;
            let (args, safe, deps) = gather(values);
            let list = e::list(&args, e::nil());
            Ok(value {
                term: if matches!(&n.kind, kind::tuple { .. }) {
                    e::one(op::tuple, list)
                } else {
                    list
                },
                safe,
                deps,
            })
        }
        kind::list {
            items,
            tail: Some(tail),
        } => {
            let mut values = items.iter().map(sub).collect::<Result<Vec<_>>>()?;
            values.push(sub(tail)?);
            let (mut args, safe, deps) = gather(values);
            let tail = args.pop().unwrap();
            Ok(value {
                term: e::list(&args, tail),
                safe,
                deps,
            })
        }
        kind::branch { cond, yes, no } => {
            let c = sub(cond)?;
            let y = sub(yes)?;
            let n = sub(no)?;
            let test = e::truthy(c.term);
            Ok(value {
                term: e::ite(test.clone(), y.term, n.term),
                safe: e::all([
                    c.safe,
                    e::implies(test.clone(), y.safe),
                    e::implies(e::negate(test), n.safe),
                ]),
                deps: c.deps.into_iter().chain(y.deps).chain(n.deps).collect(),
            })
        }
        kind::call { module, name, args } => {
            let callee = w.get(module.as_deref().unwrap_or(&f.module), name, args.len())?;
            let (args, safe, mut deps) = gather(args.iter().map(sub).collect::<Result<_>>()?);
            if let Some(at) = at.filter(|at| at.owner.id() == callee.id() && args == at.args) {
                return Ok(value {
                    term: at.result.clone(),
                    safe,
                    deps,
                });
            }
            if callee.mode != mode::ghost {
                bail!(
                    "{} in a specification is not a ghost function or the current return value",
                    callee.id()
                );
            }
            let pre = pre_depth(w, callee, &args, depth + 1)?;
            deps.insert(callee.id());
            deps.extend(pre.deps);
            Ok(value {
                term: e::call(callee.id(), args),
                safe: e::all([safe, pre.safe, e::true_term(pre.term)]),
                deps,
            })
        }
        _ => bail!(
            "statements and case are not supported inside contracts; use boolean expressions or a ghost function"
        ),
    }
}

fn gather(values: Vec<value>) -> (Vec<e>, e, BTreeSet<String>) {
    let mut args = vec![];
    let mut safe = vec![];
    let mut deps = BTreeSet::new();
    for v in values {
        args.push(v.term);
        safe.push(v.safe);
        deps.extend(v.deps);
    }
    (args, e::all(safe), deps)
}

pub fn operation(name: &str, mut values: Vec<value>) -> Result<value> {
    if matches!(name, "and" | "or" | "&&" | "||") && values.len() == 2 {
        let b = values.pop().unwrap();
        let a = values.pop().unwrap();
        let strict = matches!(name, "and" | "or");
        let test = if strict {
            e::true_term(a.term.clone())
        } else {
            e::truthy(a.term.clone())
        };
        let take = if matches!(name, "and" | "&&") {
            test
        } else {
            e::negate(test)
        };
        let term = if let (Some(left), Some(right)) = (a.term.as_bool(), b.term.as_bool()) {
            e::bool_term(if matches!(name, "and" | "&&") {
                e::and(left, right)
            } else {
                e::or(left, right)
            })
        } else if strict {
            e::ite(
                take.clone(),
                b.term,
                e::atom(if name == "and" { 0 } else { 1 }),
            )
        } else {
            e::ite(take.clone(), b.term, a.term.clone())
        };
        return Ok(value {
            term,
            safe: e::all([
                a.safe,
                if strict { e::is_bool(a.term) } else { e::yes() },
                e::implies(take, b.safe),
            ]),
            deps: a.deps.into_iter().chain(b.deps).collect(),
        });
    }
    let (args, safe, deps) = gather(values);
    let s = bif::apply(name, &args)?;
    Ok(value {
        term: s.value,
        safe: e::and(safe, s.safe),
        deps,
    })
}

pub fn bind(f: &fun, args: &[e]) -> env {
    f.args.iter().cloned().zip(args.iter().cloned()).collect()
}

pub fn pre(w: &world, f: &fun, args: &[e]) -> Result<value> {
    pre_depth(w, f, args, 0)
}

fn pre_depth(w: &world, f: &fun, args: &[e], depth: usize) -> Result<value> {
    let vars = bind(f, args);
    let mut parts = vec![];
    let mut deps = BTreeSet::new();
    for n in f.requires.iter().chain([&f.guard]) {
        let v = eval_depth(w, f, &vars, n, None, depth + 1)?;
        parts.push(e::and(v.safe, e::true_term(v.term)));
        deps.extend(v.deps);
    }
    Ok(value {
        term: e::bool_term(e::all(parts)),
        safe: e::yes(),
        deps,
    })
}

pub fn post(w: &world, f: &fun, args: &[e], result: &e) -> Result<(e, BTreeSet<String>)> {
    let vars = bind(f, args);
    let at = result_at {
        owner: f,
        args,
        result,
    };
    let mut parts = vec![];
    let mut deps = BTreeSet::new();
    for n in &f.ensures {
        let v = eval(w, f, &vars, n, Some(&at))?;
        parts.push(e::and(v.safe, e::true_term(v.term)));
        deps.extend(v.deps);
    }
    Ok((e::all(parts), deps))
}

use crate::graph::world;
use anyhow::{Result, bail};
use kernel::{
    bif,
    ir::{fun, kind, mode, node},
    logic::{expr as e, sort},
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
    pub rebuilt: &'a [e],
}

pub fn eval(w: &world, f: &fun, vars: &env, n: &node, at: Option<&result_at>) -> Result<value> {
    eval_depth(w, f, vars, n, at, 0)
}

pub(crate) fn eval_depth(
    w: &world,
    f: &fun,
    vars: &env,
    n: &node,
    at: Option<&result_at>,
    depth: usize,
) -> Result<value> {
    context {
        w,
        f,
        vars,
        at,
        depth,
    }
    .eval(n)
}

#[derive(Clone, Copy)]
struct context<'a, 'w> {
    w: &'a world<'w>,
    f: &'a fun,
    vars: &'a env,
    at: Option<&'a result_at<'a>>,
    depth: usize,
}

impl context<'_, '_> {
    fn eval(&self, n: &node) -> Result<value> {
        let Self { w, vars, depth, .. } = *self;
        if depth > 96 {
            bail!("specification nesting or cyclic preconditions exceed 96 levels");
        }
        let sub = |n| {
            context {
                depth: depth + 1,
                ..*self
            }
            .eval(n)
        };
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
            kind::quant {
                all,
                vars: names,
                body,
            } => self.quantified(*all, names, body),
            kind::tuple { items } | kind::list { items, tail: None } => {
                let values = items.iter().map(sub).collect::<Result<Vec<_>>>()?;
                let (args, safe, deps) = gather(values);
                let list = e::list(&args, e::nil());
                Ok(value {
                    term: if matches!(&n.kind, kind::tuple { .. }) {
                        e::tuple(&args)
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
            kind::call { module, name, args } => self.call(module, name, args),
            _ => bail!(
                "statements and case are not supported inside contracts; use boolean expressions or a ghost function"
            ),
        }
    }

    fn quantified(&self, all: bool, names: &[String], body: &node) -> Result<value> {
        let mut local = self.vars.clone();
        let bound: Vec<_> = names
            .iter()
            .map(|n| (format!("$q{}_{n}", self.depth), sort::term))
            .collect();
        for (n, (id, t)) in names.iter().zip(&bound) {
            local.insert(n.clone(), e::var(id, *t));
        }
        let v = context {
            vars: &local,
            depth: self.depth + 1,
            ..*self
        }
        .eval(body)?;
        Ok(value {
            term: e::bool_term(e::quantify(
                all,
                bound.clone(),
                e::true_term(v.term.clone()),
            )),
            safe: e::quantify(true, bound, e::and(v.safe, e::is_bool(v.term))),
            deps: v.deps,
        })
    }
    fn call(&self, module: &Option<String>, name: &str, args: &[node]) -> Result<value> {
        let Self {
            w, f, at, depth, ..
        } = *self;
        let sub = |n| {
            context {
                depth: depth + 1,
                ..*self
            }
            .eval(n)
        };
        let callee = w.get(module.as_deref().unwrap_or(&f.module), name, args.len())?;
        let (args, safe, mut deps) = gather(args.iter().map(sub).collect::<Result<_>>()?);
        if let Some(at) =
            at.filter(|at| at.owner.id() == callee.id() && (args == at.args || args == at.rebuilt))
        {
            return Ok(value {
                term: at.result.clone(),
                safe,
                deps,
            });
        }
        if let Some(at) = at.filter(|at| at.owner.id() == callee.id() && callee.mode != mode::ghost)
        {
            return Ok(value {
                term: at.result.clone(),
                safe: e::and(
                    safe,
                    e::all(
                        args.iter()
                            .zip(at.args)
                            .map(|(a, b)| e::eq(a.clone(), b.clone())),
                    ),
                ),
                deps,
            });
        }
        if callee.mode != mode::ghost {
            bail!(
                "{} in a specification is not a ghost function or the current return value",
                callee.id()
            );
        }
        let pre = crate::clause::pre_depth(w, callee, &args, depth + 1)?;
        deps.insert(callee.id());
        deps.extend(pre.deps);
        Ok(value {
            term: e::call(callee.id(), args),
            safe: e::all([safe, pre.safe, e::true_term(pre.term)]),
            deps,
        })
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
    crate::clause::pre_depth(w, f, args, 0)
}

pub use crate::clause::{clause_post, guard_syntax, post, select};

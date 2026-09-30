//! Pure source operators for hand-written TLA+ models.
use crate::{
    graph::world,
    spec::{self, env},
};
use anyhow::{Result, bail};
pub use kernel::model::export;
use kernel::{
    ir::{fun, kind, node},
    logic::{expr as e, sort},
};

pub fn export(w: &world, id: &str) -> Result<export> {
    let f = w
        .funs
        .get(id)
        .ok_or_else(|| anyhow::anyhow!("model export {id} has no contract"))?;
    if f.mode == kernel::ir::mode::ghost {
        bail!("TLA+ source exports require executable functions; ghost definitions are erased");
    }
    if w.recursive(f) {
        bail!("TLA+ source exports require nonrecursive functions");
    }
    let args: Vec<_> = f
        .args
        .iter()
        .map(|n| e::one(kernel::logic::op::int, e::var(n, sort::int)))
        .collect();
    let pre = spec::pre(w, f, &args)?;
    let (value, safe) = dispatch(w, f, &args, 0)?;
    Ok(export {
        args: f.args.clone(),
        pre: e::all([pre.safe, e::true_term(pre.term), safe]),
        value,
    })
}

fn body(w: &world, f: &fun, n: &node, vars: &mut env, depth: usize) -> Result<(e, e)> {
    if depth > 96 {
        bail!("source export exceeds expansion limit");
    }
    match &n.kind {
        kind::block { items } => {
            let mut last = e::atom(2);
            let mut safe = vec![];
            for n in items {
                let (v, s) = body(w, f, n, vars, depth + 1)?;
                if !n.erased() {
                    last = v;
                }
                safe.push(s);
            }
            Ok((last, e::all(safe)))
        }
        kind::bind { pattern, value } => {
            let (value, safe) = body(w, f, value, vars, depth + 1)?;
            let test = crate::pattern::matches(w, pattern, value.clone(), vars)?;
            Ok((value, e::and(safe, test)))
        }
        kind::branch { cond, yes, no } => {
            let (c, cs) = body(w, f, cond, vars, depth + 1)?;
            let (y, ys) = body(w, f, yes, &mut vars.clone(), depth + 1)?;
            let (n, ns) = body(w, f, no, &mut vars.clone(), depth + 1)?;
            let test = e::truthy(c);
            Ok((
                e::ite(test.clone(), y, n),
                e::all([
                    cs,
                    e::implies(test.clone(), ys),
                    e::implies(e::negate(test), ns),
                ]),
            ))
        }
        kind::call { module, name, args } => {
            let callee = w.get(module.as_deref().unwrap_or(&f.module), name, args.len())?;
            if w.recursive(callee) {
                bail!("recursive call in TLA+ source export");
            }
            if callee.mode == kernel::ir::mode::ghost {
                bail!("ghost call in executable source export");
            }
            if callee.mode == kernel::ir::mode::private && callee.module != f.module {
                bail!("private function {} cannot be called remotely", callee.id());
            }
            let mut values = vec![];
            let mut safe = vec![];
            for a in args {
                let (v, s) = body(w, f, a, vars, depth + 1)?;
                values.push(v);
                safe.push(s);
            }
            let pre = spec::pre(w, callee, &values)?;
            safe.extend([pre.safe, e::true_term(pre.term)]);
            let (v, s) = dispatch(w, callee, &values, depth + 1)?;
            safe.push(s);
            Ok((v, e::all(safe)))
        }
        kind::ghost { .. } | kind::assert { .. } | kind::unfold { .. } => {
            Ok((e::atom(2), e::yes()))
        }
        kind::case { .. } => {
            bail!("TLA+ source exports currently use if expressions instead of case")
        }
        _ => {
            let v = spec::eval(w, f, vars, n, None)?;
            if !v.deps.is_empty() {
                bail!("nested calls in model expressions must first be bound to a variable");
            }
            Ok((v.term, v.safe))
        }
    }
}

fn dispatch(w: &world, f: &fun, args: &[e], depth: usize) -> Result<(e, e)> {
    let mut value = e::atom(2);
    let mut safe = vec![];
    let mut domain = vec![];
    for branch in spec::select(w, f, args)?.into_iter().rev() {
        let (v, s) = body(
            w,
            f,
            &branch.clause.body,
            &mut branch.vars.clone(),
            depth + 1,
        )?;
        safe.push(e::implies(branch.test.clone(), s));
        domain.push(branch.test.clone());
        value = e::ite(branch.test, v, value);
    }
    safe.push(e::any(domain));
    Ok((value, e::all(safe)))
}

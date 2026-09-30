//! Erased proof statements, with explicit admission tracking and lexical scopes.
use crate::{
    exec::{check, paths, state},
    spec,
};
use anyhow::{Result, bail};
use kernel::{
    ir::{fun, kind, node, span},
    logic::{expr as e, sort},
};

impl check<'_, '_> {
    pub(crate) fn quantified(&mut self, f: &fun, n: &node, mut s: state) -> Result<paths> {
        if !s.proof {
            bail!("quantifiers belong in contracts and ghost code");
        }
        let v = spec::eval(self.world, f, &s.vars, n, None)?;
        for id in &v.deps {
            let callee = self.world.funs[id];
            // Specification evaluation does not emit recursive decrease obligations.
            // A quantified value must not introduce an unchecked recursive equation.
            if self.world.same_cycle(f, callee) || !self.world.total(callee) {
                bail!(
                    "quantified values require total calls outside the current recursive component"
                );
            }
        }
        s.deps.extend(v.deps);
        self.require(&mut s, n, "quantifier_domain", v.safe);
        Ok(vec![(s, v.term)])
    }

    pub(crate) fn proof(&mut self, f: &fun, n: &node, s: state, depth: usize) -> Result<paths> {
        Ok(match &n.kind {
            kind::assert { value, .. } => {
                let mut s = s;
                let v = spec::eval(self.world, f, &s.vars, value, None)?;
                s.deps.extend(v.deps);
                self.require(&mut s, n, "assert", e::and(v.safe, e::true_term(v.term)));
                vec![(s, e::atom(2))]
            }
            kind::assume { value } => {
                if !s.proof {
                    bail!("assume is only allowed inside ghost code");
                }
                let mut s = s;
                let v = spec::eval(self.world, f, &s.vars, value, None)?;
                s.deps.extend(v.deps);
                self.require(
                    &mut s,
                    n,
                    "assumption_domain",
                    e::and(v.safe, e::is_bool(v.term.clone())),
                );
                let fact = e::true_term(v.term);
                self.assumptions.push(crate::admission {
                    span: span {
                        file: self.owner.span.file.clone(),
                        line: n.line,
                    },
                    formula: fact.clone(),
                });
                s.facts.push(fact);
                vec![(s, e::atom(2))]
            }
            kind::havoc { names } => {
                if !s.proof {
                    bail!("havoc is only allowed inside ghost code");
                }
                let mut s = s;
                for name in names {
                    self.fresh += 1;
                    s.vars.insert(
                        name.clone(),
                        e::var(format!("$havoc{}_{name}", self.fresh), sort::term),
                    );
                }
                vec![(s, e::atom(2))]
            }
            kind::local { body } => {
                if !s.proof {
                    bail!("block is only allowed inside ghost code");
                }
                self.eval(f, body, s.clone(), depth + 1)?;
                vec![(s, e::atom(2))]
            }
            kind::ghost { body } => {
                let mut s = s;
                let vars = s.vars.clone();
                let proof = s.proof;
                s.proof = true;
                self.eval(f, body, s, depth + 1)?
                    .into_iter()
                    .map(|(mut s, _)| {
                        s.vars = vars.clone();
                        s.proof = proof;
                        (s, e::atom(2))
                    })
                    .collect()
            }
            kind::unfold { call } => self.unfold(f, n, call, s, depth)?,
            _ => unreachable!("runtime expressions are handled by exec"),
        })
    }
}

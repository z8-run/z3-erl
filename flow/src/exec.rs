use crate::{
    contract,
    graph::world,
    pattern,
    spec::{self, env},
};
use anyhow::{Context, Result, bail};
use kernel::{
    bif,
    ir::{arm, fun, kind, mode, node, span},
    logic::{expr as e, op, sort},
    vc::vc,
};
use std::collections::BTreeSet;

#[derive(Clone)]
struct state {
    vars: env,
    facts: Vec<e>,
    deps: BTreeSet<String>,
    rank: Option<e>,
    proof: bool,
    unfolding: bool,
}

type paths = Vec<(state, e)>;

struct check<'a, 'w> {
    world: &'w world<'a>,
    owner: &'a fun,
    vcs: Vec<vc>,
    deps: BTreeSet<String>,
}

pub fn check(w: &world, f: &fun) -> Result<(contract, Vec<vc>)> {
    let args: Vec<_> = f.args.iter().map(|n| e::var(n, sort::term)).collect();
    let mut s = state {
        vars: spec::bind(f, &args),
        facts: vec![],
        deps: BTreeSet::new(),
        rank: None,
        proof: f.mode == mode::ghost,
        unfolding: false,
    };
    let mut c = check {
        world: w,
        owner: f,
        vcs: vec![],
        deps: BTreeSet::new(),
    };
    for n in f.requires.iter().chain([&f.guard]) {
        let v = spec::eval(w, f, &s.vars, n, None)
            .with_context(|| format!("{}: invalid precondition", f.id()))?;
        s.deps.extend(v.deps);
        c.require(&mut s, n, "domain", v.safe);
        s.facts.push(e::true_term(v.term));
    }
    if let Some(rank) = w.rank(f) {
        let v = spec::eval(w, f, &s.vars, &rank, None)?;
        s.deps.extend(v.deps);
        let safe = e::all([
            v.safe,
            e::one(op::is_int, v.term.clone()),
            e::two(op::le, e::num(0), e::one(op::ival, v.term.clone())),
        ]);
        c.require(&mut s, &rank, "measure", safe);
        s.rank = Some(e::one(op::ival, v.term));
    }
    let results = c
        .eval(f, &f.body, s, 0)
        .with_context(|| format!("{}:{}: {}", f.span.file, f.span.line, f.id()))?;
    for (mut s, result) in results {
        let (post, deps) = spec::post(w, f, &args, &result)?;
        s.deps.extend(deps);
        c.emit(&s, &f.body, "post", post);
        c.deps.extend(s.deps);
    }
    let total = !w.recursive(f) || w.rank(f).is_some();
    let contract = contract {
        owner: f.id(),
        span: f.span.clone(),
        mode: f.mode,
        total,
        deps: c.deps,
        conditions: c.vcs.iter().map(|v| v.id.clone()).collect(),
    };
    Ok((contract, c.vcs))
}

impl check<'_, '_> {
    fn emit(&mut self, s: &state, n: &node, kind: &str, goal: e) {
        let loc = span {
            file: self.owner.span.file.clone(),
            line: if n.line == 0 {
                self.owner.span.line
            } else {
                n.line
            },
        };
        let v = vc::new(
            self.owner.id(),
            kind,
            loc,
            s.facts.clone(),
            goal,
            s.deps.clone(),
        );
        if !self.vcs.iter().any(|old| old.id == v.id) {
            self.vcs.push(v);
        }
        self.deps.extend(s.deps.iter().cloned());
    }
    fn require(&mut self, s: &mut state, n: &node, kind: &str, goal: e) {
        if goal != e::yes() {
            self.emit(s, n, kind, goal.clone());
            s.facts.push(goal);
        }
    }
    fn eval(&mut self, f: &fun, n: &node, s: state, depth: usize) -> Result<paths> {
        if depth > 128 || self.vcs.len() > 10000 {
            bail!("verification expansion limit exceeded; split the function into contracts");
        }
        let results = match &n.kind {
            kind::int { .. } | kind::atom { .. } | kind::var { .. } => {
                let v = spec::eval(self.world, f, &s.vars, n, None)?;
                vec![(s, v.term)]
            }
            kind::op { name, args }
                if matches!(name.as_str(), "and" | "or" | "&&" | "||") && args.len() == 2 =>
            {
                self.short(f, n, name, args, s, depth)?
            }
            kind::op { name, args } => {
                let mut out = vec![];
                for (mut s, args) in self.args(f, args, s, depth)? {
                    let v = bif::apply(name, &args)?;
                    self.require(&mut s, n, "safety", v.safe);
                    out.push((s, v.value));
                }
                out
            }
            kind::call { module, name, args } => {
                let callee =
                    self.world
                        .get(module.as_deref().unwrap_or(&f.module), name, args.len())?;
                let mut out = vec![];
                for (s, args) in self.args(f, args, s, depth)? {
                    out.push(self.call(f, callee, n, args, s)?);
                }
                out
            }
            kind::block { items } => {
                let mut current = vec![(s, e::atom(2))];
                for item in items {
                    let mut next = vec![];
                    for (s, _) in current {
                        next.extend(self.eval(f, item, s, depth + 1)?);
                    }
                    if next.len() > 1024 {
                        bail!("more than 1024 symbolic paths; split the function into contracts");
                    }
                    current = next;
                }
                current
            }
            kind::bind { pattern, value } => {
                let mut out = vec![];
                for (mut s, value) in self.eval(f, value, s, depth + 1)? {
                    let test = pattern::matches(self.world, pattern, value.clone(), &mut s.vars)?;
                    self.require(&mut s, n, "match", test);
                    out.push((s, value));
                }
                out
            }
            kind::branch { cond, yes, no } => self.branch(f, cond, yes, no, s, depth)?,
            kind::case { value, arms } => {
                let mut out = vec![];
                for (s, value) in self.eval(f, value, s, depth + 1)? {
                    out.extend(self.cases(f, n, arms, value, s, depth)?);
                }
                out
            }
            kind::tuple { items } => self
                .args(f, items, s, depth)?
                .into_iter()
                .map(|(s, items)| (s, e::one(op::tuple, e::list(&items, e::nil()))))
                .collect(),
            kind::list { items, tail } => {
                let mut out = vec![];
                for (s, items) in self.args(f, items, s, depth)? {
                    if let Some(tail) = tail {
                        for (s, tail) in self.eval(f, tail, s, depth + 1)? {
                            out.push((s, e::list(&items, tail)));
                        }
                    } else {
                        out.push((s, e::list(&items, e::nil())));
                    }
                }
                out
            }
            kind::assert { value } => {
                let mut s = s;
                let v = spec::eval(self.world, f, &s.vars, value, None)?;
                s.deps.extend(v.deps);
                self.require(&mut s, n, "assert", e::and(v.safe, e::true_term(v.term)));
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
        };
        if results.len() > 1024 {
            bail!("symbolic path limit exceeded");
        }
        Ok(results)
    }
    fn args(
        &mut self,
        f: &fun,
        args: &[node],
        s: state,
        depth: usize,
    ) -> Result<Vec<(state, Vec<e>)>> {
        let mut out = vec![(s, vec![])];
        for n in args {
            let mut next = vec![];
            for (s, values) in out {
                for (s, value) in self.eval(f, n, s, depth + 1)? {
                    let mut values = values.clone();
                    values.push(value);
                    next.push((s, values));
                }
            }
            if next.len() > 1024 {
                bail!("symbolic argument path limit exceeded");
            }
            out = next;
        }
        Ok(out)
    }
    fn call(
        &mut self,
        f: &fun,
        callee: &fun,
        n: &node,
        args: Vec<e>,
        mut s: state,
    ) -> Result<(state, e)> {
        if callee.mode == mode::ghost && !s.proof {
            bail!("ghost {} cannot be called by executable code", callee.id());
        }
        if s.proof && callee.mode != mode::ghost {
            bail!("erased proof code can only call total ghost functions");
        }
        if callee.mode == mode::private && f.module != callee.module {
            bail!("private function {} cannot be called remotely", callee.id());
        }
        let pre = spec::pre(self.world, callee, &args)?;
        s.deps.extend(pre.deps);
        s.deps.insert(callee.id());
        self.require(&mut s, n, "call", e::and(pre.safe, e::true_term(pre.term)));
        if let Some(before) = s
            .rank
            .clone()
            .filter(|_| !s.unfolding && self.world.same_cycle(f, callee))
        {
            let rank = self
                .world
                .rank(callee)
                .context("every member of a measured recursive cycle needs a measure")?;
            let v = spec::eval(self.world, callee, &spec::bind(callee, &args), &rank, None)?;
            self.require(
                &mut s,
                n,
                "decrease",
                e::all([
                    v.safe,
                    e::one(op::is_int, v.term.clone()),
                    e::two(op::le, e::num(0), e::one(op::ival, v.term.clone())),
                    e::two(op::lt, e::one(op::ival, v.term), before),
                ]),
            );
        }
        let result = e::call(callee.id(), args.clone());
        let (post, deps) = spec::post(self.world, callee, &args, &result)?;
        s.deps.extend(deps);
        s.facts.push(post);
        Ok((s, result))
    }
    fn branch(
        &mut self,
        f: &fun,
        cond: &node,
        yes: &node,
        no: &node,
        s: state,
        depth: usize,
    ) -> Result<paths> {
        let mut out = vec![];
        for (s, value) in self.eval(f, cond, s, depth + 1)? {
            let vars = s.vars.clone();
            let test = e::truthy(value);
            for (n, fact) in [(yes, test.clone()), (no, e::negate(test))] {
                if fact == e::no() {
                    continue;
                }
                let mut arm = s.clone();
                arm.facts.push(fact);
                for (mut arm, result) in self.eval(f, n, arm, depth + 1)? {
                    arm.vars = vars.clone();
                    out.push((arm, result));
                }
            }
        }
        Ok(out)
    }
    fn short(
        &mut self,
        f: &fun,
        n: &node,
        name: &str,
        args: &[node],
        s: state,
        depth: usize,
    ) -> Result<paths> {
        let mut out = vec![];
        for (mut s, a) in self.eval(f, &args[0], s, depth + 1)? {
            let strict = matches!(name, "and" | "or");
            if strict {
                self.require(&mut s, n, "boolean", e::is_bool(a.clone()));
            }
            let test = if strict {
                e::true_term(a.clone())
            } else {
                e::truthy(a.clone())
            };
            let take = if matches!(name, "and" | "&&") {
                test
            } else {
                e::negate(test)
            };
            if take != e::yes() {
                let mut skip = s.clone();
                skip.facts.push(e::negate(take.clone()));
                out.push((skip, a));
            }
            if take != e::no() {
                s.facts.push(take);
                out.extend(self.eval(f, &args[1], s, depth + 1)?);
            }
        }
        Ok(out)
    }
    fn cases(
        &mut self,
        f: &fun,
        n: &node,
        arms: &[arm],
        value: e,
        s: state,
        depth: usize,
    ) -> Result<paths> {
        let mut out = vec![];
        let mut previous = e::no();
        let vars = s.vars.clone();
        for arm in arms {
            if previous == e::yes() {
                break;
            }
            let mut branch = s.clone();
            let test = pattern::matches(self.world, &arm.pattern, value.clone(), &mut branch.vars)?;
            if test == e::no() {
                continue;
            }
            let guard = spec::eval(self.world, f, &branch.vars, &arm.guard, None)?;
            branch.deps.extend(guard.deps);
            branch.facts.push(e::negate(previous.clone()));
            branch.facts.push(test.clone());
            self.require(&mut branch, &arm.guard, "guard_domain", guard.safe.clone());
            let selected = e::all([test, guard.safe, e::true_term(guard.term)]);
            if selected == e::no() {
                continue;
            }
            branch.facts.push(selected.clone());
            for (mut branch, value) in self.eval(f, &arm.body, branch, depth + 1)? {
                branch.vars = vars.clone();
                out.push((branch, value));
            }
            previous = e::or(previous, selected);
        }
        self.emit(&s, n, "coverage", previous);
        Ok(out)
    }
    fn unfold(&mut self, f: &fun, n: &node, call: &node, s: state, depth: usize) -> Result<paths> {
        let kind::call { module, name, args } = &call.kind else {
            bail!("unfold requires a ghost function call");
        };
        let callee = self
            .world
            .get(module.as_deref().unwrap_or(&f.module), name, args.len())?;
        if callee.mode != mode::ghost {
            bail!("only total ghost definitions may be unfolded");
        }
        if self.owner.mode == mode::ghost && self.world.same_cycle(self.owner, callee) {
            bail!(
                "unfold inside a ghost definition cannot depend on its own recursive component; use a decreasing ghost call"
            );
        }
        let mut out = vec![];
        for (mut s, args) in self.args(f, args, s, depth)? {
            let vars = s.vars.clone();
            let proof = s.proof;
            let unfolding = s.unfolding;
            s.proof = true;
            s.unfolding = true;
            let (mut s, result) = self.call(f, callee, n, args.clone(), s)?;
            s.vars = spec::bind(callee, &args);
            s.unfolding = true;
            for (mut s, value) in self.eval(callee, &callee.body, s, depth + 1)? {
                s.facts.push(e::eq(result.clone(), value));
                s.vars = vars.clone();
                s.proof = proof;
                s.unfolding = unfolding;
                out.push((s, e::atom(2)));
            }
        }
        Ok(out)
    }
}

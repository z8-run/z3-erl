use anyhow::{Context, Result, bail};
use kernel::ir::{self, fun, kind, mode, node};
use std::collections::{BTreeMap, BTreeSet};

pub struct world<'a> {
    pub funs: BTreeMap<String, &'a fun>,
    pub atoms: BTreeMap<String, i64>,
    pub edges: BTreeMap<String, BTreeSet<String>>,
}

impl<'a> world<'a> {
    pub fn new(input: &'a ir::source) -> Result<Self> {
        if input.version != kernel::version {
            bail!("unsupported source protocol {}", input.version);
        }
        if input.functions.is_empty() {
            bail!("no verification contracts found in the selected sources");
        }
        let mut w = Self {
            funs: BTreeMap::new(),
            atoms: BTreeMap::new(),
            edges: BTreeMap::new(),
        };
        let mut atoms = BTreeSet::new();
        for f in &input.functions {
            crate::scope::check(&f.body, true).with_context(|| f.id())?;
            if w.funs.insert(f.id(), f).is_some() {
                bail!(
                    "{}: multiple function clauses are not yet supported; use a case body",
                    f.id()
                );
            }
            let unique: BTreeSet<_> = f.args.iter().collect();
            if unique.len() != f.args.len() || f.args.iter().any(|s| s == "_") {
                bail!("{}: arguments must be distinct names", f.id());
            }
            for n in f
                .requires
                .iter()
                .chain(&f.ensures)
                .chain([&f.guard, &f.body])
                .chain(f.decreases.iter())
            {
                walk(n, &mut |n| {
                    if let kind::atom { value } = &n.kind {
                        atoms.insert(value.clone());
                    }
                });
            }
        }
        for (a, i) in [("false", 0), ("true", 1), ("nil", 2)] {
            w.atoms.insert(a.into(), i);
            atoms.remove(a);
        }
        for (i, a) in atoms.into_iter().enumerate() {
            w.atoms.insert(a, i as i64 + 3);
        }
        for f in &input.functions {
            let mut edges = BTreeSet::new();
            walk(&f.body, &mut |n| {
                if let kind::call { module, name, args } = &n.kind {
                    edges.insert(format!(
                        "{}.{}/{}",
                        module.as_deref().unwrap_or(&f.module),
                        name,
                        args.len()
                    ));
                }
            });
            for e in &edges {
                if !w.funs.contains_key(e) {
                    bail!(
                        "{}:{}: {} calls {e}, which has no supported contract",
                        f.span.file,
                        f.span.line,
                        f.id()
                    );
                }
            }
            w.edges.insert(f.id(), edges);
        }
        for f in &input.functions {
            if f.mode == mode::ghost && w.recursive(f) && w.rank(f).is_none() {
                bail!(
                    "{}: recursive ghost functions need @verifier decreases (the one-argument default is unavailable)",
                    f.id()
                );
            }
        }
        Ok(w)
    }
    pub fn get(&self, module: &str, name: &str, arity: usize) -> Result<&'a fun> {
        let id = format!("{module}.{name}/{arity}");
        self.funs
            .get(&id)
            .copied()
            .with_context(|| format!("no verified function {id}"))
    }
    pub fn reaches(&self, from: &str, target: &str) -> bool {
        let mut todo = vec![from];
        let mut seen = BTreeSet::new();
        while let Some(id) = todo.pop() {
            if !seen.insert(id) {
                continue;
            }
            if let Some(edges) = self.edges.get(id) {
                for e in edges {
                    if e == target {
                        return true;
                    }
                    todo.push(e);
                }
            }
        }
        false
    }
    pub fn recursive(&self, f: &fun) -> bool {
        self.reaches(&f.id(), &f.id())
    }
    pub fn same_cycle(&self, a: &fun, b: &fun) -> bool {
        a.id() == b.id() || (self.reaches(&a.id(), &b.id()) && self.reaches(&b.id(), &a.id()))
    }
    pub fn rank(&self, f: &fun) -> Option<node> {
        f.decreases.clone().or_else(|| {
            if f.mode == mode::ghost && f.args.len() == 1 && self.recursive(f) {
                Some(node::var(&f.args[0]))
            } else {
                None
            }
        })
    }
}

pub fn walk(n: &node, f: &mut impl FnMut(&node)) {
    f(n);
    match &n.kind {
        kind::op { args, .. } | kind::call { args, .. } => {
            for n in args {
                walk(n, f);
            }
        }
        kind::tuple { items } | kind::block { items } => {
            for n in items {
                walk(n, f);
            }
        }
        kind::list { items, tail } => {
            for n in items {
                walk(n, f);
            }
            if let Some(n) = tail {
                walk(n, f);
            }
        }
        kind::branch { cond, yes, no } => {
            for n in [cond, yes, no] {
                walk(n, f);
            }
        }
        kind::case { value, arms } => {
            walk(value, f);
            for a in arms {
                for n in [&a.pattern, &a.guard, &a.body] {
                    walk(n, f);
                }
            }
        }
        kind::bind { pattern, value } => {
            walk(pattern, f);
            walk(value, f);
        }
        kind::assert { value } => walk(value, f),
        kind::ghost { body } => walk(body, f),
        kind::unfold { call } => walk(call, f),
        _ => {}
    }
}

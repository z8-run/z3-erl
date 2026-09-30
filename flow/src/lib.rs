#![allow(non_camel_case_types)]
//! Contract checking without any knowledge of solver syntax or processes.
mod clause;
mod exec;
pub mod graph;
pub mod model;
pub mod pattern;
mod proof;
mod rank;
mod scope;
pub mod spec;

use anyhow::Result;
use kernel::{ir, vc::vc};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize)]
pub struct contract {
    pub owner: String,
    pub span: ir::span,
    pub mode: ir::mode,
    pub total: bool,
    pub deps: BTreeSet<String>,
    pub conditions: Vec<String>,
    pub assumptions: Vec<admission>,
}

#[derive(Clone, Debug, Serialize)]
pub struct admission {
    pub span: ir::span,
    pub formula: kernel::logic::expr,
}

#[derive(Serialize)]
pub struct plan {
    pub profile: &'static str,
    pub atoms: BTreeMap<String, i64>,
    pub contracts: Vec<contract>,
    pub conditions: Vec<vc>,
}

pub fn lower(input: &ir::source) -> Result<plan> {
    let world = graph::world::new(input)?;
    let mut out = plan {
        profile: kernel::profile,
        atoms: world.atoms.clone(),
        contracts: vec![],
        conditions: vec![],
    };
    for f in world.funs.values() {
        let (contract, vcs) = exec::check(&world, f)?;
        out.contracts.push(contract);
        out.conditions.extend(vcs);
    }
    // Totality is compositional: a terminating caller cannot rely on a partial callee.
    loop {
        let partial: BTreeSet<_> = out
            .contracts
            .iter()
            .filter(|c| !c.total)
            .map(|c| c.owner.clone())
            .collect();
        let mut changed = false;
        for c in &mut out.contracts {
            if c.total && c.deps.iter().any(|d| partial.contains(d)) {
                c.total = false;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    Ok(out)
}

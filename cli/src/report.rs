use crate::jobs;
use back::solver::evidence;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize)]
pub struct contract {
    pub owner: String,
    pub span: kernel::ir::span,
    pub correctness: String,
    pub status: String,
    pub conditions: Vec<String>,
    pub depends_on: BTreeSet<String>,
}

#[derive(Serialize)]
pub struct report {
    pub build: &'static str,
    pub schema: &'static str,
    pub profile: &'static str,
    pub lean: &'static str,
    pub engine: String,
    pub scope: &'static str,
    pub success: bool,
    pub artifacts: String,
    pub files: Vec<kernel::ir::file>,
    pub unchecked: Vec<kernel::ir::skip>,
    pub tools: BTreeMap<String, String>,
    pub contracts: Vec<contract>,
    pub evidence: Vec<evidence>,
    pub models: Vec<evidence>,
}

pub fn contracts(plan: &flow::plan, results: &[Vec<evidence>], engine: &str) -> Vec<contract> {
    let passed: BTreeSet<_> = results
        .iter()
        .filter(|r| jobs::passed(r, engine))
        .filter_map(|r| r.first().map(|r| r.id.clone()))
        .collect();
    let mut failed: BTreeSet<_> = plan
        .contracts
        .iter()
        .filter(|c| c.conditions.iter().any(|id| !passed.contains(id)))
        .map(|c| c.owner.clone())
        .collect();
    loop {
        let before = failed.len();
        for c in &plan.contracts {
            if c.deps.iter().any(|id| failed.contains(id)) {
                failed.insert(c.owner.clone());
            }
        }
        if before == failed.len() {
            break;
        }
    }
    plan.contracts
        .iter()
        .map(|c| contract {
            owner: c.owner.clone(),
            span: c.span.clone(),
            correctness: if c.total { "total" } else { "partial" }.into(),
            status: if failed.contains(&c.owner) {
                "unproved"
            } else {
                "proved"
            }
            .into(),
            conditions: c.conditions.clone(),
            depends_on: c.deps.clone(),
        })
        .collect()
}

pub fn show(r: &report, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(r)?);
        return Ok(());
    }
    for c in &r.contracts {
        println!("{} {} ({})", c.status, c.owner, c.correctness);
    }
    for e in &r.evidence {
        if e.status != back::solver::status::proved {
            println!(
                "  {:?} {} {} — {}",
                e.status,
                e.engine,
                e.id,
                e.detail.lines().take(3).collect::<Vec<_>>().join(" ")
            );
        }
    }
    for m in &r.models {
        println!("model {:?}: {}", m.status, m.artifact);
    }
    if !r.unchecked.is_empty() {
        println!(
            "{} function(s) outside verification scope; use --strict to require coverage",
            r.unchecked.len()
        );
    }
    println!(
        "{}; {} contracts, {} solver results, {} models",
        if r.success { "checked" } else { "incomplete" },
        r.contracts.len(),
        r.evidence.len(),
        r.models.len()
    );
    println!("artifacts: {}", r.artifacts);
    Ok(())
}

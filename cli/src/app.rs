use crate::{
    args::{self, opts},
    config::resolved,
    jobs, project, report,
};
use anyhow::{Context, Result, bail};
use back::{run, solver};
use std::{collections::BTreeMap, path::Path, time::Duration};

pub fn run(o: opts) -> Result<i32> {
    if o.command == "help" {
        print!("{}", args::help);
        return Ok(0);
    }
    if o.command == "version" {
        println!("vex {} ({})", env!("CARGO_PKG_VERSION"), kernel::profile);
        return Ok(0);
    }
    let c = resolved::new(&o)?;
    if o.command == "kernel" {
        let path = c.root.join("kernel/lean/term.lean");
        run::save(&path, &back::print::lean::theory())?;
        println!("{}", path.display());
        return Ok(0);
    }
    let versions = versions(&c);
    if o.command == "doctor" {
        return doctor(&c, &versions, o.json);
    }
    let out = project::output(&c)?;
    if o.command == "model" {
        return models_only(&c, &o, &out, versions);
    }
    let source = project::read(&c, &out)?;
    let plan = flow::lower(&source)?;
    run::save(
        &out.join("plan.json"),
        &serde_json::to_string_pretty(&plan)?,
    )?;
    let settings = c.solver(out.join("vc"));
    if o.command == "emit" {
        let engines = if c.engine == "auto" || c.engine == "all" {
            vec!["z3", "boogie", "lean"]
        } else {
            vec![c.engine.as_str()]
        };
        for v in &plan.conditions {
            for e in &engines {
                solver::emit(v, e, &settings)?;
            }
        }
        println!(
            "emitted {} conditions in {}; no proofs executed",
            plan.conditions.len(),
            out.display()
        );
        return Ok(0);
    }
    if !o.json {
        eprintln!(
            "checking {} contracts / {} obligations with {} ({} jobs)",
            plan.contracts.len(),
            plan.conditions.len(),
            c.engine,
            c.jobs
        );
    }
    let evidence = jobs::run(&plan.conditions, &c.engine, c.jobs, &settings)?;
    let contracts = report::contracts(&plan, &evidence, &c.engine);
    let models = models(&c, Some(&source), &out)?;
    project::unchanged(&source)?;
    let success = contracts.iter().all(|c| c.status == "proved")
        && models
            .iter()
            .all(|m| m.status == solver::status::model_checked);
    let report = report::report {
        build: env!("VEX_BUILD"),
        schema: "vex.report.1",
        profile: kernel::profile,
        lean: kernel::lean,
        engine: c.engine.clone(),
        scope: "selected contracts under the discrete profile; configured finite temporal models",
        success,
        artifacts: out.display().to_string(),
        files: source.files,
        unchecked: source.skipped,
        tools: versions,
        contracts,
        evidence: evidence.into_iter().flatten().collect(),
        models,
    };
    finish(&c, &out, &report, o.json)
}

fn models(
    c: &resolved,
    source: Option<&kernel::ir::source>,
    out: &Path,
) -> Result<Vec<solver::evidence>> {
    let mut results = vec![];
    for (i, m) in c.models.iter().enumerate() {
        let file = m
            .file
            .canonicalize()
            .with_context(|| format!("missing TLA+ model {}", m.file.display()))?;
        let cfg = m
            .cfg
            .clone()
            .unwrap_or_else(|| back::tla::default_cfg(&file))
            .canonicalize()
            .context("missing TLC configuration")?;
        let mut exports = BTreeMap::new();
        if !m.exports.is_empty() {
            let source =
                source.context("model source exports require configured Elixir sources")?;
            let world = flow::graph::world::new(source)?;
            for (name, id) in &m.exports {
                exports.insert(name.clone(), flow::model::export(&world, id)?);
            }
        }
        let generated = if exports.is_empty() {
            String::new()
        } else {
            back::tla::module(&exports)?
        };
        let model_out = out.join(format!("models/{i}"));
        let settings = back::tla::settings {
            java: &c.java,
            jar: &c.tlc,
            out: &model_out,
            timeout_ms: c.timeout_ms,
        };
        results.push(back::tla::check(&file, &cfg, &generated, &settings)?);
    }
    Ok(results)
}

fn models_only(
    c: &resolved,
    o: &opts,
    out: &Path,
    versions: BTreeMap<String, String>,
) -> Result<i32> {
    if c.models.is_empty() {
        bail!("no TLA+ models selected");
    }
    let source = if c.models.iter().any(|m| !m.exports.is_empty()) {
        Some(project::read(c, out)?)
    } else {
        None
    };
    let results = models(c, source.as_ref(), out)?;
    if let Some(source) = &source {
        project::unchanged(source)?;
    }
    let report = report::report {
        build: env!("VEX_BUILD"),
        schema: "vex.report.1",
        profile: kernel::profile,
        lean: kernel::lean,
        engine: "tlc".into(),
        scope: "finite temporal models only; source contracts were not checked",
        success: results
            .iter()
            .all(|m| m.status == solver::status::model_checked),
        artifacts: out.display().to_string(),
        files: source.map(|s| s.files).unwrap_or_default(),
        unchecked: vec![],
        tools: versions,
        contracts: vec![],
        evidence: vec![],
        models: results,
    };
    finish(c, out, &report, o.json)
}

fn finish(c: &resolved, out: &Path, r: &report::report, json: bool) -> Result<i32> {
    let text = serde_json::to_string_pretty(r)?;
    run::save(&out.join("report.json"), &text)?;
    run::save(&c.out.join("report.json"), &text)?;
    report::show(r, json)?;
    Ok(if r.success { 0 } else { 1 })
}

fn versions(c: &resolved) -> BTreeMap<String, String> {
    let commands = [
        ("elixir", &c.elixir, vec!["--version".into()]),
        ("z3", &c.z3, vec!["--version".into()]),
        ("boogie", &c.boogie, vec!["/version".into()]),
        (
            "lean",
            &c.lake,
            vec![
                format!("+{}", kernel::lean).into(),
                "env".into(),
                "lean".into(),
                "--version".into(),
            ],
        ),
        ("java", &c.java, vec!["-version".into()]),
    ];
    commands
        .into_iter()
        .map(|(name, tool, args)| {
            let text = match run::command(tool, &args, &c.root, Duration::from_secs(10)) {
                Ok(r) if r.code == Some(0) => format!("{}{}", r.stdout, r.stderr).trim().into(),
                Ok(r) => format!("unavailable: {}{}", r.stdout, r.stderr),
                Err(e) => format!("unavailable: {e}"),
            };
            (name.into(), text)
        })
        .collect()
}

fn doctor(c: &resolved, versions: &BTreeMap<String, String>, json: bool) -> Result<i32> {
    let mut rows = versions.clone();
    rows.insert(
        "tlc".into(),
        if c.tlc.is_file() {
            format!(
                "{} sha256:{}",
                c.tlc.display(),
                kernel::hash(&std::fs::read(&c.tlc)?)
            )
        } else {
            format!("unavailable: {}", c.tlc.display())
        },
    );
    let ok = rows.values().all(|s| !s.starts_with("unavailable"))
        && versions["lean"].contains("version 4.28.0,");
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for (name, value) in rows {
            println!("{name}: {value}");
        }
    }
    Ok(if ok { 0 } else { 2 })
}

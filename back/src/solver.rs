pub use crate::run::locate;
use crate::{print, run};
use anyhow::{Result, bail};
use kernel::vc::vc;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::{ffi::OsString, path::PathBuf, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum status {
    proved,
    model_checked,
    counterexample,
    unknown,
    timeout,
    unavailable,
    error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct evidence {
    pub id: String,
    pub engine: String,
    pub status: status,
    pub artifact: String,
    pub log: String,
    pub detail: String,
    pub ms: u128,
}

#[derive(Clone)]
pub struct settings {
    pub root: PathBuf,
    pub out: PathBuf,
    pub proofs: Option<PathBuf>,
    pub z3: PathBuf,
    pub boogie: PathBuf,
    pub lake: PathBuf,
    pub timeout_ms: u64,
    pub lean_build: OnceLock<std::result::Result<(), String>>,
}

pub fn check(v: &vc, engine: &str, cfg: &settings) -> Result<evidence> {
    let (suffix, text, program, args) = prepare(v, engine, cfg)?;
    let path = cfg.out.join(format!("{}.{}", v.id, suffix));
    run::save(&path, &text)?;
    let log = cfg.out.join(format!("{}.{}.log", v.id, engine));
    if let Some(Err(detail)) = (engine == "lean").then(|| ensure_lean(cfg)) {
        run::save(&log, detail)?;
        return Ok(evidence {
            id: v.id.clone(),
            engine: engine.into(),
            status: status::error,
            artifact: path.display().to_string(),
            log: log.display().to_string(),
            detail: detail.clone(),
            ms: 0,
        });
    }
    let result = run::command(
        &program,
        &args,
        &cfg.root,
        Duration::from_millis(cfg.timeout_ms + 2000),
    );
    let (status, detail, ms) = match result {
        Err(e) => (status::unavailable, e.to_string(), 0),
        Ok(output) => {
            let all = format!("{}\n{}", output.stdout, output.stderr);
            run::save(&log, &all)?;
            let state = if output.timed_out {
                status::timeout
            } else {
                classify(engine, &output.stdout, output.code)
            };
            if engine == "z3" && state == status::counterexample {
                let model_path = cfg.out.join(format!("{}.model.smt2", v.id));
                run::save(&model_path, &print::smt::emit(v, cfg.timeout_ms, true))?;
                if let Ok(model) = run::command(
                    &cfg.z3,
                    &["-smt2".into(), model_path.into_os_string()],
                    &cfg.root,
                    Duration::from_millis(cfg.timeout_ms + 1000),
                ) {
                    run::save(&cfg.out.join(format!("{}.model.log", v.id)), &model.stdout)?;
                }
            }
            (state, all.chars().take(1200).collect(), output.ms)
        }
    };
    if !log.exists() {
        run::save(&log, &detail)?;
    }
    Ok(evidence {
        id: v.id.clone(),
        engine: engine.into(),
        status,
        artifact: path.display().to_string(),
        log: log.display().to_string(),
        detail,
        ms,
    })
}

fn ensure_lean(cfg: &settings) -> &std::result::Result<(), String> {
    cfg.lean_build.get_or_init(|| {
        let build = || -> Result<()> {
            let args = vec![
                format!("+{}", kernel::lean).into(),
                "build".into(),
                "kernel".into(),
            ];
            let result = run::command(
                &cfg.lake,
                &args,
                &cfg.root,
                Duration::from_millis(cfg.timeout_ms.max(120000)),
            )?;
            run::save(
                &cfg.out.join("lean_build.log"),
                &format!("{}\n{}", result.stdout, result.stderr),
            )?;
            if result.code != Some(0) || result.timed_out {
                bail!(
                    "Lean kernel library failed to build: {}{}",
                    result.stdout,
                    result.stderr
                );
            }
            Ok(())
        };
        build().map_err(|e| format!("{e:#}"))
    })
}

fn prepare(
    v: &vc,
    engine: &str,
    cfg: &settings,
) -> Result<(&'static str, String, PathBuf, Vec<OsString>)> {
    let (suffix, text, program, mut args) = match engine {
        "z3" => (
            "smt2",
            print::smt::emit(v, cfg.timeout_ms, false),
            cfg.z3.clone(),
            vec!["-smt2".into()],
        ),
        "boogie" => (
            "bpl",
            print::bpl::emit(v),
            cfg.boogie.clone(),
            vec![
                format!("/timeLimit:{}", cfg.timeout_ms.div_ceil(1000)).into(),
                "/errorLimit:1".into(),
                format!("/proverOpt:PROVER_PATH={}", cfg.z3.display()).into(),
            ],
        ),
        "lean" => {
            let proof = cfg
                .proofs
                .as_ref()
                .map(|p| p.join(format!("{}.lean", v.id)))
                .filter(|p| p.exists())
                .map(std::fs::read_to_string)
                .transpose()?;
            (
                "lean",
                print::lean::emit(v, proof.as_deref()),
                cfg.lake.clone(),
                vec![
                    format!("+{}", kernel::lean).into(),
                    "env".into(),
                    "lean".into(),
                ],
            )
        }
        _ => bail!("unknown engine {engine}"),
    };
    args.push(
        cfg.out
            .join(format!("{}.{}", v.id, suffix))
            .into_os_string(),
    );
    Ok((suffix, text, program, args))
}

pub fn emit(v: &vc, engine: &str, cfg: &settings) -> Result<PathBuf> {
    let (suffix, text, _, _) = prepare(v, engine, cfg)?;
    let path = cfg.out.join(format!("{}.{}", v.id, suffix));
    run::save(&path, &text)?;
    Ok(path)
}

pub fn classify(engine: &str, stdout: &str, code: Option<i32>) -> status {
    if stdout.contains("[vex: output truncated") {
        return status::error;
    }
    match engine {
        "z3" => {
            if code != Some(0) || stdout.contains("(error") {
                return status::error;
            }
            let answers: Vec<_> = stdout.lines().filter(|s| !s.trim().is_empty()).collect();
            match answers.as_slice() {
                ["unsat"] => status::proved,
                ["sat"] => status::counterexample,
                ["unknown"] => status::unknown,
                _ => status::error,
            }
        }
        "boogie" => {
            if stdout.contains("inconclusive")
                || stdout.contains("timed out")
                || stdout.contains("timeout")
            {
                return status::unknown;
            }
            if code == Some(0)
                && stdout.lines().any(|s| {
                    s.trim() == "Boogie program verifier finished with 1 verified, 0 errors"
                })
            {
                status::proved
            } else if stdout.contains("This assertion might not hold")
                || stdout.contains("this assertion could not be proved")
            {
                status::counterexample
            } else {
                status::error
            }
        }
        "lean" => {
            if ["unknown tactic", "failed to synthesize", "unexpected token"]
                .iter()
                .any(|s| stdout.contains(s))
            {
                status::error
            } else if code == Some(0)
                && stdout.contains("vex audit ok")
                && !stdout.contains("declaration uses 'sorry'")
            {
                status::proved
            } else if stdout.contains("unsolved goals")
                || stdout.contains("omega could not prove")
                || stdout.contains("maximum number of heartbeats")
                || stdout.contains("maximum recursion depth")
                || (stdout.contains("Tactic `") && stdout.contains("failed"))
            {
                status::unknown
            } else {
                status::error
            }
        }
        _ => status::error,
    }
}

use crate::args::opts;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct config {
    pub sources: Vec<String>,
    pub engine: Option<String>,
    pub out: Option<PathBuf>,
    pub proofs: Option<PathBuf>,
    pub jobs: Option<usize>,
    pub timeout_ms: Option<u64>,
    pub strict: bool,
    pub tools: tools,
    pub models: Vec<model>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct tools {
    pub z3: Option<String>,
    pub boogie: Option<String>,
    pub lake: Option<String>,
    pub elixir: Option<String>,
    pub java: Option<String>,
    pub tlc: Option<PathBuf>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct model {
    pub file: PathBuf,
    pub cfg: Option<PathBuf>,
    #[serde(default)]
    pub exports: BTreeMap<String, String>,
}

pub struct resolved {
    pub root: PathBuf,
    pub sources: Vec<String>,
    pub engine: String,
    pub out: PathBuf,
    pub proofs: Option<PathBuf>,
    pub jobs: usize,
    pub timeout_ms: u64,
    pub strict: bool,
    pub models: Vec<model>,
    pub z3: PathBuf,
    pub boogie: PathBuf,
    pub lake: PathBuf,
    pub elixir: PathBuf,
    pub java: PathBuf,
    pub tlc: PathBuf,
}

impl resolved {
    pub fn new(o: &opts) -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let root = std::env::var_os("VEX_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .to_path_buf()
            })
            .canonicalize()?;
        let auto = o.paths.is_empty()
            && ["check", "emit", "model"].contains(&o.command.as_str())
            && cwd.join("vex.toml").is_file();
        let file = o
            .config
            .clone()
            .or_else(|| auto.then(|| cwd.join("vex.toml")));
        let (c, base) = if let Some(file) = file {
            let file = file
                .canonicalize()
                .context("configuration file does not exist")?;
            let c: config =
                toml::from_str(&std::fs::read_to_string(&file)?).context("invalid vex.toml")?;
            (c, file.parent().unwrap().to_path_buf())
        } else {
            (config::default(), cwd.clone())
        };
        let engine = o
            .engine
            .clone()
            .or(c.engine)
            .unwrap_or_else(|| "auto".into());
        if !["auto", "z3", "boogie", "lean", "all"].contains(&engine.as_str()) {
            bail!("unknown engine {engine}");
        }
        let jobs = o.jobs.or(c.jobs).unwrap_or(2);
        let timeout_ms = o.timeout.or(c.timeout_ms).unwrap_or(10000);
        if !(1..=128).contains(&jobs) || timeout_ms == 0 || timeout_ms > 86_400_000 {
            bail!("jobs must be 1..128 and timeout_ms 1..86400000");
        }
        let sources = if !o.paths.is_empty() && o.command != "model" {
            o.paths
                .iter()
                .map(|s| cwd.join(s).to_string_lossy().into_owned())
                .collect()
        } else {
            c.sources
                .iter()
                .map(|s| base.join(s).to_string_lossy().into_owned())
                .collect()
        };
        let mut models = c.models;
        for m in &mut models {
            m.file = absolute(&base, &m.file);
            m.cfg = m.cfg.as_ref().map(|p| absolute(&base, p));
        }
        if o.command == "model" && !o.paths.is_empty() {
            if o.paths.len() != 1 {
                bail!("model accepts one .tla file; use configuration for several models");
            }
            models = vec![model {
                file: absolute(&cwd, Path::new(&o.paths[0])),
                cfg: o.cfg.as_ref().map(|p| absolute(&cwd, p)),
                exports: BTreeMap::new(),
            }];
        }
        let boogie_default = root.join(".tools/boogie/boogie");
        Ok(Self {
            sources,
            engine,
            jobs,
            timeout_ms,
            strict: o.strict || c.strict,
            models,
            out: o
                .out
                .as_ref()
                .map(|p| absolute(&cwd, p))
                .or(c.out.as_ref().map(|p| absolute(&base, p)))
                .unwrap_or_else(|| cwd.join(".vex")),
            proofs: o
                .proofs
                .as_ref()
                .map(|p| absolute(&cwd, p))
                .or(c.proofs.as_ref().map(|p| absolute(&base, p))),
            z3: tool(c.tools.z3.as_deref(), "z3", &base),
            boogie: tool(
                c.tools.boogie.as_deref(),
                if boogie_default.is_file() {
                    boogie_default.to_str().unwrap()
                } else {
                    "boogie"
                },
                &base,
            ),
            lake: tool(c.tools.lake.as_deref(), "lake", &base),
            elixir: tool(c.tools.elixir.as_deref(), "elixir", &base),
            java: tool(c.tools.java.as_deref(), "java", &base),
            tlc: o
                .jar
                .as_ref()
                .map(|p| absolute(&cwd, p))
                .or(c.tools.tlc.as_ref().map(|p| absolute(&base, p)))
                .unwrap_or_else(|| root.join(".tools/tla2tools.jar")),
            root,
        })
    }
    pub fn solver(&self, out: PathBuf) -> back::solver::settings {
        back::solver::settings {
            root: self.root.clone(),
            out,
            proofs: self.proofs.clone(),
            z3: self.z3.clone(),
            boogie: self.boogie.clone(),
            lake: self.lake.clone(),
            timeout_ms: self.timeout_ms,
            lean_build: std::sync::OnceLock::new(),
        }
    }
}

fn absolute(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}
fn tool(value: Option<&str>, default: &str, base: &Path) -> PathBuf {
    let s = value.unwrap_or(default);
    let p = Path::new(s);
    if p.components().count() > 1 {
        absolute(base, p)
    } else {
        back::solver::locate(s).unwrap_or_else(|| p.to_path_buf())
    }
}

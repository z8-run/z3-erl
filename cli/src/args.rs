use anyhow::{Context, Result, bail};
use std::path::PathBuf;

pub const help: &str = r#"vex — contracts for Elixir, proofs and temporal models

  vex check lib/ [--engine auto|z3|boogie|lean|all]
  vex check --config vex.toml [--json] [--strict]
  vex emit lib/ --engine lean
  vex model spec/queue.tla [--cfg spec/queue.cfg]
  vex doctor
  vex kernel                  regenerate the shared Lean term theory

  --out DIR                   artifact root (default .vex)
  --proofs DIR                Lean proof terms named <condition-id>.lean
  --jobs N                    simultaneous obligations (default 2)
  --timeout MS                per-obligation deadline (default 10000)
  --config FILE               project configuration
  --jar FILE                  TLC jar override
  --json                      machine-readable report
  --strict                    require every discovered function to have a contract

check exits 0 only when every selected contract and configured model succeeds.
TLC success means the configured finite model was checked. emit runs no proofs.
Lean is always leanprover/lean4:v4.28.0.
"#;

#[derive(Default)]
pub struct opts {
    pub command: String,
    pub paths: Vec<String>,
    pub config: Option<PathBuf>,
    pub engine: Option<String>,
    pub out: Option<PathBuf>,
    pub proofs: Option<PathBuf>,
    pub jobs: Option<usize>,
    pub timeout: Option<u64>,
    pub json: bool,
    pub strict: bool,
    pub cfg: Option<PathBuf>,
    pub jar: Option<PathBuf>,
}

impl opts {
    pub fn parse() -> Result<Self> {
        Self::from(std::env::args().skip(1).collect())
    }
    pub fn from(args: Vec<String>) -> Result<Self> {
        let mut args = args.into_iter();
        let mut o = Self {
            command: args.next().unwrap_or_else(|| "help".into()),
            ..Self::default()
        };
        if o.command == "--help" || o.command == "-h" {
            o.command = "help".into();
        }
        if o.command == "--version" {
            o.command = "version".into();
        }
        if ![
            "check", "emit", "model", "doctor", "kernel", "help", "version",
        ]
        .contains(&o.command.as_str())
        {
            bail!("unknown command {}; run vex --help", o.command);
        }
        while let Some(arg) = args.next() {
            if arg == "--" {
                o.paths.extend(args);
                break;
            }
            if arg == "--json" {
                o.json = true;
                continue;
            }
            if arg == "--strict" {
                o.strict = true;
                continue;
            }
            if arg == "--help" {
                o.command = "help".into();
                continue;
            }
            if !arg.starts_with('-') {
                o.paths.push(arg);
                continue;
            }
            let value = args
                .next()
                .with_context(|| format!("{arg} requires a value"))?;
            match arg.as_str() {
                "--config" => o.config = Some(value.into()),
                "--out" => o.out = Some(value.into()),
                "--proofs" => o.proofs = Some(value.into()),
                "--engine" => o.engine = Some(value),
                "--cfg" => o.cfg = Some(value.into()),
                "--jar" => o.jar = Some(value.into()),
                "--jobs" => o.jobs = Some(value.parse().context("jobs must be an integer")?),
                "--timeout" => {
                    o.timeout = Some(value.parse().context("timeout must be milliseconds")?)
                }
                _ => bail!("unknown option {arg}"),
            }
        }
        if o.jobs.is_some_and(|n| n == 0 || n > 128) {
            bail!("jobs must be between 1 and 128");
        }
        if o.timeout == Some(0) {
            bail!("timeout must be positive");
        }
        if let Some(e) = o
            .engine
            .as_ref()
            .filter(|e| !["auto", "z3", "boogie", "lean", "all"].contains(&e.as_str()))
        {
            bail!("unknown engine {e}");
        }
        Ok(o)
    }
}

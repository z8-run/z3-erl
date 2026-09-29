use crate::{
    run,
    solver::{evidence, status},
};
use anyhow::{Context, Result, bail};
use kernel::logic::{expr, op};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fmt::Write,
    path::{Path, PathBuf},
    time::Duration,
};

pub fn module(exports: &BTreeMap<String, kernel::model::export>) -> Result<String> {
    let mut s = String::from(
        "---- MODULE vex ----\nEXTENDS Integers, Sequences\n\n\\* generated pure source operators; use each _pre domain predicate\n",
    );
    for (name, x) in exports {
        if !identifier(name) {
            bail!("invalid TLA+ export name {name}");
        }
        let args = x
            .args
            .iter()
            .map(|n| kernel::name(n))
            .collect::<Vec<_>>()
            .join(", ");
        let params = if args.is_empty() {
            String::new()
        } else {
            format!("({args})")
        };
        writeln!(
            s,
            "{name}_pre{params} == {}\n{name}{params} == {}\n",
            render(&x.pre)?,
            render(&x.value)?
        )
        .unwrap();
    }
    s.push_str("====\n");
    Ok(s)
}

fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

pub fn render(e: &expr) -> Result<String> {
    Ok(match e {
        expr::integer { value } => format!("({value})"),
        expr::boolean { value } => {
            if *value {
                "TRUE".into()
            } else {
                "FALSE".into()
            }
        }
        expr::var { name, .. } => kernel::name(name),
        expr::app { .. } => {
            bail!("TLA+ export contains an uninterpreted call; export concrete nonrecursive code")
        }
        expr::ite { cond, yes, no } => format!(
            "(IF {} THEN {} ELSE {})",
            render(cond)?,
            render(yes)?,
            render(no)?
        ),
        expr::prim { op, args } => primitive(*op, args)?,
    })
}

fn primitive(p: op, args: &[expr]) -> Result<String> {
    let a = args.iter().map(render).collect::<Result<Vec<_>>>()?;
    if p.constructor() {
        let c = kernel::theory::terms.iter().find(|c| c.op == p).unwrap();
        let mut fields = vec![format!("tag |-> \"{}\"", p.name())];
        fields.extend(
            c.fields
                .iter()
                .zip(&a)
                .map(|((p, _), a)| format!("{} |-> {a}", p.name())),
        );
        return Ok(format!("[{}]", fields.join(", ")));
    }
    if let Some(c) = p.tested() {
        return Ok(format!("({}.tag = \"{}\")", a[0], c.name()));
    }
    if let Some((c, _)) = p.selected() {
        return Ok(format!(
            "(IF {}.tag = \"{}\" THEN {}.{} ELSE {})",
            a[0],
            c.name(),
            a[0],
            p.name(),
            render(&p.default())?
        ));
    }
    let sign = match p {
        op::eq => "=",
        op::not => "~",
        op::and => "/\\",
        op::or => "\\/",
        op::add => "+",
        op::sub => "-",
        op::mul => "*",
        op::div => "\\div",
        op::lt => "<",
        op::le => "<=",
        _ => unreachable!(),
    };
    Ok(if a.len() == 1 {
        format!("({sign}{})", a[0])
    } else {
        format!("({} {sign} {})", a[0], a[1])
    })
}

pub struct settings<'a> {
    pub java: &'a Path,
    pub jar: &'a Path,
    pub out: &'a Path,
    pub timeout_ms: u64,
}

pub fn check(file: &Path, cfg: &Path, exports: &str, settings: &settings) -> Result<evidence> {
    if !settings.jar.is_file() {
        bail!(
            "TLC jar is missing: {}; run bin/setup.sh",
            settings.jar.display()
        );
    }
    let stage = settings.out;
    std::fs::create_dir_all(stage)?;
    let mut inputs = BTreeMap::new();
    for entry in std::fs::read_dir(file.parent().context("model has no parent directory")?)? {
        let path = entry?.path();
        if path.extension().is_some_and(|s| s == "tla") {
            let bytes = std::fs::read(&path)?;
            inputs.insert(path.display().to_string(), kernel::hash(&bytes));
            std::fs::write(stage.join(path.file_name().unwrap()), bytes)?;
        }
    }
    let cfg_bytes = std::fs::read(cfg)?;
    inputs.insert(cfg.display().to_string(), kernel::hash(&cfg_bytes));
    std::fs::write(stage.join("model.cfg"), cfg_bytes)?;
    if !exports.is_empty() {
        run::save(&stage.join("vex.tla"), exports)?;
        inputs.insert("generated/vex.tla".into(), kernel::hash(exports.as_bytes()));
    }
    inputs.insert(
        "tlc.jar".into(),
        kernel::hash(&std::fs::read(settings.jar)?),
    );
    run::save(
        &stage.join("inputs.json"),
        &serde_json::to_string_pretty(&inputs)?,
    )?;
    let args: Vec<OsString> = vec![
        "-XX:ActiveProcessorCount=2".into(),
        "-Xmx1g".into(),
        "-cp".into(),
        settings.jar.as_os_str().into(),
        "tlc2.TLC".into(),
        "-workers".into(),
        "1".into(),
        "-tool".into(),
        "-metadir".into(),
        stage.join("states").into_os_string(),
        "-config".into(),
        "model.cfg".into(),
        file.file_name().unwrap().into(),
    ];
    let result = run::command(
        settings.java,
        &args,
        stage,
        Duration::from_millis(settings.timeout_ms),
    )?;
    let log = stage.join("tlc.log");
    let text = format!("{}\n{}", result.stdout, result.stderr);
    run::save(&log, &text)?;
    let status = if result.timed_out {
        status::timeout
    } else if result.code == Some(0)
        && text.contains("Model checking completed. No error has been found.")
    {
        status::model_checked
    } else if text.contains("is violated")
        || text.contains("Deadlock reached")
        || text.contains("Temporal properties were violated")
    {
        status::counterexample
    } else {
        status::error
    };
    Ok(evidence {
        id: kernel::hash(&serde_json::to_vec(&inputs)?)[..24].into(),
        engine: "tlc".into(),
        status,
        artifact: stage.join(file.file_name().unwrap()).display().to_string(),
        log: log.display().to_string(),
        detail: if status == status::model_checked {
            "finite model checked with the supplied configuration; no source refinement claim"
                .into()
        } else {
            text.chars().take(1200).collect()
        },
        ms: result.ms,
    })
}

pub fn default_cfg(file: &Path) -> PathBuf {
    file.with_extension("cfg")
}

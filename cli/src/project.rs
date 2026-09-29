use crate::config::resolved;
use anyhow::{Context, Result, bail};
use back::run;
use kernel::ir;
use std::{
    collections::BTreeSet,
    ffi::OsString,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn discover(patterns: &[String]) -> Result<Vec<PathBuf>> {
    let mut files = BTreeSet::new();
    for pattern in patterns {
        let found = glob::glob(pattern)?.collect::<std::result::Result<Vec<_>, _>>()?;
        if found.is_empty() {
            bail!("source path or pattern matched nothing: {pattern}");
        }
        for path in found {
            scan(&path.canonicalize()?, &mut files)?;
        }
    }
    if files.is_empty() {
        bail!("no .ex or .exs sources; pass a file/directory or set sources in vex.toml");
    }
    Ok(files.into_iter().collect())
}

fn scan(path: &Path, out: &mut BTreeSet<PathBuf>) -> Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    if meta.is_file() {
        if path.extension().is_some_and(|s| s == "ex" || s == "exs") {
            out.insert(path.to_path_buf());
        }
        return Ok(());
    }
    if !meta.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let name = entry.file_name();
        if [".git", ".vex", ".tools", "target", "deps", "_build"]
            .iter()
            .any(|s| name == *s)
        {
            continue;
        }
        scan(&entry.path(), out)?;
    }
    Ok(())
}

pub fn read(c: &resolved, out: &Path) -> Result<ir::source> {
    let files = discover(&c.sources)?;
    let manifest = out.join("paths.json");
    run::save(&manifest, &serde_json::to_string(&files)?)?;
    let ir = out.join("source.json");
    let args: Vec<OsString> = vec![
        c.root.join("front/read.exs").into_os_string(),
        "--manifest".into(),
        manifest.into_os_string(),
        "--out".into(),
        ir.as_os_str().into(),
    ];
    let result = run::command(
        &c.elixir,
        &args,
        &c.root,
        Duration::from_millis(c.timeout_ms.max(30000)),
    )?;
    run::save(
        &out.join("front.log"),
        &format!("{}\n{}", result.stdout, result.stderr),
    )?;
    if result.timed_out {
        bail!("Elixir frontend timed out");
    }
    if result.code != Some(0) {
        bail!("Elixir frontend: {}", result.stderr);
    }
    let source: ir::source =
        serde_json::from_slice(&std::fs::read(&ir)?).context("invalid frontend protocol")?;
    if source.files.len() != files.len() {
        bail!("frontend source inventory mismatch");
    }
    for (got, want) in source.files.iter().zip(files) {
        if Path::new(&got.path) != want {
            bail!("frontend file order mismatch");
        }
    }
    unchanged(&source)?;
    if c.strict && !source.skipped.is_empty() {
        bail!(
            "strict coverage: {} function(s) have no verification contract; see source.json",
            source.skipped.len()
        );
    }
    Ok(source)
}

pub fn unchanged(source: &ir::source) -> Result<()> {
    for file in &source.files {
        if kernel::hash(&std::fs::read(&file.path)?) != file.hash {
            bail!(
                "source changed during verification: {}; rerun the check",
                file.path
            );
        }
    }
    Ok(())
}

pub fn output(c: &resolved) -> Result<PathBuf> {
    std::fs::create_dir_all(&c.out)?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = c.out.join(format!("run_{stamp}_{}", std::process::id()));
    std::fs::create_dir(&path)?;
    Ok(path.canonicalize()?)
}

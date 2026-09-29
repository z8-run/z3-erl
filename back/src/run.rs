//! Processes are bounded, drained concurrently, and killed as a group on timeout.
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    ffi::OsString,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Serialize)]
pub struct output {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub ms: u128,
}

pub fn command(program: &Path, args: &[OsString], cwd: &Path, timeout: Duration) -> Result<output> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // These affect only the verifier's children, never the user's shell setup.
    cmd.env(
        "ERL_FLAGS",
        std::env::var("ERL_FLAGS").unwrap_or_else(|_| "+S 2:2".into()),
    );
    let dotnet_valid =
        std::env::var_os("DOTNET_ROOT").is_some_and(|p| Path::new(&p).join("dotnet").is_file());
    let dotnet_root = locate("dotnet")
        .filter(|_| !dotnet_valid)
        .and_then(|p| p.canonicalize().ok())
        .and_then(|p| p.parent().map(Path::to_path_buf));
    if let Some(root) = dotnet_root {
        cmd.env("DOTNET_ROOT", root);
    }
    cmd.env("DOTNET_ROLL_FORWARD", "Major")
        .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let started = Instant::now();
    let mut child = cmd
        .spawn()
        .with_context(|| format!("cannot start {}", program.display()))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || drain(stdout));
    let err = thread::spawn(move || drain(stderr));
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= timeout {
            timed_out = true;
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            #[cfg(not(unix))]
            {
                let _ = child.kill();
            }
            break child.wait()?;
        }
        thread::sleep(Duration::from_millis(10));
    };
    // A solver helper must not retain our pipes after its parent exits.
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    Ok(output {
        code: status.code(),
        stdout: out.join().unwrap()?,
        stderr: err.join().unwrap()?,
        timed_out,
        ms: started.elapsed().as_millis(),
    })
}

fn drain(mut stream: impl Read) -> Result<String> {
    let mut out = vec![];
    let mut buf = [0; 8192];
    let mut truncated = false;
    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let keep = n.min((16 * 1024 * 1024usize).saturating_sub(out.len()));
        out.extend_from_slice(&buf[..keep]);
        truncated |= keep < n;
    }
    let mut s = String::from_utf8_lossy(&out).into_owned();
    if truncated {
        s.push_str("\n[vex: output truncated at 16 MiB]\n");
    }
    Ok(s)
}

pub fn save(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file =
        File::create(path).with_context(|| format!("cannot write {}", path.display()))?;
    file.write_all(text.as_bytes())?;
    Ok(())
}

pub fn locate(name: &str) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.components().count() > 1 {
        return path.canonicalize().ok();
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|p| p.join(name))
        .find(|p| p.is_file())
}

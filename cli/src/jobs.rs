use anyhow::Result;
use back::solver::{self, evidence, settings, status};
use kernel::vc::vc;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

pub fn run(vcs: &[vc], engine: &str, jobs: usize, cfg: &settings) -> Result<Vec<Vec<evidence>>> {
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(vcs.len()) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= vcs.len() {
                        break;
                    }
                    let result = one(&vcs[i], engine, cfg);
                    results.lock().unwrap().push((i, result));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(i, _)| *i);
    results.into_iter().map(|(_, r)| r).collect()
}

fn one(v: &vc, engine: &str, cfg: &settings) -> Result<Vec<evidence>> {
    let engines = if engine == "auto" || engine == "all" {
        vec!["z3", "boogie", "lean"]
    } else {
        vec![engine]
    };
    let mut results = vec![];
    for e in engines {
        let r = solver::check(v, e, cfg)?;
        let stop = matches!(
            r.status,
            status::proved | status::counterexample | status::error
        );
        results.push(r);
        if engine == "auto" && stop {
            break;
        }
    }
    Ok(results)
}

pub fn passed(results: &[evidence], engine: &str) -> bool {
    if engine == "all" {
        results.len() == 3 && results.iter().all(|r| r.status == status::proved)
    } else {
        results.last().is_some_and(|r| r.status == status::proved)
    }
}

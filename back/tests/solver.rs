use back::solver::{classify, status};

#[test]
fn solver_success_requires_a_complete_unambiguous_result() {
    assert_eq!(classify("z3", "unsat\n", Some(0)), status::proved);
    assert_eq!(
        classify("z3", "unsat\n(error \"bad command\")\n", Some(0)),
        status::error
    );
    assert_eq!(classify("z3", "sat\nunsat\n", Some(0)), status::error);
    assert_eq!(classify("z3", "unsat\n", Some(1)), status::error);
    assert_eq!(classify("z3", "unknown\n", Some(0)), status::unknown);
    assert_eq!(classify("z3", "", Some(0)), status::error);
    assert_eq!(
        classify(
            "boogie",
            "Boogie program verifier finished with 0 verified, 0 errors\n",
            Some(0)
        ),
        status::error
    );
    assert_eq!(
        classify(
            "boogie",
            "Boogie program verifier finished with 1 verified, 0 errors\n",
            Some(0)
        ),
        status::proved
    );
    assert_eq!(classify("lean", "", Some(0)), status::error);
    assert_eq!(
        classify(
            "lean",
            "Tactic `omega` failed\nvex rejected axiom sorryAx",
            Some(1)
        ),
        status::unknown
    );
    assert_eq!(
        classify("lean", "vex audit ok\nvex rejected axiom sorryAx", Some(1)),
        status::error
    );
}

#[test]
fn all_backend_theories_share_the_kernel_description() {
    assert_eq!(
        back::print::lean::theory(),
        include_str!("../../kernel/lean/term.lean")
    );
    for constructor in kernel::theory::terms {
        assert!(back::print::smt::theory().contains(&format!("k_{}", constructor.op.name())));
        assert!(back::print::bpl::theory().contains(&format!("k_{}", constructor.op.name())));
    }
}

#[cfg(unix)]
#[test]
fn deadline_kills_descendants_that_hold_output_pipes() {
    use std::{
        path::Path,
        time::{Duration, Instant},
    };
    let started = Instant::now();
    let result = back::run::command(
        Path::new("/bin/sh"),
        &["-c".into(), "sleep 10 & wait".into()],
        Path::new("/tmp"),
        Duration::from_millis(80),
    )
    .unwrap();
    assert!(result.timed_out);
    assert!(started.elapsed() < Duration::from_secs(3));
    let result = back::run::command(
        Path::new("/bin/sh"),
        &["-c".into(), "sleep 10 & exit 0".into()],
        Path::new("/tmp"),
        Duration::from_millis(80),
    )
    .unwrap();
    assert_eq!(result.code, Some(0));
    assert!(started.elapsed() < Duration::from_secs(3));
}

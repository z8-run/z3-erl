use super::{dialect as d, render, sort_name};
use kernel::{theory::terms, vc::vc};
use std::fmt::Write;

pub fn theory() -> String {
    let mut s = String::from(
        "; generated from kernel::theory; erlang.discrete.1\n(declare-datatypes () ((term\n",
    );
    for c in terms {
        write!(s, "  (k_{}", c.op.name()).unwrap();
        for (p, t) in c.fields {
            write!(s, " (raw_{} {})", p.name(), sort_name(*t, d::smt)).unwrap();
        }
        s.push_str(")\n");
    }
    s.push_str(")))\n");
    for c in terms {
        writeln!(
            s,
            "(define-fun k_{} ((x term)) Bool ((_ is k_{}) x))",
            c.test.name(),
            c.op.name()
        )
        .unwrap();
        for (p, t) in c.fields {
            writeln!(
                s,
                "(define-fun k_{} ((x term)) {} (ite (k_{} x) (raw_{} x) {}))",
                p.name(),
                sort_name(*t, d::smt),
                c.test.name(),
                p.name(),
                render(&p.default(), d::smt)
            )
            .unwrap();
        }
    }
    s
}

pub fn emit(v: &vc, timeout_ms: u64, model: bool) -> String {
    let mut s = format!(
        "; {} {} {}:{}\n(set-option :timeout {timeout_ms})\n(set-option :produce-models true)\n",
        v.id,
        v.kind,
        v.span.file.escape_default(),
        v.span.line
    );
    s.push_str(&theory());
    let (vars, funs) = v.symbols();
    for (name, t) in vars {
        writeln!(
            s,
            "(declare-const {} {})",
            kernel::name(&name),
            sort_name(t, d::smt)
        )
        .unwrap();
    }
    for (name, n) in funs {
        writeln!(
            s,
            "(declare-fun {} ({}) term)",
            kernel::name(&name),
            vec!["term"; n].join(" ")
        )
        .unwrap();
    }
    for h in &v.hypotheses {
        writeln!(s, "(assert {})", render(h, d::smt)).unwrap();
    }
    writeln!(s, "(assert (not {}))\n(check-sat)", render(&v.goal, d::smt)).unwrap();
    if model {
        s.push_str("(get-model)\n");
    }
    s
}

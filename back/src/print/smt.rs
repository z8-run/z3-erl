use super::{dialect as d, render, sort_name};
use kernel::{
    theory::{datatypes, definitions},
    vc::vc,
};
use std::fmt::Write;

pub fn theory() -> String {
    let mut s = format!(
        "; generated from kernel::theory; {}\n(declare-datatypes () (\n",
        kernel::profile
    );
    for (t, ctors) in datatypes {
        writeln!(s, "({}", sort_name(*t, d::smt)).unwrap();
        for c in *ctors {
            write!(s, "  (k_{}", c.op.name()).unwrap();
            for (p, t) in c.fields {
                write!(s, " (raw_{} {})", p.name(), sort_name(*t, d::smt)).unwrap();
            }
            s.push_str(")\n");
        }
        s.push_str(")\n");
    }
    s.push_str("))\n");
    selectors(&mut s);
    recursive(&mut s);
    s
}

fn selectors(s: &mut String) {
    for (t, ctors) in datatypes {
        let input = sort_name(*t, d::smt);
        for c in *ctors {
            writeln!(
                s,
                "(define-fun k_{} ((x {input})) Bool ((_ is k_{}) x))",
                c.test.name(),
                c.op.name()
            )
            .unwrap();
            for (p, t) in c.fields {
                writeln!(
                    s,
                    "(define-fun k_{} ((x {input})) {} (ite (k_{} x) (raw_{} x) {}))",
                    p.name(),
                    sort_name(*t, d::smt),
                    c.test.name(),
                    p.name(),
                    render(&p.default(), d::smt)
                )
                .unwrap();
            }
        }
    }
}

fn recursive(s: &mut String) {
    s.push_str("(define-funs-rec (\n");
    let defs = definitions();
    for def in &defs {
        let params = def
            .params
            .iter()
            .map(|(n, t)| format!("({} {})", kernel::name(n), sort_name(*t, d::smt)))
            .collect::<Vec<_>>()
            .join(" ");
        writeln!(
            s,
            "(k_{} ({params}) {})",
            def.op.name(),
            sort_name(def.op.output(), d::smt)
        )
        .unwrap();
    }
    s.push_str(") (\n");
    for def in &defs {
        writeln!(s, "{}", render(&def.body, d::smt)).unwrap();
    }
    s.push_str("))\n");
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
    for law in kernel::theory::lemmas(v) {
        writeln!(s, "(assert {})", render(&law, d::smt)).unwrap();
    }
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

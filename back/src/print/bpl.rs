use super::{dialect as d, render, sort_name};
use kernel::{theory::terms, vc::vc};
use std::fmt::Write;

pub fn theory() -> String {
    let mut s = String::from(
        "// generated from kernel::theory; erlang.discrete.1\ntype term;\nfunction k_tag(x:term):int;\n",
    );
    for (tag, c) in terms.iter().enumerate() {
        let names: Vec<_> = (0..c.fields.len()).map(|i| format!("x{i}")).collect();
        let params: Vec<_> = c
            .fields
            .iter()
            .enumerate()
            .map(|(i, (_, t))| format!("x{i}:{}", sort_name(*t, d::bpl)))
            .collect();
        let ctor = if names.is_empty() {
            format!("k_{}", c.op.name())
        } else {
            format!("k_{}({})", c.op.name(), names.join(", "))
        };
        if names.is_empty() {
            writeln!(s, "const {ctor}:term;").unwrap();
        } else {
            writeln!(s, "function k_{}({}):term;", c.op.name(), params.join(", ")).unwrap();
        }
        writeln!(
            s,
            "function k_{}(x:term):bool {{ k_tag(x) == {tag} }}",
            c.test.name()
        )
        .unwrap();
        let quantify = |body: String| {
            if names.is_empty() {
                body
            } else {
                format!("(forall {} :: {{ {ctor} }} {body})", params.join(", "))
            }
        };
        writeln!(s, "axiom {};", quantify(format!("k_tag({ctor}) == {tag}"))).unwrap();
        for (i, (p, t)) in c.fields.iter().enumerate() {
            writeln!(
                s,
                "function k_{}(x:term):{};",
                p.name(),
                sort_name(*t, d::bpl)
            )
            .unwrap();
            writeln!(
                s,
                "axiom {};",
                quantify(format!("k_{}({ctor}) == x{i}", p.name()))
            )
            .unwrap();
            writeln!(
                s,
                "axiom (forall x:term :: {{ k_{}(x) }} k_tag(x) != {tag} ==> k_{}(x) == {});",
                p.name(),
                p.name(),
                render(&p.default(), d::bpl)
            )
            .unwrap();
        }
        let rebuilt = if names.is_empty() {
            ctor
        } else {
            format!(
                "k_{}({})",
                c.op.name(),
                c.fields
                    .iter()
                    .map(|(p, _)| format!("k_{}(x)", p.name()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        writeln!(
            s,
            "axiom (forall x:term :: {{ k_tag(x) }} k_tag(x) == {tag} ==> x == {rebuilt});"
        )
        .unwrap();
    }
    writeln!(
        s,
        "axiom (forall x:term :: {{ k_tag(x) }} 0 <= k_tag(x) && k_tag(x) < {});",
        terms.len()
    )
    .unwrap();
    s
}

pub fn emit(v: &vc) -> String {
    let mut s = theory();
    let (vars, funs) = v.symbols();
    for (name, n) in funs {
        writeln!(
            s,
            "function {}({}):term;",
            kernel::name(&name),
            (0..n)
                .map(|i| format!("x{i}:term"))
                .collect::<Vec<_>>()
                .join(", ")
        )
        .unwrap();
    }
    let params = vars
        .iter()
        .map(|(n, t)| format!("{}:{}", kernel::name(n), sort_name(*t, d::bpl)))
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(s, "procedure check({params})\n{{").unwrap();
    for h in &v.hypotheses {
        writeln!(s, "  assume {};", render(h, d::bpl)).unwrap();
    }
    writeln!(s, "  assert {};\n}}", render(&v.goal, d::bpl)).unwrap();
    s
}

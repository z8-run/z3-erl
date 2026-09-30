use super::{dialect as d, render, sort_name};
use kernel::{
    theory::{datatypes, definitions},
    vc::vc,
};
use std::fmt::Write;

pub fn theory() -> String {
    let mut s = format!(
        "// generated from kernel::theory; {}\ntype term;\ntype seq;\n",
        kernel::profile
    );
    for (t, ctors) in datatypes {
        let input = sort_name(*t, d::bpl);
        writeln!(s, "function k_tag_{input}(x:{input}):int;").unwrap();
        for (tag, c) in ctors.iter().enumerate() {
            constructor(&mut s, input, tag, c);
        }
        writeln!(
        s,
        "axiom (forall x:{input} :: {{ k_tag_{input}(x) }} 0 <= k_tag_{input}(x) && k_tag_{input}(x) < {});",
        ctors.len()
    )
    .unwrap();
    }
    recursive(&mut s);
    s
}

fn constructor(s: &mut String, input: &str, tag: usize, c: &kernel::theory::constructor) {
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
        writeln!(s, "const {ctor}:{input};").unwrap();
    } else {
        writeln!(
            s,
            "function k_{}({}):{input};",
            c.op.name(),
            params.join(", ")
        )
        .unwrap();
    }
    writeln!(
        s,
        "function k_{}(x:{input}):bool {{ k_tag_{input}(x) == {tag} }}",
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
    writeln!(
        s,
        "axiom {};",
        quantify(format!("k_tag_{input}({ctor}) == {tag}"))
    )
    .unwrap();
    for (i, (p, t)) in c.fields.iter().enumerate() {
        writeln!(
            s,
            "function k_{}(x:{input}):{};",
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
                "axiom (forall x:{input} :: {{ k_{}(x) }} k_tag_{input}(x) != {tag} ==> k_{}(x) == {});",
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
            "axiom (forall x:{input} :: {{ k_tag_{input}(x) }} k_tag_{input}(x) == {tag} ==> x == {rebuilt});"
        )
        .unwrap();
}

fn recursive(s: &mut String) {
    let defs = definitions();
    for def in &defs {
        let params = def
            .params
            .iter()
            .map(|(n, t)| format!("{}:{}", kernel::name(n), sort_name(*t, d::bpl)))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            s,
            "function k_{}({params}):{};",
            def.op.name(),
            sort_name(def.op.output(), d::bpl)
        )
        .unwrap();
    }
    for def in &defs {
        let params = def
            .params
            .iter()
            .map(|(n, t)| format!("{}:{}", kernel::name(n), sort_name(*t, d::bpl)))
            .collect::<Vec<_>>()
            .join(", ");
        let args = def
            .params
            .iter()
            .map(|(n, _)| kernel::name(n))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            s,
            "axiom (forall {params} :: {{ k_{}({args}) }} k_{}({args}) == {});",
            def.op.name(),
            def.op.name(),
            render(&def.body, d::bpl)
        )
        .unwrap();
    }
}

pub fn emit(v: &vc) -> String {
    let mut s = theory();
    for law in kernel::theory::lemmas(v) {
        writeln!(s, "axiom {};", render(&law, d::bpl)).unwrap();
    }
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

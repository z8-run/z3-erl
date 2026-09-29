use kernel::logic::{expr, op, sort};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum dialect {
    smt,
    bpl,
    lean,
}

pub fn sort_name(s: sort, d: dialect) -> &'static str {
    match (s, d) {
        (sort::term, _) => "term",
        (sort::int, dialect::bpl) => "int",
        (sort::int, _) => "Int",
        (sort::bool, dialect::lean) => "Prop",
        (sort::bool, dialect::smt) => "Bool",
        (sort::bool, dialect::bpl) => "bool",
    }
}

pub fn render(e: &expr, d: dialect) -> String {
    match e {
        expr::integer { value } => {
            if d == dialect::smt && value.starts_with('-') {
                format!("(- {})", &value[1..])
            } else if d == dialect::smt {
                value.clone()
            } else if d == dialect::lean {
                format!("({value} : Int)")
            } else {
                format!("({value})")
            }
        }
        expr::boolean { value } => match (value, d) {
            (true, dialect::lean) => "True",
            (false, dialect::lean) => "False",
            (true, _) => "true",
            (false, _) => "false",
        }
        .into(),
        expr::var { name, .. } => kernel::name(name),
        expr::app { name, args, .. } => apply(&kernel::name(name), args, d, false),
        expr::ite { cond, yes, no } => {
            let c = render(cond, d);
            let y = render(yes, d);
            let n = render(no, d);
            match d {
                dialect::smt => format!("(ite {c} {y} {n})"),
                _ => format!("(if {c} then {y} else {n})"),
            }
        }
        expr::prim { op, args } => primitive(*op, args, d),
    }
}

fn apply(name: &str, args: &[expr], d: dialect, constant: bool) -> String {
    let args: Vec<_> = args.iter().map(|a| render(a, d)).collect();
    match d {
        dialect::smt => {
            if args.is_empty() {
                name.into()
            } else {
                format!("({name} {})", args.join(" "))
            }
        }
        dialect::bpl => {
            if constant {
                name.into()
            } else {
                format!("{name}({})", args.join(", "))
            }
        }
        dialect::lean => {
            if args.is_empty() {
                name.into()
            } else {
                format!("({name} {})", args.join(" "))
            }
        }
    }
}

fn primitive(p: op, args: &[expr], d: dialect) -> String {
    if p.constructor() || p.tested().is_some() || p.selected().is_some() {
        let prefix = if d == dialect::lean {
            if p.constructor() { "term." } else { "kernel." }
        } else {
            "k_"
        };
        return apply(&format!("{prefix}{}", p.name()), args, d, p == op::nil);
    }
    let sign = match (p, d) {
        (op::eq, dialect::bpl) => "==",
        (op::eq, _) => "=",
        (op::not, dialect::lean) => "¬",
        (op::not, dialect::smt) => "not",
        (op::not, dialect::bpl) => "!",
        (op::and, dialect::lean) => "∧",
        (op::and, dialect::smt) => "and",
        (op::and, dialect::bpl) => "&&",
        (op::or, dialect::lean) => "∨",
        (op::or, dialect::smt) => "or",
        (op::or, dialect::bpl) => "||",
        (op::add, _) => "+",
        (op::sub, _) => "-",
        (op::mul, _) => "*",
        (op::div, dialect::lean) => "/",
        (op::div, _) => "div",
        (op::lt, _) => "<",
        (op::le, dialect::lean) => "≤",
        (op::le, _) => "<=",
        _ => unreachable!(),
    };
    if d == dialect::smt {
        return apply(sign, args, d, false);
    }
    let args: Vec<_> = args.iter().map(|a| render(a, d)).collect();
    if args.len() == 1 {
        format!("({sign} {})", args[0])
    } else {
        format!("({} {sign} {})", args[0], args[1])
    }
}

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum sort {
    term,
    seq,
    int,
    bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum expr {
    integer {
        value: String,
    },
    boolean {
        value: bool,
    },
    var {
        name: String,
        sort: sort,
    },
    app {
        name: String,
        args: Vec<expr>,
        sort: sort,
    },
    prim {
        op: op,
        args: Vec<expr>,
    },
    ite {
        cond: Box<expr>,
        yes: Box<expr>,
        no: Box<expr>,
    },
    quant {
        all: bool,
        vars: Vec<(String, sort)>,
        body: Box<expr>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum op {
    int,
    atom,
    nil,
    cons,
    tuple,
    empty,
    push,
    is_empty,
    is_push,
    first,
    rest,
    len,
    nth,
    size,
    mass,
    is_int,
    is_atom,
    is_nil,
    is_cons,
    is_tuple,
    ival,
    aval,
    head,
    tail,
    items,
    eq,
    not,
    and,
    or,
    add,
    sub,
    mul,
    div,
    lt,
    le,
}

impl expr {
    pub fn visit(&self, f: &mut impl FnMut(&Self)) {
        f(self);
        match self {
            Self::app { args, .. } | Self::prim { args, .. } => {
                for a in args {
                    a.visit(f);
                }
            }
            Self::ite { cond, yes, no } => {
                for a in [cond, yes, no] {
                    a.visit(f);
                }
            }
            Self::quant { body, .. } => body.visit(f),
            _ => {}
        }
    }
    pub fn num(n: impl ToString) -> Self {
        Self::integer {
            value: n.to_string(),
        }
    }
    pub fn yes() -> Self {
        Self::boolean { value: true }
    }
    pub fn no() -> Self {
        Self::boolean { value: false }
    }
    pub fn var(name: impl Into<String>, sort: sort) -> Self {
        Self::var {
            name: name.into(),
            sort,
        }
    }
    pub fn call(name: impl Into<String>, args: Vec<Self>) -> Self {
        Self::app {
            name: name.into(),
            args,
            sort: sort::term,
        }
    }
    pub fn one(op: op, a: Self) -> Self {
        Self::prim { op, args: vec![a] }.simplify()
    }
    pub fn two(op: op, a: Self, b: Self) -> Self {
        Self::prim {
            op,
            args: vec![a, b],
        }
        .simplify()
    }
    pub fn eq(a: Self, b: Self) -> Self {
        // Reading a boolean atom should expose its proposition, not split a term tree.
        if let (Self::ite { cond, yes, no }, Self::prim { op: op::atom, .. }) = (&a, &b) {
            return Self::ite(
                *cond.clone(),
                Self::eq(*yes.clone(), b.clone()),
                Self::eq(*no.clone(), b),
            );
        }
        if matches!(&a, Self::prim { op: op::atom, .. }) && matches!(&b, Self::ite { .. }) {
            return Self::eq(b, a);
        }
        Self::two(op::eq, a, b)
    }
    pub fn and(a: Self, b: Self) -> Self {
        Self::two(op::and, a, b)
    }
    pub fn or(a: Self, b: Self) -> Self {
        Self::two(op::or, a, b)
    }
    pub fn negate(a: Self) -> Self {
        Self::one(op::not, a)
    }
    pub fn all(xs: impl IntoIterator<Item = Self>) -> Self {
        xs.into_iter().fold(Self::yes(), Self::and)
    }
    pub fn any(xs: impl IntoIterator<Item = Self>) -> Self {
        xs.into_iter().fold(Self::no(), Self::or)
    }
    pub fn implies(a: Self, b: Self) -> Self {
        Self::or(Self::negate(a), b)
    }
    pub fn ite(c: Self, y: Self, n: Self) -> Self {
        if y.sort() == sort::bool {
            if c == y {
                return Self::or(c, n);
            }
            if c == n {
                return Self::and(c, y);
            }
        }
        match c {
            Self::boolean { value: true } => y,
            Self::boolean { value: false } => n,
            _ if y == n => y,
            _ if y == Self::yes() => Self::or(c, n),
            _ if n == Self::no() => Self::and(c, y),
            _ if y == Self::no() => Self::and(Self::negate(c), n),
            _ if n == Self::yes() => Self::or(Self::negate(c), y),
            _ => Self::ite {
                cond: Box::new(c),
                yes: Box::new(y),
                no: Box::new(n),
            },
        }
    }
    pub fn int(n: impl ToString) -> Self {
        Self::one(op::int, Self::num(n))
    }
    pub fn atom(n: impl ToString) -> Self {
        Self::one(op::atom, Self::num(n))
    }
    pub fn nil() -> Self {
        Self::prim {
            op: op::nil,
            args: vec![],
        }
    }
    pub fn empty() -> Self {
        Self::prim {
            op: op::empty,
            args: vec![],
        }
    }
    pub fn tuple(xs: &[Self]) -> Self {
        Self::one(
            op::tuple,
            xs.iter()
                .rev()
                .fold(Self::empty(), |t, h| Self::two(op::push, h.clone(), t)),
        )
    }
    pub fn quantify(all: bool, vars: Vec<(String, sort)>, body: Self) -> Self {
        if matches!(body, Self::boolean { .. }) || vars.is_empty() {
            return body;
        }
        // ∀ x:term, is_int(x) → p(x) iff ∀ i:Int, p(int(i)).
        // The existential form uses conjunction. This avoids a constructor
        // quantifier alternation for ordinary integer-indexed specifications.
        if let [(name, sort::term)] = vars.as_slice() {
            let test = Self::one(op::is_int, Self::var(name, sort::term));
            let guard = if all { Self::negate(test) } else { test };
            let connective = if all { op::or } else { op::and };
            if let Some(rest) = without(&body, &guard, connective) {
                let value = Self::one(op::int, Self::var(name, sort::int));
                return Self::quantify(
                    all,
                    vec![(name.clone(), sort::int)],
                    rest.replace(&BTreeMap::from([(name.clone(), value)])),
                );
            }
        }
        Self::quant {
            all,
            vars,
            body: Box::new(body),
        }
    }
    pub fn list(xs: &[Self], tail: Self) -> Self {
        xs.iter()
            .rev()
            .fold(tail, |t, h| Self::two(op::cons, h.clone(), t))
    }
    pub fn bool_term(c: Self) -> Self {
        Self::ite(c, Self::atom(1), Self::atom(0))
    }
    pub fn true_term(t: Self) -> Self {
        t.as_bool().unwrap_or_else(|| Self::eq(t, Self::atom(1)))
    }
    pub fn truthy(t: Self) -> Self {
        Self::negate(Self::or(
            Self::eq(t.clone(), Self::atom(0)),
            Self::eq(t, Self::atom(2)),
        ))
    }
    pub fn is_bool(t: Self) -> Self {
        if t.as_bool().is_some() {
            return Self::yes();
        }
        Self::or(
            Self::eq(t.clone(), Self::atom(0)),
            Self::eq(t, Self::atom(1)),
        )
    }
    /// Recognize expressions whose every value is a boolean atom.
    pub fn as_bool(&self) -> Option<Self> {
        if *self == Self::atom(1) {
            return Some(Self::yes());
        }
        if *self == Self::atom(0) {
            return Some(Self::no());
        }
        if let Self::ite { cond, yes, no } = self {
            return Some(Self::ite(*cond.clone(), yes.as_bool()?, no.as_bool()?));
        }
        None
    }
    pub fn sort(&self) -> sort {
        match self {
            Self::integer { .. } => sort::int,
            Self::boolean { .. } => sort::bool,
            Self::quant { .. } => sort::bool,
            Self::var { sort, .. } | Self::app { sort, .. } => *sort,
            Self::ite { yes, .. } => yes.sort(),
            Self::prim { op, .. } => op.output(),
        }
    }
    pub fn symbols(&self, vars: &mut BTreeMap<String, sort>, funs: &mut BTreeMap<String, usize>) {
        match self {
            Self::quant {
                vars: bound, body, ..
            } => {
                let mut local = BTreeMap::new();
                body.symbols(&mut local, funs);
                for (name, _) in bound {
                    local.remove(name);
                }
                vars.extend(local);
            }
            Self::var { name, sort } => {
                vars.insert(name.clone(), *sort);
            }
            Self::app { name, args, .. } => {
                funs.insert(name.clone(), args.len());
                for a in args {
                    a.symbols(vars, funs);
                }
            }
            Self::prim { args, .. } => {
                for a in args {
                    a.symbols(vars, funs);
                }
            }
            Self::ite { cond, yes, no } => {
                for a in [cond, yes, no] {
                    a.symbols(vars, funs);
                }
            }
            _ => {}
        }
    }
    /// Capture-avoiding substitution for the sorted logical algebra.
    pub fn replace(&self, values: &BTreeMap<String, Self>) -> Self {
        let sub = |e: &Self| e.replace(values);
        match self {
            Self::var { name, .. } => values.get(name).cloned().unwrap_or_else(|| self.clone()),
            Self::prim { op, args } => Self::prim {
                op: *op,
                args: args.iter().map(sub).collect(),
            }
            .simplify(),
            Self::app { name, args, sort } => Self::app {
                name: name.clone(),
                args: args.iter().map(sub).collect(),
                sort: *sort,
            },
            Self::ite { cond, yes, no } => Self::ite(sub(cond), sub(yes), sub(no)),
            Self::quant { all, vars, body } => {
                let mut local = values.clone();
                for (name, _) in vars {
                    local.remove(name);
                }
                let (bound, renamed) = freshen(vars, body, &local);
                Self::quantify(*all, bound, renamed.replace(&local))
            }
            _ => self.clone(),
        }
    }
    fn simplify(self) -> Self {
        let Self::prim { op: p, ref args } = self else {
            return self;
        };
        match (p, args.as_slice()) {
            (op::not, [Self::boolean { value }]) => return Self::boolean { value: !value },
            (op::not, [Self::prim { op: op::not, args }]) => return args[0].clone(),
            (op::eq, [Self::integer { value: a }, Self::integer { value: b }]) => {
                return Self::boolean {
                    value: integer_key(a) == integer_key(b),
                };
            }
            (op::eq, [a, b]) if a == b => return Self::yes(),
            (op::and | op::or, [a, b]) if a == b => return a.clone(),
            (op::and | op::or, [a, b])
                if *a == Self::negate(b.clone()) || *b == Self::negate(a.clone()) =>
            {
                return Self::boolean { value: p == op::or };
            }
            (op::and, [a, b]) if *a == Self::yes() => return b.clone(),
            (op::and, [a, b]) if *b == Self::yes() => return a.clone(),
            (op::and, [a, b]) if *a == Self::no() || *b == Self::no() => return Self::no(),
            (op::or, [a, b]) if *a == Self::no() => return b.clone(),
            (op::or, [a, b]) if *b == Self::no() => return a.clone(),
            (op::or, [a, b]) if *a == Self::yes() || *b == Self::yes() => return Self::yes(),
            (op::eq, [Self::prim { op: x, args: a }, Self::prim { op: y, args: b }])
                if x.constructor() && y.constructor() =>
            {
                if x != y {
                    return Self::no();
                }
                return Self::all(a.iter().zip(b).map(|(a, b)| Self::eq(a.clone(), b.clone())));
            }
            (
                _,
                [
                    Self::prim {
                        op: c,
                        args: fields,
                    },
                ],
            ) if c.constructor() => {
                if let Some(expected) = p.tested() {
                    return Self::boolean {
                        value: *c == expected,
                    };
                }
                if let Some((expected, index)) = p.selected() {
                    return if *c == expected {
                        fields[index].clone()
                    } else {
                        p.default()
                    };
                }
            }
            _ => {}
        }
        self
    }
}

fn without(body: &expr, guard: &expr, connective: op) -> Option<expr> {
    if body == guard {
        return Some(expr::boolean {
            value: connective == op::and,
        });
    }
    if let expr::prim { op, args } = body {
        if *op == connective {
            if let Some(a) = without(&args[0], guard, connective) {
                return Some(expr::two(connective, a, args[1].clone()));
            }
            if let Some(b) = without(&args[1], guard, connective) {
                return Some(expr::two(connective, args[0].clone(), b));
            }
        }
    }
    None
}

fn freshen(
    vars: &[(String, sort)],
    body: &expr,
    values: &BTreeMap<String, expr>,
) -> (Vec<(String, sort)>, expr) {
    let mut free = BTreeMap::new();
    let mut used = std::collections::BTreeSet::new();
    for v in values.values() {
        v.symbols(&mut free, &mut BTreeMap::new());
    }
    for v in values.values().chain([body]) {
        v.visit(&mut |v| match v {
            expr::var { name, .. } => {
                used.insert(name.clone());
            }
            expr::quant { vars, .. } => {
                used.extend(vars.iter().map(|(n, _)| n.clone()));
            }
            _ => {}
        });
    }
    used.extend(values.keys().cloned());
    let mut bound = vars.to_vec();
    let mut rename = BTreeMap::new();
    for (name, t) in &mut bound {
        if !free.contains_key(name) {
            continue;
        }
        let mut i = 0;
        while used.contains(&format!("$bound{i}")) {
            i += 1;
        }
        let fresh = format!("$bound{i}");
        used.insert(fresh.clone());
        rename.insert(name.clone(), expr::var(&fresh, *t));
        *name = fresh;
    }
    (
        bound,
        if rename.is_empty() {
            body.clone()
        } else {
            body.replace(&rename)
        },
    )
}

fn integer_key(s: &str) -> (bool, &str) {
    let digits = s.strip_prefix('-').unwrap_or(s).trim_start_matches('0');
    (s.starts_with('-') && !digits.is_empty(), digits)
}

impl op {
    pub fn constructor(self) -> bool {
        matches!(
            self,
            Self::int
                | Self::atom
                | Self::nil
                | Self::cons
                | Self::tuple
                | Self::empty
                | Self::push
        )
    }
    pub fn tested(self) -> Option<Self> {
        match self {
            Self::is_int => Some(Self::int),
            Self::is_atom => Some(Self::atom),
            Self::is_nil => Some(Self::nil),
            Self::is_cons => Some(Self::cons),
            Self::is_tuple => Some(Self::tuple),
            Self::is_empty => Some(Self::empty),
            Self::is_push => Some(Self::push),
            _ => None,
        }
    }
    pub fn selected(self) -> Option<(Self, usize)> {
        match self {
            Self::ival => Some((Self::int, 0)),
            Self::aval => Some((Self::atom, 0)),
            Self::head => Some((Self::cons, 0)),
            Self::tail => Some((Self::cons, 1)),
            Self::items => Some((Self::tuple, 0)),
            Self::first => Some((Self::push, 0)),
            Self::rest => Some((Self::push, 1)),
            _ => None,
        }
    }
    pub fn output(self) -> sort {
        match self {
            Self::ival
            | Self::aval
            | Self::add
            | Self::sub
            | Self::mul
            | Self::div
            | Self::len
            | Self::size
            | Self::mass => sort::int,
            Self::items | Self::empty | Self::push | Self::rest => sort::seq,
            Self::eq
            | Self::not
            | Self::and
            | Self::or
            | Self::lt
            | Self::le
            | Self::is_int
            | Self::is_atom
            | Self::is_nil
            | Self::is_cons
            | Self::is_empty
            | Self::is_push
            | Self::is_tuple => sort::bool,
            _ => sort::term,
        }
    }
    pub fn default(self) -> expr {
        if self.output() == sort::int {
            expr::num(0)
        } else if self.output() == sort::seq {
            expr::empty()
        } else {
            expr::nil()
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::int => "int",
            Self::atom => "atom",
            Self::nil => "nil",
            Self::cons => "cons",
            Self::tuple => "tuple",
            Self::empty => "empty",
            Self::push => "push",
            Self::is_empty => "is_empty",
            Self::is_push => "is_push",
            Self::first => "first",
            Self::rest => "rest",
            Self::len => "len",
            Self::nth => "nth",
            Self::size => "size",
            Self::mass => "mass",
            Self::is_int => "is_int",
            Self::is_atom => "is_atom",
            Self::is_nil => "is_nil",
            Self::is_cons => "is_cons",
            Self::is_tuple => "is_tuple",
            Self::ival => "ival",
            Self::aval => "aval",
            Self::head => "head",
            Self::tail => "tail",
            Self::items => "items",
            Self::eq => "eq",
            Self::not => "not",
            Self::and => "and",
            Self::or => "or",
            Self::add => "add",
            Self::sub => "sub",
            Self::mul => "mul",
            Self::div => "div",
            Self::lt => "lt",
            Self::le => "le",
        }
    }
}

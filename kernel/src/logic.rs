use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum sort {
    term,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum op {
    int,
    atom,
    nil,
    cons,
    tuple,
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
            Self::var { sort, .. } | Self::app { sort, .. } => *sort,
            Self::ite { yes, .. } => yes.sort(),
            Self::prim { op, .. } => op.output(),
        }
    }
    pub fn symbols(&self, vars: &mut BTreeMap<String, sort>, funs: &mut BTreeMap<String, usize>) {
        match self {
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

fn integer_key(s: &str) -> (bool, &str) {
    let digits = s.strip_prefix('-').unwrap_or(s).trim_start_matches('0');
    (s.starts_with('-') && !digits.is_empty(), digits)
}

impl op {
    pub fn constructor(self) -> bool {
        matches!(
            self,
            Self::int | Self::atom | Self::nil | Self::cons | Self::tuple
        )
    }
    pub fn tested(self) -> Option<Self> {
        match self {
            Self::is_int => Some(Self::int),
            Self::is_atom => Some(Self::atom),
            Self::is_nil => Some(Self::nil),
            Self::is_cons => Some(Self::cons),
            Self::is_tuple => Some(Self::tuple),
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
            _ => None,
        }
    }
    pub fn output(self) -> sort {
        match self {
            Self::ival | Self::aval | Self::add | Self::sub | Self::mul | Self::div => sort::int,
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
            | Self::is_tuple => sort::bool,
            _ => sort::term,
        }
    }
    pub fn default(self) -> expr {
        if self.output() == sort::int {
            expr::num(0)
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

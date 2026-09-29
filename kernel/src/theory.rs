//! A single datatype description consumed by every backend.
use crate::logic::{op, sort};

pub struct constructor {
    pub op: op,
    pub test: op,
    pub fields: &'static [(op, sort)],
}

pub const terms: &[constructor] = &[
    constructor {
        op: op::int,
        test: op::is_int,
        fields: &[(op::ival, sort::int)],
    },
    constructor {
        op: op::atom,
        test: op::is_atom,
        fields: &[(op::aval, sort::int)],
    },
    constructor {
        op: op::nil,
        test: op::is_nil,
        fields: &[],
    },
    constructor {
        op: op::cons,
        test: op::is_cons,
        fields: &[(op::head, sort::term), (op::tail, sort::term)],
    },
    constructor {
        op: op::tuple,
        test: op::is_tuple,
        fields: &[(op::items, sort::term)],
    },
];

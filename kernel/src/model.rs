//! Solver-independent interface between source lowering and temporal rendering.
use crate::logic::expr;

pub struct export {
    pub args: Vec<String>,
    pub pre: expr,
    pub value: expr,
}

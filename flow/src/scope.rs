//! Reject binding contexts whose sibling scope is outside the source profile.
use anyhow::{Result, bail};
use kernel::ir::{kind, node};

pub fn check(n: &node, statement: bool) -> Result<()> {
    match &n.kind {
        kind::bind { pattern, value } => {
            if !statement {
                bail!(
                    "line {}: bindings must be statements; move the assignment before this expression",
                    n.line
                );
            }
            let _ = pattern;
            check(value, true)?;
        }
        kind::block { items } => {
            for n in items {
                check(n, statement)?;
            }
        }
        kind::op { args, .. } | kind::call { args, .. } => {
            for n in args {
                check(n, false)?;
            }
        }
        kind::tuple { items } | kind::list { items, tail: None } => {
            for n in items {
                check(n, false)?;
            }
        }
        kind::list {
            items,
            tail: Some(tail),
        } => {
            for n in items {
                check(n, false)?;
            }
            check(tail, false)?;
        }
        kind::branch { cond, yes, no } => {
            check(cond, false)?;
            check(yes, true)?;
            check(no, true)?;
        }
        kind::case { value, arms } => {
            check(value, false)?;
            for arm in arms {
                check(&arm.guard, false)?;
                check(&arm.body, true)?;
            }
        }
        kind::ghost { body } | kind::local { body } => check(body, true)?,
        kind::assert { value, .. } | kind::assume { value } => check(value, false)?,
        kind::unfold { call } => check(call, false)?,
        _ => {}
    }
    Ok(())
}

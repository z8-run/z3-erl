#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
//! The shared Erlang profile and solver-independent verification vocabulary.
pub mod bif;
pub mod ir;
pub mod logic;
pub mod model;
pub mod theory;
pub mod vc;

pub const version: &str = "vex.ir.1";
pub const profile: &str = "erlang.discrete.1";
pub const lean: &str = "leanprover/lean4:v4.28.0";

pub fn hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

/// Encode names injectively. Source names never become target-language syntax.
pub fn name(raw: &str) -> String {
    let mut out = String::from("v");
    for b in raw.bytes() {
        use std::fmt::Write;
        write!(out, "{b:02x}").unwrap();
    }
    out
}

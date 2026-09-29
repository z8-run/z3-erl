use crate::{
    ir::span,
    logic::{expr, sort},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct vc {
    pub id: String,
    pub owner: String,
    pub kind: String,
    pub span: span,
    pub hypotheses: Vec<expr>,
    pub goal: expr,
    pub deps: BTreeSet<String>,
}

impl vc {
    pub fn new(
        owner: String,
        kind: &str,
        span: span,
        hypotheses: Vec<expr>,
        goal: expr,
        deps: BTreeSet<String>,
    ) -> Self {
        let bytes =
            serde_json::to_vec(&(&owner, kind, &hypotheses, &goal, &deps, crate::profile)).unwrap();
        Self {
            id: crate::hash(&bytes)[..24].into(),
            owner,
            kind: kind.into(),
            span,
            hypotheses,
            goal,
            deps,
        }
    }
    pub fn symbols(&self) -> (BTreeMap<String, sort>, BTreeMap<String, usize>) {
        let (mut vars, mut funs) = (BTreeMap::new(), BTreeMap::new());
        for e in self.hypotheses.iter().chain([&self.goal]) {
            e.symbols(&mut vars, &mut funs);
        }
        (vars, funs)
    }
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct source {
    pub version: String,
    pub files: Vec<file>,
    pub functions: Vec<fun>,
    pub skipped: Vec<skip>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct file {
    pub path: String,
    pub hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct skip {
    pub owner: String,
    pub span: span,
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct span {
    pub file: String,
    pub line: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct fun {
    pub module: String,
    pub name: String,
    pub args: Vec<String>,
    pub mode: mode,
    pub requires: Vec<node>,
    pub ensures: Vec<node>,
    pub guard: node,
    pub decreases: Option<node>,
    pub body: node,
    pub span: span,
}

impl fun {
    pub fn id(&self) -> String {
        format!("{}.{}/{}", self.module, self.name, self.args.len())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum mode {
    public,
    private,
    ghost,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct node {
    #[serde(default)]
    pub line: u32,
    #[serde(flatten)]
    pub kind: kind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum kind {
    int {
        value: String,
    },
    atom {
        value: String,
    },
    var {
        name: String,
    },
    op {
        name: String,
        args: Vec<node>,
    },
    call {
        module: Option<String>,
        name: String,
        args: Vec<node>,
    },
    branch {
        cond: Box<node>,
        yes: Box<node>,
        no: Box<node>,
    },
    case {
        value: Box<node>,
        arms: Vec<arm>,
    },
    block {
        items: Vec<node>,
    },
    bind {
        pattern: Box<node>,
        value: Box<node>,
    },
    tuple {
        items: Vec<node>,
    },
    list {
        items: Vec<node>,
        tail: Option<Box<node>>,
    },
    assert {
        value: Box<node>,
    },
    ghost {
        body: Box<node>,
    },
    unfold {
        call: Box<node>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct arm {
    pub pattern: node,
    pub guard: node,
    pub body: node,
}

impl node {
    pub fn var(name: &str) -> Self {
        Self {
            line: 0,
            kind: kind::var { name: name.into() },
        }
    }
    pub fn atom(value: &str) -> Self {
        Self {
            line: 0,
            kind: kind::atom {
                value: value.into(),
            },
        }
    }
}

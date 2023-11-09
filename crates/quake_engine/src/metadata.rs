use std::collections::BTreeMap;

use nu_protocol::{BlockId, Spanned, Value, VarId};
use serde::Serialize;

use quake_core::prelude::*;

#[derive(Clone, Debug, Default, Serialize)]
pub struct BuildMetadata {
    pub tasks: BTreeMap<String, Task>,
}

impl BuildMetadata {
    pub fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        // TODO check that the dependency graph makes sense
        // TODO validate that all tasks have both sources and artifacts, or none at all
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Task {
    pub name: Spanned<String>,
    pub dependencies: Vec<Dependency>,
    pub sources: Vec<Spanned<String>>,
    pub artifacts: Vec<Spanned<String>>,
    pub run_block: Option<BlockId>,
}

impl Task {
    pub fn new(name: Spanned<String>, run_block: Option<BlockId>) -> Self {
        Task {
            name,
            dependencies: Vec::new(),
            sources: Vec::new(),
            artifacts: Vec::new(),
            run_block,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub enum Dependency {
    Named(Spanned<String>),
    Anonymous {
        block_id: BlockId,
        argument: Option<(VarId, Value)>,
    },
}

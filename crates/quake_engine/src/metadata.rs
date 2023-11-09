use std::collections::BTreeMap;

use nu_protocol::{BlockId, Spanned};
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
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Task {
    pub name: Spanned<String>,
    pub depends: Vec<Spanned<String>>,
    pub sources: Vec<Spanned<String>>,
    pub produces: Vec<Spanned<String>>,
    pub run_block: BlockId,
}

impl Task {
    pub fn new(name: Spanned<String>, run_block: BlockId) -> Self {
        Task {
            name,
            depends: Vec::new(),
            sources: Vec::new(),
            produces: Vec::new(),
            run_block,
        }
    }
}

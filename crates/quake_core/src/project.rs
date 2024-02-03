use std::path::PathBuf;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::prelude::*;
use crate::BUILD_SCRIPT_NAMES;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Project {
    pub project_root: PathBuf,
    pub build_script: PathBuf,
}

impl Project {
    pub fn new(project_root: PathBuf) -> Result<Self> {
        assert!(project_root.is_dir(), "project root not a directory");

        let build_script = BUILD_SCRIPT_NAMES
            .iter()
            .map(|n| project_root.join(n))
            .find(|p| p.exists())
            .ok_or(errors::BuildScriptNotFound)?;

        Ok(Self {
            project_root,
            build_script,
        })
    }
}

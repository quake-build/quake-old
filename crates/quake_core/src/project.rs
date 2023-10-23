use std::path::{Path, PathBuf};

use crate::prelude::*;

#[derive(Clone)]
pub struct Project {
    project_root: PathBuf,
    build_script: PathBuf,
}

impl Project {
    pub fn new(project_root: PathBuf) -> Result<Self> {
        if !project_root.is_dir() {
            return Err(errors::ProjectNotFound.into());
        }

        let build_script = project_root.join("build.quake");

        if !build_script.is_file() {
            return Err(errors::BuildScriptNotFound.into());
        }

        Ok(Self {
            project_root,
            build_script,
        })
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn build_script(&self) -> &Path {
        &self.build_script
    }
}

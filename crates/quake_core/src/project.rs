use std::path::{Path, PathBuf};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::prelude::*;
use crate::BUILD_SCRIPT_NAMES;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Project {
    project_root: PathBuf,
    build_script: PathBuf,
}

impl Project {
    /// Try to open a project at a given project root.
    ///
    /// ## Errors
    ///
    /// If the project root is not a directory, this will return an `Err` with
    /// [`errors::ProjectNotFound`].
    /// If no build script file is found in the
    /// project root, this will return an `Err` with
    /// [`errors::BuildScriptNotFound`].
    pub fn new(project_root: PathBuf) -> Result<Self> {
        if !project_root.is_dir() {
            return Err(errors::ProjectNotFound.into());
        }

        let build_script = find_build_script(&project_root).ok_or(errors::BuildScriptNotFound)?;

        Ok(Self {
            project_root,
            build_script,
        })
    }

    /// Locate a project starting from the current directory and traversing
    /// upwards until a build script is found.
    ///
    /// ## Errors
    ///
    /// If the project root is not a directory or no project can be found, this
    /// will return an `Err` with [`errors::ProjectNotFound`].
    pub fn locate(current_dir: impl AsRef<Path>) -> Result<Self> {
        if !current_dir.as_ref().is_dir() {
            return Err(errors::ProjectNotFound.into());
        }

        if let Some(build_script) = find_build_script(&current_dir) {
            Ok(Self {
                project_root: current_dir.as_ref().to_owned(),
                build_script,
            })
        } else if let Some(parent) = current_dir.as_ref().parent() {
            Self::locate(parent)
        } else {
            Err(errors::ProjectNotFound.into())
        }
    }

    pub fn project_root(&self) -> &PathBuf {
        &self.project_root
    }

    pub fn build_script(&self) -> &PathBuf {
        &self.build_script
    }
}

#[inline(always)]
fn find_build_script(dir: impl AsRef<Path>) -> Option<PathBuf> {
    BUILD_SCRIPT_NAMES
        .iter()
        .map(|name| dir.as_ref().join(name))
        .find(|p| p.is_file())
}

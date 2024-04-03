use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use nu_protocol::engine::PWD_ENV;

use crate::metadata::TaskCallMetadata;
use crate::prelude::*;

pub fn get_init_cwd() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .or_else(|| std::env::var(PWD_ENV).ok().map(Into::into))
}

pub fn latest_timestamp(paths: &[impl AsRef<Path>]) -> DiagResult<Option<SystemTime>> {
    Ok(paths
        .iter()
        .filter(|p| p.as_ref().exists())
        .map(|s| fs::metadata(s).and_then(|m| m.modified()).into_diagnostic())
        .collect::<DiagResult<Vec<_>>>()?
        .into_iter()
        .max())
}

pub fn is_dirty(task: &TaskCallMetadata) -> DiagResult<bool> {
    // if either is undefined, assume dirty
    if task.sources.is_empty() || task.artifacts.is_empty() {
        return Ok(true);
    }

    Ok(latest_timestamp(&task.sources)? > latest_timestamp(&task.artifacts)?)
}

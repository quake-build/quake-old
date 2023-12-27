use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use glob::glob;
use miette::IntoDiagnostic;
use nu_protocol::engine::{Stack, PWD_ENV};
use nu_protocol::{Span, Spanned, Value};

use quake_core::prelude::*;

pub fn set_last_exit_code(stack: &mut Stack, exit_code: i64) {
    stack.add_env_var(
        "LAST_EXIT_CODE".to_string(),
        Value::int(exit_code, Span::unknown()),
    );
}

pub fn get_init_cwd() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .or_else(|| std::env::var(PWD_ENV).ok().map(Into::into))
}

pub fn expand_globs(patterns: &[Spanned<String>]) -> Result<Vec<PathBuf>> {
    let mut paths = vec![];

    for ps in patterns
        .iter()
        .map(|s| glob(&s.item).into_diagnostic())
        .collect::<Result<Vec<_>>>()?
    {
        paths.extend(
            ps.into_iter()
                .map(IntoDiagnostic::into_diagnostic)
                .collect::<Result<Vec<_>>>()?,
        );
    }

    Ok(paths)
}

pub fn latest_timestamp(paths: &[PathBuf]) -> Result<Option<SystemTime>> {
    Ok(paths
        .iter()
        .map(Path::new)
        .filter(|p| p.exists())
        .map(|s| fs::metadata(s).and_then(|m| m.modified()).into_diagnostic())
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .max())
}

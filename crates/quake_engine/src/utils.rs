use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use miette::IntoDiagnostic;
use nu_ansi_term::{Color, Style};
use nu_protocol::engine::{Stack, PWD_ENV};
use nu_protocol::{Span, Value};

use quake_core::prelude::*;

use crate::metadata::TaskCallMetadata;

pub fn set_last_exit_code(stack: &mut Stack, exit_code: i64) {
    stack.add_env_var(
        "LAST_EXIT_CODE".to_owned(),
        Value::int(exit_code, Span::unknown()),
    );
}

pub fn get_init_cwd() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .or_else(|| std::env::var(PWD_ENV).ok().map(Into::into))
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

pub fn is_dirty(task: &TaskCallMetadata) -> Result<bool> {
    // if either is undefined, assume dirty
    if task.sources.is_empty() || task.artifacts.is_empty() {
        return Ok(true);
    }

    // TODO glob from PWD?

    Ok(latest_timestamp(&task.sources)? > latest_timestamp(&task.artifacts)?)
}

pub fn print_info(prefix: &str, message: &str) {
    print_message(prefix, message, Color::White);
}

pub fn print_error(prefix: &str, message: &str) {
    print_message(prefix, message, Color::LightRed);
}

#[inline]
fn print_message(prefix: &str, message: &str, color: Color) {
    eprintln!(
        "{} {}: {message}",
        Style::new().fg(Color::DarkGray).paint(">"),
        Style::new().fg(color).bold().paint(prefix),
    );
}

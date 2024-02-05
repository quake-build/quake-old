use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use nu_ansi_term::{Color, Style};
use nu_protocol::engine::PWD_ENV;

use crate::metadata::TaskCallMetadata;
use crate::prelude::*;

pub fn get_init_cwd() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .or_else(|| std::env::var(PWD_ENV).ok().map(Into::into))
}

pub fn latest_timestamp(paths: &[impl AsRef<Path>]) -> Result<Option<SystemTime>> {
    Ok(paths
        .iter()
        .filter(|p| p.as_ref().exists())
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

    Ok(latest_timestamp(&task.sources)? > latest_timestamp(&task.artifacts)?)
}

/// Print a styled info-level prefixed message.
pub fn print_info(prefix: &str, message: &str) {
    print_message(prefix, message, Color::White);
}

/// Print a styled warning-level prefixed message.
pub fn print_warning(prefix: &str, message: &str) {
    print_message(prefix, message, Color::Yellow);
}

/// Print a styled error-level prefixed message.
pub fn print_error(prefix: &str, message: &str) {
    print_message(prefix, message, Color::LightRed);
}

#[inline(always)]
fn print_message(prefix: &str, message: &str, color: Color) {
    eprintln!(
        "{} {}: {message}",
        Style::new().fg(Color::DarkGray).paint(">"),
        Style::new().fg(color).bold().paint(prefix),
    );
}

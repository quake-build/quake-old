#![feature(iter_intersperse)]

use std::env;
use std::path::PathBuf;

use clap::builder::PathBufValueParser;
use clap::ArgMatches;
use serde_json::to_string as to_json;

use quake_core::prelude::*;
use quake_core::utils::get_init_cwd;
use quake_engine::{Engine, EngineOptions};

fn parse_args() -> ArgMatches {
    use clap::*;
    Command::new("quake")
        .about("quake: a meta-build system powered by nushell")
        .version(crate_version!())
        .color(ColorChoice::Never)
        .max_term_width(100)
        .override_usage(
            "quake [OPTIONS] <TASK> [--] [TASK_ARGS]\n       \
             quake [OPTIONS]",
        )
        .arg_required_else_help(true)
        .disable_help_subcommand(true)
        .args_conflicts_with_subcommands(true)
        .subcommand_negates_reqs(true)
        .subcommand_help_heading("Subcommands")
        .subcommands([
            Command::new("list").about("List the available tasks"),
            Command::new("inspect").about("Dump build script metadata as JSON"),
        ])
        .next_help_heading("Environment")
        .args([Arg::new("project")
            .long("project")
            .value_name("PROJECT_DIR")
            .value_parser(PathBufValueParser::new())
            .value_hint(ValueHint::DirPath)
            .help("Path to the project root directory")
            .global(true)])
        .next_help_heading("Output handling")
        .args([
            Arg::new("quiet")
                .long("quiet")
                .action(ArgAction::SetTrue)
                .help("Suppress the output (stdout and stderr) of any executed commands"),
            Arg::new("json")
                .long("json")
                .action(ArgAction::SetTrue)
                .help(
                    "Output events as a line-delimited JSON objects to stderr. See the JSON \
                    appendix in the manual for the specification of these objects.",
                )
                .global(true),
        ])
        .next_help_heading("Evaluation modes")
        .args([
            Arg::new("force")
                .long("force")
                .action(ArgAction::SetTrue)
                .help("Execute tasks regardless of initial dirtiness checks"),
            Arg::new("watch")
                .long("watch")
                .action(ArgAction::SetTrue)
                .help("Run the task, and re-run whenever sources have changed"),
        ])
        .args([
            Arg::new("task").value_name("TASK").hide(true),
            Arg::new("task-args")
                .value_name("TASK_ARGS")
                .trailing_var_arg(true)
                .allow_hyphen_values(true)
                .num_args(0..)
                .hide(true),
        ])
        .get_matches()
}

fn main() -> CliResult {
    let matches = parse_args();

    let project = {
        if let Some(project_root) = matches.get_one::<PathBuf>("project") {
            Project::new(project_root.clone())?
        } else {
            Project::locate(
                get_init_cwd()
                    .ok_or_else(|| error!("Failed to determine current working directory"))?,
            )?
        }
    };

    let json = matches.get_flag("json");

    let options = EngineOptions {
        quiet: matches.get_flag("quiet"),
        json,
        force: matches.get_flag("force"),
        watch: matches.get_flag("watch"),
    };

    let mut engine = Engine::load(project, options)?;

    match matches.subcommand() {
        None => {
            let task = matches.get_one::<String>("task").unwrap();
            let args: String = matches
                .get_many::<String>("task-args")
                .map(|args| {
                    args.filter(|s| *s != "--")
                        .cloned()
                        .intersperse(String::from(" "))
                        .collect()
                })
                .unwrap_or_default();
            engine.run(task, &args)?;
        }
        Some(("list", _)) => {
            let metadata = engine.metadata();
            let tasks: Vec<_> = metadata.task().map(|t| &t.name.item).collect();

            if json {
                println!("{}", to_json(&tasks).unwrap());
            } else if tasks.is_empty() {
                println!("No available tasks.");
            } else {
                println!("Available tasks:");
                for task in tasks {
                    println!("- {task}");
                }
            }
        }
        Some(("inspect", _)) => {
            println!("{}", to_json(&engine.metadata().clone()).unwrap());
        }
        Some((name, _)) => {
            unimplemented!("subcommand {name}")
        }
    }

    CliResult::success()
}

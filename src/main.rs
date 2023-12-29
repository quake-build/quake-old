#![feature(exact_size_is_empty)]

use std::path::PathBuf;

use quake_core::prelude::*;
use quake_engine::*;

/// Parse a config property consisting of a single key-value pair in the form
/// `PROPERTY=VALUE`.
fn parse_config_property(s: &str) -> Result<(String, String)> {
    let pos = s
        .find('=')
        .ok_or_else(|| miette!("Invalid config property `{s}`"))?;
    Ok((s[..pos].to_owned(), s[pos + 1..].to_owned()))
}

fn main() -> Result<()> {
    let online_docs_url = option_env!("QUAKE_ONLINE_DOCS").unwrap_or("https://docs.quake.build/");
    let _offline_docs_url = option_env!("QUAKE_OFFLINE_DOCS");

    let matches = {
        use clap::*;

        Command::new("quake")
            .about("quake: a meta-build system built on nushell")
            .version(crate_version!())
            .color(ColorChoice::Never)
            .max_term_width(100)
            .override_usage(
                "quake [OPTIONS] <TASK>\n       \
                 quake [OPTIONS] <SUBCOMMAND>",
            )
            .arg_required_else_help(true)
            .disable_help_subcommand(true)
            .args_conflicts_with_subcommands(true)
            .subcommand_negates_reqs(true)
            .arg(
                Arg::new("task")
                    .value_name("TASK")
                    .required_unless_present("dry-run")
                    .hide(true),
            )
            .subcommand_help_heading("Subcommands")
            .subcommands([
                Command::new("list").about("List the available tasks"),
                Command::new("docs")
                    .about("Open the quake documentation in a web browser")
                    .arg(
                        Arg::new("query")
                            .short('s')
                            .long("search")
                            .value_name("QUERY")
                            .help("Search the manual upon opening"),
                    )
                    .arg(
                        Arg::new("online")
                            .short('O')
                            .long("online")
                            .action(ArgAction::SetTrue)
                            .help(format!(
                                "Open the latest documentation online on {online_docs_url}, which \
                                 may or may not be current with this version of quake."
                            )),
                    ),
                Command::new("inspect").about("Dump build script metadata as JSON"),
            ])
            .next_help_heading("Environment")
            .args([Arg::new("project")
                .short('p')
                .long("project")
                .value_name("PROJECT_DIR")
                .value_hint(ValueHint::DirPath)
                .help("Path to the project root directory")
                .global(true)])
            .next_help_heading("Configuration")
            .args([
                Arg::new("config")
                    .short('c')
                    .long("config")
                    .value_name("FILE")
                    .help("Load from a configuration file")
                    .action(ArgAction::Append)
                    .global(true),
                Arg::new("config-prop")
                    .short('C')
                    .value_name("PROPERTY=VALUE")
                    .value_parser(parse_config_property)
                    .action(ArgAction::Append)
                    .help("Set a configuration property")
                    .global(true),
            ])
            .next_help_heading("Output handling")
            .args([
                Arg::new("quiet")
                    .short('q')
                    .long("quiet")
                    .action(ArgAction::SetTrue)
                    .help("Suppress the output (stdout and stderr) of any executed commands"),
                Arg::new("stdout")
                    .long("stdout")
                    .value_name("FILE")
                    .help("Redirect stdout of executed commands to a file"),
                Arg::new("stderr")
                    .long("stderr")
                    .value_name("FILE")
                    .help("Redirect stderr of executed commands to a file"),
            ])
            .arg(
                Arg::new("json")
                    .long("json")
                    .action(ArgAction::SetTrue)
                    .help(
                        "Output results as a JSON object to stdout, suppressing the output of any \
                         executed commands. See the JSON appendix in the manual for the \
                         specification of these objects.",
                    )
                    .global(true),
            )
            .next_help_heading("Special modes")
            .args([
                Arg::new("dry-run")
                    .short('D')
                    .long("dry-run")
                    .action(ArgAction::SetTrue)
                    .group("operation")
                    .help("Do not execute any tasks (useful for validating build script)"),
                Arg::new("force")
                    .short('F')
                    .long("force")
                    .action(ArgAction::SetTrue)
                    .help("Execute tasks regardless of initial dirtiness checks"),
                Arg::new("watch")
                    .short('W')
                    .long("watch")
                    .action(ArgAction::SetTrue)
                    .help("Retrigger tasks when sources have changed"),
            ])
            .get_matches()
    };

    let project = {
        let project_root = matches
            .get_one::<String>("project")
            .map(PathBuf::from)
            .or_else(get_init_cwd)
            .ok_or(errors::ProjectNotFound)?;
        Project::new(project_root)?
    };

    let options = {
        let quiet = matches.get_flag("quiet");
        Options { quiet }
    };

    let mut engine = Engine::new(project, options)?;

    match matches.subcommand() {
        None => {
            if !matches.get_flag("dry-run") {
                let task = matches.get_one::<String>("task").unwrap().clone();
                engine.run(&task)?;
            }
        }
        Some(("list", _)) => {
            let metadata = engine.metadata();
            let tasks: Vec<_> = metadata
                .global_tasks()
                .filter_map(|t| t.name.as_ref().map(|s| &s.item))
                .collect();

            if tasks.is_empty() {
                println!("No available tasks.");
            } else {
                println!("Available tasks:");
                for task in tasks {
                    println!("- {task}");
                }
            }
        }
        Some(("inspect", _)) => {
            let metadata = engine.metadata().clone();
            println!("{}", serde_json::to_string(&metadata).into_diagnostic()?);
        }
        Some(_) => unimplemented!(),
    }

    Ok(())
}

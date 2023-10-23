use anyhow::anyhow;

use quake_core::prelude::*;
use quake_engine::*;

/// Parse a config property consisting of a single key-value pair in the form
/// `PROPERTY=VALUE`.
fn parse_config_property(s: &str) -> anyhow::Result<(String, String)> {
    let pos = s
        .find('=')
        .ok_or_else(|| anyhow!("invalid config property `{s}`"))?;
    Ok((s[..pos].to_owned(), s[pos + 1..].to_owned()))
}

fn main() -> Result<()> {
    let online_docs_url = option_env!("QUAKE_ONLINE_DOCS").unwrap_or("https://docs.quake.build/");
    let _offline_docs_url = option_env!("QUAKE_OFFLINE_DOCS");

    let _matches = {
        use clap::*;

        Command::new("quake")
            .about("quake: a meta-build system built on nushell")
            .version(crate_version!())
            .color(ColorChoice::Never)
            .max_term_width(100)
            .override_usage(
                "quake [OPTIONS] <TASK>...\n       \
                 quake [OPTIONS] <SUBCOMMAND>",
            )
            .arg_required_else_help(true)
            .disable_help_subcommand(true)
            .args_conflicts_with_subcommands(true)
            .subcommand_negates_reqs(true)
            .arg(
                Arg::new("task")
                    .value_name("TASK")
                    .action(ArgAction::Append)
                    .required(true)
                    .num_args(1..)
                    .help("The tasks to run, in the form [SUBPROJECT]:<TASK>"),
            )
            .subcommand_help_heading("Subcommands")
            .subcommands([
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
                Command::new("validate").about("Validate all quake scripts in the current project"),
                Command::new("inspect")
                    .about("Retrieve information about the current project")
                    .subcommand_required(true)
                    .subcommands([
                        Command::new("config")
                            .about("Get configuration information")
                            .arg(
                                Arg::new("toolchain")
                                    .value_name("TOOLCHAIN")
                                    .help("A specific toolchain to inspect"),
                            ),
                        Command::new("tasks")
                            .about("Get task information")
                            .alias("task")
                            .arg(
                                Arg::new("task").value_name("TASK").help(
                                    "A specific task to inspect, in the form [SUBPROJECT]:TASK",
                                ),
                            ),
                        Command::new("toolchains")
                            .about("Get toolchain information")
                            .alias("toolchain")
                            .arg(
                                Arg::new("toolchain")
                                    .value_name("TOOLCHAIN")
                                    .help("A specific toolchain to inspect"),
                            ),
                    ]),
            ])
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
                    .help("Do not execute any tasks (useful for checking toolchains)"),
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

    // Ignore all else and just do something fun.
    let source = b"ls | length";

    let mut engine_state = create_engine_state();
    let mut stack = create_stack();
    let input = create_stdin_input();

    eval_source(
        &mut engine_state,
        &mut stack,
        source,
        "application",
        input,
        true,
    );

    Ok(())
}

# Initializing a project

quake projects are defined by a `build.quake` or `build.quake.nu` file in the project's root directory.
This file is just a nushell script that's evaluated with a few extra commands added by quake.

You can verify that your project is configured correctly with `quake --dry-run`, or `quake list` (lists all tasks), and then run any of the defined tasks with `quake <task>`.

## Quick start

The easiest way to get up and running is just to add a simple task to your build script:

```sh
def-task build [] {
    # shell commands for running your task go here
}
```

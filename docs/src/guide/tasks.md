# Tasks

Tasks are the basic building block of quake scripts, and are defined with the `def-task` command.
Tasks consist of a name, a signature, and two independently optional bodies (a declaration body and/or a run body).

The declaration body is used to declare metadata about the task (e.g. what other tasks it depends upon), while the run body contains the actual commands that make up the task.
Both bodies share the same signature and have the same arguments passed into them.

## Basic examples

Simple task:

```sh
def-task build [] {
    cargo build
}
```

Task with a dependency:

```sh
def-task build [] {
    cargo build --release
}

def-task bundle [] {
    depends build
} {
    cp target/release/foobar out/
}
```

Task with an argument:

```sh
def-task build [package: string] {
    cargo build --package $package
}

# "pure" task with no run block
def-task --pure build-my-packages [] {
    depends build my-package
    depends build my-other-package
}
```

## `def-task` usage

```
Usage:
  > def-task <name> <params> <run_body>
  > def-task <name> <params> <decl_body> <run_body>
  > def-task --pure <name> <params> <decl_body>

Parameters:
  name      <string>:    task name
  params    <signature>: task body parameters
  decl_body <closure()>: task declaration body
  run_body  <closure()>: task run body
```

## Task evaluation

When running a task, quake's evaluation model looks like this:

1. Evaluate the build script to gather definitions
2. Evaluate the declaration body of the given task (if it exists)
3. Recursively evaluate the declaration bodies of any and all of the task's dependencies
4. Evaluate run bodies in depth-first order until all have completed

This splitting of tasks into two phases allows the engine to reason about programatically-declared task metadata without actually having to run the tasks.
This enables a number of useful features which we will cover throughout this guide.

### Evaluating declaration bodies

Declaration bodies are evaluated inside of a special scope which enables the following commands:

- `depends <task> [args...]`: depend on another task with optional arguments
- `sources <files>`: declare files that this task consumes
- `produces <files>`: declare files that this task produces

This scope is carried through function calls.

## Sources and artifacts

As previously mentioned, tasks may declare the files that they consume (called *sources*) and the files they produce (called *artifacts*) in their declaration bodies, using the `sources` and `produces` commands respectively.

When both sources and artifacts are defined, quake will use that information to determine the "dirtiness" state of a task by comparing the most recent modification time in each set of files, and skip any tasks which are not dirty.

<div class="warning">
Be careful when declaring directories as sources and artifacts, as on most file systems the last modification time of directories is not updated when contents of files in that directory are updated.

When in doubt, use a glob pattern for the files you actually want to watch, preferably in addition to any directories in which those files are located as well, so new files (which do update the last modification time of the directory) are discovered.
</div>

### Basic example

For example:

```
def-task render-docs [] {
    sources in.md
    produces out.html
} {
    pandoc --quiet -s -o out.html in.md
}
```

After we run this the first time (with `quake render-docs`), subsequent runs will result in the task being skipped, usually until either the source file is updated or the artifact is removed.

### Globbing

**TODO**

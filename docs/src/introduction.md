# What is quake?

quake is a cross-platform build system designed to make architecting large multilingual projects simpler.
quake picks up where other build systems leave off, as an end-to-end build system with modern design considerations that benefit everyone.

## Features

- Declarative build scripts, written in [nushell](https://nushell.sh)[^1]
- Composable and highly extensible task model
- Hot reloading support via file watchers with partial rebuilds
- Concurrent tasks; can be used for running a client/server simultaneously, in addition to any performance improvements
- Exposed JSON metadata modes, to enable third-party tooling integration
- Lazy evaluation of tasks and their declarations

[^1]: nushell is a powerful, strongly-typed cross-platform shell written in Rust. Its syntax is intuitive enough that learning it is *not* a prerequisite for understanding the scripts used in this book, but it's certainly worth learning as you go.

## A quick example

```sh
# read our Cargo.toml to (very naively) guess the binary name
let binary_name = open Cargo.toml | get package.name

def-task build [] {
    # declare what files this task consumes
    sources (glob src/**/*.rs)
    sources [Cargo.toml, Cargo.lock]

    # declare what files this task produces
    produces target/release/$binary_name
} {
    cargo build --release
}

def-task bundle [] {
    depends build

    # the same as before, but at a directory-level
    sources static/
    produces out/
} {
    rm -rf out/
    cp -r static out/
    cp $task.dependencies.build.artifacts out/
}
```

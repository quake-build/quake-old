# quake

quake is a meta-build system powered by [nushell](https://www.nushell.sh/).

Although it is currently a work-in-progress, here is a list of features we hope to eventually support (if not already):

- Flexible and composable task system
- Community-contributable library of toolchains
- Dirty checking based on heuristics like file modification timestamps
- Hot reloading support via file watchers and partial rebuilds
- Interactive shell mode to debug build scripts
- Exposed build script metadata via JSON to enable third-party tooling integration
- Builds inside of hermetic filesystems to allow for "proofs", caching, etc.

## Demo

quake projects are defined by a `build.quake` file in the root of the project, which is evaluated by quake's custom nushell engine when it is run.

The central building block of a quake file is a *task*, which contains a run body (not immediately evaluated by the engine), and an optional declaration block, which is used to declare information like task dependencies.

### Simple

Here's a very a simple quake script which just aliases a few cargo commands:

``` nu
def-task build {
    cargo build
}

def-task clean {
    cargo clean
}
```

With this in place, we could then run `quake build` or `quake clean` to run either task body.

### Bundle

A more complicated example might look like this:

```nu
def-task compile {
    cargo build
}

def-task render-docs {
    # declaration block
    sources ["in.md"]
    produces ["out.html"]
} {
    # task body
    pandoc --quiet -s -o out.html in.md
}

# the -d flag is used here for convenience to indicate that the task is purely
# declarative (i.e. has only a declaration block, and no task body)
def-task -d bundle {
    depends compile
    depends render-docs
}

def-task clean {
    cargo clean
    rm -f out.html
}
```

When we run `quake bundle`, the tasks `compile` and `render-docs` will be run in order.
Since we defined what artifacts the `render-docs` task sources and produces, it will compare the modification timestamps between those two sets of files to determine if the task needs to be run.
We could do the same thing for the `compile` task as well, but in this case we'll just rely on cargo's internal mechanism for doing so.

### Configuration

Adding configuration is easy with nushell's native data manipulation toolset.
In this example, we read a boolean from the `config.toml` at the project root to determine whether or not we are performing a release build.

``` nu
let is_release = open config.toml | get release

def-task build {
    if $is_release {
        cargo build --release
    } else {
        cargo build
    }
}
```

### Custom functions, subtasks

With larger projects, it can be really useful to write common functions to generate tasks automatically.
We can do this two separate ways.

First, by invoking `def-task` inside of a function:

``` nu
def-task check-rust-install {
    # ensure that a sufficient rust toolchain is installed
    # ...
}

def rust-package [package: string] {
    let package_name = $package | str replace -a "_" "-"

    def-task "build-" + $package_name {
        depends check-rust-install
    } {
        cargo build $package
    }
}

rust-package my-package
```

Or second, by delegating the declaration work to a function:

``` nu
def-task check-rust-install {
    # ensure that a sufficient rust toolchain is installed
    # ...
}

def rust-package [package: string] {
    depends check-rust-install

    # define and depend upon an anonymous task that actually builds the package
    $package | subtask {|p|
        cargo build --package $p
    }
}

def-task -d build-my-package {
    rust-package my-package
}
```

In the second example, when the `rust-package` function is invoked inside the `build` task's declaration body, its "task scope" is carried with it, so we can use commands like `depends`, `sources`, `produces`, and the new `subtask` command shown above.

## Motivation

Many software projects have build system requirements that are simply not achievable in the native build system of the language or framework they are working in, particularly when it comes to requirements like:

- Generating assets at build-time (e.g. rasterizations of an SVG app icon)
- Bundling and signing an application for one or more targets (e.g. macOS Universal Binaries)
- Composing multiple build systems together (e.g. in a multilingual project)

Typically, these requirements are fulfilled by the some combination of the following kinds of solutions:

- Rules/targets-based build systems
    - Examples: make, ninja
    - Pros:
        - Very fast
        - Elegant and mathematical
    - Cons:
        - Not particularly configurable past a certain point, leading to the use of build generators
- Build generators
    - Examples: cmake, meson, autotools
    - Pros:
        - Ubiquitous, most developers already have installed
        - Can run "once" to generate faster rules-based scripts
    - Cons:
        - Tailored heavily towards C/C++ projects, support otherwise is mixed
- All-in-one systems
    - Examples: bazel, buck2, pants
    - Pros:
        - Work well in internal monorepos where all projects use the same build system, regardless of the language(s) they're written in
        - Support features like hermetic builds and remote executions by knowing explicitly the details of every action (in particular which files are being operated on and produced)
    - Cons:
        - Lead to a lot of boilerplate or a lot of magic
        - Generally do not play nice with external package managers (e.g. cargo)
            - See [reindeer](https://github.com/facebookincubator/reindeer), a solution for buck2 which downloads crates.io crates and converts them into buck2 dependencies
        - Require a lot of internal knowledge to use, limiting potential users/contributors
- Language-specific build systems adapted for other purposes
    - Examples: npm, yarn, cargo (via `build.rs`, or the ["xtask" pattern]((https://github.com/matklad/cargo-xtask)))
    - Pros:
        - Most developers already have and are familiar with the toolchain for the language they're working in
        - Can distribute to package repositories as normal
    - Cons:
        - No consistent standard for configuration, ensuring system dependencies are available, etc., thus requiring a lot of annoying boilerplate
        - Poor support for other languages, build system features and paradigms
- Shell scripts
    - Pros:
        - Easy to get up and running
        - Wide range of tools for dealing with system dependencies
    - Cons:
        - Scale poorly
        - Rarely truly cross-platform, even when using a common shell like bash
        - Poor fit for a scripting language
        - Lack useful tools, leading to build time dependencies like jq
        
quake attempts to bridge these solutions by being technology-agnostic, cross-platform, and very flexible and composable.

## License

quake is licensed under the [MIT license](./LICENSE).

# Installation

Requirements:
- rustup (to install a nightly[^1] Rust toolchain)
- git (*optional*, can alternatively download sources via HTTP)
- [pandoc](https://pandoc.org) (*optional*, used to generate manpages)

As quake is in early development, the current recommended installation path is to simply:

```sh
# install the latest rust nightly toolchain
rustup install nightly

# clone the repository (subsequent updates should use `git pull`)
git clone https://github.com/quake-build/quake && cd quake

# choose where to output quake's binary and manpages
#
# this example requires ~/.local/bin to be in your PATH
INSTALL_DIR="$HOME/.local"

# bootstrap quake to build and install itself in a single step
#
# uncomment `--no-man` to skip manpages
cargo run --release -- install $INSTALL_DIR # --no-man 

# clean up build artifacts (optional)
cargo clean
```

This installs the following:
- The binary `quake`
- man pages (see `man 1 quake`, `man 5 build.quake`)

You can verify your installation by running `quake --version`.

[^1]: quake will eventually transition into using the stable toolchain, but as it is considered under very much "alpha" software, it's convenient to depend on nightly for the time being.

## System-wide installation

Similar to to the above, except that we build as the user and install as root.

```sh
# ...

INSTALL_DIR=/usr/local # or /usr, etc.

# build as the current user
cargo run --release -- bundle-dist # --no-man

# install as root
sudo ./target/release/quake install-dist --dir $INSTALL_DIR # --no-man
```

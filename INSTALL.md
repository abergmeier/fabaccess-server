# Installation

Currently there are no distribution packages available.
However installation is reasonably straight-forward, since Diflouroborane compiles into a single
mostly static binary with few dependencies.

At the moment only Linux is supported. If you managed to compile Diflouroborane please open an issue
outlining your steps or add a merge request expanding this part. Thanks!

## Requirements

General requirements; scroll down for distribution-specific instructions

- GNU SASL (libgsasl).
  * If you want to compile Diflouroborane from source you will potentially also need development
      headers
- capnproto
- rustc stable / nightly >= 1.48
  * If your distribution does not provide a recent enough rustc, [rustup](https://rustup.rs/) helps
      installing a local toolchain and keeping it up to date.

###### Arch Linux:
```shell
$ pacman -S gsasl rust capnproto
```

## Compiling from source

Diflouroborane uses Cargo, so compilation boils down to:

```shell
$ cargo build --release
```
https://www.geeksforgeeks.org/how-to-install-rust-on-raspberry-pi/ can show you how to install rust on your Linux computer.

The compiled binary can then be found in `./target/release/bffhd`

### Cross-compiling

If you need to compile for a different CPU target than your own (e.g. you want
to use BFFH on a raspberry pi but compile on your desktop PC), you need to
setup a cross-compilation toolchain and configure your Cargo correctly.
[The `CONTRIBUTING.md` has a section on how to setup a cross-compilation system.](CONTRIBUTING.md#cross-compilation)

# Running bffhd

The server can be ran either using `cargo`, which will also compile the binary if necessary, or directly.

When running using `cargo` you need to pass arguments to bffh after a `--`, so
e.g. `cargo run --release -- --help` or `cargo run --release -- -c examples/bffh.toml`.

When running directly the `bffhd` binary can be copied anywhere.

A list of arguments for the server is found in the help, so `bffhd --help` or `cargo run --release -- --help`.

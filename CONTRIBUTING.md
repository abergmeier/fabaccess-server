# Contributing

Thank you for your interest in helping out the FabAccess system!

You found a bug, an exploit or a feature that doesn't work like it's documented? Please tell us
about it, see [Issues](#issues)

You have a feature request? Great, check out the paragraph on [Feature Requests](#feature-requests)

## Issues

While we try to not have any bugs or exploits or documentation bugs we're not perfect either. Thanks
for helping us out!

We have labels that help us sort issues better, so if you know what would be the correct ones,
please tag your issue:
- `documentation` if it's an documentation issue, be it lacking docs or even worse wrong docs.
- `bug` is for software bugs, unexpected behaviour, crashes and so on.
- `exploit` for any bugs that may be used as RCE, to escalate priviledges or some-such.
Don't worry if you aren't sure about the correct labels, an issue opened with no labels is much
better than no knowing about the issue!

Especially for bugs and exploits, please mark your issue as "confidential" if you think it impacts
the `stable` branch. If you're not sure, mark it as confidential anyway. It's easier to publish
information than it is to un-publish information.

If you found an exploit and it's high-impact enough that you do not want to open an issue but
instead want direct contact with the developers, you can find public keys respectively fingerprints
for GPG, XMPP+OMEMO and Matrix+MegOlm in the git repository as blobs with tags assigned to them.

You can import the gpg key for dequbed either from the repository like so:
```
$ git cat-file -p keys/dequbed/gpg | gpg --import-key
```
Or from your local trusted gpg keyserver, and/or verify it using [keybase](https://keybase.io/dequbed)
This key is also used to sign the other tags so to verify them you can run e.g.
```
$ git tag -v keys/dequbed/xmpp+omemo
```

## Feature Requests

We also like new feature requests of course! 
But before you open an issue in this repo for a feature request, please first check a few things:
1. Is it a feature that needs to be implemented in more than just the backend server? For example,
   is it something also having a GUI-component or something that you want to be able to do via the
   API? If so it's better suited over at the
   [Lastenheft](https://gitlab.com/fabinfra/fabaccess_lastenheft) because that's where the required
   coordination for that will end up happening
2. Who else needs that feature? Is this something super specific to your environment/application or
   something that others will want too? If it's something that's relevant for more people please
   also tell us that in the feature request.
3. Can you already get partway or all the way there using what's there already? If so please also
   tell us what you're currently doing and what doesn't work or why you dislike your current
   solution.

## Contributing Code

To help develop Diflouroborane you will need a Rust toolchain. I heavily recommend installing
[rustup](https://rustup.rs) even if your distribution provides a recent enough rustc, simply because
it allows to easily switch compilers between several versions of both stable and nightly. It also
allows you to download the respective stdlib crate, giving you the option of an offline reference.

We use a stable release branch / moving development workflow. This means that all *new* development
should happen on the `development` branch which is regularly merged into `stable` as releases. The
exception of course are bug- and hotfixes that can target whichever branch.

If you want to add a new feature please work off the development branch. We suggest you create
yourself a feature branch, e.g. using `git switch development; git checkout -b
feature/my-cool-feature`.
Using a feature branch keeps your local `development` branch clean, making it easier to later rebase
your feature branch onto it before you open a pull/merge request.

When you want feedback on your current progress or are ready to have it merged upstream open a merge
request. Don't worry we don't bite! ^^


# Development Setup

## Cross-compilation

If you want to cross-compile you need both a C-toolchain for your target
and install the Rust stdlib for said target.

As an example for the target `aarch64-unknown-linux-gnu` (64-bit ARMv8
running Linux with the glibc, e.g. a Raspberry Pi 3 or later with a 64-bit
Debian Linux installation):

1. Install C-toolchain using your distro package manager:
    - On Archlinux: `pacman -S aarch64-unknown-linux-gnu-gcc`
2. Install the Rust stdlib:
    - using rustup: `rustup target add aarch64-unknown-linux-gnu`
3. Configure your cargo config:

### Configuring cargo

You need to tell Cargo to use your C-toolchain. For this you need to have
a block in [your cargo config](https://doc.rust-lang.org/cargo/reference/config.html) setting at
least the paths to the gcc as `linker` and ar as `ar`:

```toml
[target.aarch64-unknown-linux-gnu]
# You must set the gcc as linker since a lot of magic must happen here.
linker = "aarch64-linux-gnu-gcc"
ar = "aarch64-linux-gnu-ar"
```

To actually compile for the given triple you need to call `cargo build`
with the `--target` flag:

```
$ cargo build --release --target=aarch64-unknown-linux-gnu
```

## Tests

Sadly, still very much `// TODO:`. We're working on it! :/

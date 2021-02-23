# Contributing

Thank you for your interest in helping out the FabAccess system!

To help develop Diflouroborane you will need a Rust toolchain. I heavily recommend installing
[rustup](https://rustup.rs) even if your distribution provides a recent enough rustc, simply because
it allows to easily switch compilers between several versions of both stable and nightly. It also
allows you to download the respective stdlib crate, giving you the option of an offline reference.

## Git Workflow / Branching

We use a stable master / moving development workflow. This means that all /new/ development should
happen on the `development` branch which is regularly merged into `master` as releases. The
exception of course are bug- and hotfixes that can target whichever branch.

If you want to add a new feature please work off the development branch. We suggest you create
yourself a feature branch, e.g. using `git switch development; git checkout -b
feature/my-cool-feature`.
Using a feature branch keeps your local `development` branch clean, making it easier to later rebase
your feature branch onto it before you open a pull/merge request.

When you want feedback on your current progress or are ready to have it merged upstream open a merge
request. Don't worry we don't bite! ^^


## Tests

Sadly, still very much `// TODO:`. We're working on it! :/

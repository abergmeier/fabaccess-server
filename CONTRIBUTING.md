# Contributing

Thank you for your interest in helping out the FabAccess system!

You found a bug, an exploit or a feature that doesn't work like it's documented? Please tell us
about it, see [Issues](#issues)

You have a feature request? Great, check out the paragraph on [Feature Requests](#feature_requests)

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


## Tests

Sadly, still very much `// TODO:`. We're working on it! :/

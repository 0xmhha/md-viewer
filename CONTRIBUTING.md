# Contributing to md-viewer

Thank you for your interest in improving `mdv`. This guide explains how to
propose changes.

## Before you start

- For bugs, open an issue with the Markdown input, the command you ran, your
  terminal and OS, and what you expected to see.
- For new features, open an issue first so the scope can be agreed before
  you write code. `mdv` aims to stay small and fast, so not every feature
  will fit.

## Development setup

1. Install Rust with [rustup](https://rustup.rs). The project builds with
   the stable toolchain, set in `rust-toolchain.toml` and selected
   automatically.
2. Build and run the tests:

   ```sh
   cargo build
   cargo test
   ```

3. Before opening a pull request, make sure these pass:

   ```sh
   cargo fmt --all -- --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   ```

## Making changes

- Write a failing test first, then the code that makes it pass.
- Keep each pull request focused on one change.
- Do not add a dependency without explaining in the pull request why it is
  needed and what its license is. Dependencies must use a license
  compatible with MIT and Apache-2.0. GPL-licensed code cannot be included.
- `mdv` must not access the network at run time. Assets such as fonts are
  bundled into the binary.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org):

```
<type>: <short summary>

<optional body explaining why>
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`.

## Pull requests

- Branch from `main` and open the pull request against `main`.
- Describe what changed, why, and how you tested it.
- Pull requests are merged with squash merge.

## License

By contributing, you agree that your contributions are dual licensed under
the MIT and Apache-2.0 licenses, as described in the README.

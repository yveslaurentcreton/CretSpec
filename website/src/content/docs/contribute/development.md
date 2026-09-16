---
title: Development
description: Build, test and contribute to the native CLI and static documentation.
---

Clone the product repository. Public development needs no private spec or guidelines checkout. Install Git and Rust; `rust-toolchain.toml` selects the compiler. The CLI uses system Git for repository operations and authentication.

```sh
cargo run -- --help
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

Integration tests create isolated temporary repositories and configuration. They cover cloning, movement, discovery, preservation of local work, origin checks, exact snapshots, initialization, diagnosis and interrupted update recovery.

Agent scenarios also cover scoped skill sources, adopted versus draft skills, standard host paths, existing instructions, managed-file conflicts, interrupted synchronization and platform-specific links/permissions. These tests verify filesystem integration; live discovery and invocation require a separate session in the relevant agent.

Use `cargo run -- <arguments>` for development code. A separately installed `cspec` does not change when you edit this checkout. Install deliberately with `cargo install --path . --locked` when ready.

## Structure

| Path | Responsibility |
| --- | --- |
| `src/main.rs` | Command parsing and presentation |
| `src/workspace.rs` | Discovery, preparation and editor files |
| `src/operations.rs` | Initialization, diagnostics, guideline adoption/recovery |
| `src/skills.rs` | Standard skill bundles, source validation and scoped scaffolding |
| `src/agents.rs` | Structured context, host entry points and protected synchronization |
| `assets/cspec-workspace/` | Workflow skill embedded in the native executable |
| `src/manifest.rs`, `schemas/` | Versioned project protocol |
| `src/repository.rs`, `src/git.rs` | References and Git subprocesses |
| `src/config.rs`, `src/files.rs` | Personal configuration and filesystem helpers |
| `tests/workflow.rs` | Native integration scenarios |
| `scripts/` | Release packaging and immutable upload checks |
| `website/` | Static documentation with separate build dependencies |

## Documentation

Use Node.js 24 in `website/`:

```sh
npm ci
npm run dev
npm run build
```

The site uses Astro/Starlight, static output and local search. Its build dependencies are separate from the distributed native CLI. Check navigation, examples, keyboard access and narrow screens when changing the site.

## Contributions

Keep a change focused on an observable outcome. Describe the problem and relevant validation in the pull request. Write repository content in English. Keep manifest schemas, runtime checks and documentation aligned when changing the protocol.

Use Conventional Commit titles, for example `fix: preserve editor settings` or `feat: initialize a specification`. Use `!` or a `BREAKING CHANGE:` footer when existing users must adapt. See [release rules](/CretSpec/contribute/releases/).

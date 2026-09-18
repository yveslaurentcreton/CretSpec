# Contributing

Bug reports, documentation improvements and focused fixes are welcome. Open an issue before starting a larger feature or changing the project protocol. The project maintainer decides scope, merges and releases. Support and reviews are best effort; no response time is guaranteed.

Install Git and Rust, then clone this repository. The toolchain is pinned; no private specification or guidelines repository is required.

```sh
cargo run -- --help
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Describe the user-visible problem, focused change and relevant verification in your pull request. Keep repository content in English. Use Conventional Commit titles such as `fix: preserve editor settings` or `feat: initialize a specification`; mark breaking changes explicitly.

Documentation lives in `website/`. With Node.js 24, run `npm ci`, `npm run dev` and `npm run build` there. Check examples, navigation, keyboard access and narrow layouts.

The [development guide](website/src/content/docs/contribute/development.md) explains the code structure. The [release guide](website/src/content/docs/contribute/releases.md) covers semantic releases, native artifacts and package channels.

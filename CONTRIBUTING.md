# Contributing

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

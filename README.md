<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="website/src/assets/mark-dark.svg">
    <img src="website/src/assets/mark-light.svg" width="80" height="80" alt="CretSpec logo">
  </picture>
</p>

<h1 align="center">CretSpec</h1>

<p align="center"><strong>One spec. Your whole workspace.</strong></p>

<p align="center">
  <a href="https://github.com/yveslaurentcreton/CretSpec/releases/latest"><img src="https://img.shields.io/github/v/release/yveslaurentcreton/CretSpec?label=release&amp;color=f05032" alt="Latest release"></a>
  <a href="https://github.com/yveslaurentcreton/CretSpec/actions/workflows/release.yml"><img src="https://img.shields.io/github/actions/workflow/status/yveslaurentcreton/CretSpec/release.yml?branch=main&amp;label=checks" alt="Build and test status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-5c5854" alt="License: MIT"></a>
</p>

<p align="center">
  <a href="https://yveslaurentcreton.github.io/CretSpec/"><strong>Read the documentation →</strong></a>
  &nbsp;·&nbsp;
  <a href="https://yveslaurentcreton.github.io/CretSpec/start/install/">Installation</a>
  &nbsp;·&nbsp;
  <a href="https://yveslaurentcreton.github.io/CretSpec/start/quickstart/">Quick start</a>
</p>

CretSpec is a small native CLI. One command clones your **spec, code repositories and shared AI guidelines** into one folder and gives **Codex, Claude Code, GitHub Copilot in VS Code and Cursor** the same instructions and skills.

Without it, you clone every repository by hand, copy your rules into each one and watch the copies drift between projects and machines.

## Get started

**1. Install CretSpec.** Git is required; prebuilt binaries need no Rust toolchain.

| Platform | Install |
| --- | --- |
| macOS | `brew install yveslaurentcreton/tap/cretspec` |
| Windows / Linux | [Download a native executable](https://github.com/yveslaurentcreton/CretSpec/releases/latest) and follow the [installation guide](https://yveslaurentcreton.github.io/CretSpec/start/install/). |

**2. Try the demo.** It is public, so you need no account or key:

```sh
cspec project clone https://github.com/yveslaurentcreton/cretspec-demo-spec.git
```

You get one folder:

```text
Atlas/
├── AGENTS.md                    # generated: what is here and which rules apply
├── cretspec-demo-spec/          # the specification, with project.json
├── cretspec-demo-app/           # the code
└── cretspec-demo-guidelines/    # shared AI guidelines
```

Start your agent in `Atlas/` and ask which rules apply. The instructions and skills are generated locally and never committed.

**3. Start your own project.** Three repositories and one small file are enough: [start your own project](https://yveslaurentcreton.github.io/CretSpec/guides/initialize/).

## Documentation

**[The documentation site](https://yveslaurentcreton.github.io/CretSpec/)** covers setup, everyday work and the full reference.

[How it works](https://yveslaurentcreton.github.io/CretSpec/start/concepts/) · [Working with your agent](https://yveslaurentcreton.github.io/CretSpec/guides/agents/) · [Combining repositories and guidelines](https://yveslaurentcreton.github.io/CretSpec/guides/workspaces/) · [CLI reference](https://yveslaurentcreton.github.io/CretSpec/reference/commands/)

Bug reports and focused improvements are welcome. See [Contributing](CONTRIBUTING.md).

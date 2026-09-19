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

CretSpec is a small native CLI that clones your **spec, code and shared AI guidelines** into one workspace. It prepares local instructions and skills for **Codex, Claude Code, GitHub Copilot in VS Code and Cursor**.

Your spec defines the project. Each repository keeps its own Git history. Use your existing Git credentials on Windows, macOS or Linux.

## Get started

**1. Install CretSpec.** Git is required; prebuilt binaries need no Rust toolchain.

| Platform | Install |
| --- | --- |
| macOS | `brew install yveslaurentcreton/tap/cretspec` |
| Windows / Linux | [Download a native executable](https://github.com/yveslaurentcreton/CretSpec/releases/latest) and follow the [installation guide](https://yveslaurentcreton.github.io/CretSpec/start/install/). |

**2. Clone your project.** From your projects directory, use an existing CretSpec spec repository. Replace the example URL and project name with your own:

```sh
cspec project clone https://github.com/your-account/Atlas-spec.git
cd Atlas
cspec project doctor
cspec project open
```

Your repositories are grouped in one folder:

```text
Atlas/
├── Atlas-spec/     # specification and project definition
├── Atlas/          # product code
└── ai-guidelines/  # shared AI guidelines and skills
```

Selected agent instructions and skills are generated locally and excluded from Git. Open the workspace in your editor and start working.

Starting a new project? [Create a specification first](https://yveslaurentcreton.github.io/CretSpec/guides/initialize/).

## Documentation

**[The documentation site](https://yveslaurentcreton.github.io/CretSpec/)** covers setup, everyday workflows, configuration and the full CLI reference.

[How it works](https://yveslaurentcreton.github.io/CretSpec/start/concepts/) · [Working with agents and skills](https://yveslaurentcreton.github.io/CretSpec/guides/agents/) · [CLI reference](https://yveslaurentcreton.github.io/CretSpec/reference/commands/)

Bug reports and focused improvements are welcome. See [Contributing](CONTRIBUTING.md).

---
title: Try it in two minutes
description: Clone the demo project and see what your coding agent gets.
---

You need [CretSpec and Git](/CretSpec/start/install/). The demo is public, so you need no account or key.

## 1. Clone the demo

Run this in the folder where you keep your projects:

```sh
cspec project clone https://github.com/yveslaurentcreton/cretspec-demo-spec.git
```

CretSpec clones the specification, reads its `project.json` and clones the two repositories it names:

```text
Atlas/
├── AGENTS.md                    generated instructions
├── cretspec-demo-spec/          the specification, with project.json
├── cretspec-demo-app/           the code
└── cretspec-demo-guidelines/    shared AI guidelines
```

## 2. See what your agent gets

Open `Atlas/AGENTS.md`. It names every repository and lists the rules that apply. An excerpt:

```md wrap
- Project definition and specification, repository `spec`: `cretspec-demo-spec`: What Atlas is, its rules and its skills.
- Repository `cretspec-demo-app`: `cretspec-demo-app`: The Atlas greeter, a tiny command-line program.
- Editable shared AI guidelines: `cretspec-demo-guidelines`

Before project work, read the relevant active guidance and project rules. Project rules take precedence over shared guidance:

- `cretspec-demo-guidelines/README.md`
- `cretspec-demo-guidelines/guidelines/principles.md`
- `cretspec-demo-spec/spec/project-rules.md`
```

Each repository gets its own `AGENTS.md` and the project's skills. These files are generated on your machine and never committed.

## 3. Ask your agent

Start Claude Code, Codex, Copilot or Cursor in `Atlas/` or in one of its repositories. Ask:

> Which rules apply in this project, and where do they come from?

The agent answers from the demo's project rules and guidelines. Then let it use a skill:

> Add a greeting for the evening.

`cspec project open` opens the three repositories as one VS Code workspace. `cspec project doctor` checks the project without changing it.

## Next

- [Start your own project](/CretSpec/guides/initialize/): three repositories and one small file.
- [How it works](/CretSpec/start/concepts/), in one picture.

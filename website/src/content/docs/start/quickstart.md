---
title: Clone your first project
description: Go from one spec repository to a complete local project.
---

Start with [CretSpec and Git installed](/CretSpec/start/install/), Git access to all three repositories, and a spec containing `project.json` and `spec/` (plus `guidelines.lock.json` if pinned). If you do not have that spec yet, [initialize one](/CretSpec/guides/initialize/).

## 1. Choose your projects directory

Run these commands in an ordinary directory outside existing Git repositories. For example, use `D:\cspec-projects` on Windows or `~/cspec-projects` on macOS/Linux.

Set your GitHub owner or organization once. Replace `your-account` and `Atlas-spec` with your real owner and repository:

```sh
cspec config namespace your-account
cspec project clone Atlas-spec
```

Or pass a full source without configuring anything:

```sh
cspec project clone git@github.com:your-account/Atlas-spec.git
```

## 2. Inspect the project

With a spec that names its product `Atlas` and points to a repository named `ai-guidelines` containing your shared AI guidelines, you get:

```text
Atlas/
├── AGENTS.md            generated workspace instructions
├── CLAUDE.md            generated Claude entry point
├── Atlas-spec/
│   ├── project.json
│   ├── spec/
│   └── .local/           ignored, generated context
├── Atlas/               product code
└── ai-guidelines/       editable AI guidelines
```

The outer directory groups your repositories and generated agent context. It has no project definition or Git repository of its own. Selected integrations also prepare skills and instruction entry points inside each repository; these local files are excluded from Git.

```sh
cd Atlas
cspec project doctor
cspec project info
```

`doctor` checks without changing files. `info` shows JSON with paths, manifest, guidelines state and optional lock; it can prepare missing local context.

## 3. Open your editor

```sh
cspec project open
```

This generates `Atlas-spec/.local/project.code-workspace` and asks the operating system to open it. Associate `.code-workspace` with VS Code or a compatible editor. With another editor, open the three folders directly. `cspec project open --print` generates and prints the file without launching an application.

You can now edit and commit code, specification and shared guidelines in their respective repositories. Each keeps its own branches and history. Local guideline edits are active immediately in the default working-tree mode; run `cspec sync` after changing skills. Fetch and pull shared changes with Git when you want them.

Your coding agent can now discover the prepared instructions and skills. Ask it to read the project's specification and active guidance. Follow [the agent workflow](/CretSpec/guides/agents/) to create project or shared skills and check their availability.

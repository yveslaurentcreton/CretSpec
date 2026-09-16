---
title: Clone your first project
description: Go from one spec repository to a complete local project.
---

Start with [CretSpec and Git installed](/CretSpec/start/install/), Git access to all three repositories, and a spec containing `project.json`, `guidelines.lock.json` and `spec/`. If you do not have that spec yet, [initialize one](/CretSpec/guides/initialize/).

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

With a spec that names its product `Atlas` and guidelines repository `CretAI`, you get:

```text
Atlas/
├── Atlas-spec/
│   ├── project.json
│   ├── guidelines.lock.json
│   ├── spec/
│   └── .local/           ignored, generated context
├── Atlas/               product code
└── CretAI/              editable guidelines
```

The outer directory only groups your repositories. It has no project definition or Git repository of its own.

```sh
cd Atlas
cspec project doctor
cspec project info
```

`doctor` checks without changing files. `info` shows JSON with paths, manifest and lock; it can prepare missing local context.

## 3. Open your editor

```sh
cspec project open
```

This generates `Atlas-spec/.local/project.code-workspace` and asks the operating system to open it. Associate `.code-workspace` with VS Code or a compatible editor. With another editor, open the three folders directly. `cspec project open --print` generates and prints the file without launching an application.

You can now edit and commit code, specification and shared guidelines in their respective repositories. Each keeps its own branches and history.

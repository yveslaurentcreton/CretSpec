---
title: Combine repositories and roles
description: Declare several repositories in one workspace and give agents role-specific entry points.
---

A workspace can combine several independently versioned repositories, for example an application and the infrastructure that deploys it. Each repository keeps its own Git history and origin. CretSpec needs no submodules or monorepo. Agents receive shared context for the whole workspace, each repository's own instructions, and optional roles for a specific focus.

## Declare repositories

Schema version 2 lists repositories by name. This example keeps the specification inside the application repository, which also contains the code:

```json
{
  "schemaVersion": 2,
  "name": "Atlas",
  "specRepository": {
    "name": "atlas",
    "purpose": "Application code, tests and the workspace specification.",
    "profile": "dotnet"
  },
  "repositories": [
    {
      "name": "infra",
      "url": "git@github.com:your-account/atlas-infra.git",
      "path": "atlas-infra",
      "purpose": "Infrastructure and GitOps, including the application deployment."
    }
  ],
  "guidelines": { "repository": "git@github.com:your-account/ai-guidelines.git", "ref": "main" },
  "agents": ["codex", "claude"]
}
```

The result is one ordinary folder:

```text
Atlas/
├── atlas/          # code and specification, contains project.json
├── atlas-infra/    # infrastructure repository
├── ai-guidelines/  # editable shared guidelines
└── roles/          # generated role entry points, when roles exist
```

A separate spec repository works the same way: omit the application from `specRepository` and declare it under `repositories`. Every name, path and purpose is described in the [project file reference](/CretSpec/reference/manifest/#schema-version-2). Version 1 projects keep working unchanged; convert one only when you want to add repositories.

Use the SSH or HTTPS sources your Git credentials already support. Azure DevOps SSH, HTTPS and `visualstudio.com` addresses of the same repository are recognized as equal. Credentials never belong in `project.json`.

:::caution
Combining specification and code suits a private product repository. CretSpec cannot check repository visibility. Keep internal planning out of repositories that are, or will become, public.
:::

## Clone or include existing clones

To start on a new machine, clone the repository containing `project.json`:

```sh
cspec project clone git@github.com:your-account/atlas.git
```

All declared repositories and the editable guidelines are cloned next to it.

When the repositories are already cloned, place them as siblings in an ordinary directory and attach them in place:

```sh
cspec project attach atlas
```

Attach checks every declared path first. Each one must be the root of its own Git repository with the expected origin. Branches, local changes and existing agent instructions are reported and preserved. A missing, misplaced or mismatched repository stops the command before anything is written. To clone missing repositories at their declared paths, add `--clone-missing`.

To create the definition for existing clones, initialize the repository that contains the specification. `--repository` reads each sibling's origin, and `--existing-spec` keeps an existing `spec/` directory untouched:

```sh
cspec project init atlas --name Atlas --guidelines git@github.com:your-account/ai-guidelines.git \
  --profile dotnet --existing-spec --repository atlas-infra
```

Each repository is named after its folder, here `atlas-infra`. You can choose a shorter `name` such as `infra` and keep the folder as `path`. Then add a `purpose` to each repository, review `project.json`, commit it and run `cspec project attach atlas`. See [initialization](/CretSpec/guides/initialize/#several-repositories) for all options.

`cspec project doctor` warns about Git repositories in the workspace folder that `project.json` does not declare.

## Layered instructions

| Layer | Edit here | Agents receive it in |
| --- | --- | --- |
| Shared workspace | Active guidelines, the spec's `spec/project-rules.md` and the repository purposes in `project.json` | The workspace folder and every repository |
| Repository | The repository's own tracked `AGENTS.md` | That repository, unchanged |
| Role | `spec/roles/<name>.md` in the specification | The role's folder under `roles/` |

Project rules take precedence over shared guidelines. A project can, for example, keep its existing specification method and record that choice in its project rules.

Let repositories commit only `AGENTS.md`. Every supported agent reads it, so CretSpec generates no tool-specific entry point for Claude Code. A committed `CLAUDE.md` is respected, but makes Claude Code skip `AGENTS.md`, including the workspace context; see [generated files](/CretSpec/reference/skills/#generated-files). Repository and workspace entry points never contain role instructions, so a role cannot leak into another conversation.

## Define roles

A role gives an agent shared project context with a specific focus. Store each role in the specification:

```md
---
name: app-dev
description: Develop and test the application, including related infrastructure changes.
repositories: [atlas, infra]
skills: [release-check]
---

Develop features through the application's own specification workflow.
Keep application code and infrastructure changes in their own repositories.
Check the infrastructure repository's branch and status before editing it.
```

`repositories` lists the focus repositories by name, in priority order. `skills` selects active project, repository or shared skills; omit it to include all of them. The built-in `cspec-workspace` skill is always included. The Markdown body describes the role's responsibilities and boundaries.

Run `cspec sync`. Every role gets its own folder:

```text
roles/app-dev/
├── AGENTS.md            shared context plus the role
├── .agents/skills/      the selected skills
└── .claude/skills/
```

## Select a role

Start the agent in the role's folder. Two conversations can use different roles at the same time, because each reads its own folder and nothing is switched:

```sh
cd Atlas/roles/operations && codex
cd Atlas/roles/app-dev && claude
```

In a desktop agent app that organizes work by folder, register each role folder as its own project. To give a role to a conversation that started elsewhere, ask the agent to run:

```sh
cspec context --role app-dev
```

This prints the role's instructions, focus repositories and skills without changing any file.

The role folder is the agent's working directory, so the repositories it works on are siblings. A host that only allows writes inside its working directory needs the focus repositories as additional directories; `cspec context --role <name>` lists them. For example, with Claude Code:

```sh
claude --add-dir ../../atlas --add-dir ../../atlas-infra
```

CretSpec does not change host permissions or trust settings.

:::note
Roles are working agreements, not a security boundary. Agents running as the same operating-system user can read the same files and use the same credentials, such as SSH keys or cluster access. Use your host's approval settings or separate accounts when you need enforcement.
:::

## Limits

- Linked Git worktrees outside the workspace folder, such as those created by some agent apps for parallel work, do not receive generated context. Ignored files and sibling repositories are not copied into them.
- CretSpec prepares context only. It does not run agents, coordinate conversations or manage backlogs.
- Role folders are generated outputs. Edit the role source and run `cspec sync`; removing a role removes only the files CretSpec created for it.

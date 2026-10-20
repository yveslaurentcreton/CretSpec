---
title: Combine repositories and roles
description: Declare several repositories in one workspace, reference guidelines and other projects, and give agents role-specific entry points.
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

## Reference guidelines and other projects

A workspace can build on knowledge that others own. Suppose you specify work for a customer that keeps its own AI guidelines, a technical handbook and an application with its own CretSpec specification. The customer's developers open the application's project directly. You keep your own specification for the customer and want everything in one workspace.

[Schema version 3](/CretSpec/reference/manifest/#schema-version-3) gives each repository entry a `type`:

```json
"repositories": [
  { "name": "platform", "url": "git@ssh.dev.azure.com:v3/ardeca/Industries/platform" },
  { "name": "handbook", "type": "guidelines", "url": "git@ssh.dev.azure.com:v3/ardeca/Industries/handbook",
    "ref": "main", "instructions": ["README.md"] },
  { "name": "ardisq", "type": "project", "url": "git@ssh.dev.azure.com:v3/ardeca/Industries/ardisq" }
]
```

Run `cspec project migrate` on an existing version 2 project first. `cspec project clone` or `cspec project attach <spec> --clone-missing` then clones the tree: the handbook, the application's spec and everything that project declares, such as its own AI guidelines. A referenced project can reference further projects; CretSpec resolves the whole tree and stops on a cycle.

The result stays one ordinary folder:

```text
ardeca-industries/
├── ardeca-industries/  # your specification, contains project.json
├── ai-guidelines/      # your guidelines
├── platform/
├── handbook/           # guidelines reference
├── ardisq/             # referenced project: its spec and code
└── ai-kit/             # guidelines that ardisq declares
```

### What agents read

Every generated `AGENTS.md` lists its sources from highest to lowest precedence. Your project rules always come first; you can override or extend a customer's rules there. By default the references follow in declaration order, with your own guidelines first:

```text
1. Project rules: `ardeca-industries/spec/project-rules.md`
2. Shared guidelines `ai-guidelines`: ...
3. Guidelines `handbook`: `handbook/README.md`
4. Referenced project `ardisq` (applies in `ardisq`, `ai-kit`): read its context file `ardeca-industries/.local/context/ardisq.md`
```

A referenced project's guidance applies in its own folders. In `platform/AGENTS.md` the `ardisq` entry is absent; in `ardisq/AGENTS.md` it is present without the folder list. The workspace root lists everything with the folders each entry applies in. The context file contains exactly what `ardisq` gives when it is opened on its own, so it stays as its owners intended, and `AGENTS.md` stays short however deep the tree is.

- `priority` changes the order. `"priority": 1` on `ardisq` puts it, with everything it brings, above your own guidelines, for example when its .NET profile should lead in its code.
- `scope` limits a guidelines reference to named repositories, for example `"scope": ["platform"]` for an infrastructure handbook.
- Skills of every source are distributed to every folder. When two sources define the same skill name, the one with the highest precedence wins and sync and doctor say which skill was overridden.

Commands run inside a referenced project's folder still act on your workspace. When the customer's developers clone `ardisq` themselves, they see only its own context. Nothing from your specification appears there.

## Define roles

A role gives an agent shared project context with a specific focus. Store each role in the specification:

```md
---
name: app-dev
description: Develop and test the application, including related infrastructure changes.
repositories: [atlas, infra]
---

Develop features through the application's own specification workflow.
Keep application code and infrastructure changes in their own repositories.
Check the infrastructure repository's branch and status before editing it.
```

`repositories` lists the focus repositories by name, in priority order. The optional `skills` field selects active project, repository or shared skills; omit it, as recommended, to give the role every skill of the workspace. The built-in `cspec-workspace` skill is always included. The Markdown body describes the role's responsibilities and boundaries.

Agents use a skill only when it matches the request, so a role rarely needs a narrower selection. Describe what the role does and leaves to other roles in its body instead. Select skills only to keep a long list focused. The selection is not isolation: Claude Code also loads `.claude/skills/` from the directories above its working directory, so in a role folder it also sees the workspace's skills.

Run `cspec sync`. Every role gets its own folder:

```text
roles/app-dev/
├── AGENTS.md            shared context plus the role
├── .agents/skills/      the role's skills
└── .claude/skills/
```

## Select a role

Agents take their instructions from the directory in which they start, and most hosts cannot pass different instructions per conversation. A role folder gives each role its own starting directory. Start the agent in the role's folder. Two conversations can use different roles at the same time, because each reads its own folder and nothing is switched:

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

- References resolve inside one workspace folder. A referenced project's own roles are not generated; define roles in your own specification.
- Linked Git worktrees outside the workspace folder, such as those created by some agent apps for parallel work, do not receive generated context. Ignored files and sibling repositories are not copied into them.
- CretSpec prepares context only. It does not run agents, coordinate conversations or manage backlogs.
- Role folders are generated outputs. Edit the role source and run `cspec sync`; removing a role removes only the files CretSpec created for it.

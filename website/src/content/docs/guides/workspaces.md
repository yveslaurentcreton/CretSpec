---
title: Combine repositories
description: Put several repositories in one workspace, and build on guidelines and projects that others own.
---

A workspace can hold several repositories, for example an application and the infrastructure that deploys it. Each keeps its own Git history and origin. CretSpec needs no submodules or monorepo.

## Declare repositories

List every repository in the spec's `project.json`:

```json
"repositories": [
  { "name": "atlas", "url": "../atlas", "purpose": "The product code." },
  { "name": "infra", "url": "../atlas-infra", "path": "atlas-infra", "purpose": "Infrastructure and deployment." }
]
```

`name` is how instructions refer to the repository. `path` is its folder and defaults to the name. The result is one ordinary folder:

```text
Atlas/
├── AGENTS.md
├── atlas-spec/      # the specification, contains project.json
├── atlas/           # product code
├── atlas-infra/     # infrastructure
└── ai-guidelines/   # shared guidelines
```

Use the SSH or HTTPS sources your Git credentials already support. Credentials never belong in `project.json`.

### The spec inside a code repository

The specification can also live in the repository that holds the code. Describe that repository with `specRepository` and list only the others:

```json
{
  "schemaVersion": 3,
  "name": "Atlas",
  "specRepository": { "name": "atlas", "purpose": "Application code and the specification." },
  "repositories": [
    { "name": "infra", "url": "../atlas-infra", "path": "atlas-infra", "purpose": "Infrastructure and deployment." }
  ],
  "guidelines": { "repository": "../ai-guidelines", "ref": "main" }
}
```

:::caution
This suits a private product repository. CretSpec cannot check repository visibility. Keep internal planning out of repositories that are, or will become, public.
:::

## Clone or attach

On a new machine, clone the repository that contains `project.json`. Everything it declares is cloned next to it:

```sh
cspec project clone git@github.com:your-account/atlas-spec.git
```

When the repositories are already cloned next to each other, attach them in place. Add `--clone-missing` to clone the ones that are absent:

```sh
cspec project attach atlas-spec
```

Attach checks every folder and origin before it writes anything, and keeps your branches and local changes. See [move or attach a project](/CretSpec/guides/existing-projects/). `cspec project doctor` warns about Git repositories in the workspace folder that `project.json` does not declare.

## Each repository keeps its own instructions

A repository can commit its own `AGENTS.md`. Agents read it in that repository, next to the workspace context. Commit only `AGENTS.md`: every supported agent reads it, and a committed `CLAUDE.md` makes Claude Code skip the workspace context. See [generated files](/CretSpec/reference/skills/#generated-files).

## Reference guidelines and other projects

A workspace can build on knowledge that others own. Suppose you build Atlas for a customer, Acme, that keeps its own AI guidelines and an application with its own CretSpec specification. You want everything in one workspace.

Give an entry a `type`:

```json
"repositories": [
  { "name": "atlas", "url": "../atlas" },
  { "name": "acme-handbook", "type": "guidelines", "url": "git@ssh.dev.azure.com:v3/acme/Platform/handbook",
    "ref": "main", "instructions": ["README.md"] },
  { "name": "acme-billing", "type": "project", "url": "git@ssh.dev.azure.com:v3/acme/Platform/billing" }
]
```

Clone or `attach --clone-missing` then fetches the whole tree: the handbook, the referenced project and everything that project declares, such as its own guidelines:

```text
Atlas/
├── AGENTS.md          # generated overview for every agent
├── atlas-spec/        # your specification, contains project.json
├── atlas/             # your code
├── ai-guidelines/     # your guidelines
├── acme-handbook/     # the customer's guidelines
├── acme-billing/      # referenced project: its spec and code
└── acme-guidelines/   # guidelines that acme-billing declares
```

A definition with an older `schemaVersion` needs `cspec project migrate` first.

### Which source wins

Every generated `AGENTS.md` lists its sources from highest to lowest precedence. Your project rules always come first, so you can override or extend a customer's rules there. The references follow in the order you declare them, with your own guidelines first:

```text
1. Project rules: `atlas-spec/spec/project-rules.md`
2. Shared guidelines `ai-guidelines`: ...
3. Guidelines `acme-handbook`: `acme-handbook/README.md`
4. Referenced project `acme-billing` (applies in `acme-billing`, `acme-guidelines`): read its context file `atlas-spec/.local/context/acme-billing.md`
```

- `priority` changes the order. `"priority": 1` on `acme-billing` puts it, with everything it brings, above your own guidelines.
- `scope` limits a guidelines reference to named repositories, for example `"scope": ["atlas"]`.
- A referenced project's guidance applies in its own folders only.
- Skills of every source reach every folder. When two sources define the same skill name, the one with the highest precedence wins, and sync and doctor say which skill was overridden.

When the customer's developers clone `acme-billing` themselves, they see only its own context. Nothing from your specification appears there. The [reference](/CretSpec/reference/manifest/#schema-version-3) describes every field and the reference tree.

## Limits

- References resolve inside one workspace folder.
- Linked Git worktrees outside the workspace folder, such as those created by some agent apps for parallel work, do not receive generated context.
- CretSpec prepares context only. It does not run agents, coordinate conversations or manage backlogs.

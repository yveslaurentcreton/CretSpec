---
title: Skills and agent context
description: Skill and role source layout, validation, generated files and structured CLI output.
---

CretSpec uses the [Agent Skills format](https://agentskills.io/specification). Each skill is a directory named after its `name`, containing `SKILL.md` and optional resources:

```text
spec/skills/query-validation/
├── SKILL.md
└── references/
    └── examples.json
```

```md
---
name: query-validation
description: Validate query inputs and expected results for this project.
---

Use the cases in [examples](references/examples.json) to check the query behavior.
Record the observed results and unresolved cases in the specification.
```

The description explains when to use the skill. Detailed instructions and links to supporting files belong in the body. Keep relative resource references inside the skill bundle. Host-specific optional YAML fields are preserved; CretSpec does not translate their semantics or guarantee support in every agent.

## Sources and validation

- Project skills: `<spec>/spec/skills/<name>/`, read from the working tree.
- Repository skills: `<repository>/<skills>/<name>/` for each repository that declares a [`skills` directory](/CretSpec/reference/manifest/#schema-version-2), read from the working tree. Use them for knowledge that belongs to one repository's own code or method, so it stays with that repository in every workspace. A declared directory that does not exist yet has no skills. The directory and its parents must be real directories, not links.
- Shared skills in working-tree mode: `<editable-guidelines>/skills/<name>/`, including uncommitted changes.
- Shared skills in pinned mode: `<spec>/.local/guidelines/<commit>/skills/<name>/`; the editable source is a separate draft catalog.
- Built-in `cspec-workspace`: embedded in the executable, reserved and versioned with CretSpec.
- With [typed references](/CretSpec/reference/manifest/#schema-version-3): the `skills/` directory of each further guidelines reference (its snapshot when pinned), and the project, repository and guidelines skills of each referenced project.

Names contain 1–64 lowercase ASCII letters, digits and single hyphens, with no leading/trailing hyphen or Windows device name. The YAML `name` must match its directory. The description must be nonempty and at most 1024 characters. UTF-8 and CRLF are accepted, including folded YAML descriptions. Duplicate active names across the workspace's own project, repository and shared sources are rejected instead of silently overriding one another; the error names both sources. A name that also exists in a further guidelines reference or a referenced project resolves to the source with the highest [precedence](/CretSpec/reference/manifest/#precedence); sync and doctor warn which skill overrides which; identical copies, such as one repository reached twice, are silent. Invalid skills in a referenced project skip that source with a warning instead of stopping sync.

The source directory may contain a `README.md` catalog alongside its skill directories. Each skill directory must contain `SKILL.md`. Resource names start with an ASCII letter/digit and contain only letters, digits, `.`, `_` or `-`; hidden names, device names, trailing dots and names differing only by case are rejected. Resources must be regular files and directories, with no symlinks. Limits are 128 KiB per `SKILL.md`, 256 files and 16 MiB per skill, eight nested directories, 128 skills per source and 64 MiB of active resources in total.

Full bundles are copied, including references, assets and scripts. Unix executable permissions are preserved. Scripts are never executed during clone, sync or skill creation. Declare platform dependencies and use cross-platform commands when intended for all supported operating systems.

In pinned mode, an invalid shared draft is reported separately by `skill list` and does not invalidate adopted skills. In working-tree mode the editable source is active, so invalid shared skills fail validation. Pinning and unlocking validate the proposed active skill set before changing the definition.

## Role files

A role is one Markdown file in the spec's `spec/roles/<name>.md`, read from the working tree:

```md
---
name: operations
description: "Operate the system: monitoring, incidents and the backlog."
repositories: [atlas, infra]
skills: []
---

Investigate system status only when asked. Record decisions in the specification.
```

| Field | Meaning |
| --- | --- |
| `name` | Must match the file name; same rules as skill names. |
| `description` | Nonempty, at most 1024 characters. Quote it when it contains a colon followed by a space; invalid YAML is reported with its line and field. |
| `repositories` | Optional focus repositories by name, in priority order: the spec repository's name or a declared repository's name. |
| `skills` | Optional active skill names. Omit to include every active skill; `[]` includes only the built-in workflow skill. |

The Markdown body is required and holds the role's responsibilities. Unknown fields, unknown repositories or skills, and files other than `<name>.md` fail validation before generated outputs change. Each role file is limited to 128 KiB, and a project to 64 roles. Hidden files in `spec/roles/` are ignored. See [combining repositories and roles](/CretSpec/guides/workspaces/).

## Generated files

Selected integrations are generated at the outer workspace, spec, every declared repository, every repository of the reference tree and editable guidelines roots, and in each role folder under `roles/<name>/`. Each referenced project also gets a context file in the spec's `.local/context/`, with paths relative to the workspace root; `AGENTS.md` files refer to it where that project applies. Paths in instructions are relative to their corresponding root. Moving the whole workspace keeps them usable; no generated absolute project binding exists.

Workspace and repository entry points are role-neutral. They describe every repository with its purpose and list the available roles. A role folder contains an `AGENTS.md` with the shared context and the role, Copilot and Cursor entry points with the shared context and the role, and the role's selected skills.

When an existing `AGENTS.md` is present, its content is included in a generated `AGENTS.override.md`, because Codex reads the override instead of `AGENTS.md`. The original is unchanged. When the tracked `AGENTS.md` changes, the copy is stale until `cspec sync`; doctor names the changed file. Existing instructions have a 24 KiB input limit; larger inputs need consolidation. A pre-existing conflicting override destination is never adopted or overwritten automatically.

We recommend that repositories commit only `AGENTS.md`. Codex, GitHub Copilot and Cursor read it, and Claude Code 2.1.277 or later reads it too when no `CLAUDE.md` or `CLAUDE.local.md` exists in the working directory or above it. CretSpec generates no `CLAUDE.md` or `CLAUDE.local.md`. Claude Code reads every `AGENTS.md` in its working directory and above it, so in a repository with a tracked `AGENTS.md` it also reads the workspace `AGENTS.md`; it does not need the Codex `AGENTS.override.md`. Older versions, or sessions that cannot read `AGENTS.md`, receive no CretSpec instructions. An existing `CLAUDE.md` is kept; doctor reports that it makes Claude Code skip `AGENTS.md` there. Sync removes the `CLAUDE.md` and `CLAUDE.local.md` files that earlier versions generated. A personal `CLAUDE.local.md` in such a repository stops Claude Code from reading `AGENTS.md`; set its **Project instructions** option to `claude-md-and-agents-md` instead.

Switching branches can replace a generated file with a tracked one, for example when only one branch tracks `AGENTS.md`: Git overwrites the excluded file. Sync then stops generating that path, keeps the tracked file and generates the alternative, `AGENTS.override.md`; doctor reports it until then. Back on a branch without the tracked file, sync generates it again. A tracked file that still contains generated content was added to Git by mistake and stops sync until it is untracked.

The spec's ignored `.local/` contains:

| File | Purpose |
| --- | --- |
| `agents-state.json` | Owned output paths and SHA-256/permission fingerprints |
| `agents-pending.json` | Present during synchronization; permits safe retry after interruption |
| `agents.lock` | Operating-system lock preventing concurrent synchronizations; released when the process exits |

Synchronization validates sources and conflicts before changing outputs, replaces individual files atomically, then commits ownership metadata. It is not one atomic filesystem transaction across all repositories. If interrupted, run `cspec sync` again. A pending journal and manual edits are preserved when they conflict. Do not delete ownership metadata while generated files exist: without it, CretSpec treats those files as unmanaged.

When a source skill or role disappears or integrations are disabled, only unchanged files recorded as owned are removed. Empty folders of a removed role are removed too. Unrelated resources and directories are preserved. Local Git exclusion entries may remain and are harmless. A completed guidelines adoption remains adopted if a later integration refresh fails; reconcile the reported file and run `cspec sync`.

## Structured output

The new JSON responses have `schemaVersion: 1`. Errors go to stderr and command failures return a nonzero status. Optional future JSON fields may be added; consumers should read the fields they need.

| Command | Top-level fields |
| --- | --- |
| `context --json` | `schemaVersion`, `cliVersion`, `project`, `instructionSources`, `instructionLayers`, `referenceWarnings`, `skillSources`, `skills`, `integration`, `compatibilityNotice`, `roleSource`, `roles`, `roleErrors`, `repositoryStates`, `undeclaredRepositories`, and `role` with `--role` |
| `skill list --json` | `schemaVersion`, `active`, `sharedDrafts`, `draftErrors`, `warnings` |
| `skill create ... --json` | `schemaVersion`, `name`, `scope`, `repository`, `source`, `nextStep` |
| `sync --json` | `schemaVersion`, `written`, `removed`, `unchanged`, `released`, `compatibilityNotice`, `warnings` |

`project` has the same fields as `project info`, including `tree`: `projects` (`name`, `path`, `source`, `depth`, `via`, `context`), `references` (the declaring `project` index, `name`, `type`, `path`, `active`, `priority`, `position`, `scope`, `child` and more) and `warnings`. Repositories in `project` have a `type`. `instructionLayers` lists the workspace's sources in precedence order, each with `label`, `files`, an optional `context` file and `appliesIn` folders when restricted. `skillSources` has `project`, `shared` and `activeShared` paths, and `repositories` mapping each repository with a declared skill directory to its path. Skills have `name`, `description`, `scope` (`project`, `shared`, `repository` or `builtin`), `repository` for repository skills, and `source`; the built-in source is `null` because it is embedded. In working-tree mode, `shared` and `activeShared` point to the same source and `sharedDrafts` is empty. In pinned mode, working-copy entries are a separate editable catalog and may match adopted skills.

`project.lock` is `null` in working-tree mode. `project.guidelines` contains `mode`, `commit` (active HEAD), `branch`, `dirty`, `upstream`, `ahead` and `behind`. Branch and upstream may be `null`; divergence counts are `null` when unavailable. Counts use locally cached upstream refs and say nothing about remote freshness. With local changes, HEAD does not identify the exact active contents. Pinned snapshots have a detached HEAD and no upstream.

`roles` lists each role's `name`, `description` and `entryPoint`; invalid role sources are reported in `roleErrors`. `released` lists generated paths that Git now tracks with other content; sync kept them and stopped generating them. Repository entries in `project` include `skills` when declared. `repositoryStates` has `name`, `path`, `branch`, `head` and `dirty` for the spec repository and every declared repository. `undeclaredRepositories` lists Git repositories in the project root that the definition does not declare. `role` has `name`, `description`, `source`, `entryPoint`, `repositories` (`name`, `path`, `purpose`), `additionalDirectories`, `skills` and `instructions`.

`integration` contains `ready` and `detail`. A context report can succeed with `ready: false`: the sources are readable but generated context needs attention. Use `project doctor --json` when a readiness failure must produce exit status 1. Filesystem access or invalid source errors still fail `context` itself.

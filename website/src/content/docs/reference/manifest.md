---
title: Project and lock files
description: Schema versions 1, 2 and 3, references, repository resolution and generated context.
---

Commit `project.json` in the spec repository root, alongside a `spec/` directory. Schema version 1 names one code repository. [Schema version 2](#schema-version-2) names any number of repositories, and the spec repository may also contain the product code. [Schema version 3](#schema-version-3) adds typed references to further guidelines and to other CretSpec projects. Commit `guidelines.lock.json` only for pinned projects. JSON must be UTF-8; a UTF-8 BOM is accepted. Configuration files are limited to 128 KiB. Unknown fields and unsupported schema versions are rejected.

## project.json

```json
{
  "schemaVersion": 1,
  "name": "Atlas",
  "code": { "repository": "../Atlas" },
  "guidelines": { "repository": "../ai-guidelines", "ref": "main", "mode": "workingTree" },
  "profile": "rust"
}
```

| Field | Meaning |
| --- | --- |
| `schemaVersion` | Must be `1`. |
| `name` | Default outer folder name and required code folder name. Starts with an ASCII letter, then letters, digits, `.`, `_` or `-`. Windows device names and trailing dots are rejected. |
| `code.repository` | Code Git source. |
| `guidelines.repository` | Git source of your shared AI guidelines repository. |
| `guidelines.ref` | Branch name in working-tree mode. In pinned mode, a locally resolvable tag or commit; prefer immutable tags. Starts with a letter/digit and contains only letters, digits, `.`, `_`, `/` or `-`. |
| `guidelines.mode` | Optional `workingTree` or `pinned`. Without it, lock presence selects pinned mode; no lock selects working-tree mode. |
| `profile` | Selects `profiles/<profile>.md` in the active guidelines source. Starts with a letter, followed by letters, digits, `_` or `-`. |
| `agents` | Optional array of unique `codex`, `claude`, `copilot`, `cursor` values. Omission selects all four; `[]` disables generated integrations. Every selection uses the generated `AGENTS.md`; `claude` adds only `.claude/skills/`. See [integrations](/CretSpec/guides/agents/#choose-integrations). |

Spec and guidelines folder names come from their repository basenames, with one trailing `.git` removed. The code folder uses `name`. All three must be distinct, ignoring case, and use portable names.

Existing version 1 definitions with a lock remain pinned. An explicit `pinned` mode requires a lock; an explicit `workingTree` mode requires its absence. Accidental lock deletion therefore fails for explicitly pinned projects. Use `cspec guidelines unlock` instead of editing one file at a time. For old manifests without a mode, retain their lock until intentionally migrating.

The branch in a working-tree manifest must exist locally or in cached `origin` refs. Clone selects it in the new guidelines checkout. Later commands read the actual working tree even if you use Git to change branches or detach HEAD; context and doctor report that state. Changing the manifest's branch does not switch an existing checkout.

Upgrade older executables before using `guidelines.mode`, `agents` or schema version 2; earlier versions reject unknown fields and may require a lock. CretSpec 0.5.0 or later reads schema version 2. The project definition remains solely in this repository.

Relative references resolve against the **spec source**, not the current terminal directory:

| Spec source | Declared source | Resolved source |
| --- | --- | --- |
| `git@github.com:team/Atlas-spec.git` | `../Atlas` | `git@github.com:team/Atlas` |
| `https://git.example.com/team/Atlas-spec.git` | `../ai-guidelines` | `https://git.example.com/team/ai-guidelines` |
| Local `sources/Atlas-spec` repository | `../Atlas` | Sibling `sources/Atlas` repository |

Use full HTTPS or SSH sources for other servers or owners. URLs containing credentials, query parameters or fragments are rejected. Remote Git helper syntax is unsupported. Remote specs cannot reference local absolute paths.

Existing clones are compared by repository identity, not by exact text. SSH and HTTPS addresses of the same GitHub repository match. Azure DevOps addresses match across `ssh.dev.azure.com`, `dev.azure.com` and `<organization>.visualstudio.com`, ignoring case. A user name in an existing HTTPS origin, such as Azure DevOps' `https://organization@dev.azure.com/...`, is ignored for comparison. An origin containing a password is rejected without being displayed.

## Schema version 2

```json
{
  "schemaVersion": 2,
  "name": "Atlas",
  "specRepository": { "name": "atlas", "purpose": "Application code and specification.", "profile": "dotnet" },
  "repositories": [
    { "name": "infra", "url": "../atlas-infra", "purpose": "Infrastructure and GitOps." }
  ],
  "guidelines": { "repository": "../ai-guidelines", "ref": "main" },
  "agents": ["codex", "claude"]
}
```

| Field | Meaning |
| --- | --- |
| `schemaVersion` | `2`. Version 2 has no `code` or top-level `profile`. |
| `name` | Default outer folder name for a new clone. It no longer names a code folder. |
| `specRepository` | Optional description of the repository containing `project.json`: `name` (default `spec`), `purpose` and `profile`. Use it when that repository also contains code. |
| `repositories` | Required list of the other repositories; `[]` when the spec repository contains everything. At most 64. |
| `repositories[].name` | Name used in instructions and roles. 1–64 lowercase letters, digits and single hyphens; unique, including the spec repository's name. |
| `repositories[].url` | Git source, resolved like `code.repository` in version 1. |
| `repositories[].path` | Optional folder in the project root; defaults to `name`. One portable, non-hidden folder name. |
| `repositories[].purpose` | Optional single line, at most 500 characters, describing the repository's function. Shown to agents. |
| `repositories[].profile` | Optional profile that applies to this repository. |
| `repositories[].skills` | Optional skill directory of this repository, relative to it, such as `spec/skills`. At most eight portable, non-hidden folder names separated by `/`. Its skills are committed with the repository. See [repository skills](/CretSpec/reference/skills/#sources-and-validation). Requires CretSpec 0.6.0 or later. |
| `guidelines`, `agents` | Same as version 1. |

Folder names of the spec, guidelines and every repository must be distinct, ignoring case. `roles` is reserved for generated [role entry points](/CretSpec/guides/workspaces/#define-roles). Two members cannot refer to the same repository. Every selected profile must exist in the active guidelines.

## Schema version 3

Version 3 is version 2 plus a `type` for each entry in `repositories`. A `code` entry is a repository as in version 2. A `guidelines` entry adds further guidance next to the project's own `guidelines`, for example a customer's own AI guidelines. A `project` entry references another CretSpec project, whose own references are resolved as well, so the workspace becomes a tree.

```json
{
  "schemaVersion": 3,
  "name": "ardeca-industries",
  "specRepository": { "name": "ardeca-industries", "purpose": "Our specification for the customer." },
  "repositories": [
    { "name": "platform", "url": "git@ssh.dev.azure.com:v3/ardeca/Industries/platform", "purpose": "Infrastructure." },
    { "name": "handbook", "type": "guidelines", "url": "git@ssh.dev.azure.com:v3/ardeca/Industries/handbook",
      "ref": "main", "instructions": ["README.md", "standards/dotnet.md"], "scope": ["platform"] },
    { "name": "ardisq", "type": "project", "url": "git@ssh.dev.azure.com:v3/ardeca/Industries/ardisq",
      "purpose": "The customer's application, with its own specification." }
  ],
  "guidelines": { "repository": "git@github.com:your-account/ai-guidelines.git", "ref": "main" },
  "agents": ["codex", "claude"]
}
```

| Field | Meaning |
| --- | --- |
| `schemaVersion` | `3`. Everything from version 2 stays valid. |
| `repositories[].type` | `code` (the default), `guidelines` or `project`. |
| `repositories[].ref` | Guidelines, required: the branch of a working tree, or the selected version when `commit` pins it. |
| `repositories[].commit` | Guidelines: the full commit ID that pins the reference. The ref must resolve to it. Pinned guidelines are read from a snapshot in the spec's `.local/guidelines/<commit>/`. |
| `repositories[].scope` | Guidelines: names of repositories of this definition where the guidance applies. Omit it to apply the guidance everywhere. Naming a `project` entry includes everything that project brings. |
| `repositories[].instructions` | Guidelines: up to 32 files to read, relative to the repository. Defaults to `README.md`, `guidelines/principles.md`, `guidelines/specification.md` and the profiles the project selects, as far as they exist. |
| `repositories[].skills` | Code: the repository's own skill directory, as in version 2. Guidelines: the skill directory, by default `skills`. Not valid for projects. |
| `repositories[].priority` | Guidelines and projects: an integer, by default `0`. A higher priority comes earlier in the precedence order. Ties keep declaration order, and the project's own `guidelines` come first. A project's priority moves it together with everything it brings. |

`profile` is valid only for code. A `project` entry needs a repository with `project.json` and `spec/`; it may contain code as well. Version 2 definitions with any of these fields are rejected with a request to migrate. CretSpec 0.8.0 or later reads version 3.

### The reference tree

Clone and `attach --clone-missing` clone the whole tree into the workspace root, next to the other repositories. Each project contributes its own guidelines and repositories, and its referenced projects in turn. Relative URLs in a referenced project resolve against that project's own source.

- A repository that appears more than once, recognized by its origin, is cloned once. Pinned guidelines at different commits each get their own snapshot.
- A working tree has one branch. When two references ask for different branches of the same guidelines, the reference closest to the root decides and sync and doctor warn.
- A folder name that a different repository already uses gets the referencing project's folder name as a prefix, for example `ardisq-handbook`.
- A cycle, including a project that references itself directly or through other projects, stops with an error that names the path, before generated files are written.
- A repository deeper in the tree that is missing, unreachable or invalid is reported as a warning; the rest of the workspace keeps working. Missing repositories that this definition declares itself still stop the command, as in version 2.

### Precedence

Each project's own rules come first, followed by its references in priority and declaration order. A referenced project is one entry in that order and expands into its own rules and references. The default order for the example is: the workspace's project rules, `ai-guidelines`, `handbook`, then `ardisq` with its own rules and guidelines. The consumer can thus override or extend a referenced project's rules in its own project rules.

The same order decides skills with the same name: the source with the highest precedence wins, and sync and doctor warn which skill overrides which. Name collisions between the workspace's own spec, repository skill directories and its own `guidelines` still stop synchronization, as in version 2.

### Convert to version 3

Run `cspec project migrate` (or `--preview` first) to convert a version 2 definition. It only changes `schemaVersion`; generated context stays the same. Then add typed references and commit `project.json`. Older executables cannot read the migrated file.

To convert a version 1 project to version 2, set `schemaVersion` to `2`, move `code.repository` into a `repositories` entry with `path` set to the existing code folder, and move `profile` into that entry. Run `cspec sync`; generated files in the existing folder are taken over without conflicts. Older executables cannot read the converted file.

## guidelines.lock.json

This file is optional. In pinned mode, the manifest uses the same `ref` as the lock and `mode: "pinned"`.

```json
{
  "schemaVersion": 1,
  "ref": "v0.4.0",
  "commit": "FULL_COMMIT_ID_OF_THE_SELECTED_TAG"
}
```

Replace the placeholder with a real, complete lowercase hexadecimal commit ID: 40 characters for SHA-1 or 64 for SHA-256. The `ref` must equal the manifest's ref, and resolving it in the guidelines repository must produce this exact commit. `cspec project init --lock --ref <ref>` and `cspec guidelines update <ref>` write it for you.

This lock selects guidelines only. Code and spec clone their normal default branches. Record product releases or implementation evidence separately in your specification.

## Generated context

```text
Atlas-spec/.local/
├── guidelines/<commit>/       pinned guidelines, one snapshot per commit
├── context/<folder>.md        context of each referenced project (schema version 3)
├── project.code-workspace    optional editor file
├── definition-update.json    present only during an update/recovery
├── agents-state.json         generated file ownership
├── agents-pending.json       present during sync/retry
└── agents.lock               process lock
```

Role sources live in the spec's `spec/roles/<name>.md`. Their generated folders are created in the outer project root under `roles/<name>/`; see [roles](/CretSpec/guides/workspaces/#define-roles).

Working-tree mode still uses `.local/` for generated-file ownership and editor settings; it needs no guidelines snapshot. Unlocking preserves existing snapshots.

CretSpec adds a local Git exclusion when needed. `.local/` must not contain tracked files. Snapshots and editor configuration can be regenerated after preserving local changes; an update journal must be recovered before it is removed.

The outer project root has no definition, binding file or Git repository. It can contain generated agent entry points and role folders. Personal settings only expand short source names before cloning.

Agent instructions and skills are also generated in the outer root and sibling repositories. They are derived outputs, not project definitions. Preserve their ownership metadata and follow the [synchronization rules](/CretSpec/reference/skills/#generated-files) when rebuilding them.

See the [project JSON Schema](https://github.com/yveslaurentcreton/CretSpec/blob/main/schemas/project.schema.json) and [lock JSON Schema](https://github.com/yveslaurentcreton/CretSpec/blob/main/schemas/guidelines-lock.schema.json). Runtime validation also checks repository relationships, portable filenames and ref/commit agreement.

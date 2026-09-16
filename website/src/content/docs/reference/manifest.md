---
title: Project and lock files
description: Schema version 1, repository resolution and generated context.
---

Commit both files in the spec repository root, alongside a `spec/` directory. JSON must be UTF-8; a UTF-8 BOM is accepted. Configuration files are limited to 128 KiB. Unknown fields and unsupported schema versions are rejected.

## project.json

```json
{
  "schemaVersion": 1,
  "name": "Atlas",
  "code": { "repository": "../Atlas" },
  "guidelines": { "repository": "../ai-guidelines", "ref": "v0.4.0" },
  "profile": "rust"
}
```

| Field | Meaning |
| --- | --- |
| `schemaVersion` | Must be `1`. |
| `name` | Default outer folder name and required code folder name. Starts with an ASCII letter, then letters, digits, `.`, `_` or `-`. Windows device names and trailing dots are rejected. |
| `code.repository` | Code Git source. |
| `guidelines.repository` | Git source of your shared AI guidelines repository. |
| `guidelines.ref` | Selected tag, commit or locally resolvable ref; prefer immutable tags. Starts with a letter/digit and contains only letters, digits, `.`, `_`, `/` or `-`. |
| `profile` | Selects `profiles/<profile>.md` at the locked version. Starts with a letter, followed by letters, digits, `_` or `-`. |
| `agents` | Optional array of unique `codex`, `claude`, `copilot`, `cursor` values. Omission selects all four; `[]` disables generated integrations. |

Spec and guidelines folder names come from their repository basenames, with one trailing `.git` removed. The code folder uses `name`. All three must be distinct, ignoring case, and use portable names.

Existing version 1 definitions need no migration. Older executables that predate agent integrations reject an explicit `agents` field; upgrade the executable before adding it. Initialization with the new executable writes the selected defaults. The project definition remains solely in this repository.

Relative references resolve against the **spec source**, not the current terminal directory:

| Spec source | Declared source | Resolved source |
| --- | --- | --- |
| `git@github.com:team/Atlas-spec.git` | `../Atlas` | `git@github.com:team/Atlas` |
| `https://git.example.com/team/Atlas-spec.git` | `../ai-guidelines` | `https://git.example.com/team/ai-guidelines` |
| Local `sources/Atlas-spec` repository | `../Atlas` | Sibling `sources/Atlas` repository |

Use full HTTPS or SSH sources for other servers or owners. URLs containing credentials, query parameters or fragments are rejected. Remote Git helper syntax is unsupported. Remote specs cannot reference local absolute paths.

## guidelines.lock.json

```json
{
  "schemaVersion": 1,
  "ref": "v0.4.0",
  "commit": "FULL_COMMIT_ID_OF_THE_SELECTED_TAG"
}
```

Replace the placeholder with a real, complete lowercase hexadecimal commit ID: 40 characters for SHA-1 or 64 for SHA-256. The `ref` must equal the manifest's ref, and resolving it in the guidelines repository must produce this exact commit. `cspec project init` and `cspec guidelines update` write it for you.

This lock selects guidelines only. Code and spec clone their normal default branches. Record product releases or implementation evidence separately in your specification.

## Generated context

```text
Atlas-spec/.local/
├── guidelines/<commit>/       detached adopted snapshot
├── project.code-workspace    optional editor file
├── definition-update.json    present only during an update/recovery
├── agents-state.json         generated file ownership
├── agents-pending.json       present during sync/retry
└── agents.lock               process lock
```

CretSpec adds a local Git exclusion when needed. `.local/` must not contain tracked files. Snapshots and editor configuration can be regenerated after preserving local changes; an update journal must be recovered before it is removed.

The outer project root has no definition, binding file or Git repository. Personal settings only expand short source names before cloning.

Agent instructions and skills are also generated in the outer root and sibling repositories. They are derived outputs, not project definitions. Preserve their ownership metadata and follow the [synchronization rules](/CretSpec/reference/skills/#generated-files) when rebuilding them.

See the [project JSON Schema](https://github.com/yveslaurentcreton/CretSpec/blob/main/schemas/project.schema.json) and [lock JSON Schema](https://github.com/yveslaurentcreton/CretSpec/blob/main/schemas/guidelines-lock.schema.json). Runtime validation also checks repository relationships, portable filenames and ref/commit agreement.

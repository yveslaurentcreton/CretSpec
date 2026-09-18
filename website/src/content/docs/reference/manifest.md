---
title: Project and lock files
description: Schema version 1, repository resolution and generated context.
---

Commit `project.json` in the spec repository root, alongside a `spec/` directory. Commit `guidelines.lock.json` only for pinned projects. JSON must be UTF-8; a UTF-8 BOM is accepted. Configuration files are limited to 128 KiB. Unknown fields and unsupported schema versions are rejected.

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
| `agents` | Optional array of unique `codex`, `claude`, `copilot`, `cursor` values. Omission selects all four; `[]` disables generated integrations. |

Spec and guidelines folder names come from their repository basenames, with one trailing `.git` removed. The code folder uses `name`. All three must be distinct, ignoring case, and use portable names.

Existing version 1 definitions with a lock remain pinned. An explicit `pinned` mode requires a lock; an explicit `workingTree` mode requires its absence. Accidental lock deletion therefore fails for explicitly pinned projects. Use `cspec guidelines unlock` instead of editing one file at a time. For old manifests without a mode, retain their lock until intentionally migrating.

The branch in a working-tree manifest must exist locally or in cached `origin` refs. Clone selects it in the new guidelines checkout. Later commands read the actual working tree even if you use Git to change branches or detach HEAD; context and doctor report that state. Changing the manifest's branch does not switch an existing checkout.

Upgrade older executables before using `guidelines.mode` or `agents`; earlier versions reject unknown fields and may require a lock. The project definition remains solely in this repository.

Relative references resolve against the **spec source**, not the current terminal directory:

| Spec source | Declared source | Resolved source |
| --- | --- | --- |
| `git@github.com:team/Atlas-spec.git` | `../Atlas` | `git@github.com:team/Atlas` |
| `https://git.example.com/team/Atlas-spec.git` | `../ai-guidelines` | `https://git.example.com/team/ai-guidelines` |
| Local `sources/Atlas-spec` repository | `../Atlas` | Sibling `sources/Atlas` repository |

Use full HTTPS or SSH sources for other servers or owners. URLs containing credentials, query parameters or fragments are rejected. Remote Git helper syntax is unsupported. Remote specs cannot reference local absolute paths.

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
├── guidelines/<commit>/       pinned projects only
├── project.code-workspace    optional editor file
├── definition-update.json    present only during an update/recovery
├── agents-state.json         generated file ownership
├── agents-pending.json       present during sync/retry
└── agents.lock               process lock
```

Working-tree mode still uses `.local/` for generated-file ownership and editor settings; it needs no guidelines snapshot. Unlocking preserves existing snapshots.

CretSpec adds a local Git exclusion when needed. `.local/` must not contain tracked files. Snapshots and editor configuration can be regenerated after preserving local changes; an update journal must be recovered before it is removed.

The outer project root has no definition, binding file or Git repository. Personal settings only expand short source names before cloning.

Agent instructions and skills are also generated in the outer root and sibling repositories. They are derived outputs, not project definitions. Preserve their ownership metadata and follow the [synchronization rules](/CretSpec/reference/skills/#generated-files) when rebuilding them.

See the [project JSON Schema](https://github.com/yveslaurentcreton/CretSpec/blob/main/schemas/project.schema.json) and [lock JSON Schema](https://github.com/yveslaurentcreton/CretSpec/blob/main/schemas/guidelines-lock.schema.json). Runtime validation also checks repository relationships, portable filenames and ref/commit agreement.

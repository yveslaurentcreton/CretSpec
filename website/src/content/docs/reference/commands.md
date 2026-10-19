---
title: Commands
description: Complete command and option reference for cspec.
---

Use `cspec --help` or append `--help` at any command level. `cspec --version` reports the installed executable version. With no arguments, CretSpec prints help successfully.

`[location]` defaults to the current directory. Project discovery works from the outer project root, a role folder, the spec, any declared repository or the editable guidelines clone, and their subdirectories. Inaccessible directories are skipped. An explicit spec directory resolves ambiguity.

## Configuration

```text
cspec config namespace [value]
```

Without a value, prints the configured namespace. With a value, saves a GitHub owner, namespace URL, SCP-style namespace or local source directory. This resolves short spec names only.

## Project commands

Clone, attach, info and open also synchronize selected local agent integrations. Existing tracked instructions are preserved; conflicting generated paths are reported. Run `sync` after editing skill sources or changing the selected integrations.

```text
cspec project clone <spec> [destination]
```

Clones the spec first, validates its definition, then clones every declared repository and the editable guidelines and selects the configured branch in the new guidelines checkout, or prepares the locked snapshot for a pinned project. Sources can be a configured short name, full Git URL or explicit local path. The destination must not already exist. Without it, the manifest's project name becomes the outer directory name. Progress goes to stderr; the successful summary goes to stdout.

```text
cspec project init <directory> --name <name> --guidelines <repository>
  (--code <repository> --profile <profile> | [--repository <path[=url]>]... [--profile <profile>])
  [--existing-spec] [--ref <ref>] [--lock]
```

Initializes a spec Git repository from guidelines templates. With `--code`, writes schema version 1 and requires `--profile`. Without it, writes [schema version 2](/CretSpec/reference/manifest/#schema-version-2); each `--repository` declares a sibling folder. `<path>` alone reads the existing sibling clone's origin, removing a user name and refusing a password; `<path>=<url>` declares a repository that is not cloned yet. In version 2, `--profile` applies to the spec repository. `--existing-spec` keeps an existing `spec/` directory and writes only the definition; without it, an existing `spec/` stops initialization.

Defaults to working-tree mode on the remote default branch. `--ref` selects a branch; add `--lock --ref <ref>` to initialize pinned. Does not create remote repositories, commit or push. The wrapped syntax describes one command. See [initialization](/CretSpec/guides/initialize/).

```text
cspec project attach <spec> [--clone-missing]
cspec project attach <code> <spec>
```

Includes existing clones in place. The first form checks every declared repository at its declared path: it must be the root of its own Git repository with a matching origin. All problems are reported together before anything is written. Missing repositories stop the command unless `--clone-missing` clones them. The project root must not be inside a Git repository. The second form is the version 1 syntax and verifies the given code folder.

Attach then prepares missing guideline context and synchronizes agent integrations. It reports each repository's branch and whether local changes were preserved, and warns about undeclared Git repositories in the project root. It never changes branches, commits, origins or tracked files.

```text
cspec project info [location]
```

Prints JSON with `projectRoot`, `spec`, `code` (version 1 only), `specRepository`, `repositories`, `editableGuidelines`, `activeGuidelines`, `manifest`, `lock` and `guidelines`. `specRepository` has `name`, `path`, `purpose` and `profile`. Each repository has `name`, `path`, `source`, `purpose` and `profile`; a version 1 code repository is named `code`. `lock` is nullable; `guidelines` reports mode and local Git state. May clone missing guidelines and generate ignored context. Every declared repository must exist.

```text
cspec project doctor [location] [--json]
```

Checks Git availability, definition, origins and active guidelines (plus lock/snapshot when pinned) without changing files or fetching. `--json` prints an array of `{ "name", "ok", "detail" }` checks to stdout. Missing generated context is a failed check with preparation instructions.

Also checks active skill sources, role sources, generated instructions/resources, Git exclusions and pending synchronization. The `Repositories` check confirms the declared repositories and warns about undeclared Git repositories in the project root. A stale Codex override names the tracked `AGENTS.md` that changed. An `Agent instruction size` warning appears when a generated Codex entry point exceeds Codex's default 32 KiB instruction limit. Working-tree notices report dirty files, a detached or unexpected branch, missing upstream and cached ahead/behind counts without failing an otherwise healthy project. It does not fetch to determine remote freshness, launch an agent or verify its runtime discovery.

```text
cspec project open [location] [--print]
```

Generates the editor workspace inside the spec's `.local/` and launches its OS association. Existing non-folder editor settings are preserved. `--print` generates and prints the path without launching it. Invalid existing JSON is preserved and reported.

## Guidelines commands

Use ordinary Git fetch/pull/commit/push to exchange guidelines changes. CretSpec does not automatically update existing Git checkouts.

```text
cspec guidelines path [location]
cspec guidelines edit [location] [--print]
```

Prints or opens the **editable** guidelines directory. Both may prepare missing context. `edit --print` does not launch an application.

```text
cspec guidelines update <reference> [location] [--preview] [--fetch]
```

Resolves the selected ref and verifies its profile. Without `--preview`, sets pinned mode and writes the matching manifest ref and exact lock commit. `--preview` leaves these two files unchanged but may prepare the selected snapshot. `--fetch` runs `git fetch --tags origin` in the editable guidelines clone. Otherwise, the requested ref must exist locally. Drafts and the editable branch remain unchanged.

The proposed active skills are validated before adoption. Successful adoption refreshes local agent integrations. If that refresh fails, the error distinguishes the already-saved definition from the remaining `cspec sync` repair.

```text
cspec guidelines unlock [location] [--preview]
```

Selects working-tree mode using the editable clone's current branch. Validates the profile and active skills, saves the manifest, removes the lock and refreshes integrations. Requires an attached branch. Preserves local edits and existing snapshots; never fetches or switches the checkout. Preview leaves the definition and integrations unchanged but may prepare a missing pinned snapshot. Review and commit the manifest change and lock deletion in the spec.

```text
cspec guidelines recover [location]
```

Restores the original definition after an interrupted update, provided its files have not independently changed. Requires a local recovery journal.

## Agent context and skills

```text
cspec context [location] [--json] [--role <name>]
```

Read-only workspace context: source paths, repositories with their branch and local-change state, guidelines mode and local Git state, instruction sources, active skills, shared drafts, roles and integration readiness. `--role` adds that role's instructions, focus repositories, the additional directories a restricted host needs, and its skills; an unknown role fails. A readable project with stale integrations produces a report with `integration.ready: false`; use doctor for a failing readiness exit code. Missing snapshots are not created. JSON uses the [versioned context format](/CretSpec/reference/skills/#structured-output).

```text
cspec skill list [location] [--json]
```

Lists active built-in, project, repository and shared skills, with the declaring repository of each repository skill. Working-tree mode reads local shared changes directly; pinned mode also provides a separate draft catalog. Invalid pinned-mode drafts are reported in `draftErrors`; invalid active skills fail the command. No files are changed and no scripts run.

```text
cspec skill create <name> --scope <project|shared|repository> [--repository <name>]
  --description <description> [--location <location>] [--json]
```

Creates a `SKILL.md` scaffold in the spec's `spec/skills/<name>/`, the editable guidelines clone's `skills/<name>/`, or, with `--scope repository --repository <name>`, the declared skill directory of that repository. Missing folders of a declared repository skill directory are created. Both scope and description are required, so automation cannot silently choose ownership. Returns the source path and next step. Refuses reserved/invalid names, active name collisions and existing destinations. It does not synchronize, commit, publish or adopt the new skill. Complete the source before activating it. The wrapped syntax describes one command.

```text
cspec sync [location] [--json]
```

Rebuilds selected agent instructions, role folders and full skill bundles from the current spec and active guidelines source. Uses ordinary files, no symlinks, and preserves existing user content. Removes only unchanged outputs it previously owned. A pending synchronization can be retried with this command. It does not fetch, change the definition or create a missing snapshot; first use `project info` if preparation is missing.

`--json` reports written, removed and unchanged counts plus any compatibility notice. See [generated-file ownership and recovery](/CretSpec/reference/skills/#generated-files).

## Exit status and output

| Status | Meaning |
| --- | --- |
| `0` | Success, including help/version and a valid doctor report |
| `1` | Operation failure or failed diagnostic check |
| `2` | Invalid syntax or missing required argument |

Errors go to stderr. JSON-producing commands keep stdout machine-readable, including `doctor --json` when checks fail. Parse documented JSON outputs for scripts; text help and progress can improve between releases.

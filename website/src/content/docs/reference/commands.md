---
title: Commands
description: Complete command and option reference for cspec.
---

Use `cspec --help` or append `--help` at any command level. `cspec --version` reports the installed executable version. With no arguments, CretSpec prints help successfully.

`[location]` defaults to the current directory. Project discovery works from the outer project root, code, spec or editable guidelines clone and their subdirectories. An explicit spec directory resolves ambiguity.

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

Clones the spec first, validates its definition, then clones code and editable guidelines and prepares the locked snapshot. Sources can be a configured short name, full Git URL or explicit local path. The destination must not already exist. Without it, the manifest's project name becomes the outer directory name. Progress goes to stderr; the successful summary goes to stdout.

```text
cspec project init <directory> --name <name> --code <repository>
  --guidelines <repository> --ref <ref> --profile <profile>
```

Initializes an existing, uninitialized spec Git repository from a guidelines version. All options are required. Does not create remote repositories, commit or push. The wrapped example above describes one command. See [initialization](/CretSpec/guides/initialize/).

```text
cspec project attach <code> <spec>
```

Validates correctly placed existing code and spec repositories, then prepares missing guideline context. Refuses unrelated origins or incorrect sibling names.

```text
cspec project info [location]
```

Prints JSON with `projectRoot`, `spec`, `code`, `editableGuidelines`, `activeGuidelines`, `manifest` and `lock`. May clone missing guidelines and generate ignored context. It requires the code clone to exist.

```text
cspec project doctor [location] [--json]
```

Checks Git availability, definition, origins, lock and prepared snapshot without changing files or fetching. `--json` prints an array of `{ "name", "ok", "detail" }` checks to stdout. Missing generated context is a failed check with preparation instructions.

Also checks active skill sources, generated instructions/resources, Git exclusions and pending synchronization. It does not launch an agent or verify its runtime discovery.

```text
cspec project open [location] [--print]
```

Generates the editor workspace inside the spec's `.local/` and launches its OS association. Existing non-folder editor settings are preserved. `--print` generates and prints the path without launching it. Invalid existing JSON is preserved and reported.

## Guidelines commands

```text
cspec guidelines path [location]
cspec guidelines edit [location] [--print]
```

Prints or opens the **editable** guidelines directory. Both may prepare missing context. `edit --print` does not launch an application.

```text
cspec guidelines update <reference> [location] [--preview] [--fetch]
```

Resolves the selected ref and verifies its profile. Without `--preview`, writes the matching manifest ref and exact lock commit. `--preview` leaves these two files unchanged but may prepare the selected snapshot. `--fetch` runs `git fetch --tags origin` in the editable guidelines clone. Otherwise, the requested ref must exist locally. Drafts and the editable branch remain unchanged.

The proposed active skills are validated before adoption. Successful adoption refreshes local agent integrations. If that refresh fails, the error distinguishes the already-saved definition from the remaining `cspec sync` repair.

```text
cspec guidelines recover [location]
```

Restores the original definition after an interrupted update, provided its files have not independently changed. Requires a local recovery journal.

## Agent context and skills

```text
cspec context [location] [--json]
```

Read-only workspace context: source paths, exact guideline selection, instruction sources, active skills, shared drafts and integration readiness. A readable project with stale integrations produces a report with `integration.ready: false`; use doctor for a failing readiness exit code. Missing snapshots are not created. JSON uses the [versioned context format](/CretSpec/reference/skills/#structured-output).

```text
cspec skill list [location] [--json]
```

Lists active built-in, project and adopted shared skills, plus a separate shared working-copy catalog. Invalid shared drafts are reported in `draftErrors` without changing adopted skills. No files are changed and no scripts run.

```text
cspec skill create <name> --scope <project|shared> --description <description>
  [--location <location>] [--json]
```

Creates a `SKILL.md` scaffold in the spec's `spec/skills/<name>/` or editable guidelines clone's `skills/<name>/`. Both scope and description are required, so automation cannot silently choose ownership. Returns the source path and next step. Refuses reserved/invalid names, active name collisions and existing destinations. It does not synchronize, commit, publish or adopt the new skill. Complete the source before activating it. The wrapped syntax describes one command.

```text
cspec sync [location] [--json]
```

Rebuilds selected agent instructions and full skill bundles from the current spec and adopted snapshot. Uses ordinary files, no symlinks, and preserves existing user content. Removes only unchanged outputs it previously owned. A pending synchronization can be retried with this command. It does not fetch, change the definition or create a missing snapshot; first use `project info` if preparation is missing.

`--json` reports written, removed and unchanged counts plus any compatibility notice. See [generated-file ownership and recovery](/CretSpec/reference/skills/#generated-files).

## Exit status and output

| Status | Meaning |
| --- | --- |
| `0` | Success, including help/version and a valid doctor report |
| `1` | Operation failure or failed diagnostic check |
| `2` | Invalid syntax or missing required argument |

Errors go to stderr. JSON-producing commands keep stdout machine-readable, including `doctor --json` when checks fail. Parse documented JSON outputs for scripts; text help and progress can improve between releases.

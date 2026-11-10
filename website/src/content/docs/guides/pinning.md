---
title: Pin guidelines
description: Fix a project to an exact guidelines version, and return to the local clone.
---

By default a project reads the guidelines clone next to its code, including uncommitted changes. Pin a version when a project needs a reproducible selection of rules and skills. Most projects do not need this.

## Pin a version

```sh
cspec guidelines update v0.5.0 --fetch --preview
cspec guidelines update v0.5.0
cspec project doctor
```

Replace the tag with your own. `--fetch` runs `git fetch --tags origin` in the guidelines clone; without it the tag must exist locally. `--preview` validates the version and shows a summary without changing the project.

The update records the tag and its exact commit in `project.json` and `guidelines.lock.json`. Commit both in the spec. Prefer tags that never move: a tag that no longer matches its lock fails validation.

A pinned project reads a snapshot under the spec's ignored `.local/guidelines/<commit>/`. Your guidelines clone stays editable. Its uncommitted rules and skills are drafts that the project ignores until you adopt a new version. The lock does not pin your code, your spec or the installed CLI.

Run the same command with an earlier tag to go back to that version. To start a project pinned, use `cspec project init --lock --ref <tag>`.

## Return to the local clone

```sh
cspec guidelines unlock --preview
cspec guidelines unlock
cspec project doctor
```

Unlock uses the branch that the guidelines clone is on. It keeps your local edits and existing snapshots, and it never fetches or switches a checkout. Commit the changed `project.json` and the removed lock in the spec.

If a pin or unlock saved the definition but could not refresh the generated files, the error says so. Fix the reported file, then run `cspec sync`. The [command reference](/CretSpec/reference/commands/#guidelines-commands) and the [lock file reference](/CretSpec/reference/manifest/#guidelineslockjson) hold the details.

## Recover an interrupted update

Before it changes the definition, CretSpec saves a journal under `.local/definition-update.json`. An interrupted pin or unlock blocks other project commands until you recover:

```sh
cspec guidelines recover
cspec sync
```

Recovery restores the original `project.json` and lock, including a lock that was absent. If you edited either file afterwards, recovery refuses to overwrite it; reconcile your edits first. Recovery rolls back an interrupted operation. It does not undo a completed pin or unlock.

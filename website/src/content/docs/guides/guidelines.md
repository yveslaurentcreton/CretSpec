---
title: Work with guidelines
description: Edit local guidelines, share them with Git and pin a version when needed.
---

## Edit and use locally

```sh
cspec guidelines path
cspec guidelines edit
```

These select the editable guidelines clone next to your code. In the default working-tree mode, it is also the active source: local rule edits are readable immediately, including uncommitted changes. After changing a skill, run `cspec sync` to refresh its generated copies. An agent that already loaded a file may need to reread it or start a new session.

## Share with ordinary Git

From the outer project folder, using your guidelines repository's name:

```sh
git -C ai-guidelines status
git -C ai-guidelines fetch
git -C ai-guidelines pull --ff-only
cspec sync
```

Review and commit your local improvements in that repository, then push them when ready. Each project has its own clone; repeat fetch/pull and sync in another project to use published changes there. Uncommitted edits remain local to the current clone.

Clone retrieves the repositories and prepares skills once. Info, open, context, doctor and sync never fetch, pull, merge or switch an existing checkout. `sync` refreshes local generated files; it does not synchronize Git repositories. If branches diverge, resolve that with your normal Git workflow before pulling.

`cspec context --json` reports the active branch, HEAD, local changes and cached upstream divergence. Doctor reports local-state notices. Neither checks whether the remote has newer commits. A dirty working tree's HEAD does not identify its exact contents.

## Optionally pin a version

Pinning selects committed shared rules and skills independently of the editable working tree. Use it when a project needs a reproducible guidelines selection. Existing locked projects remain pinned until explicitly unlocked.

```sh
cspec guidelines update v0.5.0 --fetch --preview
cspec guidelines update v0.5.0
cspec project doctor
```

Replace the tag with your own. Only the explicit `--fetch` requests `git fetch --tags origin`; otherwise the ref must be available locally. Preview validates the profile and skills and shows a committed-file diff summary. Inspect the actual changes with Git; that summary does not include uncommitted edits. Preview can create a snapshot, but leaves the definition and integrations unchanged.

Applying the update sets `guidelines.mode` to `pinned`, records the ref and exact commit, and refreshes agent instructions and skills. Review and commit `project.json` and `guidelines.lock.json` in the spec. Prefer immutable tags; a ref that no longer matches its lock fails validation. The editable branch and local drafts are preserved. Other projects are unaffected.

Use the same update command with an earlier supported tag to return to that version. Pinned initialization is also available with `project init --lock --ref <ref>`.

## Return to local guidelines

```sh
cspec guidelines unlock --preview
cspec guidelines unlock
cspec project doctor
```

Unlock uses the editable clone's current branch after validating its profile and skills. Select a branch with Git first if HEAD is detached. It sets `mode: "workingTree"`, records that branch, removes the lock and refreshes integrations. Review and commit the manifest change and lock deletion in the spec.

Local edits and existing snapshots are preserved. Preview leaves the definition and integrations unchanged, though a missing pinned snapshot may be prepared. No fetch or checkout is performed. Changing a branch in the manifest later does not switch an existing checkout; use Git for that.

If a completed pin or unlock saved the definition but agent refresh encounters an edited output, the error says so. Preserve and reconcile the reported file, then run `cspec sync`.

## Recover an interrupted update

Before changing the definition, CretSpec saves a journal under `.local/definition-update.json`. An interrupted pin or unlock blocks ordinary project commands until recovery:

```sh
cspec guidelines recover
cspec sync
```

Recovery restores the original manifest and lock presence, including an originally absent lock. If either file was independently edited afterward, recovery refuses to overwrite those edits. Preserve and reconcile them first. Recovery rolls back an interrupted operation; it does not undo a completed adoption or mode change.

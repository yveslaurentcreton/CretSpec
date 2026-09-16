---
title: Work with guidelines
description: Propose shared improvements and explicitly adopt a guidelines release.
---

## Edit from any project

```sh
cspec guidelines path
cspec guidelines edit
```

These select the editable guidelines clone next to your code. Use your normal Git workflow there: create a branch, edit, commit and propose the change. Fetch or merge shared changes explicitly. Each project has an independent editable clone.

## Preview a release

After the shared change has a tag, run:

```sh
cspec guidelines update v0.5.0 --fetch --preview
```

This fetches tags from the editable clone's origin, resolves the requested commit, validates its profile and displays a file-change summary. The manifest and lock stay unchanged. Preview may create a local snapshot; `--fetch` updates Git remote refs. It does not change the editable clone's branch or discard drafts.

Inspect the actual changes in that clone when needed:

```sh
git diff v0.4.0 v0.5.0 -- guidelines profiles templates
```

## Adopt the version

```sh
cspec guidelines update v0.5.0
cspec project doctor
```

Review and commit both `project.json` and `guidelines.lock.json` in your spec repository. Other projects keep their current locks. The requested ref must exist locally unless you add `--fetch`. Prefer immutable tags; reusing a tag for another commit fails the lock check.

To return to an earlier supported version, run the same update command with its tag, review the change and commit both files.

## Recover an interrupted update

CretSpec records the original definition in `.local/definition-update.json` before changing its two files. An interrupted update blocks ordinary project commands until you run:

```sh
cspec guidelines recover
```

Recovery restores the original pair and removes the journal. If either file was independently edited afterward, recovery refuses to overwrite those edits. Preserve and reconcile them first. Recovery rolls back an interrupted operation; it does not undo a completed, committed adoption.

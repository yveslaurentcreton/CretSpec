---
title: Move or attach a project
description: Use existing clones and move projects without a registry.
---

## Attach existing clones

Put the repositories next to each other in an ordinary folder. Each uses the folder that `project.json` declares, and the guidelines folder is named after its repository:

```text
Atlas/
├── atlas-spec/
├── atlas/
└── ai-guidelines/
```

From that folder:

```sh
cspec project attach atlas-spec
```

Attach checks the folder and origin of every declared repository before it writes anything. It never moves a repository, switches a branch, rewrites a remote or discards local changes, and it keeps instructions that a repository already tracks. A missing repository stops the command; add `--clone-missing` to clone it. A missing guidelines clone is prepared from its declared source.

## Move a project

Close programs that hold files open, then move the outer folder. Keep the repositories together. Run `cspec project doctor` in the new location, and `cspec project open --print` to regenerate the editor file.

Moving folders does not change Git remotes. Local Git sources must stay where those remotes point.

## Choose another folder name

```sh
cspec project clone atlas-spec atlas-review
```

Only the outer folder gets the other name. The destination must not exist yet.

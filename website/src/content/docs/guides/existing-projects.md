---
title: Move or attach a project
description: Use existing clones and move projects without a registry.
---

## Move a complete project

Close programs holding files open, then move the outer project directory with your file manager. Keep code, spec and editable guidelines together. Run `cspec project doctor` from the new location. Regenerate the optional editor file with `cspec project open --print`.

Relative references identify remote sources relative to the spec's origin. Moving local folders does not alter Git remotes. For purely local Git sources, those origins must still point to valid source repositories when you fetch or clone again.

## Attach existing clones

Arrange your repositories as siblings inside an ordinary directory. The code directory must match `project.json`'s `name`; the guidelines directory uses its source basename. Origins must match the sources declared in the spec.

```text
Atlas/
├── Atlas-spec/
├── Atlas/
└── CretAI/
```

From the outer directory:

```sh
cspec project attach Atlas Atlas-spec
```

Attach validates placement and origins and prepares missing guideline context. It does not relocate your repositories, rewrite remotes or discard local changes. A missing editable guidelines clone can be prepared from its declared source.

## Choose a different outer directory name

```sh
cspec project clone Atlas-spec Atlas-review
```

Only the outer folder name changes. The code directory inside remains `Atlas`, as specified by the manifest. CretSpec requires a new destination to avoid mixing projects.

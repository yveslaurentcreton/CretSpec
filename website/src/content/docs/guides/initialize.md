---
title: Start a specification
description: Create a project definition and Markdown specification from shared templates.
---

Create the code repository and spec repository on your Git host first. Commit at least an initial file in the code repository so it has a branch to clone. Clone the new spec locally. CretSpec does not create remote repositories or commit on your behalf.

Your AI guidelines repository's selected branch or pinned version must contain `profiles/<profile>.md` and these Markdown templates under `templates/spec/spec/`:

```text
vision.md          scope.md
requirements.md    architecture.md
decisions.md       roadmap.md
acceptance.md      project-rules.md
```

Templates may include subdirectories and additional Markdown files. Hidden files and symlinks are rejected. The limit is 128 Markdown files, 128 KiB per file and eight subdirectory levels.

## Initialize

From the directory containing the spec clone, run this as one command on any platform. Replace repository names and profile with your own:

```sh
cspec project init Atlas-spec --name Atlas --code ../Atlas --guidelines ../ai-guidelines --profile rust
```

Relative sources resolve against the spec's Git origin. Without an origin, they resolve against its local repository path; this is useful for a local trial.

By default CretSpec uses the guidelines remote's default branch, copies its specification templates and writes `project.json` with `mode: "workingTree"`. No lock is created. Use `--ref <branch>` to select another branch. It validates the selected profile before writing. Existing README and license files are preserved. Existing definition files or a `spec/` directory cause initialization to stop.

For a pinned project, add `--lock --ref v0.4.0` using your own published tag. This also creates `guidelines.lock.json` at the exact selected commit. `--ref` alone selects a branch; it does not enable pinning.

## Define your project

Edit the copied Markdown to describe your product. Use your guidelines' method for requirements, milestones, stories, decisions and evidence. Review the manifest, then commit and push from the spec repository:

```sh
cd Atlas-spec
git add project.json spec
git commit -m "docs: define Atlas"
git push
```

For pinned initialization, also stage `guidelines.lock.json`.

From your projects directory, clone that spec with `cspec project clone <spec-source>`. An existing outer destination is never replaced; choose another destination for a trial.

## Create a definition manually

You can also author [project.json](/CretSpec/reference/manifest/) and `spec/` directly. Set a valid guidelines branch and `mode: "workingTree"` for the default workflow. CretSpec needs no special bootstrap service. Prefer the CLI for creating or removing an optional pin so the manifest and lock remain consistent.

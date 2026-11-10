---
title: Start your own project
description: Three repositories and one small file are enough.
---

A project needs a spec repository, your code and a guidelines repository. Each can start almost empty. The [demo](/CretSpec/start/quickstart/) is built exactly this way.

## 1. Create the repositories

Create three repositories on your Git host, or reuse the ones you have, and clone them next to each other:

```text
atlas-spec/       the specification
atlas/            your code
ai-guidelines/    the rules you share between projects
```

Write your shared rules in `ai-guidelines/README.md`, for example:

```md
# AI guidelines

- Make the smallest change that solves the problem.
- Run the tests before you say that a change works.
```

Every repository needs at least one commit. Commit and push.

## 2. Describe the project

In `atlas-spec`, create `project.json`:

```json
{
  "schemaVersion": 3,
  "name": "Atlas",
  "repositories": [
    { "name": "atlas", "url": "../atlas", "purpose": "The product code." }
  ],
  "guidelines": { "repository": "../ai-guidelines", "ref": "main" }
}
```

A relative source such as `../atlas` means the repository with that name beside the spec on your Git host. Use a full URL for a repository elsewhere. The `purpose` is shown to your agents.

Add a `spec/` folder with the rules of this project in `spec/project-rules.md`. Commit and push.

## 3. Prepare the workspace

The three clones already sit next to each other, so attach them in place:

```sh
cspec project attach atlas-spec
```

On another machine, one command clones everything:

```sh
cspec project clone git@github.com:your-account/atlas-spec.git
```

Start your agent and [work with it](/CretSpec/guides/agents/). Every field of `project.json` is in the [reference](/CretSpec/reference/manifest/).

## Or start from templates

`cspec project init` writes `project.json` for you and copies specification templates from your guidelines. The guidelines then need these files under `templates/spec/spec/`:

```text
vision.md          scope.md
requirements.md    architecture.md
decisions.md       roadmap.md
acceptance.md      project-rules.md
```

Run it next to a cloned, still empty spec repository:

```sh
cspec project init atlas-spec --name Atlas --guidelines ../ai-guidelines --repository atlas
```

`--repository` reads the origin of the sibling clone `atlas`. Repeat it for more repositories, or write `<path>=<url>` for one that is not cloned yet. Edit the copied Markdown, add a `purpose` to each repository, then commit and push `project.json` and `spec/`.

CretSpec never creates remote repositories and never commits for you. The [command reference](/CretSpec/reference/commands/#project-commands) lists every option, such as `--profile`, `--existing-spec` and `--lock`.

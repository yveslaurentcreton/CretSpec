---
title: Work with guidelines
description: Edit your shared rules in any project and share them with Git.
---

Every project has its own clone of your guidelines, next to the code. That clone is what your agents read.

## Edit

Change a rule in the guidelines clone and it applies at once, even before you commit. After you change a skill, run `cspec sync`.

```sh
cspec guidelines path
cspec guidelines edit
```

The first prints the clone's location. The second opens it. An agent that already read a file may need to read it again or start a new session.

## Share

The guidelines are an ordinary Git repository. Commit and push your improvements there. To get the changes of another project or a colleague:

```sh
git -C ai-guidelines pull --ff-only
cspec sync
```

CretSpec never fetches, pulls or switches an existing clone for you. `cspec context --json` shows the branch and local changes that are active.

## Go further

- [Add a second set of guidelines](/CretSpec/guides/workspaces/#reference-guidelines-and-other-projects), such as a customer's, next to your own.
- [Pin a version](/CretSpec/guides/pinning/) when a project needs a fixed, reproducible selection.

---
title: How it works
description: Understand the three repositories and the exact guidelines lock.
---

CretSpec connects three ordinary Git repositories. The specification is the starting point.

| Repository | Responsibility |
| --- | --- |
| Project spec | Product intent, requirements, decisions and project definition |
| Product code | Buildable, testable product and public documentation |
| Shared guidelines | Reusable development principles, profiles, skills and templates |

`project.json` belongs in the spec. It names the code and guidelines sources, a guideline ref and a profile. `guidelines.lock.json` records the exact guidelines commit. The lock does not pin your product code, specification revision or installed CLI version.

## Editable and adopted guidelines

The guidelines clone next to your code is editable. It is where you propose improvements, create branches and publish shared changes using Git.

The spec's ignored `.local/guidelines/<commit>/` directory is a separate, detached checkout at the locked commit. It provides stable guidance while you work on a proposal. CretSpec rejects modified snapshots instead of silently treating local edits as adopted guidance.

Publishing a new guidelines release does not change every project. Each project explicitly [adopts a version](/CretSpec/guides/guidelines/).

## Method and mechanism

Your guidelines can describe how requirements, milestones, stories, decisions and verification evidence work. They can contain language profiles, development principles and reusable skills. CretAI is the shared-guidelines repository used by this project's maintainer; CretSpec accepts another repository with the same supported structure.

CretSpec initializes Markdown from templates and prepares repository context. It does not decide story status, implement requirements or automatically activate skills in an editor. Use the adopted guidance to direct that work.

## Portable by construction

Commands discover the spec from the enclosing project and sibling repositories. No personal project registry is needed. Move the whole outer directory to another location and keep the three repositories together. Git origins continue to identify their sources.

Public product builds do not need access to private specifications or guidelines. Keep internal planning in the spec and document the public product in its code repository.

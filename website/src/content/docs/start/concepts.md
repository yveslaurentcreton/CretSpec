---
title: How it works
description: Understand the three repositories and the exact guidelines lock.
---

CretSpec connects three ordinary Git repositories. The specification is the starting point.

| Repository | Responsibility |
| --- | --- |
| Project spec | Product intent, requirements, decisions and project definition |
| Product code | Buildable, testable product and public documentation |
| AI guidelines | Shared development principles, profiles, skills and specification templates |

`project.json` belongs in the spec. It names the code and guidelines sources, a guideline ref and a profile. `guidelines.lock.json` records the exact guidelines commit. The lock does not pin your product code, specification revision or installed CLI version.

## Editable and adopted guidelines

The guidelines clone next to your code is editable. It is where you propose improvements, create branches and publish shared changes using Git.

The spec's ignored `.local/guidelines/<commit>/` directory is a separate, detached checkout at the locked commit. It provides stable guidance while you work on a proposal. CretSpec rejects modified snapshots instead of silently treating local edits as adopted guidance.

Publishing a new guidelines release does not change every project. Each project explicitly [adopts a version](/CretSpec/guides/guidelines/).

## Method and mechanism

The AI guidelines repository defines your shared way of working: development principles, language profiles, reusable skills and how requirements, milestones, stories, decisions and verification evidence are organized. CretSpec works with any guidelines repository that provides the supported profile and template structure. Its name and location are defined in your spec; `ai-guidelines` is the example name used throughout these docs.

CretSpec initializes Markdown from templates and prepares instructions and discoverable skills for supported coding agents. Its built-in workflow skill explains how to use the spec and where to store new knowledge. Your agent follows the adopted method; CretSpec does not decide story status or implement requirements. See [working with your coding agent](/CretSpec/guides/agents/).

## Portable by construction

Commands discover the spec from the enclosing project and sibling repositories. No personal project registry is needed. Move the whole outer directory to another location and keep the three repositories together. Git origins continue to identify their sources.

Public product builds do not need access to private specifications or guidelines. Keep internal planning in the spec and document the public product in its code repository.

---
title: How it works
description: Understand the three repositories, local guidelines and optional pinning.
---

CretSpec connects three ordinary Git repositories. The specification is the starting point.

| Repository | Responsibility |
| --- | --- |
| Project spec | Product intent, requirements, decisions and project definition |
| Product code | Buildable, testable product and public documentation |
| AI guidelines | Shared development principles, profiles, skills and specification templates |

`project.json` belongs in the spec. It names the code and guidelines sources, a guidelines branch and a profile. New projects use that editable guidelines checkout directly. An optional `guidelines.lock.json` selects an exact commit instead.

## Local guidelines by default

The guidelines clone next to your code is the active source. Local edits to rules are readable immediately, including uncommitted changes. Run `cspec sync` after editing skills to refresh the generated copies your agents discover.

Clone fetches the repositories once. After that, use ordinary Git fetch/pull/commit/push to exchange changes. Opening a project or running info, context, doctor or sync never fetches, pulls or switches an existing guidelines checkout. Each project has its own clone.

## Pin a version when needed

A pinned project reads a detached checkout under the spec's ignored `.local/guidelines/<commit>/`. Its lock selects shared rules and skills; local guideline drafts stay separate. The lock does not pin your product code, spec revision or installed CLI.

Existing locked projects stay pinned. Use [guidelines update or unlock](/CretSpec/guides/guidelines/) to switch explicitly. CretSpec preserves drafts and existing snapshots during a mode change.

## Method and mechanism

The AI guidelines repository defines your shared way of working: development principles, language profiles, reusable skills and how requirements, milestones, stories, decisions and verification evidence are organized. CretSpec works with any guidelines repository that provides the supported profile and template structure. Its name and location are defined in your spec; `ai-guidelines` is the example name used throughout these docs.

CretSpec initializes Markdown from templates and prepares instructions and discoverable skills for supported coding agents. Its built-in workflow skill explains how to use the spec and where to store new knowledge. Your agent follows the active method; CretSpec does not decide story status or implement requirements. See [working with your coding agent](/CretSpec/guides/agents/).

## Portable by construction

Commands discover the spec from the enclosing project and sibling repositories. No personal project registry is needed. Move the whole outer directory to another location and keep the three repositories together. Git origins continue to identify their sources.

Public product builds do not need access to private specifications or guidelines. Keep internal planning in the spec and document the public product in its code repository.

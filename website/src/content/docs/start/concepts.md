---
title: How it works
description: Understand the repositories, local guidelines and optional pinning.
---

CretSpec connects ordinary Git repositories. The specification is the starting point.

| Repository | Responsibility |
| --- | --- |
| Project spec | Product intent, requirements, decisions and project definition |
| Product code | Buildable, testable product and public documentation |
| AI guidelines | Shared development principles, profiles, skills and specification templates |

A workspace can [combine several repositories](/CretSpec/guides/workspaces/), such as an application and its infrastructure, and the specification can live inside a code repository.

## Where knowledge lives

Knowledge has three levels. Put it at the narrowest level that owns it.

| Level | Repository | Rules and agreements | Skills | Specification |
| --- | --- | --- | --- | --- |
| Shared | The AI guidelines | `guidelines/` and technology `profiles/` | `skills/` | The specification method, not concrete specs |
| Workspace | The spec | `spec/project-rules.md`, which takes precedence over shared guidance | `spec/skills/` | `spec/`: vision, scope, decisions, requirements, roadmap and stories |
| Repository | A declared member repository | Its own tracked `AGENTS.md` | Its declared skill directory, such as `spec/skills/` | Its own documents, which CretSpec does not manage; project rules can make them authoritative for that repository |

Ask whether knowledge applies to all your work, to this workspace, or to one repository's own code and method; the last kind travels with that repository to every workspace that includes it. [Roles](/CretSpec/guides/workspaces/#define-roles) are not a fourth level: they live in the spec and add a focus to the same knowledge. Generated instructions and skill copies are derived from these sources and are never edited.

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

Commands discover the spec from the enclosing project and sibling repositories. No personal project registry is needed. Move the whole outer directory to another location and keep the repositories together. Git origins continue to identify their sources.

Public product builds do not need access to private specifications or guidelines. Keep internal planning in the spec and document the public product in its code repository.

---
title: Work with your coding agent
description: Open a prepared workspace, use its skills and save knowledge in the right repository.
---

Cloning a spec prepares local instructions and skills for Codex, Claude Code, GitHub Copilot in VS Code and Cursor. CretSpec supplies the workspace context; your existing agent does the work.

## Start working

```sh
cspec project clone Atlas-spec
cd Atlas
cspec project doctor
cspec project open
```

Open your agent in the outer workspace or one of its three repositories. CretSpec prepares entry points at each location. Allow access to sibling repositories when your host requires it. You can ask:

> Read this project's active guidelines and specification, then explain the next agreed story.

The generated instructions identify the source repositories, selected profile and active guidelines mode. The built-in `cspec-workspace` skill explains the workflow and CLI. Your shared guidelines define your development principles and specification method; your spec defines concrete requirements, milestones, stories, decisions and evidence.

For an existing workspace, run `cspec project info` once with the new executable. It prepares missing integrations and, for pinned projects, snapshots. Use `cspec sync` for subsequent source changes without fetching or adopting another guidelines version.

## Save an approach

Ask your agent to turn a useful approach into a skill. If the intended scope is unclear, the workspace instructions direct it to ask whether the skill belongs to this project or should be shared. An ordinary agreement can stay in project rules or shared guidelines without becoming a skill.

| Scope | Editable source | When it becomes available |
| --- | --- | --- |
| Project | `Atlas-spec/spec/skills/<name>/` | After `cspec sync` |
| Shared, working-tree mode | `ai-guidelines/skills/<name>/` | After `cspec sync`, including uncommitted changes |
| Shared, pinned mode | `ai-guidelines/skills/<name>/` | After explicitly adopting the committed guidelines version |
| Built-in | Included in the installed CretSpec executable | When its integrations are synchronized |

```sh
cspec skill create query-validation --scope project --description "Validate this project's query inputs and expected results"
cspec skill create rust-release --scope shared --description "Prepare and verify a native Rust CLI release"
```

These commands create a scaffold. Complete its `SKILL.md`, add any required resources, and review the instructions. They do not generate the finished workflow or execute scripts.

```sh
cspec skill list
cspec sync
```

Project skills always use the current spec checkout. Shared skills use the editable guidelines checkout by default; `skill list` includes local changes in its active catalog. Editing a guideline needs no commit to make its contents readable. An already running agent may need to reread the source; skills may need a new session after synchronization, depending on the host.

Clone retrieves shared skills with the guidelines repository and prepares their generated copies. Later, use Git fetch/pull inside that repository to bring in changes from another project, then `cspec sync`. CretSpec performs no automatic Git update.

In pinned mode, shared working-copy skills are listed separately as drafts. The lock selects shared rules and skills together. Follow [guidelines management](/CretSpec/guides/guidelines/) to pin or unlock explicitly.

## Choose integrations

The optional `agents` array in the spec's `project.json` selects integrations:

```json
"agents": ["codex", "copilot", "cursor"]
```

Omit it to prepare all four integrations. Use `[]` to disable generated integrations, then run `cspec sync`. CretSpec removes only files it owns and refuses to discard locally edited outputs.

| Agent | Instruction entry point | Skill directory |
| --- | --- | --- |
| Codex | `AGENTS.md`, or a generated `AGENTS.override.md` preserving existing instructions | `.agents/skills/` |
| Claude Code | `CLAUDE.md`, or `CLAUDE.local.md` alongside an existing file | `.claude/skills/` |
| GitHub Copilot in VS Code | `.github/instructions/cspec.instructions.md` | Shares a generated skills directory |
| Cursor | `.cursor/rules/cspec.mdc` | Shares a generated skills directory |

Copilot and Cursor can read both compatibility directories. CretSpec creates `.agents/skills/` when Codex is selected, `.claude/skills/` when Claude is selected, and only `.agents/skills/` when neither is selected. Selecting both Codex and Claude requires both directories; compatibility readers may show duplicate skills. Select the integrations you use if this occurs. Opening the outer multi-repository workspace may also surface skills from more than one repository; open the relevant repository to narrow discovery.

These are local filesystem integrations, not installations of the agents. Discovery and invocation depend on the host version, trust settings and skill support. The CLI tests generated files and paths; it does not certify that an installed editor loaded or invoked a skill. Remote/cloud agents need their own prepared workspace with access to the spec and guidelines; local excluded files are not delivered through the public code repository.

## Keep sources authoritative

Generated instructions and full skill bundles are regular files, so Windows needs no symlink privileges. They are excluded locally through Git's `info/exclude`; public product builds have no private-context dependency. CretSpec does not change global agent settings.

Edit a skill's source, then synchronize. Existing `AGENTS.md` and `CLAUDE.md` files are preserved. Conflicting override/local files, tracked generated files or edited output stop synchronization with a useful error. Preserve those edits in the appropriate source before removing the reported generated file and retrying.

`cspec context --json` reports sources and readiness. `cspec project doctor --json` checks without repairing anything. A missing or stale integration can be repaired with `cspec sync`; a missing snapshot first needs `cspec project info`.

See the [skill format](/CretSpec/reference/skills/) and [CLI reference](/CretSpec/reference/commands/). Host behavior is described in the official [Codex instructions](https://learn.chatgpt.com/docs/agent-configuration/agents-md), [Codex skills](https://learn.chatgpt.com/docs/build-skills), [Claude memory](https://code.claude.com/docs/en/memory), [Claude skills](https://code.claude.com/docs/en/skills), [VS Code skills](https://code.visualstudio.com/docs/agent-customization/agent-skills) and [Cursor skills](https://prod.cursor.com/docs/skills) documentation.

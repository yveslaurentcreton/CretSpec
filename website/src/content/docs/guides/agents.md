---
title: Work with your coding agent
description: Start your agent in a prepared workspace and save what works as a skill.
---

CretSpec prepares instructions and skills for Codex, Claude Code, GitHub Copilot in VS Code and Cursor. Your agent does the work. CretSpec gives it the context.

## Start your agent

Start it in the outer folder or in any repository. Each has an `AGENTS.md` and the project's skills. Then ask, for example:

> Read this project's rules and specification, then explain the next agreed story.

If your agent can only reach its starting folder, allow it to read the sibling repositories.

## Save an approach as a skill

When something works, ask your agent to turn it into a skill. It stores the skill where it belongs:

| The skill is for | It goes in |
| --- | --- |
| This project | The spec: `spec/skills/<name>/` |
| One repository's own code | That repository's [skill directory](/CretSpec/reference/manifest/#schema-version-2) |
| All your projects | The guidelines: `skills/<name>/` |

You can also create the scaffold yourself:

```sh
cspec skill create query-validation --scope project --description "Validate this project's query inputs and expected results"
```

Complete its `SKILL.md`, then run `cspec sync` so every agent finds it. `cspec skill list` shows the active skills. A running agent may need a new session to see a new skill.

An agreement that is not a procedure belongs in the project rules or the guidelines, not in a skill. See [the narrowest source wins](/CretSpec/start/concepts/#the-narrowest-source-wins).

## Choose your agents

All four integrations are prepared by default. To select some, list them in `project.json` and run `cspec sync`:

```json
"agents": ["codex", "claude"]
```

| Agent | Instructions | Skills |
| --- | --- | --- |
| Codex | `AGENTS.md` | `.agents/skills/` |
| Claude Code | `AGENTS.md` | `.claude/skills/` |
| GitHub Copilot in VS Code | `.github/instructions/cspec.instructions.md` | Either skill directory |
| Cursor | `.cursor/rules/cspec.mdc` | Either skill directory |

A repository that already tracks its own `AGENTS.md` keeps it. [Generated files](/CretSpec/reference/skills/#generated-files) covers every case, including an existing `CLAUDE.md`.

## After you change a source

Generated files are copies. Edit the source, such as the rules, a skill or `project.json`, then run `cspec sync`. `cspec project doctor` reports what is stale without changing anything.
